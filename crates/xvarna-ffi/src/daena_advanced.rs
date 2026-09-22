//! Fixed-layout advanced DAENA target-view, corridor, and observer-path ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};
use xvarna_daena::{
    ObserverPath, ObserverPathOptions, TargetViewOptions, ViewCorridor, ViewCorridorOptions,
    ViewObserver, ViewTargetPatch, VisibilityError, analyze_observer_paths, analyze_target_view,
    analyze_view_corridors,
};
use xvarna_geometry::Vec3;
use xvarna_types::{SensorId, TargetId};

const TARGET_TWO_SIDED_FLAG: u32 = 1 << 0;

/// Fixed-layout oriented observer camera.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewObserver {
    /// Stable observer identifier.
    pub observer_id: u64,
    /// Eye position.
    pub position_x: f64,
    /// Eye position.
    pub position_y: f64,
    /// Eye position.
    pub position_z: f64,
    /// Camera-forward direction.
    pub forward_x: f64,
    /// Camera-forward direction.
    pub forward_y: f64,
    /// Camera-forward direction.
    pub forward_z: f64,
    /// Camera-up direction.
    pub up_x: f64,
    /// Camera-up direction.
    pub up_y: f64,
    /// Camera-up direction.
    pub up_z: f64,
    /// Observer importance.
    pub weight: f64,
}

/// Fixed-layout triangular logical-target patch.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewTargetPatch {
    /// Logical target identifier; repeated IDs are aggregated.
    pub target_id: u64,
    /// First vertex.
    pub first_x: f64,
    /// First vertex.
    pub first_y: f64,
    /// First vertex.
    pub first_z: f64,
    /// Second vertex.
    pub second_x: f64,
    /// Second vertex.
    pub second_y: f64,
    /// Second vertex.
    pub second_z: f64,
    /// Third vertex.
    pub third_x: f64,
    /// Third vertex.
    pub third_y: f64,
    /// Third vertex.
    pub third_z: f64,
    /// Target category bits.
    pub category_mask: u64,
    /// Category/desirability weight.
    pub category_weight: f64,
}

/// Fixed-layout Target/Weighted/Green View policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTargetViewOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Stable option flags.
    pub flags: u32,
    /// Samples per triangular target patch.
    pub samples_per_patch: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Rectangular horizontal FOV in radians.
    pub horizontal_fov_radians: f64,
    /// Rectangular vertical FOV in radians.
    pub vertical_fov_radians: f64,
    /// Maximum target distance.
    pub maximum_distance_meters: f64,
    /// Endpoint clearance.
    pub endpoint_clearance_meters: f64,
    /// Distance weighting reference.
    pub distance_reference_meters: f64,
    /// Distance falloff exponent.
    pub distance_exponent: f64,
    /// Camera-centre direction exponent.
    pub direction_exponent: f64,
    /// Included occluder categories.
    pub occluder_category_mask: u64,
    /// Included target categories.
    pub target_category_mask: u64,
    /// Categories counted as green.
    pub green_category_mask: u64,
}

/// Fixed-layout observer/logical-target view aggregate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTargetViewEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Observer identifier.
    pub observer_id: u64,
    /// Target identifier.
    pub target_id: u64,
    /// Union of target categories.
    pub category_mask: u64,
    /// Potential target solid angle.
    pub potential_solid_angle_steradians: f64,
    /// Visible target solid angle.
    pub visible_solid_angle_steradians: f64,
    /// Visible/potential target ratio.
    pub visibility_fraction: f64,
    /// Visible solid-angle share of FOV.
    pub fov_fraction: f64,
    /// Weighted contribution share of FOV.
    pub weighted_fov_score: f64,
    /// Interleaved sampling convergence delta.
    pub convergence_delta_steradians: f64,
    /// Eligible samples.
    pub eligible_sample_count: u64,
    /// Visible samples.
    pub visible_sample_count: u64,
    /// Dominant blocker object.
    pub dominant_blocker_object_id: u64,
    /// Dominant blocked solid angle.
    pub dominant_blocked_solid_angle_steradians: f64,
}

