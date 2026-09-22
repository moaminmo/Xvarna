//! Fixed-layout ZURVAN/HVARE solar-analysis ABI.

use super::{XvStatus, hash_from_words, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{DirectSunOptions, SolarSensor, analyze_direct_sun};
use xvarna_types::SensorId;
use xvarna_zurvan::{
    SolarLocation, SolarOptions, SunSample, SunSet, TimeSample, calculate_sun_set,
};

const SUN_FLAG_ACTIVE: u32 = 1 << 0;
const SUN_SET_FLAG_REFRACTION: u32 = 1 << 0;

/// Fixed-layout location and NREL SPA calculation options.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// WGS84 latitude in degrees, positive north.
    pub latitude_degrees: f64,
    /// WGS84 longitude in degrees, positive east.
    pub longitude_degrees: f64,
    /// Observer elevation above sea level in metres.
    pub elevation_meters: f64,
    /// Terrestrial Time minus Universal Time in seconds.
    pub delta_t_seconds: f64,
    /// Atmospheric pressure in millibars; zero disables refraction.
    pub pressure_millibars: f64,
    /// Ambient temperature in Celsius.
    pub temperature_celsius: f64,
    /// Counter-clockwise model rotation of true north about +Z.
    pub north_rotation_degrees: f64,
    /// Minimum included apparent sun altitude.
    pub minimum_altitude_degrees: f64,
}

/// Fixed-layout weighted UTC schedule interval.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTimeSample {
    /// Unix timestamp at interval centre.
    pub unix_seconds_utc: i64,
    /// Interval duration in hours.
    pub duration_hours: f64,
    /// Dimensionless schedule weight.
    pub weight: f64,
}

/// Fixed-layout calculated solar direction.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSunSample {
    /// Source Unix timestamp in UTC.
    pub unix_seconds_utc: i64,
    /// Model-space direction X, where unrotated +X is east.
    pub direction_x: f64,
    /// Model-space direction Y, where unrotated +Y is north.
    pub direction_y: f64,
    /// Model-space direction Z, where +Z is up.
    pub direction_z: f64,
    /// Apparent altitude in degrees.
    pub altitude_degrees: f64,
    /// Azimuth clockwise from true north in degrees.
    pub azimuth_degrees: f64,
    /// Interval duration in hours.
    pub duration_hours: f64,
    /// Dimensionless schedule weight.
    pub weight: f64,
    /// Active and future sample flags.
    pub flags: u32,
    /// Reserved; always zero.
    pub reserved: u32,
}

/// Fixed-layout identity and counts for a calculated sun set.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSunSetMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Refraction and future metadata flags.
    pub flags: u32,
    /// Total ordered sample count.
    pub sample_count: u64,
    /// Samples active after altitude and weight filtering.
    pub active_sample_count: u64,
    /// First little-endian word of the BLAKE3 sun-set hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Fixed-layout oriented sensor in canonical metre coordinates.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarSensor {
    /// Stable sensor identifier.
    pub sensor_id: u64,
    /// Position X in canonical metres.
    pub position_x: f64,
    /// Position Y in canonical metres.
    pub position_y: f64,
    /// Position Z in canonical metres.
    pub position_z: f64,
    /// Surface-normal X.
    pub normal_x: f64,
    /// Surface-normal Y.
    pub normal_y: f64,
    /// Surface-normal Z.
    pub normal_z: f64,
}

/// Fixed-layout direct-sun analysis policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDirectSunOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Sensor normal offset in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum occluder distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Minimum normal-to-sun cosine.
    pub minimum_incidence_cosine: f64,
    /// Opaque scene category mask.
    pub category_mask: u64,
}

/// Fixed-layout direct-sun aggregate for one sensor.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDirectSunSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable sensor identifier.
    pub sensor_id: u64,
    /// Weighted received direct-sun hours.
    pub direct_sun_hours: f64,
    /// Weighted blocked hours.
    pub shadow_hours: f64,
    /// Weighted hours eligible after incidence filtering.
    pub eligible_hours: f64,
    /// Direct sun divided by eligible hours.
    pub solar_access_ratio: f64,
    /// Visible interval count.
    pub visible_count: u64,
    /// Blocked interval count.
    pub blocked_count: u64,
    /// Back-facing interval count.
    pub back_facing_count: u64,
    /// Object with the greatest weighted blocking contribution.
    pub dominant_occluder_object_id: u64,
    /// Weighted hours assigned to the dominant blocker.
    pub dominant_occluder_hours: f64,
}

