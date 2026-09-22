//! Fixed-layout DAENA isovist, intervisibility, privacy, and graph ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};
use xvarna_daena::{
    IntervisibilityOptions, IsovistOptions, SparseVisibilityGraphOptions, Viewpoint,
    VisibilityError, VisibilityGraphOptions, VisibilityNode, VisibilityObserver, VisibilityTarget,
    analyze_intervisibility, analyze_isovists, analyze_sparse_visibility_graph,
    analyze_visibility_graph,
};
use xvarna_geometry::Vec3;
use xvarna_types::{SensorId, TargetId};

const TARGET_HAS_FACING_FLAG: u32 = 1 << 0;
const GRAPH_COMPUTE_CENTRALITY_FLAG: u32 = 1 << 0;

/// Fixed-layout eye point and planar frame.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvViewpoint {
    /// Stable viewpoint identifier.
    pub viewpoint_id: u64,
    /// Eye X coordinate in canonical metres.
    pub position_x: f64,
    /// Eye Y coordinate.
    pub position_y: f64,
    /// Eye Z coordinate.
    pub position_z: f64,
    /// Plane-normal X.
    pub plane_normal_x: f64,
    /// Plane-normal Y.
    pub plane_normal_y: f64,
    /// Plane-normal Z.
    pub plane_normal_z: f64,
    /// Forward X.
    pub forward_x: f64,
    /// Forward Y.
    pub forward_y: f64,
    /// Forward Z.
    pub forward_z: f64,
}

/// Fixed-layout planar isovist policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovistOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Angular sample count.
    pub sample_count: u32,
    /// Field of view in radians.
    pub field_of_view_radians: f64,
    /// Finite radial clipping distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Eye offset along the plane normal.
    pub eye_offset_meters: f64,
    /// Included instance categories.
    pub category_mask: u64,
}

/// Fixed-layout isovist summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovistSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable viewpoint identifier.
    pub viewpoint_id: u64,
    /// Polygon area in square canonical metres.
    pub area_square_meters: f64,
    /// Polygon perimeter in canonical metres.
    pub perimeter_meters: f64,
    /// Eye-to-centroid distance.
    pub centroid_distance_meters: f64,
    /// Mean radial length.
    pub mean_radial_meters: f64,
    /// Minimum radial length.
    pub minimum_radial_meters: f64,
    /// Maximum radial length.
    pub maximum_radial_meters: f64,
    /// Radial population standard deviation.
    pub radial_standard_deviation_meters: f64,
    /// Radial skewness.
    pub radial_skewness: f64,
    /// Circular compactness.
    pub compactness: f64,
    /// Interleaved half-set area convergence delta.
    pub area_convergence_delta_square_meters: f64,
    /// Occluded ray count.
    pub occluded_count: u64,
    /// Open-at-limit ray count.
    pub open_count: u64,
    /// Dominant blocker object.
    pub dominant_occluder_object_id: u64,
    /// Dominant blocker share of all directions.
    pub dominant_occluder_fraction: f64,
}

/// Fixed-layout ordered isovist boundary sample.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovistRay {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Numeric [`xvarna_daena::IsovistRayState`].
    pub state: u32,
    /// Unit direction X.
    pub direction_x: f64,
    /// Unit direction Y.
    pub direction_y: f64,
    /// Unit direction Z.
    pub direction_z: f64,
    /// Boundary endpoint X.
    pub endpoint_x: f64,
    /// Boundary endpoint Y.
    pub endpoint_y: f64,
    /// Boundary endpoint Z.
    pub endpoint_z: f64,
    /// Eye-to-boundary distance.
    pub distance_meters: f64,
    /// First blocking object, or zero.
    pub object_id: u64,
    /// First blocking occurrence, or zero.
    pub instance_id: u64,
    /// First blocking mesh resource, or zero.
    pub mesh_id: u64,
    /// First blocking triangle, or `UINT32_MAX`.
    pub triangle_id: u32,
    /// Reserved; always zero.
    pub reserved: u32,
}