/// Fixed-layout per-observer view summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTargetViewSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Observer identifier.
    pub observer_id: u64,
    /// Camera FOV solid angle.
    pub fov_solid_angle_steradians: f64,
    /// Potential target solid angle.
    pub potential_target_solid_angle_steradians: f64,
    /// Visible target solid angle.
    pub visible_target_solid_angle_steradians: f64,
    /// Target share of camera FOV.
    pub target_view_fraction: f64,
    /// Visibility relative to target universe.
    pub target_universe_visibility_fraction: f64,
    /// Weighted view quality.
    pub weighted_view_score: f64,
    /// Green View Index.
    pub green_view_index: f64,
    /// Green share of visible targets.
    pub green_share_of_visible_targets: f64,
    /// Dominant target identifier.
    pub dominant_target_id: u64,
    /// Total convergence delta.
    pub convergence_delta_steradians: f64,
}

/// Fixed-layout Target View dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTargetViewMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Observer count.
    pub observer_count: u64,
    /// Distinct target count.
    pub target_count: u64,
    /// Observer-target entry count.
    pub entry_count: u64,
    /// Native runtime.
    pub analysis_time_microseconds: u64,
    /// Content hash words.
    pub content_hash_0: u64,
    /// Content hash words.
    pub content_hash_1: u64,
    /// Content hash words.
    pub content_hash_2: u64,
    /// Content hash words.
    pub content_hash_3: u64,
}