/// Fixed-layout first-hit attribution for one sensor/time pair.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSunTimelineEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// [`SunState`] numeric value and future flags.
    pub state: u32,
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

/// Fixed-layout result identity, dimensions, and runtime.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDirectSunMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source sensor count.
    pub sensor_count: u64,
    /// Source sun sample count.
    pub sun_count: u64,
    /// Sensor-major timeline entry count.
    pub timeline_count: u64,
    /// Analysis runtime in microseconds.
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

/// Calculates an ordered NREL SPA sun-position batch.
///
/// # Safety
///
/// Every pointer must reference readable or writable storage for its declared
/// count. Output capacity must be at least `time_count`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_sun_positions(
    options: *const XvSolarOptions,
    times: *const XvTimeSample,
    time_count: usize,
    output_metadata: *mut XvSunSetMetadata,
    output_samples: *mut XvSunSample,
    sample_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if sample_capacity < time_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Required pointers are checked and fixed-size structures are caller-owned.
        let native_options = unsafe { options.read() };
        validate_structure::<XvSolarOptions>(
            native_options.structure_size,
            native_options.reserved,
        )?;
        // SAFETY: Slice pointer and count are validated by the shared helper.
        let times = unsafe { input_slice(times, time_count)? };
        // SAFETY: Output pointer and requested length are validated by the shared helper.
        let output = unsafe { output_slice(output_samples, time_count)? };
        let location = SolarLocation::try_new(
            native_options.latitude_degrees,
            native_options.longitude_degrees,
            native_options.elevation_meters,
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        let samples = times
            .iter()
            .map(|sample| {
                TimeSample::try_new(
                    sample.unix_seconds_utc,
                    sample.duration_hours,
                    sample.weight,
                )
                .map_err(|_| XvStatus::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sun_set = calculate_sun_set(
            location,
            &samples,
            SolarOptions {
                delta_t_seconds: native_options.delta_t_seconds,
                pressure_millibars: native_options.pressure_millibars,
                temperature_celsius: native_options.temperature_celsius,
                north_rotation_degrees: native_options.north_rotation_degrees,
                minimum_altitude_degrees: native_options.minimum_altitude_degrees,
            },
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        for (destination, sample) in output.iter_mut().zip(&sun_set.samples) {
            *destination = sun_sample_to_ffi(*sample);
        }
        let metadata = sun_metadata_from_set(&sun_set);
        // SAFETY: Metadata pointer was checked and is writable for this call.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

/// Computes direct sun, shadow, solar access, and first-hit attribution.
///
/// # Safety
///
/// Input pointers must reference their declared counts. Summary capacity must
/// cover `sensor_count`; timeline capacity must cover `sensor_count * sun_count`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_direct_sun(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    sun_metadata: *const XvSunSetMetadata,
    sun_samples: *const XvSunSample,
    sun_count: usize,
    options: *const XvDirectSunOptions,
    output_metadata: *mut XvDirectSunMetadata,
    output_summaries: *mut XvDirectSunSummary,
    summary_capacity: usize,
    output_timeline: *mut XvSunTimelineEntry,
    timeline_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if sun_metadata.is_null() || options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let required_timeline = sensor_count
            .checked_mul(sun_count)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < sensor_count || timeline_capacity < required_timeline {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Fixed input structures are non-null and readable by contract.
        let native_sun_metadata = unsafe { sun_metadata.read() };
        // SAFETY: Fixed input structures are non-null and readable by contract.
        let native_options = unsafe { options.read() };
        validate_structure::<XvSunSetMetadata>(native_sun_metadata.structure_size, 0)?;
        validate_structure::<XvDirectSunOptions>(
            native_options.structure_size,
            native_options.reserved,
        )?;
        if native_sun_metadata.sample_count
            != u64::try_from(sun_count).map_err(|_| XvStatus::InvalidLength)?
        {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: All slice pointers and counts are checked by shared helpers.
        let sensors = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: All slice pointers and counts are checked by shared helpers.
        let sun_samples = unsafe { input_slice(sun_samples, sun_count)? };
        // SAFETY: Summary capacity was validated above.
        let output_summaries = unsafe { output_slice(output_summaries, sensor_count)? };
        // SAFETY: Timeline capacity and multiplication were validated above.
        let output_timeline = unsafe { output_slice(output_timeline, required_timeline)? };
        let sensors = sensors
            .iter()
            .map(|sensor| {
                SolarSensor::try_new(
                    SensorId::new(sensor.sensor_id),
                    Vec3::new(sensor.position_x, sensor.position_y, sensor.position_z),
                    Vec3::new(sensor.normal_x, sensor.normal_y, sensor.normal_z),
                )
                .map_err(|_| XvStatus::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sun_samples = sun_samples
            .iter()
            .copied()
            .map(sun_sample_from_ffi)
            .collect::<Result<Vec<_>, _>>()?;
        let sun_set = SunSet {
            location: SolarLocation::try_new(0.0, 0.0, 0.0).expect("zero location is valid"),
            options: SolarOptions::default(),
            samples: sun_samples,
            content_hash: hash_from_words([
                native_sun_metadata.content_hash_0,
                native_sun_metadata.content_hash_1,
                native_sun_metadata.content_hash_2,
                native_sun_metadata.content_hash_3,
            ]),
        };
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_direct_sun(
            &scene,
            &sensors,
            &sun_set,
            DirectSunOptions {
                sensor_offset_meters: native_options.sensor_offset_meters,
                maximum_distance_meters: native_options.maximum_distance_meters,
                minimum_incidence_cosine: native_options.minimum_incidence_cosine,
                category_mask: native_options.category_mask,
            },
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        for (destination, summary) in output_summaries.iter_mut().zip(&result.summaries) {
            *destination = XvDirectSunSummary {
                structure_size: structure_size::<XvDirectSunSummary>(),
                reserved: 0,
                sensor_id: summary.sensor_id.get(),
                direct_sun_hours: summary.direct_sun_hours,
                shadow_hours: summary.shadow_hours,
                eligible_hours: summary.eligible_hours,
                solar_access_ratio: summary.solar_access_ratio,
                visible_count: u64::try_from(summary.visible_count).unwrap_or(u64::MAX),
                blocked_count: u64::try_from(summary.blocked_count).unwrap_or(u64::MAX),
                back_facing_count: u64::try_from(summary.back_facing_count).unwrap_or(u64::MAX),
                dominant_occluder_object_id: summary.dominant_occluder_object_id.get(),
                dominant_occluder_hours: summary.dominant_occluder_hours,
            };
        }
        for (destination, entry) in output_timeline.iter_mut().zip(&result.timeline) {
            *destination = XvSunTimelineEntry {
                structure_size: structure_size::<XvSunTimelineEntry>(),
                state: u32::from(entry.state as u8),
                object_id: entry.object_id.get(),
                instance_id: entry.instance_id.get(),
                mesh_id: entry.mesh_id.get(),
                triangle_id: entry.triangle_id,
                reserved: 0,
                distance_meters: entry.distance_meters,
            };
        }
        let metadata = XvDirectSunMetadata {
            structure_size: structure_size::<XvDirectSunMetadata>(),
            reserved: 0,
            sensor_count: u64::try_from(sensor_count).unwrap_or(u64::MAX),
            sun_count: u64::try_from(sun_count).unwrap_or(u64::MAX),
            timeline_count: u64::try_from(required_timeline).unwrap_or(u64::MAX),
            analysis_time_microseconds: elapsed,
            content_hash_0: hash_word(&result.content_hash, 0),
            content_hash_1: hash_word(&result.content_hash, 1),
            content_hash_2: hash_word(&result.content_hash, 2),
            content_hash_3: hash_word(&result.content_hash, 3),
        };
        // SAFETY: Metadata output is non-null and caller-owned writable memory.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

fn validate_structure<T>(size: u32, reserved: u32) -> Result<(), XvStatus> {
    if size != structure_size::<T>() || reserved != 0 {
        return Err(XvStatus::InvalidArgument);
    }
    Ok(())
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

const fn sun_sample_to_ffi(sample: SunSample) -> XvSunSample {
    XvSunSample {
        unix_seconds_utc: sample.unix_seconds_utc,
        direction_x: sample.direction.x,
        direction_y: sample.direction.y,
        direction_z: sample.direction.z,
        altitude_degrees: sample.altitude_degrees,
        azimuth_degrees: sample.azimuth_degrees,
        duration_hours: sample.duration_hours,
        weight: sample.weight,
        flags: if sample.is_active { SUN_FLAG_ACTIVE } else { 0 },
        reserved: 0,
    }
}

pub fn sun_sample_from_ffi(sample: XvSunSample) -> Result<SunSample, XvStatus> {
    let direction = Vec3::new(sample.direction_x, sample.direction_y, sample.direction_z)
        .normalized()
        .ok_or(XvStatus::InvalidArgument)?;
    if !sample.altitude_degrees.is_finite()
        || !sample.azimuth_degrees.is_finite()
        || !sample.duration_hours.is_finite()
        || sample.duration_hours <= 0.0
        || !sample.weight.is_finite()
        || sample.weight < 0.0
        || sample.reserved != 0
    {
        return Err(XvStatus::InvalidArgument);
    }
    Ok(SunSample {
        unix_seconds_utc: sample.unix_seconds_utc,
        direction,
        altitude_degrees: sample.altitude_degrees,
        azimuth_degrees: sample.azimuth_degrees,
        duration_hours: sample.duration_hours,
        weight: sample.weight,
        is_active: sample.flags & SUN_FLAG_ACTIVE != 0,
    })
}

fn sun_metadata_from_set(sun_set: &SunSet) -> XvSunSetMetadata {
    XvSunSetMetadata {
        structure_size: structure_size::<XvSunSetMetadata>(),
        flags: if sun_set.options.pressure_millibars > 0.0 {
            SUN_SET_FLAG_REFRACTION
        } else {
            0
        },
        sample_count: u64::try_from(sun_set.samples.len()).unwrap_or(u64::MAX),
        active_sample_count: u64::try_from(
            sun_set
                .samples
                .iter()
                .filter(|sample| sample.is_active)
                .count(),
        )
        .unwrap_or(u64::MAX),
        content_hash_0: hash_word(&sun_set.content_hash, 0),
        content_hash_1: hash_word(&sun_set.content_hash, 1),
        content_hash_2: hash_word(&sun_set.content_hash, 2),
        content_hash_3: hash_word(&sun_set.content_hash, 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{
        XvSceneOptions, XvSceneStats, xv_scene_add_instance, xv_scene_add_mesh, xv_scene_build,
        xv_scene_create, xv_scene_release,
    };

    #[test]
    fn solar_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvSolarOptions>(), 72);
        assert_eq!(mem::size_of::<XvTimeSample>(), 24);
        assert_eq!(mem::size_of::<XvSunSample>(), 72);
        assert_eq!(mem::size_of::<XvSunSetMetadata>(), 56);
        assert_eq!(mem::size_of::<XvSolarSensor>(), 56);
        assert_eq!(mem::size_of::<XvDirectSunOptions>(), 40);
        assert_eq!(mem::size_of::<XvDirectSunSummary>(), 88);
        assert_eq!(mem::size_of::<XvSunTimelineEntry>(), 48);
        assert_eq!(mem::size_of::<XvDirectSunMetadata>(), 72);
    }

    #[test]
    fn solar_positions_cross_abi_with_nrel_golden_case() {
        let timestamp = 1_066_419_030;
        let options = XvSolarOptions {
            structure_size: structure_size::<XvSolarOptions>(),
            reserved: 0,
            latitude_degrees: 39.742_476,
            longitude_degrees: -105.178_6,
            elevation_meters: 1_830.14,
            delta_t_seconds: 67.0,
            pressure_millibars: 820.0,
            temperature_celsius: 11.0,
            north_rotation_degrees: 0.0,
            minimum_altitude_degrees: 0.0,
        };
        let times = [XvTimeSample {
            unix_seconds_utc: timestamp,
            duration_hours: 1.0,
            weight: 1.0,
        }];
        let mut metadata = XvSunSetMetadata {
            structure_size: 0,
            flags: 0,
            sample_count: 0,
            active_sample_count: 0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        };
        let mut output = [XvSunSample {
            unix_seconds_utc: 0,
            direction_x: 0.0,
            direction_y: 0.0,
            direction_z: 0.0,
            altitude_degrees: 0.0,
            azimuth_degrees: 0.0,
            duration_hours: 0.0,
            weight: 0.0,
            flags: 0,
            reserved: 0,
        }];
        // SAFETY: All test pointers reference live correctly sized values.
        let status = unsafe {
            xv_sun_positions(
                &raw const options,
                times.as_ptr(),
                times.len(),
                &raw mut metadata,
                output.as_mut_ptr(),
                output.len(),
            )
        };
        assert_eq!(status, XvStatus::Success as i32);
        assert_eq!(metadata.active_sample_count, 1);
        assert!((output[0].azimuth_degrees - 194.340_24).abs() < 0.000_3);
        assert!((output[0].altitude_degrees - 39.888_38).abs() < 0.000_3);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn direct_sun_crosses_scene_and_solar_abi_with_attribution() {
        let scene_options = XvSceneOptions::default();
        let mut scene_handle = 0;
        // SAFETY: Options and output handle are live and correctly sized.
        assert_eq!(
            unsafe { xv_scene_create(&raw const scene_options, &raw mut scene_handle) },
            0
        );
        let positions = [-1.0, -1.0, 1.0, 2.0, -1.0, 1.0, -1.0, 2.0, 1.0];
        let triangles = [0, 1, 2];
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
            0
        );
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        // SAFETY: Transform contains sixteen finite row-major values.
        assert_eq!(
            unsafe { xv_scene_add_instance(scene_handle, mesh_id, identity.as_ptr(), 99, 7, 4) },
            0
        );
        let mut scene_stats = XvSceneStats {
            structure_size: 0,
            thread_count: 0,
            mesh_resource_count: 0,
            instance_count: 0,
            unique_triangle_count: 0,
            instanced_triangle_count: 0,
            blas_node_count: 0,
            tlas_node_count: 0,
            maximum_bvh_depth: 0,
            approximate_memory_bytes: 0,
            build_time_microseconds: 0,
            rebase_origin_x: 0.0,
            rebase_origin_y: 0.0,
            rebase_origin_z: 0.0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        };
        // SAFETY: Statistics output is live and writable.
        assert_eq!(
            unsafe { xv_scene_build(scene_handle, &raw mut scene_stats) },
            0
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
        let sun_metadata = XvSunSetMetadata {
            structure_size: structure_size::<XvSunSetMetadata>(),
            flags: 0,
            sample_count: 1,
            active_sample_count: 1,
            content_hash_0: 1,
            content_hash_1: 2,
            content_hash_2: 3,
            content_hash_3: 4,
        };
        let sun_samples = [XvSunSample {
            unix_seconds_utc: 0,
            direction_x: 0.0,
            direction_y: 0.0,
            direction_z: 1.0,
            altitude_degrees: 90.0,
            azimuth_degrees: 0.0,
            duration_hours: 1.0,
            weight: 1.0,
            flags: SUN_FLAG_ACTIVE,
            reserved: 0,
        }];
        let direct_options = XvDirectSunOptions {
            structure_size: structure_size::<XvDirectSunOptions>(),
            reserved: 0,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            minimum_incidence_cosine: 0.0,
            category_mask: 4,
        };
        let mut metadata = XvDirectSunMetadata {
            structure_size: 0,
            reserved: 0,
            sensor_count: 0,
            sun_count: 0,
            timeline_count: 0,
            analysis_time_microseconds: 0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        };
        let mut summaries = [XvDirectSunSummary {
            structure_size: 0,
            reserved: 0,
            sensor_id: 0,
            direct_sun_hours: 0.0,
            shadow_hours: 0.0,
            eligible_hours: 0.0,
            solar_access_ratio: 0.0,
            visible_count: 0,
            blocked_count: 0,
            back_facing_count: 0,
            dominant_occluder_object_id: 0,
            dominant_occluder_hours: 0.0,
        }];
        let mut timeline = [XvSunTimelineEntry {
            structure_size: 0,
            state: 0,
            object_id: 0,
            instance_id: 0,
            mesh_id: 0,
            triangle_id: 0,
            reserved: 0,
            distance_meters: 0.0,
        }];
        // SAFETY: Every input and output pointer references a live array of the declared size.
        let status = unsafe {
            xv_scene_direct_sun(
                scene_handle,
                sensors.as_ptr(),
                sensors.len(),
                &raw const sun_metadata,
                sun_samples.as_ptr(),
                sun_samples.len(),
                &raw const direct_options,
                &raw mut metadata,
                summaries.as_mut_ptr(),
                summaries.len(),
                timeline.as_mut_ptr(),
                timeline.len(),
            )
        };
        assert_eq!(status, XvStatus::Success as i32);
        assert_eq!(metadata.sensor_count, 1);
        assert_eq!(metadata.sun_count, 1);
        assert!((summaries[0].shadow_hours - 1.0).abs() < f64::EPSILON);
        assert_eq!(summaries[0].dominant_occluder_object_id, 99);
        assert_eq!(timeline[0].state, 3);
        assert_eq!(timeline[0].object_id, 99);
        assert_eq!(timeline[0].instance_id, 7);
        assert!((timeline[0].distance_meters - 0.999_9).abs() < 1.0e-12);
        assert_eq!(xv_scene_release(scene_handle), XvStatus::Success as i32);
    }
}