/// Isovist dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovistMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source viewpoint count.
    pub viewpoint_count: u64,
    /// Samples per viewpoint.
    pub sample_count: u64,
    /// Total ray output count.
    pub ray_count: u64,
    /// Native analysis duration in microseconds.
    pub analysis_time_microseconds: u64,
    /// First BLAKE3 hash word.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Fixed-layout weighted observer.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityObserver {
    /// Stable observer identifier.
    pub observer_id: u64,
    /// Eye X.
    pub position_x: f64,
    /// Eye Y.
    pub position_y: f64,
    /// Eye Z.
    pub position_z: f64,
    /// Observer importance from zero through one.
    pub weight: f64,
}

/// Fixed-layout privacy/view target.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityTarget {
    /// Stable target identifier.
    pub target_id: u64,
    /// Target X.
    pub position_x: f64,
    /// Target Y.
    pub position_y: f64,
    /// Target Z.
    pub position_z: f64,
    /// Optional facing X.
    pub facing_x: f64,
    /// Optional facing Y.
    pub facing_y: f64,
    /// Optional facing Z.
    pub facing_z: f64,
    /// Privacy sensitivity from zero through one.
    pub sensitivity: f64,
    /// Bit zero marks the facing vector as present.
    pub flags: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
}

/// Fixed-layout directed visibility and privacy policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIntervisibilityOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Clearance removed from both endpoints.
    pub endpoint_clearance_meters: f64,
    /// Maximum pair distance.
    pub maximum_distance_meters: f64,
    /// Privacy distance where the multiplier is one half.
    pub privacy_reference_distance_meters: f64,
    /// Directional cosine exponent.
    pub facing_exponent: f64,
    /// Included instance categories.
    pub category_mask: u64,
}

/// Fixed-layout row-major observer-target entry.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIntervisibilityEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Numeric [`xvarna_daena::VisibilityState`].
    pub state: u32,
    /// Endpoint distance.
    pub distance_meters: f64,
    /// Observer-to-target unit direction X.
    pub direction_x: f64,
    /// Direction Y.
    pub direction_y: f64,
    /// Direction Z.
    pub direction_z: f64,
    /// Pair privacy risk.
    pub privacy_risk: f64,
    /// Directional multiplier.
    pub facing_factor: f64,
    /// Distance multiplier.
    pub distance_factor: f64,
    /// First blocking object, or zero.
    pub blocker_object_id: u64,
    /// First blocking occurrence, or zero.
    pub blocker_instance_id: u64,
    /// First blocking mesh resource, or zero.
    pub blocker_mesh_id: u64,
    /// First blocking triangle, or `UINT32_MAX`.
    pub blocker_triangle_id: u32,
    /// Reserved; always zero.
    pub reserved: u32,
}

/// Fixed-layout observer row aggregate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvObserverVisibilitySummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable observer identifier.
    pub observer_id: u64,
    /// Visible target count.
    pub visible_count: u64,
    /// Visible fraction.
    pub visible_fraction: f64,
    /// Sum of pair privacy risk.
    pub total_privacy_risk: f64,
    /// Peak pair privacy risk.
    pub peak_privacy_risk: f64,
}

/// Fixed-layout target column aggregate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvTargetVisibilitySummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable target identifier.
    pub target_id: u64,
    /// Visible observer count.
    pub visible_observer_count: u64,
    /// Exposure fraction.
    pub exposure_fraction: f64,
    /// Sum of pair risks.
    pub cumulative_privacy_risk: f64,
    /// Complement-product combined risk.
    pub combined_privacy_risk: f64,
}

