//! Fixed-layout DAENA 3D-isovist, material, landmark, and attribution ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};
use xvarna_daena::{Isovist3dOptions, Isovist3dResult, LandmarkVisibilityResult};
use xvarna_daena::{
    Landmark, LandmarkObserver, LandmarkVisibilityOptions, SpatialViewpoint, VisibilityError,
    analyze_isovist_3d, analyze_landmark_visibility,
};
use xvarna_geometry::Vec3;
use xvarna_scene::{AnalysisMaterial, MaterialLibrary};
use xvarna_types::{ObjectId, SensorId, TargetId};

/// Fixed-layout optical analysis material.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnalysisMaterial {
    /// Stable non-zero material identity.
    pub material_id: u64,
    /// Visible transmittance per geometric interaction.
    pub visible_transmittance: f64,
    /// Direct-solar transmittance per geometric interaction.
    pub solar_transmittance: f64,
    /// Diffuse reflectance retained for daylight workflows.
    pub reflectance: f64,
}

/// Fixed-layout source-object material assignment.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvMaterialAssignment {
    /// Source object identity.
    pub object_id: u64,
    /// Material identity present in the supplied material array.
    pub material_id: u64,
}

/// Fixed-layout spatial eye point.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSpatialViewpoint {
    /// Stable viewpoint identifier.
    pub viewpoint_id: u64,
    /// Eye coordinates in canonical metres.
    pub position_x: f64,
    /// Eye coordinates in canonical metres.
    pub position_y: f64,
    /// Eye coordinates in canonical metres.
    pub position_z: f64,
}

/// Fixed-layout 3D-isovist policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovist3dOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Equal-solid-angle ray count.
    pub sample_count: u32,
    /// Finite radial clipping distance.
    pub maximum_distance_meters: f64,
    /// Ray start offset.
    pub eye_offset_meters: f64,
    /// Included scene categories.
    pub category_mask: u64,
    /// Ranked object count per viewpoint.
    pub top_k: u32,
    /// Leading ranked objects evaluated with exact remove-one retracing.
    pub counterfactual_count: u32,
    /// Transparent geometric-layer cap.
    pub maximum_material_layers: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Throughput below this threshold is zero.
    pub minimum_transmission: f64,
}

/// Fixed-layout 3D-isovist aggregate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovist3dSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable viewpoint identity.
    pub viewpoint_id: u64,
    /// Radial volume integral.
    pub volume_cubic_meters: f64,
    /// Radial surface measure.
    pub radial_surface_square_meters: f64,
    /// Mean radial depth.
    pub mean_radial_meters: f64,
    /// Minimum radial depth.
    pub minimum_radial_meters: f64,
    /// Maximum radial depth.
    pub maximum_radial_meters: f64,
    /// Transmission-weighted visible solid angle.
    pub visible_solid_angle_steradians: f64,
    /// Visible share of the full sphere.
    pub openness_ratio: f64,
    /// Full-versus-half sample volume delta.
    pub volume_convergence_delta_cubic_meters: f64,
    /// Dominant source object.
    pub dominant_occluder_object_id: u64,
    /// Dominant share of attributed loss.
    pub dominant_occluder_fraction: f64,
}

/// Fixed-layout 3D-isovist ray.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovist3dRay {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// First-hit triangle, or `UINT32_MAX`.
    pub triangle_id: u32,
    /// Unit direction.
    pub direction_x: f64,
    /// Unit direction.
    pub direction_y: f64,
    /// Unit direction.
    pub direction_z: f64,
    /// Ray endpoint.
    pub endpoint_x: f64,
    /// Ray endpoint.
    pub endpoint_y: f64,
    /// Ray endpoint.
    pub endpoint_z: f64,
    /// First-hit or clipping distance.
    pub distance_meters: f64,
    /// Remaining visible transmission.
    pub transmission: f64,
    /// First source object.
    pub object_id: u64,
    /// First occurrence.
    pub instance_id: u64,
    /// First mesh resource.
    pub mesh_id: u64,
    /// Exact first-occurrence category combination.
    pub category_mask: u64,
}

