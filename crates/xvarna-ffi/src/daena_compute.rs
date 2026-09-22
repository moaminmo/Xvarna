//! VAYU-backed advanced DAENA C ABI entry points.

use super::{XvStatus, hash_word, input_slice, output_slice};
use crate::{
    compute::get_session,
    daena_advanced::{
        XvObserverPath, XvObserverPathMetadata, XvObserverPathOptions, XvObserverPathSample,
        XvObserverPathSummary, XvPoint3, XvTargetViewEntry, XvTargetViewMetadata,
        XvTargetViewOptions, XvTargetViewSummary, XvViewCorridor, XvViewCorridorMetadata,
        XvViewCorridorOptions, XvViewCorridorSample, XvViewCorridorSummary, XvViewObserver,
        XvViewTargetPatch, ffi_status, micros, path_sample_count, structure_size, target_options,
        usize_u64, visibility_error_status,
    },
};
use std::time::Instant;
use xvarna_daena::{
    ObserverPath, ObserverPathOptions, ViewCorridor, ViewCorridorOptions, ViewObserver,
    ViewTargetPatch, analyze_observer_paths_with_executor, analyze_target_view_with_executor,
    analyze_view_corridors_with_executor,
};
use xvarna_geometry::Vec3;
use xvarna_scene::{RayQueryBackend, RayQueryExecutionSummary};
use xvarna_types::{SensorId, TargetId};

const EXECUTION_FLAG_CREATION_FALLBACK: u32 = 1 << 0;
const EXECUTION_FLAG_HAS_ADAPTER: u32 = 1 << 1;
const EXECUTION_FLAG_REUSABLE_BUFFERS: u32 = 1 << 2;

/// Fixed-layout aggregate execution provenance for one DAENA result.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaenaExecutionInfo {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Zero CPU, one portable GPU, two mixed GPU/CPU.
    pub backend: u32,
    /// Creation fallback and adapter flags.
    pub flags: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Number of domain query batches that retried on CPU.
    pub runtime_fallback_batch_count: u64,
    /// Number of domain query batches.
    pub batch_count: u64,
    /// Total ordered rays.
    pub ray_count: u64,
    /// Total portable dispatches.
    pub dispatch_count: u64,
    /// Total upload/encoding microseconds.
    pub upload_microseconds: u64,
    /// Total execution/wait microseconds.
    pub execution_microseconds: u64,
    /// Total readback/decode microseconds.
    pub readback_microseconds: u64,
    /// Maximum measured scene/ray conversion error in metres.
    pub maximum_precision_error_meters: f64,
    /// UTF-8 selected adapter name.
    pub adapter_name: [u8; 128],
    /// UTF-8 first creation/runtime fallback reason.
    pub fallback_reason: [u8; 256],
}

impl Default for XvDaenaExecutionInfo {
    fn default() -> Self {
        Self {
            structure_size: structure_size::<Self>(),
            backend: 0,
            flags: 0,
            reserved: 0,
            runtime_fallback_batch_count: 0,
            batch_count: 0,
            ray_count: 0,
            dispatch_count: 0,
            upload_microseconds: 0,
            execution_microseconds: 0,
            readback_microseconds: 0,
            maximum_precision_error_meters: 0.0,
            adapter_name: [0; 128],
            fallback_reason: [0; 256],
        }
    }
}