/// Directed visibility matrix dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIntervisibilityMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Observer row count.
    pub observer_count: u64,
    /// Target column count.
    pub target_count: u64,
    /// Matrix entry count.
    pub entry_count: u64,
    /// Native analysis duration in microseconds.
    pub analysis_time_microseconds: u64,
    /// First BLAKE3 hash word.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Fixed-layout graph node.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityNode {
    /// Stable node identifier.
    pub node_id: u64,
    /// Position X.
    pub position_x: f64,
    /// Position Y.
    pub position_y: f64,
    /// Position Z.
    pub position_z: f64,
}

/// Fixed-layout all-pairs graph policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityGraphOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Bit zero enables centrality calculation.
    pub flags: u32,
    /// Endpoint clearance.
    pub endpoint_clearance_meters: f64,
    /// Maximum edge distance.
    pub maximum_distance_meters: f64,
    /// Included instance categories.
    pub category_mask: u64,
}

/// Fixed-layout spatially indexed sparse-graph policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSparseVisibilityGraphOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Bit zero enables centrality calculation.
    pub flags: u32,
    /// Endpoint clearance.
    pub endpoint_clearance_meters: f64,
    /// Finite spatial search radius.
    pub maximum_distance_meters: f64,
    /// Included instance categories.
    pub category_mask: u64,
    /// Hard maximum candidate degree.
    pub maximum_neighbors: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
}

/// Fixed-layout unordered graph pair.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityGraphPair {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Numeric pair state.
    pub state: u32,
    /// First node index.
    pub first_index: u64,
    /// Second node index.
    pub second_index: u64,
    /// Endpoint distance.
    pub distance_meters: f64,
    /// First blocking object, or zero.
    pub blocker_object_id: u64,
}

/// Fixed-layout per-node graph metrics.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityNodeMetrics {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable node identifier.
    pub node_id: u64,
    /// Visible degree.
    pub degree: u64,
    /// Normalized degree centrality.
    pub degree_centrality: f64,
    /// Connected component index.
    pub component_index: u64,
    /// Normalized harmonic closeness.
    pub harmonic_closeness: f64,
    /// Normalized Brandes betweenness.
    pub betweenness_centrality: f64,
}

/// Graph dimensions, topology, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVisibilityGraphMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source node count.
    pub node_count: u64,
    /// Unordered pair count.
    pub pair_count: u64,
    /// Visible edge count.
    pub visible_edge_count: u64,
    /// Connected component count.
    pub connected_component_count: u64,
    /// Native analysis duration in microseconds.
    pub analysis_time_microseconds: u64,
    /// First BLAKE3 hash word.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Computes complete viewpoint-major planar isovists.
