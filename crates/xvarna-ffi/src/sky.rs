//! Fixed-layout ASMAN Sky View and Shadow Mask ABI.

use super::{XvStatus, hash_word, input_slice, job::get_job, output_slice};
use crate::solar::XvSolarSensor;
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::Ordering,
    time::Instant,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{SkyViewError, SkyViewOptions, analyze_sky_view_controlled};
use xvarna_types::SensorId;

/// Fixed-layout deterministic hemispherical sampling and ray policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSkyViewOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Fibonacci directions per sensor, from 16 through 262144.
    pub sample_count: u32,
    /// Stable azimuthal sampling rotation seed.
    pub seed: u64,
    /// Normal offset in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Opaque scene instance-category mask.
    pub category_mask: u64,
}

/// Fixed-layout Sky View summary for one sensor.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSkyViewSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable sensor identifier.
    pub sensor_id: u64,
    /// Unweighted visible hemisphere fraction.
    pub visible_hemisphere_fraction: f64,
    /// Lambert cosine-weighted Sky View Factor.
    pub cosine_weighted_svf: f64,
    /// Approximate visible unweighted solid angle in steradians.
    pub visible_solid_angle_steradians: f64,
    /// Interleaved-subset cosine-weighted convergence delta.
    pub cosine_convergence_delta: f64,
    /// Interleaved-subset unweighted convergence delta.
    pub unweighted_convergence_delta: f64,
    /// Visible direction count.
    pub visible_count: u64,
    /// Blocked direction count.
    pub blocked_count: u64,
    /// Object with greatest projected blocked contribution.
    pub dominant_occluder_object_id: u64,
    /// Unweighted solid angle blocked by the dominant object.
    pub dominant_occluder_solid_angle_steradians: f64,
    /// Projected blocked fraction attributed to the dominant object.
    pub dominant_occluder_projected_fraction: f64,
}

/// Fixed-layout Shadow Mask direction and first-hit attribution.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSkyRayEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// [`SkyRayState`] numeric value.
    pub state: u32,
    /// World direction X.
    pub direction_x: f64,
    /// World direction Y.
    pub direction_y: f64,
    /// World direction Z.
    pub direction_z: f64,
    /// First blocking object, or zero.
    pub object_id: u64,
    /// First blocking instance, or zero.
    pub instance_id: u64,
    /// First blocking mesh resource, or zero.
    pub mesh_id: u64,
    /// First blocking triangle, or `UINT32_MAX`.
    pub triangle_id: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// First-hit distance in canonical metres, or infinity.
    pub distance_meters: f64,
}

/// Fixed-layout Sky View result dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSkyViewMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source sensor count.
    pub sensor_count: u64,
    /// Hemisphere samples per sensor.
    pub sample_count: u64,
    /// Sensor-major shadow-mask entry count.
    pub timeline_count: u64,
    /// Native analysis duration in microseconds.
    pub analysis_time_microseconds: u64,
    /// First little-endian word of the BLAKE3 result hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Computes Sky View metrics and a complete sensor-major Shadow Mask.
