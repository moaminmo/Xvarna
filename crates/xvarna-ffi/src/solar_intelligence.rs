//! Fixed-layout material-aware solar scenario and envelope ABI.

use super::daena_intelligence::{XvAnalysisMaterial, XvMaterialAssignment};
use super::solar::{XvSolarSensor, XvSunSample, XvSunSetMetadata, sun_sample_from_ffi};
use super::{XvStatus, hash_from_words, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{
    EnvelopeCandidate, SolarEnvelopeOptions, SolarScenario, SolarScenarioOptions, SolarSensor,
    analyze_solar_envelope, compare_solar_scenarios,
};
use xvarna_scene::{AnalysisMaterial, MaterialLibrary};
use xvarna_types::{ObjectId, SensorId};
use xvarna_zurvan::{SolarLocation, SolarOptions, SunSet};

/// One scenario referencing ranges in shared material and assignment arrays.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenario {
    /// Stable scenario identity.
    pub scenario_id: u64,
    /// First material index.
    pub material_offset: u64,
    /// Material range length.
    pub material_count: u64,
    /// First assignment index.
    pub assignment_offset: u64,
    /// Assignment range length.
    pub assignment_count: u64,
}

/// Fixed-layout material-aware solar comparison policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenarioOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Ranked object rows retained per scenario/sensor.
    pub top_k: u32,
    /// Sensor normal offset.
    pub sensor_offset_meters: f64,
    /// Maximum context distance.
    pub maximum_distance_meters: f64,
    /// Minimum eligible incidence cosine.
    pub minimum_incidence_cosine: f64,
    /// Included context categories.
    pub category_mask: u64,
    /// Transparent layer cap.
    pub maximum_material_layers: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Throughput cutoff.
    pub minimum_transmission: f64,
}

/// Fixed-layout scenario/sensor aggregate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenarioSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Scenario identity.
    pub scenario_id: u64,
    /// Sensor identity.
    pub sensor_id: u64,
    /// Transmission-weighted received hours.
    pub received_sun_hours: f64,
    /// Eligible hours removed by geometry/material.
    pub lost_sun_hours: f64,
    /// Total eligible scheduled hours.
    pub eligible_sun_hours: f64,
    /// Received divided by eligible.
    pub solar_access_ratio: f64,
    /// Dominant loss source.
    pub dominant_occluder_object_id: u64,
    /// Hours attributed to the dominant source.
    pub dominant_occluder_hours: f64,
}

/// Fixed-layout scenario/sensor/time optical cell.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenarioTimelineEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Numeric state: zero inactive, one back-facing, two evaluated.
    pub state: u32,
    /// Remaining solar transmission.
    pub transmission: f64,
    /// First interacting object.
    pub first_object_id: u64,
    /// Geometric/material layer count.
    pub layer_count: u32,
    /// Bit zero indicates that the layer limit was reached.
    pub flags: u32,
}

/// Fixed-layout ranked solar loss attribution.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarAttributionEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// One-based rank.
    pub rank: u32,
    /// Scenario identity.
    pub scenario_id: u64,
    /// Sensor identity.
    pub sensor_id: u64,
    /// Source object.
    pub object_id: u64,
    /// Union of occurrence categories.
    pub category_mask: u64,
    /// Attributed lost hours.
    pub lost_sun_hours: f64,
    /// Share of all attributed loss.
    pub fraction: f64,
}

/// Fixed-layout exact solar category partition.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarCategoryBreakdown {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Scenario identity.
    pub scenario_id: u64,
    /// Sensor identity.
    pub sensor_id: u64,
    /// Exact category combination.
    pub category_mask: u64,
    /// Attributed lost hours.
    pub lost_sun_hours: f64,
    /// Share of all attributed loss.
    pub fraction: f64,
}