/// Fixed-layout reusable ranked DAENA obstruction attribution.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAttributionEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// One-based rank.
    pub rank: u32,
    /// Source observer or viewpoint.
    pub observer_id: u64,
    /// Optional target identity; zero denotes whole-viewpoint scope.
    pub target_id: u64,
    /// Source object.
    pub object_id: u64,
    /// Union of source occurrence categories.
    pub category_mask: u64,
    /// Attributed blocked weight.
    pub blocked_weight: f64,
    /// Share of all scope loss.
    pub fraction: f64,
    /// Loss-weighted mean interaction distance.
    pub mean_distance_meters: f64,
    /// Exact remove-one recovered visible weight, when evaluated.
    pub counterfactual_recovered_weight: f64,
    /// Exact remove-one volume delta, when evaluated.
    pub counterfactual_volume_delta_cubic_meters: f64,
}

/// Fixed-layout exact category-mask partition.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvCategoryBreakdown {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source observer or viewpoint.
    pub observer_id: u64,
    /// Optional target identity.
    pub target_id: u64,
    /// Exact category-mask combination.
    pub category_mask: u64,
    /// Attributed blocked weight.
    pub blocked_weight: f64,
    /// Share of all scope loss.
    pub fraction: f64,
}

/// Dimensions, actual variable-row counts, runtime, and identity for 3D isovists.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIsovist3dMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source viewpoint count.
    pub viewpoint_count: u64,
    /// Samples per viewpoint.
    pub sample_count: u64,
    /// Total ray count.
    pub ray_count: u64,
    /// Actual ranked attribution row count.
    pub attribution_count: u64,
    /// Actual category row count.
    pub category_count: u64,
    /// Native execution duration.
    pub analysis_time_microseconds: u64,
    /// BLAKE3 result hash words.
    pub content_hash_0: u64,
    /// BLAKE3 result hash words.
    pub content_hash_1: u64,
    /// BLAKE3 result hash words.
    pub content_hash_2: u64,
    /// BLAKE3 result hash words.
    pub content_hash_3: u64,
}