///
/// # Safety
///
/// Input pointers reference their declared counts. Output capacities must cover
/// `sensor_count` summaries and `sensor_count * sample_count` mask entries.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_sky_view(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    options: *const XvSkyViewOptions,
    job_handle: u64,
    output_metadata: *mut XvSkyViewMetadata,
    output_summaries: *mut XvSkyViewSummary,
    summary_capacity: usize,
    output_timeline: *mut XvSkyRayEntry,
    timeline_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Fixed options pointer is non-null and readable for this call.
        let native_options = unsafe { options.read() };
        if native_options.structure_size != structure_size::<XvSkyViewOptions>() {
            return Err(XvStatus::InvalidArgument);
        }
        let sample_count =
            usize::try_from(native_options.sample_count).map_err(|_| XvStatus::InvalidLength)?;
        let required_timeline = sensor_count
            .checked_mul(sample_count)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < sensor_count || timeline_capacity < required_timeline {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Slice pointers and counts are validated by shared helpers.
        let sensors = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: Summary capacity was validated above.
        let output_summaries = unsafe { output_slice(output_summaries, sensor_count)? };
        // SAFETY: Timeline capacity and multiplication were validated above.
        let output_timeline = unsafe { output_slice(output_timeline, required_timeline)? };
        let sensors = sensors
            .iter()
            .map(|sensor| {
                xvarna_hvare::SolarSensor::try_new(
                    SensorId::new(sensor.sensor_id),
                    Vec3::new(sensor.position_x, sensor.position_y, sensor.position_z),
                    Vec3::new(sensor.normal_x, sensor.normal_y, sensor.normal_z),
                )
                .map_err(|_| XvStatus::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let job = get_job(job_handle)?;
        job.total_units.store(
            u64::try_from(required_timeline).map_err(|_| XvStatus::InvalidLength)?,
            Ordering::Relaxed,
        );
        job.completed_units.store(0, Ordering::Relaxed);
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_sky_view_controlled(
            &scene,
            &sensors,
            SkyViewOptions {
                sample_count,
                seed: native_options.seed,
                sensor_offset_meters: native_options.sensor_offset_meters,
                maximum_distance_meters: native_options.maximum_distance_meters,
                category_mask: native_options.category_mask,
            },
            &job.cancelled,
            &job.completed_units,
        )
        .map_err(sky_error_status)?;
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        for (destination, summary) in output_summaries.iter_mut().zip(&result.summaries) {
            *destination = XvSkyViewSummary {
                structure_size: structure_size::<XvSkyViewSummary>(),
                reserved: 0,
                sensor_id: summary.sensor_id.get(),
                visible_hemisphere_fraction: summary.visible_hemisphere_fraction,
                cosine_weighted_svf: summary.cosine_weighted_svf,
                visible_solid_angle_steradians: summary.visible_solid_angle_steradians,
                cosine_convergence_delta: summary.cosine_convergence_delta,
                unweighted_convergence_delta: summary.unweighted_convergence_delta,
                visible_count: u64::try_from(summary.visible_count).unwrap_or(u64::MAX),
                blocked_count: u64::try_from(summary.blocked_count).unwrap_or(u64::MAX),
                dominant_occluder_object_id: summary.dominant_occluder_object_id.get(),
                dominant_occluder_solid_angle_steradians: summary
                    .dominant_occluder_solid_angle_steradians,
                dominant_occluder_projected_fraction: summary.dominant_occluder_projected_fraction,
            };
        }
        for (destination, entry) in output_timeline.iter_mut().zip(&result.timeline) {
            *destination = XvSkyRayEntry {
                structure_size: structure_size::<XvSkyRayEntry>(),
                state: u32::from(entry.state as u8),
                direction_x: entry.direction.x,
                direction_y: entry.direction.y,
                direction_z: entry.direction.z,
                object_id: entry.object_id.get(),
                instance_id: entry.instance_id.get(),
                mesh_id: entry.mesh_id.get(),
                triangle_id: entry.triangle_id,
                reserved: 0,
                distance_meters: entry.distance_meters,
            };
        }
        let metadata = XvSkyViewMetadata {
            structure_size: structure_size::<XvSkyViewMetadata>(),
            reserved: 0,
            sensor_count: u64::try_from(sensor_count).unwrap_or(u64::MAX),
            sample_count: u64::try_from(sample_count).unwrap_or(u64::MAX),
            timeline_count: u64::try_from(required_timeline).unwrap_or(u64::MAX),
            analysis_time_microseconds: elapsed,
            content_hash_0: hash_word(&result.content_hash, 0),
            content_hash_1: hash_word(&result.content_hash, 1),
            content_hash_2: hash_word(&result.content_hash, 2),
            content_hash_3: hash_word(&result.content_hash, 3),
        };
        // SAFETY: Metadata pointer is non-null and caller-owned writable storage.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

const fn sky_error_status(error: SkyViewError) -> XvStatus {
    match error {
        SkyViewError::Cancelled => XvStatus::Cancelled,
        SkyViewError::EmptySensors
        | SkyViewError::InvalidSampleCount
        | SkyViewError::InvalidSensorOffset
        | SkyViewError::InvalidMaximumDistance
        | SkyViewError::InvalidSensor
        | SkyViewError::ResultTooLarge
        | SkyViewError::InvalidQuery => XvStatus::InvalidArgument,
    }
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        job::{xv_job_create, xv_job_progress, xv_job_release},
        scene::{
            XvSceneOptions, XvSceneStats, xv_scene_add_instance, xv_scene_add_mesh, xv_scene_build,
            xv_scene_create, xv_scene_release,
        },
    };
    use core::mem::MaybeUninit;

    #[test]
    fn sky_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvSkyViewOptions>(), 40);
        assert_eq!(mem::size_of::<XvSkyViewSummary>(), 96);
        assert_eq!(mem::size_of::<XvSkyRayEntry>(), 72);
        assert_eq!(mem::size_of::<XvSkyViewMetadata>(), 72);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn sky_view_crosses_scene_job_and_result_abi() {
        let scene_options = XvSceneOptions::default();
        let mut scene_handle = 0;
        // SAFETY: Options and output handle are live and correctly sized.
        assert_eq!(
            unsafe { xv_scene_create(&raw const scene_options, &raw mut scene_handle) },
            XvStatus::Success as i32
        );
        let positions = [-1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 0.0, 1.0, -1.0];
        let triangles = [0_u32, 1, 2];
        let mut mesh_id = 0;
        // SAFETY: Geometry arrays and output ID are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    scene_handle,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut mesh_id,
                )
            },
            XvStatus::Success as i32
        );
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        // SAFETY: Transform contains sixteen finite row-major values.
        assert_eq!(
            unsafe { xv_scene_add_instance(scene_handle, mesh_id, identity.as_ptr(), 99, 7, 4) },
            XvStatus::Success as i32
        );
        let mut scene_stats = MaybeUninit::<XvSceneStats>::uninit();
        // SAFETY: Statistics output points to correctly sized writable storage.
        assert_eq!(
            unsafe { xv_scene_build(scene_handle, scene_stats.as_mut_ptr()) },
            XvStatus::Success as i32
        );
        let mut job_handle = 0;
        // SAFETY: Job output points to writable storage.
        assert_eq!(
            unsafe { xv_job_create(&raw mut job_handle) },
            XvStatus::Success as i32
        );

        let sensors = [XvSolarSensor {
            sensor_id: 5,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            normal_x: 0.0,
            normal_y: 0.0,
            normal_z: 1.0,
        }];
        let options = XvSkyViewOptions {
            structure_size: structure_size::<XvSkyViewOptions>(),
            sample_count: 128,
            seed: 42,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: 4,
        };
        let mut metadata = MaybeUninit::<XvSkyViewMetadata>::uninit();
        let mut summaries = [XvSkyViewSummary {
            structure_size: 0,
            reserved: 0,
            sensor_id: 0,
            visible_hemisphere_fraction: 0.0,
            cosine_weighted_svf: 0.0,
            visible_solid_angle_steradians: 0.0,
            cosine_convergence_delta: 0.0,
            unweighted_convergence_delta: 0.0,
            visible_count: 0,
            blocked_count: 0,
            dominant_occluder_object_id: 0,
            dominant_occluder_solid_angle_steradians: 0.0,
            dominant_occluder_projected_fraction: 0.0,
        }];
        let mut timeline = [XvSkyRayEntry {
            structure_size: 0,
            state: 0,
            direction_x: 0.0,
            direction_y: 0.0,
            direction_z: 0.0,
            object_id: 0,
            instance_id: 0,
            mesh_id: 0,
            triangle_id: 0,
            reserved: 0,
            distance_meters: 0.0,
        }; 128];
        // SAFETY: Every pointer references a live buffer with the declared capacity.
        assert_eq!(
            unsafe {
                xv_scene_sky_view(
                    scene_handle,
                    sensors.as_ptr(),
                    sensors.len(),
                    &raw const options,
                    job_handle,
                    metadata.as_mut_ptr(),
                    summaries.as_mut_ptr(),
                    summaries.len(),
                    timeline.as_mut_ptr(),
                    timeline.len(),
                )
            },
            XvStatus::Success as i32
        );
        // SAFETY: Successful calls initialized the metadata storage.
        let metadata = unsafe { metadata.assume_init() };
        assert_eq!(metadata.sensor_count, 1);
        assert_eq!(metadata.sample_count, 128);
        assert_eq!(metadata.timeline_count, 128);
        assert_eq!(summaries[0].sensor_id, 5);
        assert_eq!(summaries[0].visible_count, 128);
        assert!((summaries[0].cosine_weighted_svf - 1.0).abs() < f64::EPSILON);
        assert!(timeline.iter().all(|entry| entry.state == 0));

        let mut progress = MaybeUninit::uninit();
        // SAFETY: Progress output points to correctly sized writable storage.
        assert_eq!(
            unsafe { xv_job_progress(job_handle, progress.as_mut_ptr()) },
            XvStatus::Success as i32
        );
        // SAFETY: Successful progress call initialized the output.
        let progress = unsafe { progress.assume_init() };
        assert_eq!(progress.total_units, 128);
        assert_eq!(progress.completed_units, 128);
        assert_eq!(xv_job_release(job_handle), XvStatus::Success as i32);
        assert_eq!(xv_scene_release(scene_handle), XvStatus::Success as i32);
    }
}