///
/// # Safety
///
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_isovist(
    scene_handle: u64,
    viewpoints: *const XvViewpoint,
    viewpoint_count: usize,
    options: *const XvIsovistOptions,
    output_metadata: *mut XvIsovistMetadata,
    output_summaries: *mut XvIsovistSummary,
    summary_capacity: usize,
    output_rays: *mut XvIsovistRay,
    ray_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size pointer is non-null and readable for this call.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvIsovistOptions>() {
            return Err(XvStatus::InvalidArgument);
        }
        let sample_count =
            usize::try_from(options.sample_count).map_err(|_| XvStatus::InvalidLength)?;
        let required_rays = viewpoint_count
            .checked_mul(sample_count)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < viewpoint_count || ray_capacity < required_rays {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared slice helper validates pointer and declared element count.
        let viewpoints = unsafe { input_slice(viewpoints, viewpoint_count)? };
        // SAFETY: Capacities were validated against the required result dimensions.
        let summaries = unsafe { output_slice(output_summaries, viewpoint_count)? };
        // SAFETY: Capacity multiplication was checked above.
        let rays = unsafe { output_slice(output_rays, required_rays)? };
        let viewpoints = viewpoints
            .iter()
            .map(|value| {
                Viewpoint::try_new(
                    SensorId::new(value.viewpoint_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    Vec3::new(
                        value.plane_normal_x,
                        value.plane_normal_y,
                        value.plane_normal_z,
                    ),
                    Vec3::new(value.forward_x, value.forward_y, value.forward_z),
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_isovists(
            &scene,
            &viewpoints,
            IsovistOptions {
                sample_count,
                field_of_view_radians: options.field_of_view_radians,
                maximum_distance_meters: options.maximum_distance_meters,
                eye_offset_meters: options.eye_offset_meters,
                category_mask: options.category_mask,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
            *destination = XvIsovistSummary {
                structure_size: structure_size::<XvIsovistSummary>(),
                reserved: 0,
                viewpoint_id: value.viewpoint_id.get(),
                area_square_meters: value.area_square_meters,
                perimeter_meters: value.perimeter_meters,
                centroid_distance_meters: value.centroid_distance_meters,
                mean_radial_meters: value.mean_radial_meters,
                minimum_radial_meters: value.minimum_radial_meters,
                maximum_radial_meters: value.maximum_radial_meters,
                radial_standard_deviation_meters: value.radial_standard_deviation_meters,
                radial_skewness: value.radial_skewness,
                compactness: value.compactness,
                area_convergence_delta_square_meters: value.area_convergence_delta_square_meters,
                occluded_count: usize_u64(value.occluded_count),
                open_count: usize_u64(value.open_count),
                dominant_occluder_object_id: value.dominant_occluder_object_id.get(),
                dominant_occluder_fraction: value.dominant_occluder_fraction,
            };
        }
        for (destination, value) in rays.iter_mut().zip(&result.rays) {
            *destination = XvIsovistRay {
                structure_size: structure_size::<XvIsovistRay>(),
                state: u32::from(value.state as u8),
                direction_x: value.direction.x,
                direction_y: value.direction.y,
                direction_z: value.direction.z,
                endpoint_x: value.endpoint.x,
                endpoint_y: value.endpoint.y,
                endpoint_z: value.endpoint.z,
                distance_meters: value.distance_meters,
                object_id: value.object_id.get(),
                instance_id: value.instance_id.get(),
                mesh_id: value.mesh_id.get(),
                triangle_id: value.triangle_id,
                reserved: 0,
            };
        }
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
            output_metadata.write(XvIsovistMetadata {
                structure_size: structure_size::<XvIsovistMetadata>(),
                reserved: 0,
                viewpoint_count: usize_u64(viewpoint_count),
                sample_count: usize_u64(sample_count),
                ray_count: usize_u64(required_rays),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        };
        Ok(())
    })
}

/// Computes a complete directed observer-target visibility and privacy matrix.
///
/// # Safety
///
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_intervisibility(
    scene_handle: u64,
    observers: *const XvVisibilityObserver,
    observer_count: usize,
    targets: *const XvVisibilityTarget,
    target_count: usize,
    options: *const XvIntervisibilityOptions,
    output_metadata: *mut XvIntervisibilityMetadata,
    output_observer_summaries: *mut XvObserverVisibilitySummary,
    observer_summary_capacity: usize,
    output_target_summaries: *mut XvTargetVisibilitySummary,
    target_summary_capacity: usize,
    output_entries: *mut XvIntervisibilityEntry,
    entry_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size pointer is non-null and readable.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvIntervisibilityOptions>()
            || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let entry_count = observer_count
            .checked_mul(target_count)
            .ok_or(XvStatus::InvalidLength)?;
        if observer_summary_capacity < observer_count
            || target_summary_capacity < target_count
            || entry_capacity < entry_count
        {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate every pointer and element count.
        let observer_values = unsafe { input_slice(observers, observer_count)? };
        // SAFETY: Shared helpers validate every pointer and element count.
        let target_values = unsafe { input_slice(targets, target_count)? };
        // SAFETY: Output capacities were validated above.
        let observer_output = unsafe { output_slice(output_observer_summaries, observer_count)? };
        // SAFETY: Output capacities were validated above.
        let target_output = unsafe { output_slice(output_target_summaries, target_count)? };
        // SAFETY: Output capacities were validated above.
        let entry_output = unsafe { output_slice(output_entries, entry_count)? };
        let observers = observer_values
            .iter()
            .map(|value| {
                VisibilityObserver::try_new(
                    SensorId::new(value.observer_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    value.weight,
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let targets = target_values
            .iter()
            .map(|value| {
                if value.flags & !TARGET_HAS_FACING_FLAG != 0 || value.reserved != 0 {
                    return Err(XvStatus::InvalidArgument);
                }
                let facing = (value.flags & TARGET_HAS_FACING_FLAG != 0)
                    .then(|| Vec3::new(value.facing_x, value.facing_y, value.facing_z));
                VisibilityTarget::try_new(
                    TargetId::new(value.target_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    facing,
                    value.sensitivity,
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_intervisibility(
            &scene,
            &observers,
            &targets,
            IntervisibilityOptions {
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                maximum_distance_meters: options.maximum_distance_meters,
                privacy_reference_distance_meters: options.privacy_reference_distance_meters,
                facing_exponent: options.facing_exponent,
                category_mask: options.category_mask,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        for (destination, value) in observer_output.iter_mut().zip(&result.observer_summaries) {
            *destination = XvObserverVisibilitySummary {
                structure_size: structure_size::<XvObserverVisibilitySummary>(),
                reserved: 0,
                observer_id: value.observer_id.get(),
                visible_count: usize_u64(value.visible_count),
                visible_fraction: value.visible_fraction,
                total_privacy_risk: value.total_privacy_risk,
                peak_privacy_risk: value.peak_privacy_risk,
            };
        }
        for (destination, value) in target_output.iter_mut().zip(&result.target_summaries) {
            *destination = XvTargetVisibilitySummary {
                structure_size: structure_size::<XvTargetVisibilitySummary>(),
                reserved: 0,
                target_id: value.target_id.get(),
                visible_observer_count: usize_u64(value.visible_observer_count),
                exposure_fraction: value.exposure_fraction,
                cumulative_privacy_risk: value.cumulative_privacy_risk,
                combined_privacy_risk: value.combined_privacy_risk,
            };
        }
        for (destination, value) in entry_output.iter_mut().zip(&result.entries) {
            *destination = XvIntervisibilityEntry {
                structure_size: structure_size::<XvIntervisibilityEntry>(),
                state: u32::from(value.state as u8),
                distance_meters: value.distance_meters,
                direction_x: value.direction.x,
                direction_y: value.direction.y,
                direction_z: value.direction.z,
                privacy_risk: value.privacy_risk,
                facing_factor: value.facing_factor,
                distance_factor: value.distance_factor,
                blocker_object_id: value.blocker_object_id.get(),
                blocker_instance_id: value.blocker_instance_id.get(),
                blocker_mesh_id: value.blocker_mesh_id.get(),
                blocker_triangle_id: value.blocker_triangle_id,
                reserved: 0,
            };
        }
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
            output_metadata.write(XvIntervisibilityMetadata {
                structure_size: structure_size::<XvIntervisibilityMetadata>(),
                reserved: 0,
                observer_count: usize_u64(observer_count),
                target_count: usize_u64(target_count),
                entry_count: usize_u64(entry_count),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        };
        Ok(())
    })
}

/// Computes an undirected all-pairs visibility graph and topology metrics.
///
/// # Safety
///
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_visibility_graph(
    scene_handle: u64,
    nodes: *const XvVisibilityNode,
    node_count: usize,
    options: *const XvVisibilityGraphOptions,
    output_metadata: *mut XvVisibilityGraphMetadata,
    output_metrics: *mut XvVisibilityNodeMetrics,
    metric_capacity: usize,
    output_pairs: *mut XvVisibilityGraphPair,
    pair_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size pointer is non-null and readable.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvVisibilityGraphOptions>()
            || options.flags & !GRAPH_COMPUTE_CENTRALITY_FLAG != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let pair_count = node_count
            .checked_mul(node_count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or(XvStatus::InvalidLength)?;
        if metric_capacity < node_count || pair_capacity < pair_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate input pointer and count.
        let node_values = unsafe { input_slice(nodes, node_count)? };
        // SAFETY: Capacities were checked above.
        let metric_output = unsafe { output_slice(output_metrics, node_count)? };
        // SAFETY: Pair-count arithmetic and capacity were checked above.
        let pair_output = unsafe { output_slice(output_pairs, pair_count)? };
        let nodes = node_values
            .iter()
            .map(|value| {
                VisibilityNode::try_new(
                    SensorId::new(value.node_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_visibility_graph(
            &scene,
            &nodes,
            VisibilityGraphOptions {
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                maximum_distance_meters: options.maximum_distance_meters,
                category_mask: options.category_mask,
                compute_centrality: options.flags & GRAPH_COMPUTE_CENTRALITY_FLAG != 0,
            },
        )
        .map_err(visibility_error_status)?;
        let elapsed = micros(started);
        for (destination, value) in metric_output.iter_mut().zip(&result.metrics) {
            *destination = XvVisibilityNodeMetrics {
                structure_size: structure_size::<XvVisibilityNodeMetrics>(),
                reserved: 0,
                node_id: value.node_id.get(),
                degree: usize_u64(value.degree),
                degree_centrality: value.degree_centrality,
                component_index: usize_u64(value.component_index),
                harmonic_closeness: value.harmonic_closeness,
                betweenness_centrality: value.betweenness_centrality,
            };
        }
        for (destination, value) in pair_output.iter_mut().zip(&result.pairs) {
            *destination = XvVisibilityGraphPair {
                structure_size: structure_size::<XvVisibilityGraphPair>(),
                state: u32::from(value.state as u8),
                first_index: usize_u64(value.first_index),
                second_index: usize_u64(value.second_index),
                distance_meters: value.distance_meters,
                blocker_object_id: value.blocker_object_id.get(),
            };
        }
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
            output_metadata.write(XvVisibilityGraphMetadata {
                structure_size: structure_size::<XvVisibilityGraphMetadata>(),
                reserved: 0,
                node_count: usize_u64(node_count),
                pair_count: usize_u64(pair_count),
                visible_edge_count: usize_u64(result.visible_edge_count),
                connected_component_count: usize_u64(result.connected_component_count),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        };
        Ok(())
    })
}

/// Computes a radius-indexed, degree-bounded visibility graph without an all-pairs matrix.
///
/// # Safety
///
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_sparse_visibility_graph(
    scene_handle: u64,
    nodes: *const XvVisibilityNode,
    node_count: usize,
    options: *const XvSparseVisibilityGraphOptions,
    output_metadata: *mut XvVisibilityGraphMetadata,
    output_metrics: *mut XvVisibilityNodeMetrics,
    metric_capacity: usize,
    output_pairs: *mut XvVisibilityGraphPair,
    pair_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size pointer is non-null and readable.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvSparseVisibilityGraphOptions>()
            || options.flags & !GRAPH_COMPUTE_CENTRALITY_FLAG != 0
            || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let neighbor_count =
            usize::try_from(options.maximum_neighbors).map_err(|_| XvStatus::InvalidLength)?;
        let pair_upper_bound = node_count
            .checked_mul(neighbor_count)
            .and_then(|value| value.checked_div(2))
            .ok_or(XvStatus::InvalidLength)?
            .min(16_000_000);
        if metric_capacity < node_count || pair_capacity < pair_upper_bound {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate input pointer and count.
        let node_values = unsafe { input_slice(nodes, node_count)? };
        // SAFETY: Metric capacity was checked above.
        let metric_output = unsafe { output_slice(output_metrics, node_count)? };
        let nodes = node_values
            .iter()
            .map(|value| {
                VisibilityNode::try_new(
                    SensorId::new(value.node_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                )
                .map_err(visibility_error_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_sparse_visibility_graph(
            &scene,
            &nodes,
            SparseVisibilityGraphOptions {
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                maximum_distance_meters: options.maximum_distance_meters,
                category_mask: options.category_mask,
                maximum_neighbors: neighbor_count,
                compute_centrality: options.flags & GRAPH_COMPUTE_CENTRALITY_FLAG != 0,
            },
        )
        .map_err(visibility_error_status)?;
        if result.pairs.len() > pair_capacity {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: The result length is bounded by the capacity checked above.
        let pair_output = unsafe { output_slice(output_pairs, result.pairs.len())? };
        let elapsed = micros(started);
        for (destination, value) in metric_output.iter_mut().zip(&result.metrics) {
            *destination = XvVisibilityNodeMetrics {
                structure_size: structure_size::<XvVisibilityNodeMetrics>(),
                reserved: 0,
                node_id: value.node_id.get(),
                degree: usize_u64(value.degree),
                degree_centrality: value.degree_centrality,
                component_index: usize_u64(value.component_index),
                harmonic_closeness: value.harmonic_closeness,
                betweenness_centrality: value.betweenness_centrality,
            };
        }
        for (destination, value) in pair_output.iter_mut().zip(&result.pairs) {
            *destination = XvVisibilityGraphPair {
                structure_size: structure_size::<XvVisibilityGraphPair>(),
                state: u32::from(value.state as u8),
                first_index: usize_u64(value.first_index),
                second_index: usize_u64(value.second_index),
                distance_meters: value.distance_meters,
                blocker_object_id: value.blocker_object_id.get(),
            };
        }
        // SAFETY: Caller provides writable metadata storage.
        unsafe {
            output_metadata.write(XvVisibilityGraphMetadata {
                structure_size: structure_size::<XvVisibilityGraphMetadata>(),
                reserved: 0,
                node_count: usize_u64(node_count),
                pair_count: usize_u64(result.pairs.len()),
                visible_edge_count: usize_u64(result.visible_edge_count),
                connected_component_count: usize_u64(result.connected_component_count),
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

const fn visibility_error_status(_: VisibilityError) -> XvStatus {
    XvStatus::InvalidArgument
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

fn usize_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
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

    #[test]
    fn daena_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvViewpoint>(), 80);
        assert_eq!(mem::size_of::<XvIsovistOptions>(), 40);
        assert_eq!(mem::size_of::<XvIsovistSummary>(), 128);
        assert_eq!(mem::size_of::<XvIsovistRay>(), 96);
        assert_eq!(mem::size_of::<XvIsovistMetadata>(), 72);
        assert_eq!(mem::size_of::<XvVisibilityObserver>(), 40);
        assert_eq!(mem::size_of::<XvVisibilityTarget>(), 72);
        assert_eq!(mem::size_of::<XvIntervisibilityOptions>(), 48);
        assert_eq!(mem::size_of::<XvIntervisibilityEntry>(), 96);
        assert_eq!(mem::size_of::<XvObserverVisibilitySummary>(), 48);
        assert_eq!(mem::size_of::<XvTargetVisibilitySummary>(), 48);
        assert_eq!(mem::size_of::<XvIntervisibilityMetadata>(), 72);
        assert_eq!(mem::size_of::<XvVisibilityNode>(), 32);
        assert_eq!(mem::size_of::<XvVisibilityGraphOptions>(), 32);
        assert_eq!(mem::size_of::<XvSparseVisibilityGraphOptions>(), 40);
        assert_eq!(mem::size_of::<XvVisibilityGraphPair>(), 40);
        assert_eq!(mem::size_of::<XvVisibilityNodeMetrics>(), 56);
        assert_eq!(mem::size_of::<XvVisibilityGraphMetadata>(), 80);
    }
}