/// Computes solid-angle Target/Weighted/Green View.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
pub unsafe extern "C" fn xv_scene_target_view(
    scene_handle: u64,
    observers: *const XvViewObserver,
    observer_count: usize,
    patches: *const XvViewTargetPatch,
    patch_count: usize,
    options: *const XvTargetViewOptions,
    output_metadata: *mut XvTargetViewMetadata,
    output_summaries: *mut XvTargetViewSummary,
    summary_capacity: usize,
    output_entries: *mut XvTargetViewEntry,
    entry_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvTargetViewOptions>()
            || options.reserved != 0
            || options.flags & !TARGET_TWO_SIDED_FLAG != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate array pointers and lengths.
        let observer_values = unsafe { input_slice(observers, observer_count)? };
        // SAFETY: Shared helpers validate array pointers and lengths.
        let patch_values = unsafe { input_slice(patches, patch_count)? };
        let distinct_targets = patch_values
            .iter()
            .filter(|patch| patch.category_mask & options.target_category_mask != 0)
            .map(|patch| patch.target_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let entry_count = observer_count
            .checked_mul(distinct_targets)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < observer_count || entry_capacity < entry_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacities were validated.
        let summary_output = unsafe { output_slice(output_summaries, observer_count)? };
        // SAFETY: Capacity multiplication was checked.
        let entry_output = unsafe { output_slice(output_entries, entry_count)? };
        let observers = observer_values
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
            .collect::<Result<Vec<_>, _>>()?;
        let patches = patch_values
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
            .collect::<Result<Vec<_>, _>>()?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_target_view(
            &scene,
            &observers,
            &patches,
            TargetViewOptions {
                samples_per_patch: usize::try_from(options.samples_per_patch)
                    .map_err(|_| XvStatus::InvalidLength)?,
                horizontal_fov_radians: options.horizontal_fov_radians,
                vertical_fov_radians: options.vertical_fov_radians,
                maximum_distance_meters: options.maximum_distance_meters,
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                occluder_category_mask: options.occluder_category_mask,
                target_category_mask: options.target_category_mask,
                green_category_mask: options.green_category_mask,
                distance_reference_meters: options.distance_reference_meters,
                distance_exponent: options.distance_exponent,
                direction_exponent: options.direction_exponent,
                two_sided_targets: options.flags & TARGET_TWO_SIDED_FLAG != 0,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        for (destination, value) in summary_output.iter_mut().zip(&result.summaries) {
            *destination = XvTargetViewSummary {
                structure_size: structure_size::<XvTargetViewSummary>(),
                reserved: 0,
                observer_id: value.observer_id.get(),
                fov_solid_angle_steradians: value.fov_solid_angle_steradians,
                potential_target_solid_angle_steradians: value
                    .potential_target_solid_angle_steradians,
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
        for (destination, value) in entry_output.iter_mut().zip(&result.entries) {
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
                dominant_blocked_solid_angle_steradians: value
                    .dominant_blocked_solid_angle_steradians,
            };
        }
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
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

/// Fixed-layout view-corridor definition.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewCorridor {
    /// Stable corridor identifier.
    pub corridor_id: u64,
    /// Apex/origin.
    pub origin_x: f64,
    /// Apex/origin.
    pub origin_y: f64,
    /// Apex/origin.
    pub origin_z: f64,
    /// Aperture centre.
    pub target_x: f64,
    /// Aperture centre.
    pub target_y: f64,
    /// Aperture centre.
    pub target_z: f64,
    /// Camera up.
    pub up_x: f64,
    /// Camera up.
    pub up_y: f64,
    /// Camera up.
    pub up_z: f64,
    /// Aperture radius.
    pub target_radius_meters: f64,
}

/// Fixed-layout corridor policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewCorridorOptions {
    /// Byte size.
    pub structure_size: u32,
    /// Uniform-area sample count.
    pub sample_count: u32,
    /// Endpoint clearance.
    pub endpoint_clearance_meters: f64,
    /// Included occluder categories.
    pub category_mask: u64,
}

/// Fixed-layout protected-corridor summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewCorridorSummary {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Corridor identifier.
    pub corridor_id: u64,
    /// Aperture solid angle.
    pub aperture_solid_angle_steradians: f64,
    /// Open samples.
    pub open_sample_count: u64,
    /// Blocked samples.
    pub blocked_sample_count: u64,
    /// Open fraction.
    pub open_fraction: f64,
    /// Open solid angle.
    pub open_solid_angle_steradians: f64,
    /// Convergence delta.
    pub convergence_delta: f64,
    /// Dominant blocker.
    pub dominant_blocker_object_id: u64,
    /// Dominant blocker share.
    pub dominant_blocker_fraction: f64,
    /// Nearest blocker distance.
    pub nearest_blocker_distance_meters: f64,
}

/// Fixed-layout protected-corridor sample.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewCorridorSample {
    /// Byte size.
    pub structure_size: u32,
    /// Numeric visibility state.
    pub state: u32,
    /// Corridor identifier.
    pub corridor_id: u64,
    /// Aperture point.
    pub aperture_x: f64,
    /// Aperture point.
    pub aperture_y: f64,
    /// Aperture point.
    pub aperture_z: f64,
    /// Ray direction.
    pub direction_x: f64,
    /// Ray direction.
    pub direction_y: f64,
    /// Ray direction.
    pub direction_z: f64,
    /// Aperture distance.
    pub aperture_distance_meters: f64,
    /// First-hit distance.
    pub first_hit_distance_meters: f64,
    /// Blocker source identity.
    pub blocker_object_id: u64,
    /// Blocker source identity.
    pub blocker_instance_id: u64,
    /// Blocker source identity.
    pub blocker_mesh_id: u64,
    /// Blocker triangle identity.
    pub blocker_triangle_id: u32,
    /// Reserved.
    pub reserved: u32,
}

/// Fixed-layout corridor result metadata.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewCorridorMetadata {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Corridor count.
    pub corridor_count: u64,
    /// Samples per corridor.
    pub samples_per_corridor: u64,
    /// Total sample count.
    pub sample_count: u64,
    /// Native runtime.
    pub analysis_time_microseconds: u64,
    /// Content hash.
    pub content_hash_0: u64,
    /// Content hash.
    pub content_hash_1: u64,
    /// Content hash.
    pub content_hash_2: u64,
    /// Content hash.
    pub content_hash_3: u64,
}