/// Computes Target/Weighted/Green View through a cached VAYU session.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
pub unsafe extern "C" fn xv_compute_target_view(
    compute_handle: u64,
    observers: *const XvViewObserver,
    observer_count: usize,
    patches: *const XvViewTargetPatch,
    patch_count: usize,
    options: *const XvTargetViewOptions,
    output_execution: *mut XvDaenaExecutionInfo,
    output_metadata: *mut XvTargetViewMetadata,
    output_summaries: *mut XvTargetViewSummary,
    summary_capacity: usize,
    output_entries: *mut XvTargetViewEntry,
    entry_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_execution.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Fixed-size input was validated above.
        let native_options = unsafe { options.read() };
        if native_options.structure_size != structure_size::<XvTargetViewOptions>()
            || native_options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let policy = target_options(native_options)?;
        // SAFETY: Shared helpers validate array pointers and lengths.
        let observer_values = unsafe { input_slice(observers, observer_count)? };
        // SAFETY: Shared helpers validate array pointers and lengths.
        let patch_values = unsafe { input_slice(patches, patch_count)? };
        let distinct_targets = patch_values
            .iter()
            .filter(|patch| patch.category_mask & native_options.target_category_mask != 0)
            .map(|patch| patch.target_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let entry_count = observer_count
            .checked_mul(distinct_targets)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < observer_count || entry_capacity < entry_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacities were checked.
        let summary_output = unsafe { output_slice(output_summaries, observer_count)? };
        // SAFETY: Capacity multiplication was checked.
        let entry_output = unsafe { output_slice(output_entries, entry_count)? };
        let observers = convert_observers(observer_values)?;
        let patches = convert_patches(patch_values)?;
        let session = get_session(compute_handle)?;
        let started = Instant::now();
        let result = analyze_target_view_with_executor(&*session, &observers, &patches, policy)
            .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        write_target_outputs(&result, summary_output, entry_output);
        // SAFETY: Caller-owned fixed-size outputs were checked for null.
        unsafe {
            output_execution.write(execution_info(&result.execution));
            output_metadata.write(XvTargetViewMetadata {
                structure_size: structure_size::<XvTargetViewMetadata>(),
                reserved: 0,
                observer_count: usize_u64(observer_count),
                target_count: usize_u64(result.target_ids.len()),
                entry_count: usize_u64(result.entries.len()),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        }
        Ok(())
    })
}

/// Computes protected View Corridor through a cached VAYU session.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_compute_view_corridor(
    compute_handle: u64,
    corridors: *const XvViewCorridor,
    corridor_count: usize,
    options: *const XvViewCorridorOptions,
    output_execution: *mut XvDaenaExecutionInfo,
    output_metadata: *mut XvViewCorridorMetadata,
    output_summaries: *mut XvViewCorridorSummary,
    summary_capacity: usize,
    output_samples: *mut XvViewCorridorSample,
    sample_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_execution.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Fixed-size input was validated above.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvViewCorridorOptions>() {
            return Err(XvStatus::InvalidArgument);
        }
        let samples_per_corridor =
            usize::try_from(options.sample_count).map_err(|_| XvStatus::InvalidLength)?;
        let sample_count = corridor_count
            .checked_mul(samples_per_corridor)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < corridor_count || sample_capacity < sample_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate pointers and lengths.
        let corridor_values = unsafe { input_slice(corridors, corridor_count)? };
        // SAFETY: Capacities were checked.
        let summary_output = unsafe { output_slice(output_summaries, corridor_count)? };
        // SAFETY: Capacity multiplication was checked.
        let sample_output = unsafe { output_slice(output_samples, sample_count)? };
        let corridors = corridor_values
            .iter()
            .map(|value| {
                ViewCorridor::try_new(
                    value.corridor_id,
                    Vec3::new(value.origin_x, value.origin_y, value.origin_z),
                    Vec3::new(value.target_x, value.target_y, value.target_z),
                    Vec3::new(value.up_x, value.up_y, value.up_z),
                    value.target_radius_meters,
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let session = get_session(compute_handle)?;
        let started = Instant::now();
        let result = analyze_view_corridors_with_executor(
            &*session,
            &corridors,
            ViewCorridorOptions {
                sample_count: samples_per_corridor,
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                category_mask: options.category_mask,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        write_corridor_outputs(&result, summary_output, sample_output);
        // SAFETY: Caller-owned fixed-size outputs were checked for null.
        unsafe {
            output_execution.write(execution_info(&result.execution));
            output_metadata.write(XvViewCorridorMetadata {
                structure_size: structure_size::<XvViewCorridorMetadata>(),
                reserved: 0,
                corridor_count: usize_u64(corridor_count),
                samples_per_corridor: usize_u64(samples_per_corridor),
                sample_count: usize_u64(sample_count),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        }
        Ok(())
    })
}

/// Computes Dynamic Observer Path through a cached VAYU session.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
pub unsafe extern "C" fn xv_compute_observer_path(
    compute_handle: u64,
    paths: *const XvObserverPath,
    path_count: usize,
    vertices: *const XvPoint3,
    vertex_count: usize,
    patches: *const XvViewTargetPatch,
    patch_count: usize,
    options: *const XvObserverPathOptions,
    output_execution: *mut XvDaenaExecutionInfo,
    output_metadata: *mut XvObserverPathMetadata,
    output_summaries: *mut XvObserverPathSummary,
    summary_capacity: usize,
    output_samples: *mut XvObserverPathSample,
    sample_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_execution.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Fixed-size input was validated above.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvObserverPathOptions>()
            || options.reserved != 0
            || options.view.structure_size != structure_size::<XvTargetViewOptions>()
            || options.view.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate input pointers and lengths.
        let path_values = unsafe { input_slice(paths, path_count)? };
        // SAFETY: Shared helpers validate input pointers and lengths.
        let vertex_values = unsafe { input_slice(vertices, vertex_count)? };
        // SAFETY: Shared helpers validate input pointers and lengths.
        let patch_values = unsafe { input_slice(patches, patch_count)? };
        let paths = convert_paths(path_values, vertex_values)?;
        let patches = convert_patches(patch_values)?;
        let expected_samples = paths
            .iter()
            .map(|path| path_sample_count(&path.vertices, options.spacing_meters))
            .try_fold(0_usize, |sum, count| {
                sum.checked_add(count?).ok_or(XvStatus::InvalidLength)
            })?;
        if summary_capacity < path_count || sample_capacity < expected_samples {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacities were checked.
        let summary_output = unsafe { output_slice(output_summaries, path_count)? };
        // SAFETY: Expected sample arithmetic was checked.
        let sample_output = unsafe { output_slice(output_samples, expected_samples)? };
        let session = get_session(compute_handle)?;
        let started = Instant::now();
        let result = analyze_observer_paths_with_executor(
            &*session,
            &paths,
            &patches,
            ObserverPathOptions {
                spacing_meters: options.spacing_meters,
                view: target_options(options.view)?,
            },
        )
        .map_err(visibility_error_status)?;
        if result.samples.len() != expected_samples {
            return Err(XvStatus::InvalidState);
        }
        let elapsed = micros(started);
        write_path_outputs(&result, summary_output, sample_output);
        // SAFETY: Caller-owned fixed-size outputs were checked for null.
        unsafe {
            output_execution.write(execution_info(&result.execution));
            output_metadata.write(XvObserverPathMetadata {
                structure_size: structure_size::<XvObserverPathMetadata>(),
                reserved: 0,
                path_count: usize_u64(path_count),
                sample_count: usize_u64(result.samples.len()),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        }
        Ok(())
    })
}

fn convert_observers(values: &[XvViewObserver]) -> Result<Vec<ViewObserver>, XvStatus> {
    values
        .iter()
        .map(|value| {
            ViewObserver::try_new(
                SensorId::new(value.observer_id),
                Vec3::new(value.position_x, value.position_y, value.position_z),
                Vec3::new(value.forward_x, value.forward_y, value.forward_z),
                Vec3::new(value.up_x, value.up_y, value.up_z),
                value.weight,
            )
            .map_err(visibility_error_status)
        })
        .collect()
}

fn convert_patches(values: &[XvViewTargetPatch]) -> Result<Vec<ViewTargetPatch>, XvStatus> {
    values
        .iter()
        .map(|value| {
            ViewTargetPatch::try_new(
                TargetId::new(value.target_id),
                [
                    Vec3::new(value.first_x, value.first_y, value.first_z),
                    Vec3::new(value.second_x, value.second_y, value.second_z),
                    Vec3::new(value.third_x, value.third_y, value.third_z),
                ],
                value.category_mask,
                value.category_weight,
            )
            .map_err(visibility_error_status)
        })
        .collect()
}

fn convert_paths(
    values: &[XvObserverPath],
    vertices: &[XvPoint3],
) -> Result<Vec<ObserverPath>, XvStatus> {
    values
        .iter()
        .map(|value| {
            let start =
                usize::try_from(value.vertex_offset).map_err(|_| XvStatus::InvalidLength)?;
            let length =
                usize::try_from(value.vertex_count).map_err(|_| XvStatus::InvalidLength)?;
            let end = start.checked_add(length).ok_or(XvStatus::InvalidLength)?;
            let points = vertices
                .get(start..end)
                .ok_or(XvStatus::InvalidLength)?
                .iter()
                .map(|point| Vec3::new(point.x, point.y, point.z))
                .collect();
            ObserverPath::try_new(
                value.path_id,
                points,
                Vec3::new(value.up_x, value.up_y, value.up_z),
                value.weight,
            )
            .map_err(visibility_error_status)
        })
        .collect()
}

fn write_target_outputs(
    result: &xvarna_daena::TargetViewResult,
    summaries: &mut [XvTargetViewSummary],
    entries: &mut [XvTargetViewEntry],
) {
    for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
        *destination = XvTargetViewSummary {
            structure_size: structure_size::<XvTargetViewSummary>(),
            reserved: 0,
            observer_id: value.observer_id.get(),
            fov_solid_angle_steradians: value.fov_solid_angle_steradians,
            potential_target_solid_angle_steradians: value.potential_target_solid_angle_steradians,
            visible_target_solid_angle_steradians: value.visible_target_solid_angle_steradians,
            target_view_fraction: value.target_view_fraction,
            target_universe_visibility_fraction: value.target_universe_visibility_fraction,
            weighted_view_score: value.weighted_view_score,
            green_view_index: value.green_view_index,
            green_share_of_visible_targets: value.green_share_of_visible_targets,
            dominant_target_id: value.dominant_target_id.get(),
            convergence_delta_steradians: value.convergence_delta_steradians,
        };
    }
    for (destination, value) in entries.iter_mut().zip(&result.entries) {
        *destination = XvTargetViewEntry {
            structure_size: structure_size::<XvTargetViewEntry>(),
            reserved: 0,
            observer_id: value.observer_id.get(),
            target_id: value.target_id.get(),
            category_mask: value.category_mask,
            potential_solid_angle_steradians: value.potential_solid_angle_steradians,
            visible_solid_angle_steradians: value.visible_solid_angle_steradians,
            visibility_fraction: value.visibility_fraction,
            fov_fraction: value.fov_fraction,
            weighted_fov_score: value.weighted_fov_score,
            convergence_delta_steradians: value.convergence_delta_steradians,
            eligible_sample_count: usize_u64(value.eligible_sample_count),
            visible_sample_count: usize_u64(value.visible_sample_count),
            dominant_blocker_object_id: value.dominant_blocker_object_id.get(),
            dominant_blocked_solid_angle_steradians: value.dominant_blocked_solid_angle_steradians,
        };
    }
}

fn write_corridor_outputs(
    result: &xvarna_daena::ViewCorridorResult,
    summaries: &mut [XvViewCorridorSummary],
    samples: &mut [XvViewCorridorSample],
) {
    for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
        *destination = XvViewCorridorSummary {
            structure_size: structure_size::<XvViewCorridorSummary>(),
            reserved: 0,
            corridor_id: value.corridor_id,
            aperture_solid_angle_steradians: value.aperture_solid_angle_steradians,
            open_sample_count: usize_u64(value.open_sample_count),
            blocked_sample_count: usize_u64(value.blocked_sample_count),
            open_fraction: value.open_fraction,
            open_solid_angle_steradians: value.open_solid_angle_steradians,
            convergence_delta: value.convergence_delta,
            dominant_blocker_object_id: value.dominant_blocker_object_id.get(),
            dominant_blocker_fraction: value.dominant_blocker_fraction,
            nearest_blocker_distance_meters: value.nearest_blocker_distance_meters,
        };
    }
    for (destination, value) in samples.iter_mut().zip(&result.samples) {
        *destination = XvViewCorridorSample {
            structure_size: structure_size::<XvViewCorridorSample>(),
            state: u32::from(value.state as u8),
            corridor_id: value.corridor_id,
            aperture_x: value.aperture_point.x,
            aperture_y: value.aperture_point.y,
            aperture_z: value.aperture_point.z,
            direction_x: value.direction.x,
            direction_y: value.direction.y,
            direction_z: value.direction.z,
            aperture_distance_meters: value.aperture_distance_meters,
            first_hit_distance_meters: value.first_hit_distance_meters,
            blocker_object_id: value.blocker_object_id.get(),
            blocker_instance_id: value.blocker_instance_id.get(),
            blocker_mesh_id: value.blocker_mesh_id.get(),
            blocker_triangle_id: value.blocker_triangle_id,
            reserved: 0,
        };
    }
}

fn write_path_outputs(
    result: &xvarna_daena::ObserverPathResult,
    summaries: &mut [XvObserverPathSummary],
    samples: &mut [XvObserverPathSample],
) {
    for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
        *destination = XvObserverPathSummary {
            structure_size: structure_size::<XvObserverPathSummary>(),
            reserved: 0,
            path_id: value.path_id,
            path_length_meters: value.path_length_meters,
            sample_count: usize_u64(value.sample_count),
            mean_target_view_fraction: value.mean_target_view_fraction,
            mean_weighted_view_score: value.mean_weighted_view_score,
            mean_green_view_index: value.mean_green_view_index,
            minimum_weighted_view_score: value.minimum_weighted_view_score,
            maximum_weighted_view_score: value.maximum_weighted_view_score,
            worst_sample_index: usize_u64(value.worst_sample_index),
            best_sample_index: usize_u64(value.best_sample_index),
        };
    }
    for (destination, value) in samples.iter_mut().zip(&result.samples) {
        *destination = XvObserverPathSample {
            structure_size: structure_size::<XvObserverPathSample>(),
            reserved: 0,
            path_id: value.path_id,
            sample_index: usize_u64(value.sample_index),
            distance_along_path_meters: value.distance_along_path_meters,
            position_x: value.position.x,
            position_y: value.position.y,
            position_z: value.position.z,
            forward_x: value.forward.x,
            forward_y: value.forward.y,
            forward_z: value.forward.z,
            target_view_fraction: value.target_view_fraction,
            weighted_view_score: value.weighted_view_score,
            green_view_index: value.green_view_index,
            dominant_target_id: value.dominant_target_id.get(),
            convergence_delta_steradians: value.convergence_delta_steradians,
        };
    }
}

fn execution_info(execution: &RayQueryExecutionSummary) -> XvDaenaExecutionInfo {
    let mut flags = 0;
    if execution.used_creation_fallback {
        flags |= EXECUTION_FLAG_CREATION_FALLBACK;
    }
    if execution.adapter_name.is_some() {
        flags |= EXECUTION_FLAG_HAS_ADAPTER;
    }
    if execution.reusable_buffer_batch_count > 0 {
        flags |= EXECUTION_FLAG_REUSABLE_BUFFERS;
    }
    let mut output = XvDaenaExecutionInfo {
        backend: match execution.backend {
            RayQueryBackend::Cpu => 0,
            RayQueryBackend::PortableGpu => 1,
            RayQueryBackend::Mixed => 2,
        },
        flags,
        runtime_fallback_batch_count: usize_u64(execution.runtime_fallback_batch_count),
        batch_count: usize_u64(execution.batch_count),
        ray_count: usize_u64(execution.ray_count),
        dispatch_count: usize_u64(execution.dispatch_count),
        upload_microseconds: execution.upload_microseconds,
        execution_microseconds: execution.execution_microseconds,
        readback_microseconds: execution.readback_microseconds,
        maximum_precision_error_meters: execution.maximum_precision_error_meters,
        ..XvDaenaExecutionInfo::default()
    };
    if let Some(adapter) = &execution.adapter_name {
        write_text(&mut output.adapter_name, adapter);
    }
    if let Some(reason) = &execution.fallback_reason {
        write_text(&mut output.fallback_reason, reason);
    }
    output
}

fn write_text(destination: &mut [u8], value: &str) {
    destination.fill(0);
    let length = value.len().min(destination.len().saturating_sub(1));
    destination[..length].copy_from_slice(&value.as_bytes()[..length]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_layout_is_stable() {
        assert_eq!(core::mem::size_of::<XvDaenaExecutionInfo>(), 464);
    }
}