/// Computes complete material-aware equal-solid-angle 3D isovists.
///
/// # Safety
///
/// Every pointer references its declared live array and all output arrays are non-overlapping.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_isovist_3d(
    scene_handle: u64,
    viewpoints: *const XvSpatialViewpoint,
    viewpoint_count: usize,
    materials: *const XvAnalysisMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    options: *const XvIsovist3dOptions,
    output_metadata: *mut XvIsovist3dMetadata,
    output_summaries: *mut XvIsovist3dSummary,
    summary_capacity: usize,
    output_rays: *mut XvIsovist3dRay,
    ray_capacity: usize,
    output_attribution: *mut XvAttributionEntry,
    attribution_capacity: usize,
    output_categories: *mut XvCategoryBreakdown,
    category_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null and readable for this call.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvIsovist3dOptions>() || options.reserved != 0
        {
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
        // SAFETY: Shared helpers validate pointers for all declared element counts.
        let viewpoint_values = unsafe { input_slice(viewpoints, viewpoint_count)? };
        // SAFETY: Shared helpers validate pointers for all declared element counts.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate pointers for all declared element counts.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let viewpoints = viewpoint_values
            .iter()
            .map(|value| {
                SpatialViewpoint::try_new(
                    SensorId::new(value.viewpoint_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                )
                .map_err(visibility_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let material_library = material_library(material_values, assignment_values)?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_isovist_3d(
            &scene,
            &viewpoints,
            &material_library,
            Isovist3dOptions {
                sample_count,
                maximum_distance_meters: options.maximum_distance_meters,
                eye_offset_meters: options.eye_offset_meters,
                category_mask: options.category_mask,
                top_k: usize::try_from(options.top_k).map_err(|_| XvStatus::InvalidLength)?,
                counterfactual_count: usize::try_from(options.counterfactual_count)
                    .map_err(|_| XvStatus::InvalidLength)?,
                maximum_material_layers: usize::try_from(options.maximum_material_layers)
                    .map_err(|_| XvStatus::InvalidLength)?,
                minimum_transmission: options.minimum_transmission,
            },
        )
        .map_err(visibility_status)?;
        let elapsed = micros(started);
        write_isovist_3d(
            &result,
            output_summaries,
            summary_capacity,
            output_rays,
            ray_capacity,
            output_attribution,
            attribution_capacity,
            output_categories,
            category_capacity,
        )?;
        // SAFETY: Caller provided writable metadata storage.
        unsafe {
            output_metadata.write(XvIsovist3dMetadata {
                structure_size: structure_size::<XvIsovist3dMetadata>(),
                reserved: 0,
                viewpoint_count: usize_u64(viewpoint_count),
                sample_count: usize_u64(sample_count),
                ray_count: usize_u64(result.rays.len()),
                attribution_count: usize_u64(result.attribution.len()),
                category_count: usize_u64(result.categories.len()),
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

/// Fixed-layout oriented landmark observer.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvLandmarkObserver {
    /// Stable observer identity.
    pub observer_id: u64,
    /// Eye position.
    pub position_x: f64,
    /// Eye position.
    pub position_y: f64,
    /// Eye position.
    pub position_z: f64,
    /// Camera forward.
    pub forward_x: f64,
    /// Camera forward.
    pub forward_y: f64,
    /// Camera forward.
    pub forward_z: f64,
    /// Camera up.
    pub up_x: f64,
    /// Camera up.
    pub up_y: f64,
    /// Camera up.
    pub up_z: f64,
}

/// Fixed-layout spherical landmark proxy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvLandmark {
    /// Stable landmark identity.
    pub landmark_id: u64,
    /// Landmark centre.
    pub position_x: f64,
    /// Landmark centre.
    pub position_y: f64,
    /// Landmark centre.
    pub position_z: f64,
    /// Positive proxy radius.
    pub radius_meters: f64,
    /// Importance from zero through one.
    pub weight: f64,
}

/// Fixed-layout landmark visibility policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvLandmarkOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Disk sample count per pair.
    pub sample_count: u32,
    /// Horizontal camera FOV in radians.
    pub horizontal_field_of_view_radians: f64,
    /// Vertical camera FOV in radians.
    pub vertical_field_of_view_radians: f64,
    /// Maximum landmark distance.
    pub maximum_distance_meters: f64,
    /// Endpoint clearance.
    pub endpoint_clearance_meters: f64,
    /// Included context categories.
    pub category_mask: u64,
    /// Ranked objects retained per pair.
    pub top_k: u32,
    /// Transparent layer cap.
    pub maximum_material_layers: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Throughput cutoff.
    pub minimum_transmission: f64,
}

/// Fixed-layout observer/landmark result cell.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvLandmarkVisibilityEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Bit zero marks the landmark as inside the camera FOV.
    pub flags: u32,
    /// Observer identity.
    pub observer_id: u64,
    /// Landmark identity.
    pub landmark_id: u64,
    /// Centre distance.
    pub distance_meters: f64,
    /// Geometric apparent solid angle.
    pub apparent_solid_angle_steradians: f64,
    /// Mean optical visible fraction.
    pub visible_fraction: f64,
    /// Visible apparent solid angle.
    pub visible_solid_angle_steradians: f64,
    /// Importance-weighted FOV-normalized score.
    pub weighted_visibility_score: f64,
    /// Dominant blocker object.
    pub dominant_blocker_object_id: u64,
    /// Dominant share of attributed blocking.
    pub dominant_blocker_fraction: f64,
}

/// Dimensions, variable row counts, runtime, and identity for landmark visibility.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvLandmarkMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Observer rows.
    pub observer_count: u64,
    /// Landmark columns.
    pub landmark_count: u64,
    /// Matrix cell count.
    pub entry_count: u64,
    /// Actual ranked attribution rows.
    pub attribution_count: u64,
    /// Actual exact category rows.
    pub category_count: u64,
    /// Native execution duration.
    pub analysis_time_microseconds: u64,
    /// BLAKE3 result hash words.
    pub content_hash_0: u64,
    /// BLAKE3 result hash words.
    pub content_hash_1: u64,
    /// BLAKE3 result hash words.
    pub content_hash_2: u64,
    /// BLAKE3 result hash words.
    pub content_hash_3: u64,
}

/// Computes material-aware, partially occluded landmark visibility.
///
/// # Safety
///
/// Every pointer references its declared live array and all output arrays are non-overlapping.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_landmark_visibility(
    scene_handle: u64,
    observers: *const XvLandmarkObserver,
    observer_count: usize,
    landmarks: *const XvLandmark,
    landmark_count: usize,
    materials: *const XvAnalysisMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    options: *const XvLandmarkOptions,
    output_metadata: *mut XvLandmarkMetadata,
    output_entries: *mut XvLandmarkVisibilityEntry,
    entry_capacity: usize,
    output_attribution: *mut XvAttributionEntry,
    attribution_capacity: usize,
    output_categories: *mut XvCategoryBreakdown,
    category_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null and readable for this call.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvLandmarkOptions>() || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let required_entries = observer_count
            .checked_mul(landmark_count)
            .ok_or(XvStatus::InvalidLength)?;
        if entry_capacity < required_entries {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate all declared pointers and lengths.
        let observer_values = unsafe { input_slice(observers, observer_count)? };
        // SAFETY: Shared helpers validate all declared pointers and lengths.
        let landmark_values = unsafe { input_slice(landmarks, landmark_count)? };
        // SAFETY: Shared helpers validate all declared pointers and lengths.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate all declared pointers and lengths.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let observers = observer_values
            .iter()
            .map(|value| {
                LandmarkObserver::try_new(
                    SensorId::new(value.observer_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    Vec3::new(value.forward_x, value.forward_y, value.forward_z),
                    Vec3::new(value.up_x, value.up_y, value.up_z),
                )
                .map_err(visibility_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let landmarks = landmark_values
            .iter()
            .map(|value| {
                Landmark::try_new(
                    TargetId::new(value.landmark_id),
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    value.radius_meters,
                    value.weight,
                )
                .map_err(visibility_status)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let material_library = material_library(material_values, assignment_values)?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_landmark_visibility(
            &scene,
            &observers,
            &landmarks,
            &material_library,
            LandmarkVisibilityOptions {
                sample_count: usize::try_from(options.sample_count)
                    .map_err(|_| XvStatus::InvalidLength)?,
                horizontal_field_of_view_radians: options.horizontal_field_of_view_radians,
                vertical_field_of_view_radians: options.vertical_field_of_view_radians,
                maximum_distance_meters: options.maximum_distance_meters,
                endpoint_clearance_meters: options.endpoint_clearance_meters,
                category_mask: options.category_mask,
                top_k: usize::try_from(options.top_k).map_err(|_| XvStatus::InvalidLength)?,
                maximum_material_layers: usize::try_from(options.maximum_material_layers)
                    .map_err(|_| XvStatus::InvalidLength)?,
                minimum_transmission: options.minimum_transmission,
            },
        )
        .map_err(visibility_status)?;
        let elapsed = micros(started);
        write_landmarks(
            &result,
            output_entries,
            entry_capacity,
            output_attribution,
            attribution_capacity,
            output_categories,
            category_capacity,
        )?;
        // SAFETY: Caller provided writable metadata storage.
        unsafe {
            output_metadata.write(XvLandmarkMetadata {
                structure_size: structure_size::<XvLandmarkMetadata>(),
                reserved: 0,
                observer_count: usize_u64(observer_count),
                landmark_count: usize_u64(landmark_count),
                entry_count: usize_u64(result.entries.len()),
                attribution_count: usize_u64(result.attribution.len()),
                category_count: usize_u64(result.categories.len()),
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

#[allow(clippy::too_many_arguments)]
fn write_isovist_3d(
    result: &Isovist3dResult,
    output_summaries: *mut XvIsovist3dSummary,
    summary_capacity: usize,
    output_rays: *mut XvIsovist3dRay,
    ray_capacity: usize,
    output_attribution: *mut XvAttributionEntry,
    attribution_capacity: usize,
    output_categories: *mut XvCategoryBreakdown,
    category_capacity: usize,
) -> Result<(), XvStatus> {
    if summary_capacity < result.summaries.len()
        || ray_capacity < result.rays.len()
        || attribution_capacity < result.attribution.len()
        || category_capacity < result.categories.len()
    {
        return Err(XvStatus::InvalidLength);
    }
    // SAFETY: Capacities were validated against exact result lengths.
    let summaries = unsafe { output_slice(output_summaries, result.summaries.len())? };
    // SAFETY: Capacities were validated against exact result lengths.
    let rays = unsafe { output_slice(output_rays, result.rays.len())? };
    // SAFETY: Capacities were validated against exact result lengths.
    let attribution = unsafe { output_slice(output_attribution, result.attribution.len())? };
    // SAFETY: Capacities were validated against exact result lengths.
    let categories = unsafe { output_slice(output_categories, result.categories.len())? };
    for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
        *destination = XvIsovist3dSummary {
            structure_size: structure_size::<XvIsovist3dSummary>(),
            reserved: 0,
            viewpoint_id: value.viewpoint_id.get(),
            volume_cubic_meters: value.volume_cubic_meters,
            radial_surface_square_meters: value.radial_surface_square_meters,
            mean_radial_meters: value.mean_radial_meters,
            minimum_radial_meters: value.minimum_radial_meters,
            maximum_radial_meters: value.maximum_radial_meters,
            visible_solid_angle_steradians: value.visible_solid_angle_steradians,
            openness_ratio: value.openness_ratio,
            volume_convergence_delta_cubic_meters: value.volume_convergence_delta_cubic_meters,
            dominant_occluder_object_id: value.dominant_occluder_object_id.get(),
            dominant_occluder_fraction: value.dominant_occluder_fraction,
        };
    }
    for (destination, value) in rays.iter_mut().zip(&result.rays) {
        *destination = XvIsovist3dRay {
            structure_size: structure_size::<XvIsovist3dRay>(),
            triangle_id: value.triangle_id,
            direction_x: value.direction.x,
            direction_y: value.direction.y,
            direction_z: value.direction.z,
            endpoint_x: value.endpoint.x,
            endpoint_y: value.endpoint.y,
            endpoint_z: value.endpoint.z,
            distance_meters: value.distance_meters,
            transmission: value.transmission,
            object_id: value.object_id.get(),
            instance_id: value.instance_id.get(),
            mesh_id: value.mesh_id.get(),
            category_mask: value.category_mask,
        };
    }
    write_attribution(&result.attribution, attribution);
    write_categories(&result.categories, categories);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_landmarks(
    result: &LandmarkVisibilityResult,
    output_entries: *mut XvLandmarkVisibilityEntry,
    entry_capacity: usize,
    output_attribution: *mut XvAttributionEntry,
    attribution_capacity: usize,
    output_categories: *mut XvCategoryBreakdown,
    category_capacity: usize,
) -> Result<(), XvStatus> {
    if entry_capacity < result.entries.len()
        || attribution_capacity < result.attribution.len()
        || category_capacity < result.categories.len()
    {
        return Err(XvStatus::InvalidLength);
    }
    // SAFETY: Capacities were validated against exact result lengths.
    let entries = unsafe { output_slice(output_entries, result.entries.len())? };
    // SAFETY: Capacities were validated against exact result lengths.
    let attribution = unsafe { output_slice(output_attribution, result.attribution.len())? };
    // SAFETY: Capacities were validated against exact result lengths.
    let categories = unsafe { output_slice(output_categories, result.categories.len())? };
    for (destination, value) in entries.iter_mut().zip(&result.entries) {
        *destination = XvLandmarkVisibilityEntry {
            structure_size: structure_size::<XvLandmarkVisibilityEntry>(),
            flags: u32::from(value.inside_field_of_view),
            observer_id: value.observer_id.get(),
            landmark_id: value.landmark_id.get(),
            distance_meters: value.distance_meters,
            apparent_solid_angle_steradians: value.apparent_solid_angle_steradians,
            visible_fraction: value.visible_fraction,
            visible_solid_angle_steradians: value.visible_solid_angle_steradians,
            weighted_visibility_score: value.weighted_visibility_score,
            dominant_blocker_object_id: value.dominant_blocker_object_id.get(),
            dominant_blocker_fraction: value.dominant_blocker_fraction,
        };
    }
    write_attribution(&result.attribution, attribution);
    write_categories(&result.categories, categories);
    Ok(())
}

fn write_attribution(source: &[xvarna_daena::AttributionEntry], output: &mut [XvAttributionEntry]) {
    for (destination, value) in output.iter_mut().zip(source) {
        *destination = XvAttributionEntry {
            structure_size: structure_size::<XvAttributionEntry>(),
            rank: u32::try_from(value.rank).unwrap_or(u32::MAX),
            observer_id: value.observer_id.get(),
            target_id: value.target_id.get(),
            object_id: value.object_id.get(),
            category_mask: value.category_mask,
            blocked_weight: value.blocked_weight,
            fraction: value.fraction,
            mean_distance_meters: value.mean_distance_meters,
            counterfactual_recovered_weight: value.counterfactual_recovered_weight,
            counterfactual_volume_delta_cubic_meters: value
                .counterfactual_volume_delta_cubic_meters,
        };
    }
}

fn write_categories(
    source: &[xvarna_daena::CategoryBreakdown],
    output: &mut [XvCategoryBreakdown],
) {
    for (destination, value) in output.iter_mut().zip(source) {
        *destination = XvCategoryBreakdown {
            structure_size: structure_size::<XvCategoryBreakdown>(),
            reserved: 0,
            observer_id: value.observer_id.get(),
            target_id: value.target_id.get(),
            category_mask: value.category_mask,
            blocked_weight: value.blocked_weight,
            fraction: value.fraction,
        };
    }
}

fn material_library(
    materials: &[XvAnalysisMaterial],
    assignments: &[XvMaterialAssignment],
) -> Result<MaterialLibrary, XvStatus> {
    let materials = materials
        .iter()
        .map(|value| {
            AnalysisMaterial::try_new(
                value.material_id,
                value.visible_transmittance,
                value.solar_transmittance,
                value.reflectance,
            )
            .map_err(|_| XvStatus::InvalidArgument)
        })
        .collect::<Result<Vec<_>, _>>()?;
    MaterialLibrary::try_new(
        materials,
        assignments
            .iter()
            .map(|value| (ObjectId::new(value.object_id), value.material_id)),
    )
    .map_err(|_| XvStatus::InvalidArgument)
}

const fn visibility_status(error: VisibilityError) -> XvStatus {
    match error {
        VisibilityError::ResultTooLarge => XvStatus::InvalidLength,
        VisibilityError::ExecutionFailed => XvStatus::InvalidState,
        _ => XvStatus::InvalidArgument,
    }
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intelligence_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvAnalysisMaterial>(), 32);
        assert_eq!(mem::size_of::<XvMaterialAssignment>(), 16);
        assert_eq!(mem::size_of::<XvSpatialViewpoint>(), 32);
        assert_eq!(mem::size_of::<XvIsovist3dOptions>(), 56);
        assert_eq!(mem::size_of::<XvIsovist3dSummary>(), 96);
        assert_eq!(mem::size_of::<XvIsovist3dRay>(), 104);
        assert_eq!(mem::size_of::<XvAttributionEntry>(), 80);
        assert_eq!(mem::size_of::<XvCategoryBreakdown>(), 48);
        assert_eq!(mem::size_of::<XvIsovist3dMetadata>(), 88);
        assert_eq!(mem::size_of::<XvLandmarkObserver>(), 80);
        assert_eq!(mem::size_of::<XvLandmark>(), 48);
        assert_eq!(mem::size_of::<XvLandmarkOptions>(), 72);
        assert_eq!(mem::size_of::<XvLandmarkVisibilityEntry>(), 80);
        assert_eq!(mem::size_of::<XvLandmarkMetadata>(), 88);
    }
}