/// Computes deterministic protected view-corridor conflicts.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_view_corridor(
    scene_handle: u64,
    corridors: *const XvViewCorridor,
    corridor_count: usize,
    options: *const XvViewCorridorOptions,
    output_metadata: *mut XvViewCorridorMetadata,
    output_summaries: *mut XvViewCorridorSummary,
    summary_capacity: usize,
    output_samples: *mut XvViewCorridorSample,
    sample_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null.
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
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_view_corridors(
            &scene,
            &corridors,
            ViewCorridorOptions {
                sample_count: samples_per_corridor,
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                category_mask: options.category_mask,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        for (destination, value) in summary_output.iter_mut().zip(&result.summaries) {
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
        for (destination, value) in sample_output.iter_mut().zip(&result.samples) {
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
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
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

/// Fixed-layout 3D point.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPoint3 {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
}

/// Fixed-layout descriptor into a shared observer-path vertex array.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverPath {
    /// Stable path identifier.
    pub path_id: u64,
    /// First vertex in the shared array.
    pub vertex_offset: u64,
    /// Number of vertices.
    pub vertex_count: u64,
    /// Camera-up direction.
    pub up_x: f64,
    /// Camera-up direction.
    pub up_y: f64,
    /// Camera-up direction.
    pub up_z: f64,
    /// Observer importance.
    pub weight: f64,
}

/// Fixed-layout dynamic observer-path policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverPathOptions {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Uniform path spacing.
    pub spacing_meters: f64,
    /// Nested Target View options.
    pub view: XvTargetViewOptions,
}

/// Fixed-layout dynamic observer-path sample.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverPathSample {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Source path.
    pub path_id: u64,
    /// Sample index.
    pub sample_index: u64,
    /// Accumulated path distance.
    pub distance_along_path_meters: f64,
    /// Sample position.
    pub position_x: f64,
    /// Sample position.
    pub position_y: f64,
    /// Sample position.
    pub position_z: f64,
    /// Camera tangent.
    pub forward_x: f64,
    /// Camera tangent.
    pub forward_y: f64,
    /// Camera tangent.
    pub forward_z: f64,
    /// Raw target view.
    pub target_view_fraction: f64,
    /// Weighted view.
    pub weighted_view_score: f64,
    /// Green View Index.
    pub green_view_index: f64,
    /// Dominant target.
    pub dominant_target_id: u64,
    /// Convergence delta.
    pub convergence_delta_steradians: f64,
}

/// Fixed-layout dynamic observer-path summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverPathSummary {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Path identifier.
    pub path_id: u64,
    /// Path length.
    pub path_length_meters: f64,
    /// Sample count.
    pub sample_count: u64,
    /// Mean Target View.
    pub mean_target_view_fraction: f64,
    /// Mean Weighted View.
    pub mean_weighted_view_score: f64,
    /// Mean Green View.
    pub mean_green_view_index: f64,
    /// Minimum weighted score.
    pub minimum_weighted_view_score: f64,
    /// Maximum weighted score.
    pub maximum_weighted_view_score: f64,
    /// Worst sample index.
    pub worst_sample_index: u64,
    /// Best sample index.
    pub best_sample_index: u64,
}

/// Fixed-layout observer-path dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverPathMetadata {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Path count.
    pub path_count: u64,
    /// Total sample count.
    pub sample_count: u64,
    /// Native runtime.
    pub analysis_time_microseconds: u64,
    /// Content hash.
    pub content_hash_0: u64,
    /// Content hash.
    pub content_hash_1: u64,
    /// Content hash.
    pub content_hash_2: u64,
    /// Content hash.
    pub content_hash_3: u64,
}