/// Fixed-layout non-baseline scenario delta.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenarioDelta {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Compared scenario identity.
    pub scenario_id: u64,
    /// Sensor identity.
    pub sensor_id: u64,
    /// Compared minus baseline received hours.
    pub received_sun_hours_delta: f64,
    /// Compared minus baseline lost hours.
    pub lost_sun_hours_delta: f64,
    /// Compared minus baseline access ratio.
    pub solar_access_ratio_delta: f64,
}

/// Variable result counts, dimensions, runtime, and identity for solar scenarios.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarScenarioMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Scenario count.
    pub scenario_count: u64,
    /// Sensor count.
    pub sensor_count: u64,
    /// Sun sample count.
    pub sun_count: u64,
    /// Summary count.
    pub summary_count: u64,
    /// Timeline count.
    pub timeline_count: u64,
    /// Actual attribution count.
    pub attribution_count: u64,
    /// Actual category count.
    pub category_count: u64,
    /// Delta count.
    pub delta_count: u64,
    /// Native analysis duration.
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

/// Compares material alternatives over one geometry and sun schedule.
///
/// # Safety
///
/// All pointers reference their declared live arrays and output arrays are non-overlapping.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_compare_solar_scenarios(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    sun_metadata: *const XvSunSetMetadata,
    sun_samples: *const XvSunSample,
    sun_count: usize,
    scenarios: *const XvSolarScenario,
    scenario_count: usize,
    materials: *const XvAnalysisMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    options: *const XvSolarScenarioOptions,
    output_metadata: *mut XvSolarScenarioMetadata,
    output_summaries: *mut XvSolarScenarioSummary,
    summary_capacity: usize,
    output_timeline: *mut XvSolarScenarioTimelineEntry,
    timeline_capacity: usize,
    output_attribution: *mut XvSolarAttributionEntry,
    attribution_capacity: usize,
    output_categories: *mut XvSolarCategoryBreakdown,
    category_capacity: usize,
    output_deltas: *mut XvSolarScenarioDelta,
    delta_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if sun_metadata.is_null() || options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed inputs are non-null and readable.
        let native_sun_metadata = unsafe { sun_metadata.read() };
        // SAFETY: Required fixed inputs are non-null and readable.
        let options = unsafe { options.read() };
        if native_sun_metadata.structure_size != structure_size::<XvSunSetMetadata>()
            || native_sun_metadata.sample_count != usize_u64(sun_count)
            || options.structure_size != structure_size::<XvSolarScenarioOptions>()
            || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let summary_count = scenario_count
            .checked_mul(sensor_count)
            .ok_or(XvStatus::InvalidLength)?;
        let timeline_count = summary_count
            .checked_mul(sun_count)
            .ok_or(XvStatus::InvalidLength)?;
        let delta_count = scenario_count
            .saturating_sub(1)
            .checked_mul(sensor_count)
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < summary_count
            || timeline_capacity < timeline_count
            || delta_capacity < delta_count
        {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sensor_values = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sun_values = unsafe { input_slice(sun_samples, sun_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let scenario_values = unsafe { input_slice(scenarios, scenario_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let sensors = sensors_from_ffi(sensor_values)?;
        let sun_set = sun_set_from_ffi(native_sun_metadata, sun_values)?;
        let scenarios = scenarios_from_ffi(scenario_values, material_values, assignment_values)?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = compare_solar_scenarios(
            &scene,
            &sensors,
            &sun_set,
            &scenarios,
            SolarScenarioOptions {
                sensor_offset_meters: options.sensor_offset_meters,
                maximum_distance_meters: options.maximum_distance_meters,
                minimum_incidence_cosine: options.minimum_incidence_cosine,
                category_mask: options.category_mask,
                maximum_material_layers: usize::try_from(options.maximum_material_layers)
                    .map_err(|_| XvStatus::InvalidLength)?,
                minimum_transmission: options.minimum_transmission,
                top_k: usize::try_from(options.top_k).map_err(|_| XvStatus::InvalidLength)?,
            },
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        if attribution_capacity < result.attribution.len()
            || category_capacity < result.categories.len()
        {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Every capacity was checked against the exact result size.
        let summaries = unsafe { output_slice(output_summaries, result.summaries.len())? };
        // SAFETY: Every capacity was checked against the exact result size.
        let timeline = unsafe { output_slice(output_timeline, result.timeline.len())? };
        // SAFETY: Every capacity was checked against the exact result size.
        let attribution = unsafe { output_slice(output_attribution, result.attribution.len())? };
        // SAFETY: Every capacity was checked against the exact result size.
        let categories = unsafe { output_slice(output_categories, result.categories.len())? };
        // SAFETY: Every capacity was checked against the exact result size.
        let deltas = unsafe { output_slice(output_deltas, result.deltas.len())? };
        for (destination, value) in summaries.iter_mut().zip(&result.summaries) {
            *destination = XvSolarScenarioSummary {
                structure_size: structure_size::<XvSolarScenarioSummary>(),
                reserved: 0,
                scenario_id: value.scenario_id,
                sensor_id: value.sensor_id.get(),
                received_sun_hours: value.received_sun_hours,
                lost_sun_hours: value.lost_sun_hours,
                eligible_sun_hours: value.eligible_sun_hours,
                solar_access_ratio: value.solar_access_ratio,
                dominant_occluder_object_id: value.dominant_occluder_object_id.get(),
                dominant_occluder_hours: value.dominant_occluder_hours,
            };
        }
        for (destination, value) in timeline.iter_mut().zip(&result.timeline) {
            *destination = XvSolarScenarioTimelineEntry {
                structure_size: structure_size::<XvSolarScenarioTimelineEntry>(),
                state: u32::from(value.state as u8),
                transmission: value.transmission,
                first_object_id: value.first_object_id.get(),
                layer_count: u32::try_from(value.layer_count).unwrap_or(u32::MAX),
                flags: u32::from(value.layer_limit_reached),
            };
        }
        for (destination, value) in attribution.iter_mut().zip(&result.attribution) {
            *destination = XvSolarAttributionEntry {
                structure_size: structure_size::<XvSolarAttributionEntry>(),
                rank: u32::try_from(value.rank).unwrap_or(u32::MAX),
                scenario_id: value.scenario_id,
                sensor_id: value.sensor_id.get(),
                object_id: value.object_id.get(),
                category_mask: value.category_mask,
                lost_sun_hours: value.lost_sun_hours,
                fraction: value.fraction,
            };
        }
        for (destination, value) in categories.iter_mut().zip(&result.categories) {
            *destination = XvSolarCategoryBreakdown {
                structure_size: structure_size::<XvSolarCategoryBreakdown>(),
                reserved: 0,
                scenario_id: value.scenario_id,
                sensor_id: value.sensor_id.get(),
                category_mask: value.category_mask,
                lost_sun_hours: value.lost_sun_hours,
                fraction: value.fraction,
            };
        }
        for (destination, value) in deltas.iter_mut().zip(&result.deltas) {
            *destination = XvSolarScenarioDelta {
                structure_size: structure_size::<XvSolarScenarioDelta>(),
                reserved: 0,
                scenario_id: value.scenario_id,
                sensor_id: value.sensor_id.get(),
                received_sun_hours_delta: value.received_sun_hours_delta,
                lost_sun_hours_delta: value.lost_sun_hours_delta,
                solar_access_ratio_delta: value.solar_access_ratio_delta,
            };
        }
        let elapsed = micros(started);
        // SAFETY: Caller provided writable metadata storage.
        unsafe {
            output_metadata.write(XvSolarScenarioMetadata {
                structure_size: structure_size::<XvSolarScenarioMetadata>(),
                reserved: 0,
                scenario_count: usize_u64(scenario_count),
                sensor_count: usize_u64(sensor_count),
                sun_count: usize_u64(sun_count),
                summary_count: usize_u64(result.summaries.len()),
                timeline_count: usize_u64(result.timeline.len()),
                attribution_count: usize_u64(result.attribution.len()),
                category_count: usize_u64(result.categories.len()),
                delta_count: usize_u64(result.deltas.len()),
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

/// Fixed-layout backwards-compatible vertical-column envelope candidate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvEnvelopeCandidate {
    /// Stable candidate identity.
    pub candidate_id: u64,
    /// Candidate base point.
    pub position_x: f64,
    /// Candidate base point.
    pub position_y: f64,
    /// Candidate base point.
    pub position_z: f64,
}

/// Fixed-layout freeform oriented-column envelope candidate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvOrientedEnvelopeCandidate {
    /// Stable candidate identity.
    pub candidate_id: u64,
    /// Candidate base point.
    pub position_x: f64,
    /// Candidate base point.
    pub position_y: f64,
    /// Candidate base point.
    pub position_z: f64,
    /// Candidate build-axis direction; normalized by the engine.
    pub axis_x: f64,
    /// Candidate build-axis direction; normalized by the engine.
    pub axis_y: f64,
    /// Candidate build-axis direction; normalized by the engine.
    pub axis_z: f64,
}

/// Fixed-layout solar/shading envelope policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarEnvelopeOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Transparent layer cap.
    pub maximum_material_layers: u32,
    /// Candidate column radius.
    pub candidate_radius_meters: f64,
    /// Required preserved fraction.
    pub required_preserved_fraction: f64,
    /// Target shaded fraction.
    pub target_shaded_fraction: f64,
    /// Vertical threshold clearance.
    pub vertical_clearance_meters: f64,
    /// Sensor normal offset.
    pub sensor_offset_meters: f64,
    /// Maximum context query distance.
    pub maximum_distance_meters: f64,
    /// Included context categories.
    pub category_mask: u64,
    /// Throughput cutoff.
    pub minimum_transmission: f64,
}

/// Fixed-layout threshold controlling ray.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvEnvelopeControl {
    /// Protected sensor identity.
    pub sensor_id: u64,
    /// UTC interval timestamp.
    pub unix_seconds_utc: i64,
    /// Ray elevation at candidate XY.
    pub ray_elevation_meters: f64,
    /// Transmission-weighted interval hours.
    pub weighted_hours: f64,
}

/// Fixed-layout solar and shading thresholds for one candidate.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarEnvelopeCell {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Candidate identity.
    pub candidate_id: u64,
    /// Candidate base point.
    pub position_x: f64,
    /// Candidate base point.
    pub position_y: f64,
    /// Candidate base point.
    pub position_z: f64,
    /// Maximum access-preserving absolute elevation.
    pub maximum_solar_access_elevation_meters: f64,
    /// Maximum access-preserving height above base.
    pub maximum_solar_access_height_meters: f64,
    /// Minimum target-shading absolute elevation.
    pub minimum_shading_elevation_meters: f64,
    /// Minimum target-shading height above base.
    pub minimum_shading_height_meters: f64,
    /// Baseline transmitted hours considered.
    pub considered_baseline_sun_hours: f64,
    /// Projected constraint count.
    pub constraint_count: u64,
    /// Access threshold controlling ray.
    pub access_control: XvEnvelopeControl,
    /// Shading threshold controlling ray.
    pub shading_control: XvEnvelopeControl,
}

/// Dimensions, runtime, and identity for the solar envelope.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSolarEnvelopeMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Protected sensor count.
    pub sensor_count: u64,
    /// Candidate/cell count.
    pub candidate_count: u64,
    /// Sun sample count.
    pub sun_count: u64,
    /// Native analysis duration.
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

/// Computes material-aware solar-access and target-shading envelope thresholds.
///
/// # Safety
///
/// All pointers reference their declared live arrays and output cells are writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_solar_envelope(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    candidates: *const XvEnvelopeCandidate,
    candidate_count: usize,
    sun_metadata: *const XvSunSetMetadata,
    sun_samples: *const XvSunSample,
    sun_count: usize,
    materials: *const XvAnalysisMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    options: *const XvSolarEnvelopeOptions,
    output_metadata: *mut XvSolarEnvelopeMetadata,
    output_cells: *mut XvSolarEnvelopeCell,
    cell_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if sun_metadata.is_null() || options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if cell_capacity < candidate_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Required fixed inputs are non-null and readable.
        let native_sun_metadata = unsafe { sun_metadata.read() };
        // SAFETY: Required fixed inputs are non-null and readable.
        let options = unsafe { options.read() };
        if native_sun_metadata.structure_size != structure_size::<XvSunSetMetadata>()
            || native_sun_metadata.sample_count != usize_u64(sun_count)
            || options.structure_size != structure_size::<XvSolarEnvelopeOptions>()
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sensor_values = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let candidate_values = unsafe { input_slice(candidates, candidate_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sun_values = unsafe { input_slice(sun_samples, sun_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        // SAFETY: Cell capacity was checked above.
        let output = unsafe { output_slice(output_cells, candidate_count)? };
        let sensors = sensors_from_ffi(sensor_values)?;
        let candidates = candidate_values
            .iter()
            .map(|value| {
                EnvelopeCandidate::try_new(
                    value.candidate_id,
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                )
                .map_err(|_| XvStatus::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // SAFETY: Output metadata and cells were validated above and remain live for this call.
        unsafe {
            execute_solar_envelope(
                scene_handle,
                &sensors,
                &candidates,
                native_sun_metadata,
                sun_values,
                material_values,
                assignment_values,
                options,
                output_metadata,
                output,
            )
        }
    })
}

/// Computes a freeform material-aware solar/shading envelope along per-candidate axes.
///
/// This is an additive ABI entry point. The original vertical-candidate function remains
/// available and produces identical results for a positive Z axis.
///
/// # Safety
///
/// All pointers reference their declared live arrays and output cells are writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_solar_envelope_oriented(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    candidates: *const XvOrientedEnvelopeCandidate,
    candidate_count: usize,
    sun_metadata: *const XvSunSetMetadata,
    sun_samples: *const XvSunSample,
    sun_count: usize,
    materials: *const XvAnalysisMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    options: *const XvSolarEnvelopeOptions,
    output_metadata: *mut XvSolarEnvelopeMetadata,
    output_cells: *mut XvSolarEnvelopeCell,
    cell_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if sun_metadata.is_null() || options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if cell_capacity < candidate_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Required fixed inputs are non-null and readable.
        let native_sun_metadata = unsafe { sun_metadata.read() };
        // SAFETY: Required fixed inputs are non-null and readable.
        let options = unsafe { options.read() };
        if native_sun_metadata.structure_size != structure_size::<XvSunSetMetadata>()
            || native_sun_metadata.sample_count != usize_u64(sun_count)
            || options.structure_size != structure_size::<XvSolarEnvelopeOptions>()
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sensor_values = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let candidate_values = unsafe { input_slice(candidates, candidate_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let sun_values = unsafe { input_slice(sun_samples, sun_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate every pointer against its declared count.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        // SAFETY: Cell capacity was checked above.
        let output = unsafe { output_slice(output_cells, candidate_count)? };
        let sensors = sensors_from_ffi(sensor_values)?;
        let candidates = candidate_values
            .iter()
            .map(|value| {
                EnvelopeCandidate::try_new_oriented(
                    value.candidate_id,
                    Vec3::new(value.position_x, value.position_y, value.position_z),
                    Vec3::new(value.axis_x, value.axis_y, value.axis_z),
                )
                .map_err(|_| XvStatus::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // SAFETY: Output metadata and cells were validated above and remain live for this call.
        unsafe {
            execute_solar_envelope(
                scene_handle,
                &sensors,
                &candidates,
                native_sun_metadata,
                sun_values,
                material_values,
                assignment_values,
                options,
                output_metadata,
                output,
            )
        }
    })
}

#[allow(clippy::too_many_arguments)]
unsafe fn execute_solar_envelope(
    scene_handle: u64,
    sensors: &[SolarSensor],
    candidates: &[EnvelopeCandidate],
    native_sun_metadata: XvSunSetMetadata,
    sun_values: &[XvSunSample],
    material_values: &[XvAnalysisMaterial],
    assignment_values: &[XvMaterialAssignment],
    options: XvSolarEnvelopeOptions,
    output_metadata: *mut XvSolarEnvelopeMetadata,
    output: &mut [XvSolarEnvelopeCell],
) -> Result<(), XvStatus> {
    let sun_set = sun_set_from_ffi(native_sun_metadata, sun_values)?;
    let material_library = material_library(material_values, assignment_values)?;
    let scene = super::scene::get_ready_scene(scene_handle)?;
    let started = Instant::now();
    let result = analyze_solar_envelope(
        &scene,
        sensors,
        candidates,
        &sun_set,
        &material_library,
        SolarEnvelopeOptions {
            candidate_radius_meters: options.candidate_radius_meters,
            required_preserved_fraction: options.required_preserved_fraction,
            target_shaded_fraction: options.target_shaded_fraction,
            vertical_clearance_meters: options.vertical_clearance_meters,
            sensor_offset_meters: options.sensor_offset_meters,
            maximum_distance_meters: options.maximum_distance_meters,
            category_mask: options.category_mask,
            maximum_material_layers: usize::try_from(options.maximum_material_layers)
                .map_err(|_| XvStatus::InvalidLength)?,
            minimum_transmission: options.minimum_transmission,
        },
    )
    .map_err(|_| XvStatus::InvalidArgument)?;
    let elapsed = micros(started);
    for (destination, value) in output.iter_mut().zip(&result.cells) {
        *destination = XvSolarEnvelopeCell {
            structure_size: structure_size::<XvSolarEnvelopeCell>(),
            reserved: 0,
            candidate_id: value.candidate_id,
            position_x: value.position.x,
            position_y: value.position.y,
            position_z: value.position.z,
            maximum_solar_access_elevation_meters: value.maximum_solar_access_elevation_meters,
            maximum_solar_access_height_meters: value.maximum_solar_access_height_meters,
            minimum_shading_elevation_meters: value.minimum_shading_elevation_meters,
            minimum_shading_height_meters: value.minimum_shading_height_meters,
            considered_baseline_sun_hours: value.considered_baseline_sun_hours,
            constraint_count: usize_u64(value.constraint_count),
            access_control: control_to_ffi(value.access_control),
            shading_control: control_to_ffi(value.shading_control),
        };
    }
    // SAFETY: The caller validated writable metadata storage.
    unsafe {
        output_metadata.write(XvSolarEnvelopeMetadata {
            structure_size: structure_size::<XvSolarEnvelopeMetadata>(),
            reserved: 0,
            sensor_count: usize_u64(result.sensor_count),
            candidate_count: usize_u64(result.cells.len()),
            sun_count: usize_u64(result.sun_count),
            analysis_time_microseconds: elapsed,
            content_hash_0: hash_word(&result.content_hash, 0),
            content_hash_1: hash_word(&result.content_hash, 1),
            content_hash_2: hash_word(&result.content_hash, 2),
            content_hash_3: hash_word(&result.content_hash, 3),
        });
    }
    Ok(())
}

fn scenarios_from_ffi(
    scenarios: &[XvSolarScenario],
    materials: &[XvAnalysisMaterial],
    assignments: &[XvMaterialAssignment],
) -> Result<Vec<SolarScenario>, XvStatus> {
    scenarios
        .iter()
        .map(|value| {
            let material_start =
                usize::try_from(value.material_offset).map_err(|_| XvStatus::InvalidLength)?;
            let material_count =
                usize::try_from(value.material_count).map_err(|_| XvStatus::InvalidLength)?;
            let assignment_start =
                usize::try_from(value.assignment_offset).map_err(|_| XvStatus::InvalidLength)?;
            let assignment_count =
                usize::try_from(value.assignment_count).map_err(|_| XvStatus::InvalidLength)?;
            let material_end = material_start
                .checked_add(material_count)
                .ok_or(XvStatus::InvalidLength)?;
            let assignment_end = assignment_start
                .checked_add(assignment_count)
                .ok_or(XvStatus::InvalidLength)?;
            let library = material_library(
                materials
                    .get(material_start..material_end)
                    .ok_or(XvStatus::InvalidLength)?,
                assignments
                    .get(assignment_start..assignment_end)
                    .ok_or(XvStatus::InvalidLength)?,
            )?;
            SolarScenario::try_new(
                value.scenario_id,
                format!("scenario-{}", value.scenario_id),
                library,
            )
            .map_err(|_| XvStatus::InvalidArgument)
        })
        .collect()
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

fn sensors_from_ffi(values: &[XvSolarSensor]) -> Result<Vec<SolarSensor>, XvStatus> {
    values
        .iter()
        .map(|value| {
            SolarSensor::try_new(
                SensorId::new(value.sensor_id),
                Vec3::new(value.position_x, value.position_y, value.position_z),
                Vec3::new(value.normal_x, value.normal_y, value.normal_z),
            )
            .map_err(|_| XvStatus::InvalidArgument)
        })
        .collect()
}

fn sun_set_from_ffi(
    metadata: XvSunSetMetadata,
    values: &[XvSunSample],
) -> Result<SunSet, XvStatus> {
    Ok(SunSet {
        location: SolarLocation::try_new(0.0, 0.0, 0.0).expect("zero location is valid"),
        options: SolarOptions::default(),
        samples: values
            .iter()
            .copied()
            .map(sun_sample_from_ffi)
            .collect::<Result<Vec<_>, _>>()?,
        content_hash: hash_from_words([
            metadata.content_hash_0,
            metadata.content_hash_1,
            metadata.content_hash_2,
            metadata.content_hash_3,
        ]),
    })
}

const fn control_to_ffi(value: xvarna_hvare::EnvelopeControl) -> XvEnvelopeControl {
    XvEnvelopeControl {
        sensor_id: value.sensor_id.get(),
        unix_seconds_utc: value.unix_seconds_utc,
        ray_elevation_meters: value.ray_elevation_meters,
        weighted_hours: value.weighted_hours,
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
    fn solar_intelligence_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvSolarScenario>(), 40);
        assert_eq!(mem::size_of::<XvSolarScenarioOptions>(), 56);
        assert_eq!(mem::size_of::<XvSolarScenarioSummary>(), 72);
        assert_eq!(mem::size_of::<XvSolarScenarioTimelineEntry>(), 32);
        assert_eq!(mem::size_of::<XvSolarAttributionEntry>(), 56);
        assert_eq!(mem::size_of::<XvSolarCategoryBreakdown>(), 48);
        assert_eq!(mem::size_of::<XvSolarScenarioDelta>(), 48);
        assert_eq!(mem::size_of::<XvSolarScenarioMetadata>(), 112);
        assert_eq!(mem::size_of::<XvEnvelopeCandidate>(), 32);
        assert_eq!(mem::size_of::<XvSolarEnvelopeOptions>(), 72);
        assert_eq!(mem::size_of::<XvEnvelopeControl>(), 32);
        assert_eq!(mem::size_of::<XvSolarEnvelopeCell>(), 152);
        assert_eq!(mem::size_of::<XvSolarEnvelopeMetadata>(), 72);
        assert_eq!(mem::size_of::<XvOrientedEnvelopeCandidate>(), 56);
    }
}