/// Computes Dynamic Observer Path Target/Weighted/Green View.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
pub unsafe extern "C" fn xv_scene_observer_path(
    scene_handle: u64,
    paths: *const XvObserverPath,
    path_count: usize,
    vertices: *const XvPoint3,
    vertex_count: usize,
    patches: *const XvViewTargetPatch,
    patch_count: usize,
    options: *const XvObserverPathOptions,
    output_metadata: *mut XvObserverPathMetadata,
    output_summaries: *mut XvObserverPathSummary,
    summary_capacity: usize,
    output_samples: *mut XvObserverPathSample,
    sample_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvObserverPathOptions>()
            || options.reserved != 0
            || options.view.structure_size != structure_size::<XvTargetViewOptions>()
            || options.view.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate pointers and lengths.
        let path_values = unsafe { input_slice(paths, path_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let vertex_values = unsafe { input_slice(vertices, vertex_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let patch_values = unsafe { input_slice(patches, patch_count)? };
        let paths = path_values
            .iter()
            .map(|value| {
                let start =
                    usize::try_from(value.vertex_offset).map_err(|_| XvStatus::InvalidLength)?;
                let length =
                    usize::try_from(value.vertex_count).map_err(|_| XvStatus::InvalidLength)?;
                let end = start.checked_add(length).ok_or(XvStatus::InvalidLength)?;
                let points = vertex_values
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
            .collect::<Result<Vec<_>, _>>()?;
        let patches = patch_values
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
            .collect::<Result<Vec<_>, _>>()?;
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
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_observer_paths(
            &scene,
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
        for (destination, value) in summary_output.iter_mut().zip(&result.summaries) {
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
        for (destination, value) in sample_output.iter_mut().zip(&result.samples) {
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
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
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

pub fn target_options(options: XvTargetViewOptions) -> Result<TargetViewOptions, XvStatus> {
    if options.flags & !TARGET_TWO_SIDED_FLAG != 0 {
        return Err(XvStatus::InvalidArgument);
    }
    Ok(TargetViewOptions {
        samples_per_patch: usize::try_from(options.samples_per_patch)
            .map_err(|_| XvStatus::InvalidLength)?,
        horizontal_fov_radians: options.horizontal_fov_radians,
        vertical_fov_radians: options.vertical_fov_radians,
        maximum_distance_meters: options.maximum_distance_meters,
        endpoint_clearance_meters: options.endpoint_clearance_meters,
        occluder_category_mask: options.occluder_category_mask,
        target_category_mask: options.target_category_mask,
        green_category_mask: options.green_category_mask,
        distance_reference_meters: options.distance_reference_meters,
        distance_exponent: options.distance_exponent,
        direction_exponent: options.direction_exponent,
        two_sided_targets: options.flags & TARGET_TWO_SIDED_FLAG != 0,
    })
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn path_sample_count(vertices: &[Vec3], spacing: f64) -> Result<usize, XvStatus> {
    if !spacing.is_finite() || spacing <= 0.0 {
        return Err(XvStatus::InvalidArgument);
    }
    let length = vertices
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).length_squared().sqrt())
        .filter(|length| *length > 1.0e-12)
        .sum::<f64>();
    if !length.is_finite() || length <= 0.0 {
        return Err(XvStatus::InvalidArgument);
    }
    let intervals = (length / spacing).ceil();
    if intervals > usize::MAX as f64 {
        return Err(XvStatus::InvalidLength);
    }
    Ok((intervals as usize).saturating_add(1).max(2))
}

pub const fn visibility_error_status(error: VisibilityError) -> XvStatus {
    match error {
        VisibilityError::ExecutionFailed => XvStatus::InvalidState,
        _ => XvStatus::InvalidArgument,
    }
}

pub fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

pub fn usize_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

pub fn micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

pub fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_daena_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvViewObserver>(), 88);
        assert_eq!(mem::size_of::<XvViewTargetPatch>(), 96);
        assert_eq!(mem::size_of::<XvTargetViewOptions>(), 96);
        assert_eq!(mem::size_of::<XvTargetViewEntry>(), 112);
        assert_eq!(mem::size_of::<XvTargetViewSummary>(), 96);
        assert_eq!(mem::size_of::<XvTargetViewMetadata>(), 72);
        assert_eq!(mem::size_of::<XvViewCorridor>(), 88);
        assert_eq!(mem::size_of::<XvViewCorridorOptions>(), 24);
        assert_eq!(mem::size_of::<XvViewCorridorSummary>(), 88);
        assert_eq!(mem::size_of::<XvViewCorridorSample>(), 112);
        assert_eq!(mem::size_of::<XvViewCorridorMetadata>(), 72);
        assert_eq!(mem::size_of::<XvPoint3>(), 24);
        assert_eq!(mem::size_of::<XvObserverPath>(), 56);
        assert_eq!(mem::size_of::<XvObserverPathOptions>(), 112);
        assert_eq!(mem::size_of::<XvObserverPathSample>(), 120);
        assert_eq!(mem::size_of::<XvObserverPathSummary>(), 88);
        assert_eq!(mem::size_of::<XvObserverPathMetadata>(), 64);
    }
}
