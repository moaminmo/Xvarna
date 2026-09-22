//! Fixed-layout daylight, optical material, Radiance export, and validation ABI.

use super::{XvStatus, hash_word, input_slice, job::get_job, output_slice};
use crate::daena_intelligence::XvMaterialAssignment;
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::Ordering,
    time::Instant,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{
    AnnualDaylightOptions, DaylightError, DaylightMatrixOptions, DaylightMoment, DaylightSensor,
    DaylightSkyModel, OpticalMaterial, OpticalMaterialKind, OpticalMaterialLibrary, RadianceError,
    analyze_annual_daylight_controlled, analyze_daylight_factor,
    analyze_point_illuminance_controlled, compare_daylight_results, export_radiance_bundle,
};
use xvarna_types::{ObjectId, SensorId};
use xvarna_zurvan::{SolarOptions, parse_epw};

const MATRIX_REUSED_FLAG: u32 = 1 << 0;
const ACCEPTED_FLAG: u32 = 1 << 0;
const OCCUPIED_FLAG: u32 = 1 << 0;
const ASE_FAILS_FLAG: u32 = 1 << 0;

/// Fixed-layout Radiance-compatible RGB optical material.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvOpticalMaterial {
    /// Stable non-zero material identity.
    pub material_id: u64,
    /// [`OpticalMaterialKind`] numeric value.
    pub kind: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Linear RGB reflectance.
    pub reflectance_red: f64,
    /// Linear RGB reflectance.
    pub reflectance_green: f64,
    /// Linear RGB reflectance.
    pub reflectance_blue: f64,
    /// Linear RGB visible transmittance.
    pub transmittance_red: f64,
    /// Linear RGB visible transmittance.
    pub transmittance_green: f64,
    /// Linear RGB visible transmittance.
    pub transmittance_blue: f64,
    /// Specular fraction.
    pub specularity: f64,
    /// Surface roughness.
    pub roughness: f64,
    /// Refractive index.
    pub refractive_index: f64,
}

/// Fixed-layout daylight sensor in canonical metre coordinates.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightSensor {
    /// Stable sensor ID.
    pub sensor_id: u64,
    /// Position X.
    pub position_x: f64,
    /// Position Y.
    pub position_y: f64,
    /// Position Z.
    pub position_z: f64,
    /// Normal X.
    pub normal_x: f64,
    /// Normal Y.
    pub normal_y: f64,
    /// Normal Z.
    pub normal_z: f64,
    /// Represented area in square metres.
    pub area_square_meters: f64,
}

/// Fixed-layout reusable daylight matrix policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightMatrixOptions {
    /// Structure byte size.
    pub structure_size: u32,
    /// [`DaylightSkyModel`] numeric value.
    pub sky_model: u32,
    /// Equal-solid-angle sky patch count.
    pub sky_patch_count: u32,
    /// Transparent interaction cap.
    pub maximum_material_layers: u32,
    /// Sensor offset in metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in metres.
    pub maximum_distance_meters: f64,
    /// Included categories.
    pub category_mask: u64,
    /// Throughput cutoff.
    pub minimum_transmission: f64,
}

/// Fixed-layout point-in-time sun and photometric sky condition.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightMoment {
    /// UTC Unix timestamp.
    pub unix_seconds_utc: i64,
    /// Sun direction X.
    pub sun_direction_x: f64,
    /// Sun direction Y.
    pub sun_direction_y: f64,
    /// Sun direction Z.
    pub sun_direction_z: f64,
    /// Direct-normal illuminance in lux.
    pub direct_normal_illuminance_lux: f64,
    /// Diffuse-horizontal illuminance in lux.
    pub diffuse_horizontal_illuminance_lux: f64,
}

/// Fixed-layout point-in-time result row.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPointIlluminanceEntry {
    /// Structure byte size.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source sensor ID.
    pub sensor_id: u64,
    /// Received direct illuminance in lux.
    pub direct_lux: f64,
    /// Received diffuse illuminance in lux.
    pub diffuse_lux: f64,
    /// Total workplane illuminance in lux.
    pub total_lux: f64,
    /// Remaining direct visible transmission.
    pub direct_transmission: f64,
}

/// Shared point/factor matrix and identity metadata.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightMetadata {
    /// Structure byte size.
    pub structure_size: u32,
    /// Matrix-reuse and future flags.
    pub flags: u32,
    /// Source sensor count.
    pub sensor_count: u64,
    /// Native wall-clock analysis duration.
    pub analysis_time_microseconds: u64,
    /// Matrix hash word zero.
    pub matrix_hash_0: u64,
    /// Matrix hash word one.
    pub matrix_hash_1: u64,
    /// Matrix hash word two.
    pub matrix_hash_2: u64,
    /// Matrix hash word three.
    pub matrix_hash_3: u64,
    /// Result hash word zero.
    pub content_hash_0: u64,
    /// Result hash word one.
    pub content_hash_1: u64,
    /// Result hash word two.
    pub content_hash_2: u64,
    /// Result hash word three.
    pub content_hash_3: u64,
}

/// Fixed-layout CIE-overcast daylight-factor row.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightFactorEntry {
    /// Structure byte size.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source sensor ID.
    pub sensor_id: u64,
    /// Interior illuminance in lux.
    pub interior_illuminance_lux: f64,
    /// Daylight factor in percent.
    pub daylight_factor_percent: f64,
}

/// Fixed-layout annual climate-based daylight policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualDaylightOptions {
    /// Structure byte size.
    pub structure_size: u32,
    /// [`DaylightSkyModel`] numeric value.
    pub sky_model: u32,
    /// Equal-solid-angle sky patch count.
    pub sky_patch_count: u32,
    /// Transparent interaction cap.
    pub maximum_material_layers: u32,
    /// Sensor offset in metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in metres.
    pub maximum_distance_meters: f64,
    /// Included scene category mask.
    pub category_mask: u64,
    /// Throughput cutoff.
    pub minimum_transmission: f64,
    /// SPA Delta-T seconds.
    pub delta_t_seconds: f64,
    /// SPA pressure in millibars.
    pub pressure_millibars: f64,
    /// SPA temperature Celsius.
    pub temperature_celsius: f64,
    /// True-north model rotation.
    pub north_rotation_degrees: f64,
    /// Minimum sun altitude.
    pub minimum_altitude_degrees: f64,
    /// Direct luminous efficacy, lm/W.
    pub direct_luminous_efficacy_lm_per_w: f64,
    /// Diffuse luminous efficacy, lm/W.
    pub diffuse_luminous_efficacy_lm_per_w: f64,
    /// Occupancy start hour.
    pub occupied_start_hour: f64,
    /// Occupancy end hour.
    pub occupied_end_hour: f64,
    /// sDA illuminance threshold.
    pub sda_threshold_lux: f64,
    /// sDA occupied-time fraction.
    pub sda_required_fraction: f64,
    /// ASE direct illuminance threshold.
    pub ase_threshold_lux: f64,
    /// ASE maximum exceedance hours.
    pub ase_maximum_hours: f64,
    /// UDI lower threshold.
    pub udi_lower_lux: f64,
    /// UDI preferred threshold.
    pub udi_preferred_lux: f64,
    /// UDI upper threshold.
    pub udi_upper_lux: f64,
}

/// Fixed-layout annual sensor summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualDaylightSummary {
    /// Structure byte size.
    pub structure_size: u32,
    /// Bit zero means ASE failure.
    pub flags: u32,
    /// Source sensor ID.
    pub sensor_id: u64,
    /// Occupied hours.
    pub occupied_hours: f64,
    /// Hours meeting sDA threshold.
    pub sda_qualified_hours: f64,
    /// Occupied sDA fraction.
    pub sda_occupied_fraction: f64,
    /// Direct ASE exceedance hours.
    pub ase_exceedance_hours: f64,
    /// UDI below fraction.
    pub udi_below_fraction: f64,
    /// UDI supplemental fraction.
    pub udi_supplemental_fraction: f64,
    /// UDI useful fraction.
    pub udi_useful_fraction: f64,
    /// UDI exceeded fraction.
    pub udi_exceeded_fraction: f64,
    /// Occupied mean lux.
    pub mean_occupied_lux: f64,
    /// Occupied minimum lux.
    pub minimum_occupied_lux: f64,
    /// Occupied maximum lux.
    pub maximum_occupied_lux: f64,
}

/// Fixed-layout annual sensor/time cell.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualDaylightTimelineEntry {
    /// Structure byte size.
    pub structure_size: u32,
    /// Bit zero means occupied.
    pub flags: u32,
    /// UTC Unix midpoint.
    pub unix_seconds_utc: i64,
    /// Direct illuminance in lux.
    pub direct_lux: f64,
    /// Diffuse illuminance in lux.
    pub diffuse_lux: f64,
    /// Total illuminance in lux.
    pub total_lux: f64,
}

/// Fixed-layout project metrics plus annual provenance.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualDaylightMetadata {
    /// Structure byte size.
    pub structure_size: u32,
    /// Bit zero indicates coefficient-matrix reuse.
    pub flags: u32,
    /// Sensor count.
    pub sensor_count: u64,
    /// Weather count.
    pub weather_count: u64,
    /// Timeline count.
    pub timeline_count: u64,
    /// Native wall-clock duration.
    pub analysis_time_microseconds: u64,
    /// Area meeting sDA, percent.
    pub sda_area_percent: f64,
    /// Area failing ASE, percent.
    pub ase_area_percent: f64,
    /// Area-weighted UDI below, percent.
    pub udi_below_percent: f64,
    /// Area-weighted UDI supplemental, percent.
    pub udi_supplemental_percent: f64,
    /// Area-weighted UDI useful, percent.
    pub udi_useful_percent: f64,
    /// Area-weighted UDI exceeded, percent.
    pub udi_exceeded_percent: f64,
    /// Total sensor area, square metres.
    pub total_sensor_area_square_meters: f64,
    /// Matrix hash word zero.
    pub matrix_hash_0: u64,
    /// Matrix hash word one.
    pub matrix_hash_1: u64,
    /// Matrix hash word two.
    pub matrix_hash_2: u64,
    /// Matrix hash word three.
    pub matrix_hash_3: u64,
    /// Result hash word zero.
    pub content_hash_0: u64,
    /// Result hash word one.
    pub content_hash_1: u64,
    /// Result hash word two.
    pub content_hash_2: u64,
    /// Result hash word three.
    pub content_hash_3: u64,
}

/// Fixed-layout Fast Path versus Radiance validation statistics.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvDaylightValidationReport {
    /// Structure byte size.
    pub structure_size: u32,
    /// Bit zero means all rows accepted.
    pub flags: u32,
    /// Pair count.
    pub count: u64,
    /// Mean signed bias in lux.
    pub mean_bias_lux: f64,
    /// Mean absolute error in lux.
    pub mean_absolute_error_lux: f64,
    /// Root mean square error in lux.
    pub root_mean_square_error_lux: f64,
    /// Mean absolute percentage error.
    pub mean_absolute_percentage_error: f64,
    /// Maximum absolute error in lux.
    pub maximum_absolute_error_lux: f64,
    /// Coefficient of determination.
    pub r_squared: f64,
    /// Fraction meeting the tolerance envelope.
    pub accepted_fraction: f64,
    /// Hash word zero.
    pub content_hash_0: u64,
    /// Hash word one.
    pub content_hash_1: u64,
    /// Hash word two.
    pub content_hash_2: u64,
    /// Hash word three.
    pub content_hash_3: u64,
}

/// Radiance export byte count and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvRadianceExportMetadata {
    /// Structure byte size.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// UTF-8 bytes required including a trailing NUL.
    pub required_bytes: u64,
    /// Export hash word zero.
    pub content_hash_0: u64,
    /// Export hash word one.
    pub content_hash_1: u64,
    /// Export hash word two.
    pub content_hash_2: u64,
    /// Export hash word three.
    pub content_hash_3: u64,
}

/// Computes point-in-time direct and diffuse illuminance with cooperative cancellation.
///
/// # Safety
///
/// All pointers reference their declared counts and output capacities.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_point_illuminance(
    scene_handle: u64,
    sensors: *const XvDaylightSensor,
    sensor_count: usize,
    materials: *const XvOpticalMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    moment: *const XvDaylightMoment,
    options: *const XvDaylightMatrixOptions,
    job_handle: u64,
    output_metadata: *mut XvDaylightMetadata,
    output_entries: *mut XvPointIlluminanceEntry,
    entry_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if moment.is_null() || options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if entry_capacity < sensor_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Pointers and counts are caller-declared and validated by shared helpers.
        let sensors = parse_sensors(unsafe { input_slice(sensors, sensor_count)? })?;
        // SAFETY: Pointers and counts are caller-declared and validated by shared helpers.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Pointers and counts are caller-declared and validated by shared helpers.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let materials = parse_materials(material_values, assignment_values)?;
        // SAFETY: Required pointers were checked non-null.
        let moment = parse_moment(unsafe { moment.read() })?;
        // SAFETY: Required pointers were checked non-null.
        let (matrix_options, sky_model) = parse_matrix_options(unsafe { options.read() })?;
        // SAFETY: Capacity was checked above.
        let output = unsafe { output_slice(output_entries, sensor_count)? };
        let job = get_job(job_handle)?;
        let total = sensor_count
            .checked_mul(matrix_options.sky_patch_count.saturating_add(1))
            .ok_or(XvStatus::InvalidLength)?;
        job.total_units
            .store(u64::try_from(total).unwrap_or(u64::MAX), Ordering::Relaxed);
        job.completed_units.store(0, Ordering::Relaxed);
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_point_illuminance_controlled(
            &scene,
            &sensors,
            &materials,
            moment,
            sky_model,
            matrix_options,
            Some(&job.cancelled),
            Some(&job.completed_units),
        )
        .map_err(daylight_status)?;
        for (destination, source) in output.iter_mut().zip(&result.entries) {
            *destination = XvPointIlluminanceEntry {
                structure_size: structure_size::<XvPointIlluminanceEntry>(),
                reserved: 0,
                sensor_id: source.sensor_id.get(),
                direct_lux: source.direct_lux,
                diffuse_lux: source.diffuse_lux,
                total_lux: source.total_lux,
                direct_transmission: source.direct_transmission,
            };
        }
        let metadata = daylight_metadata(
            sensor_count,
            started,
            result.matrix_reused,
            result.matrix_hash,
            result.content_hash,
        );
        // SAFETY: Metadata pointer is non-null and writable.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

/// Computes CIE-overcast daylight factor.
///
/// # Safety
///
/// All pointers reference their declared counts and output capacities.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_scene_daylight_factor(
    scene_handle: u64,
    sensors: *const XvDaylightSensor,
    sensor_count: usize,
    materials: *const XvOpticalMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    exterior_horizontal_illuminance_lux: f64,
    options: *const XvDaylightMatrixOptions,
    output_metadata: *mut XvDaylightMetadata,
    output_entries: *mut XvDaylightFactorEntry,
    entry_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if entry_capacity < sensor_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate pointer/count pairs.
        let sensors = parse_sensors(unsafe { input_slice(sensors, sensor_count)? })?;
        // SAFETY: Shared helpers validate pointer/count pairs.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate pointer/count pairs.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let materials = parse_materials(material_values, assignment_values)?;
        // SAFETY: Required pointer was checked non-null.
        let (matrix_options, _) = parse_matrix_options(unsafe { options.read() })?;
        // SAFETY: Capacity was checked.
        let output = unsafe { output_slice(output_entries, sensor_count)? };
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_daylight_factor(
            &scene,
            &sensors,
            &materials,
            exterior_horizontal_illuminance_lux,
            matrix_options,
        )
        .map_err(daylight_status)?;
        for (destination, source) in output.iter_mut().zip(&result.entries) {
            *destination = XvDaylightFactorEntry {
                structure_size: structure_size::<XvDaylightFactorEntry>(),
                reserved: 0,
                sensor_id: source.sensor_id.get(),
                interior_illuminance_lux: source.interior_illuminance_lux,
                daylight_factor_percent: source.daylight_factor_percent,
            };
        }
        // SAFETY: Metadata pointer is non-null and writable.
        unsafe {
            output_metadata.write(daylight_metadata(
                sensor_count,
                started,
                result.matrix_reused,
                result.matrix_hash,
                result.content_hash,
            ));
        };
        Ok(())
    })
}

/// Computes annual sDA, ASE, and UDI with reusable coefficients and cooperative cancellation.
///
/// # Safety
///
/// All pointers reference their declared counts and output capacities.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_annual_daylight(
    scene_handle: u64,
    sensors: *const XvDaylightSensor,
    sensor_count: usize,
    materials: *const XvOpticalMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    epw_utf8: *const u8,
    epw_length: usize,
    options: *const XvAnnualDaylightOptions,
    job_handle: u64,
    output_metadata: *mut XvAnnualDaylightMetadata,
    output_summaries: *mut XvAnnualDaylightSummary,
    summary_capacity: usize,
    output_timeline: *mut XvAnnualDaylightTimelineEntry,
    timeline_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: EPW pointer/count pair is validated by shared helper.
        let epw = unsafe { input_slice(epw_utf8, epw_length)? };
        let text = core::str::from_utf8(epw).map_err(|_| XvStatus::InvalidArgument)?;
        let weather = parse_epw(text).map_err(|_| XvStatus::InvalidArgument)?;
        let timeline_count = sensor_count
            .checked_mul(weather.records.len())
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < sensor_count || timeline_capacity < timeline_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate pointer/count pairs.
        let sensors = parse_sensors(unsafe { input_slice(sensors, sensor_count)? })?;
        // SAFETY: Shared helpers validate pointer/count pairs.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate pointer/count pairs.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let materials = parse_materials(material_values, assignment_values)?;
        // SAFETY: Options pointer was checked non-null.
        let options = parse_annual_options(unsafe { options.read() })?;
        // SAFETY: Capacities were checked.
        let summaries = unsafe { output_slice(output_summaries, sensor_count)? };
        // SAFETY: Capacities were checked.
        let timeline = unsafe { output_slice(output_timeline, timeline_count)? };
        let job = get_job(job_handle)?;
        let total = sensor_count
            .checked_mul(
                options
                    .matrix
                    .sky_patch_count
                    .saturating_add(weather.records.len()),
            )
            .ok_or(XvStatus::InvalidLength)?;
        job.total_units
            .store(u64::try_from(total).unwrap_or(u64::MAX), Ordering::Relaxed);
        job.completed_units.store(0, Ordering::Relaxed);
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_annual_daylight_controlled(
            &scene,
            &sensors,
            &weather,
            &materials,
            options,
            Some(&job.cancelled),
            Some(&job.completed_units),
        )
        .map_err(daylight_status)?;
        for (destination, source) in summaries.iter_mut().zip(&result.summaries) {
            *destination = XvAnnualDaylightSummary {
                structure_size: structure_size::<XvAnnualDaylightSummary>(),
                flags: if source.ase_fails { ASE_FAILS_FLAG } else { 0 },
                sensor_id: source.sensor_id.get(),
                occupied_hours: source.occupied_hours,
                sda_qualified_hours: source.sda_qualified_hours,
                sda_occupied_fraction: source.sda_occupied_fraction,
                ase_exceedance_hours: source.ase_exceedance_hours,
                udi_below_fraction: source.udi_below_fraction,
                udi_supplemental_fraction: source.udi_supplemental_fraction,
                udi_useful_fraction: source.udi_useful_fraction,
                udi_exceeded_fraction: source.udi_exceeded_fraction,
                mean_occupied_lux: source.mean_occupied_lux,
                minimum_occupied_lux: source.minimum_occupied_lux,
                maximum_occupied_lux: source.maximum_occupied_lux,
            };
        }
        for (destination, source) in timeline.iter_mut().zip(&result.timeline) {
            *destination = XvAnnualDaylightTimelineEntry {
                structure_size: structure_size::<XvAnnualDaylightTimelineEntry>(),
                flags: if source.occupied { OCCUPIED_FLAG } else { 0 },
                unix_seconds_utc: source.unix_seconds_utc,
                direct_lux: source.direct_lux,
                diffuse_lux: source.diffuse_lux,
                total_lux: source.total_lux,
            };
        }
        let project = result.project;
        let metadata = XvAnnualDaylightMetadata {
            structure_size: structure_size::<XvAnnualDaylightMetadata>(),
            flags: if result.matrix_reused {
                MATRIX_REUSED_FLAG
            } else {
                0
            },
            sensor_count: u64::try_from(sensor_count).unwrap_or(u64::MAX),
            weather_count: u64::try_from(weather.records.len()).unwrap_or(u64::MAX),
            timeline_count: u64::try_from(result.timeline.len()).unwrap_or(u64::MAX),
            analysis_time_microseconds: u64::try_from(started.elapsed().as_micros())
                .unwrap_or(u64::MAX),
            sda_area_percent: project.sda_area_percent,
            ase_area_percent: project.ase_area_percent,
            udi_below_percent: project.udi_below_percent,
            udi_supplemental_percent: project.udi_supplemental_percent,
            udi_useful_percent: project.udi_useful_percent,
            udi_exceeded_percent: project.udi_exceeded_percent,
            total_sensor_area_square_meters: project.total_sensor_area_square_meters,
            matrix_hash_0: hash_word(&result.matrix_hash, 0),
            matrix_hash_1: hash_word(&result.matrix_hash, 1),
            matrix_hash_2: hash_word(&result.matrix_hash, 2),
            matrix_hash_3: hash_word(&result.matrix_hash, 3),
            content_hash_0: hash_word(&result.content_hash, 0),
            content_hash_1: hash_word(&result.content_hash, 1),
            content_hash_2: hash_word(&result.content_hash, 2),
            content_hash_3: hash_word(&result.content_hash, 3),
        };
        // SAFETY: Metadata pointer is non-null and writable.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

/// Compares aligned Fast Path and Radiance lux arrays.
///
/// # Safety
///
/// Both arrays reference `count` readable values and output is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_daylight_compare(
    fast_lux: *const f64,
    radiance_lux: *const f64,
    count: usize,
    absolute_tolerance_lux: f64,
    relative_tolerance: f64,
    output_report: *mut XvDaylightValidationReport,
) -> i32 {
    ffi_status(|| {
        if output_report.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Shared helpers validate pointer/count pairs.
        let fast = unsafe { input_slice(fast_lux, count)? };
        // SAFETY: Shared helpers validate pointer/count pairs.
        let reference = unsafe { input_slice(radiance_lux, count)? };
        let report =
            compare_daylight_results(fast, reference, absolute_tolerance_lux, relative_tolerance)
                .map_err(daylight_status)?;
        let output = XvDaylightValidationReport {
            structure_size: structure_size::<XvDaylightValidationReport>(),
            flags: if report.accepted { ACCEPTED_FLAG } else { 0 },
            count: u64::try_from(report.count).unwrap_or(u64::MAX),
            mean_bias_lux: report.mean_bias_lux,
            mean_absolute_error_lux: report.mean_absolute_error_lux,
            root_mean_square_error_lux: report.root_mean_square_error_lux,
            mean_absolute_percentage_error: report.mean_absolute_percentage_error,
            maximum_absolute_error_lux: report.maximum_absolute_error_lux,
            r_squared: report.r_squared,
            accepted_fraction: report.accepted_fraction,
            content_hash_0: hash_word(&report.content_hash, 0),
            content_hash_1: hash_word(&report.content_hash, 1),
            content_hash_2: hash_word(&report.content_hash, 2),
            content_hash_3: hash_word(&report.content_hash, 3),
        };
        // SAFETY: Output pointer is non-null and writable.
        unsafe { output_report.write(output) };
        Ok(())
    })
}

/// Exports one UTF-8 Radiance bundle section. Section zero materials, one geometry, two sensors,
/// and three manifest. A null output with zero capacity performs a sizing pass.
///
/// # Safety
///
/// Input pointers reference their counts. Output references `output_capacity` writable bytes.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_scene_radiance_export_section(
    scene_handle: u64,
    sensors: *const XvDaylightSensor,
    sensor_count: usize,
    materials: *const XvOpticalMaterial,
    material_count: usize,
    assignments: *const XvMaterialAssignment,
    assignment_count: usize,
    section: u32,
    output_utf8: *mut u8,
    output_capacity: usize,
    output_metadata: *mut XvRadianceExportMetadata,
) -> i32 {
    ffi_status(|| {
        if output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Shared helpers validate pointer/count pairs.
        let sensors = parse_sensors(unsafe { input_slice(sensors, sensor_count)? })?;
        // SAFETY: Shared helpers validate pointer/count pairs.
        let material_values = unsafe { input_slice(materials, material_count)? };
        // SAFETY: Shared helpers validate pointer/count pairs.
        let assignment_values = unsafe { input_slice(assignments, assignment_count)? };
        let materials = parse_materials(material_values, assignment_values)?;
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let bundle = export_radiance_bundle(&scene, &sensors, &materials)
            .map_err(|error| radiance_status(&error))?;
        let value = match section {
            0 => &bundle.materials_rad,
            1 => &bundle.geometry_rad,
            2 => &bundle.sensors_pts,
            3 => &bundle.manifest_json,
            _ => return Err(XvStatus::InvalidArgument),
        };
        let required = value.len().checked_add(1).ok_or(XvStatus::InvalidLength)?;
        let metadata = XvRadianceExportMetadata {
            structure_size: structure_size::<XvRadianceExportMetadata>(),
            reserved: 0,
            required_bytes: u64::try_from(required).map_err(|_| XvStatus::InvalidLength)?,
            content_hash_0: hash_word(&bundle.content_hash, 0),
            content_hash_1: hash_word(&bundle.content_hash, 1),
            content_hash_2: hash_word(&bundle.content_hash, 2),
            content_hash_3: hash_word(&bundle.content_hash, 3),
        };
        // SAFETY: Metadata pointer is non-null and writable.
        unsafe { output_metadata.write(metadata) };
        if output_capacity == 0 {
            return Ok(());
        }
        if output_capacity < required {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacity is positive and validated.
        let output = unsafe { output_slice(output_utf8, required)? };
        output[..value.len()].copy_from_slice(value.as_bytes());
        output[value.len()] = 0;
        Ok(())
    })
}

fn parse_materials(
    materials: &[XvOpticalMaterial],
    assignments: &[XvMaterialAssignment],
) -> Result<OpticalMaterialLibrary, XvStatus> {
    let materials = materials
        .iter()
        .map(|value| {
            if value.reserved != 0 {
                return Err(XvStatus::InvalidArgument);
            }
            let kind = match value.kind {
                0 => OpticalMaterialKind::Plastic,
                1 => OpticalMaterialKind::Glass,
                2 => OpticalMaterialKind::Metal,
                3 => OpticalMaterialKind::Trans,
                4 => OpticalMaterialKind::Mirror,
                _ => return Err(XvStatus::InvalidArgument),
            };
            OpticalMaterial::try_new(
                value.material_id,
                kind,
                [
                    value.reflectance_red,
                    value.reflectance_green,
                    value.reflectance_blue,
                ],
                [
                    value.transmittance_red,
                    value.transmittance_green,
                    value.transmittance_blue,
                ],
                value.specularity,
                value.roughness,
                value.refractive_index,
            )
            .map_err(daylight_status)
        })
        .collect::<Result<Vec<_>, _>>()?;
    OpticalMaterialLibrary::try_new(
        materials,
        assignments
            .iter()
            .map(|value| (ObjectId::new(value.object_id), value.material_id)),
    )
    .map_err(daylight_status)
}

fn parse_sensors(values: &[XvDaylightSensor]) -> Result<Vec<DaylightSensor>, XvStatus> {
    values
        .iter()
        .map(|sensor| {
            DaylightSensor::try_new(
                SensorId::new(sensor.sensor_id),
                Vec3::new(sensor.position_x, sensor.position_y, sensor.position_z),
                Vec3::new(sensor.normal_x, sensor.normal_y, sensor.normal_z),
                sensor.area_square_meters,
            )
            .map_err(daylight_status)
        })
        .collect()
}

fn parse_moment(value: XvDaylightMoment) -> Result<DaylightMoment, XvStatus> {
    DaylightMoment::try_new(
        value.unix_seconds_utc,
        Vec3::new(
            value.sun_direction_x,
            value.sun_direction_y,
            value.sun_direction_z,
        ),
        value.direct_normal_illuminance_lux,
        value.diffuse_horizontal_illuminance_lux,
    )
    .map_err(daylight_status)
}

fn parse_matrix_options(
    value: XvDaylightMatrixOptions,
) -> Result<(DaylightMatrixOptions, DaylightSkyModel), XvStatus> {
    if value.structure_size != structure_size::<XvDaylightMatrixOptions>() {
        return Err(XvStatus::InvalidArgument);
    }
    let sky = parse_sky(value.sky_model)?;
    Ok((
        DaylightMatrixOptions {
            sky_patch_count: usize::try_from(value.sky_patch_count)
                .map_err(|_| XvStatus::InvalidLength)?,
            sensor_offset_meters: value.sensor_offset_meters,
            maximum_distance_meters: value.maximum_distance_meters,
            category_mask: value.category_mask,
            maximum_material_layers: usize::try_from(value.maximum_material_layers)
                .map_err(|_| XvStatus::InvalidLength)?,
            minimum_transmission: value.minimum_transmission,
        },
        sky,
    ))
}

fn parse_annual_options(value: XvAnnualDaylightOptions) -> Result<AnnualDaylightOptions, XvStatus> {
    if value.structure_size != structure_size::<XvAnnualDaylightOptions>() {
        return Err(XvStatus::InvalidArgument);
    }
    Ok(AnnualDaylightOptions {
        matrix: DaylightMatrixOptions {
            sky_patch_count: usize::try_from(value.sky_patch_count)
                .map_err(|_| XvStatus::InvalidLength)?,
            sensor_offset_meters: value.sensor_offset_meters,
            maximum_distance_meters: value.maximum_distance_meters,
            category_mask: value.category_mask,
            maximum_material_layers: usize::try_from(value.maximum_material_layers)
                .map_err(|_| XvStatus::InvalidLength)?,
            minimum_transmission: value.minimum_transmission,
        },
        solar: SolarOptions {
            delta_t_seconds: value.delta_t_seconds,
            pressure_millibars: value.pressure_millibars,
            temperature_celsius: value.temperature_celsius,
            north_rotation_degrees: value.north_rotation_degrees,
            minimum_altitude_degrees: value.minimum_altitude_degrees,
        },
        direct_luminous_efficacy_lm_per_w: value.direct_luminous_efficacy_lm_per_w,
        diffuse_luminous_efficacy_lm_per_w: value.diffuse_luminous_efficacy_lm_per_w,
        occupied_start_hour: value.occupied_start_hour,
        occupied_end_hour: value.occupied_end_hour,
        sda_threshold_lux: value.sda_threshold_lux,
        sda_required_fraction: value.sda_required_fraction,
        ase_threshold_lux: value.ase_threshold_lux,
        ase_maximum_hours: value.ase_maximum_hours,
        udi_lower_lux: value.udi_lower_lux,
        udi_preferred_lux: value.udi_preferred_lux,
        udi_upper_lux: value.udi_upper_lux,
        sky_model: parse_sky(value.sky_model)?,
    })
}

const fn parse_sky(value: u32) -> Result<DaylightSkyModel, XvStatus> {
    match value {
        0 => Ok(DaylightSkyModel::Isotropic),
        1 => Ok(DaylightSkyModel::CieOvercast),
        _ => Err(XvStatus::InvalidArgument),
    }
}

fn daylight_metadata(
    sensor_count: usize,
    started: Instant,
    matrix_reused: bool,
    matrix_hash: [u8; 32],
    content_hash: [u8; 32],
) -> XvDaylightMetadata {
    XvDaylightMetadata {
        structure_size: structure_size::<XvDaylightMetadata>(),
        flags: if matrix_reused { MATRIX_REUSED_FLAG } else { 0 },
        sensor_count: u64::try_from(sensor_count).unwrap_or(u64::MAX),
        analysis_time_microseconds: u64::try_from(started.elapsed().as_micros())
            .unwrap_or(u64::MAX),
        matrix_hash_0: hash_word(&matrix_hash, 0),
        matrix_hash_1: hash_word(&matrix_hash, 1),
        matrix_hash_2: hash_word(&matrix_hash, 2),
        matrix_hash_3: hash_word(&matrix_hash, 3),
        content_hash_0: hash_word(&content_hash, 0),
        content_hash_1: hash_word(&content_hash, 1),
        content_hash_2: hash_word(&content_hash, 2),
        content_hash_3: hash_word(&content_hash, 3),
    }
}

const fn daylight_status(error: DaylightError) -> XvStatus {
    match error {
        DaylightError::Cancelled => XvStatus::Cancelled,
        _ => XvStatus::InvalidArgument,
    }
}

const fn radiance_status(error: &RadianceError) -> XvStatus {
    match error {
        RadianceError::Cancelled => XvStatus::Cancelled,
        RadianceError::Io | RadianceError::ToolUnavailable | RadianceError::CommandFailed(_) => {
            XvStatus::InvalidState
        }
        RadianceError::InvalidInput | RadianceError::Export | RadianceError::InvalidOutput => {
            XvStatus::InvalidArgument
        }
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
        job::{xv_job_create, xv_job_release},
        scene::{
            XvSceneOptions, XvSceneStats, xv_scene_add_instance, xv_scene_add_mesh, xv_scene_build,
            xv_scene_create, xv_scene_release,
        },
    };
    use core::mem::MaybeUninit;

    #[test]
    fn daylight_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvOpticalMaterial>(), 88);
        assert_eq!(mem::size_of::<XvDaylightSensor>(), 64);
        assert_eq!(mem::size_of::<XvDaylightMatrixOptions>(), 48);
        assert_eq!(mem::size_of::<XvDaylightMoment>(), 48);
        assert_eq!(mem::size_of::<XvPointIlluminanceEntry>(), 48);
        assert_eq!(mem::size_of::<XvDaylightMetadata>(), 88);
        assert_eq!(mem::size_of::<XvDaylightFactorEntry>(), 32);
        assert_eq!(mem::size_of::<XvAnnualDaylightOptions>(), 176);
        assert_eq!(mem::size_of::<XvAnnualDaylightSummary>(), 104);
        assert_eq!(mem::size_of::<XvAnnualDaylightTimelineEntry>(), 40);
        assert_eq!(mem::size_of::<XvAnnualDaylightMetadata>(), 160);
        assert_eq!(mem::size_of::<XvDaylightValidationReport>(), 104);
        assert_eq!(mem::size_of::<XvRadianceExportMetadata>(), 48);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn point_factor_validation_and_radiance_export_cross_the_abi() {
        let options = XvSceneOptions::default();
        let mut scene = 0;
        // SAFETY: Test pointers reference live fixed-layout values.
        assert_eq!(
            unsafe { xv_scene_create(&raw const options, &raw mut scene) },
            XvStatus::Success as i32
        );
        let positions = [-1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 0.0, 1.0, -1.0];
        let triangles = [0_u32, 1, 2];
        let mut mesh = 0;
        // SAFETY: Geometry and output arrays are live with declared counts.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    scene,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut mesh,
                )
            },
            0
        );
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        // SAFETY: Transform contains sixteen finite values.
        assert_eq!(
            unsafe { xv_scene_add_instance(scene, mesh, identity.as_ptr(), 1, 1, 1) },
            0
        );
        let mut stats = MaybeUninit::<XvSceneStats>::uninit();
        // SAFETY: Output is writable.
        assert_eq!(unsafe { xv_scene_build(scene, stats.as_mut_ptr()) }, 0);
        let sensor = [XvDaylightSensor {
            sensor_id: 7,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            normal_x: 0.0,
            normal_y: 0.0,
            normal_z: 1.0,
            area_square_meters: 1.0,
        }];
        let matrix = XvDaylightMatrixOptions {
            structure_size: structure_size::<XvDaylightMatrixOptions>(),
            sky_model: 0,
            sky_patch_count: 64,
            maximum_material_layers: 8,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
            minimum_transmission: 1.0e-4,
        };
        let moment = XvDaylightMoment {
            unix_seconds_utc: 0,
            sun_direction_x: 0.0,
            sun_direction_y: 0.0,
            sun_direction_z: 1.0,
            direct_normal_illuminance_lux: 50_000.0,
            diffuse_horizontal_illuminance_lux: 10_000.0,
        };
        let mut job = 0;
        // SAFETY: Job output is writable.
        assert_eq!(unsafe { xv_job_create(&raw mut job) }, 0);
        let mut metadata = MaybeUninit::<XvDaylightMetadata>::uninit();
        let mut point = MaybeUninit::<XvPointIlluminanceEntry>::uninit();
        // SAFETY: All pointer/count pairs reference live storage.
        assert_eq!(
            unsafe {
                xv_scene_point_illuminance(
                    scene,
                    sensor.as_ptr(),
                    1,
                    core::ptr::null(),
                    0,
                    core::ptr::null(),
                    0,
                    &raw const moment,
                    &raw const matrix,
                    job,
                    metadata.as_mut_ptr(),
                    point.as_mut_ptr(),
                    1,
                )
            },
            0
        );
        // SAFETY: Successful call initialized output.
        let point = unsafe { point.assume_init() };
        assert!((point.total_lux - 60_000.0).abs() < 1.0e-6);
        assert_eq!(xv_job_release(job), 0);

        let mut factor = MaybeUninit::<XvDaylightFactorEntry>::uninit();
        // SAFETY: All pointer/count pairs reference live storage.
        assert_eq!(
            unsafe {
                xv_scene_daylight_factor(
                    scene,
                    sensor.as_ptr(),
                    1,
                    core::ptr::null(),
                    0,
                    core::ptr::null(),
                    0,
                    10_000.0,
                    &raw const matrix,
                    metadata.as_mut_ptr(),
                    factor.as_mut_ptr(),
                    1,
                )
            },
            0
        );
        // SAFETY: Successful call initialized output.
        assert!((unsafe { factor.assume_init() }.daylight_factor_percent - 100.0).abs() < 1.0e-6);

        let fast = [100.0, 500.0];
        let reference = [100.0, 500.0];
        let mut validation = MaybeUninit::<XvDaylightValidationReport>::uninit();
        // SAFETY: Arrays are aligned and output is writable.
        assert_eq!(
            unsafe {
                xv_daylight_compare(
                    fast.as_ptr(),
                    reference.as_ptr(),
                    2,
                    1.0,
                    0.01,
                    validation.as_mut_ptr(),
                )
            },
            0
        );
        // SAFETY: Successful call initialized output.
        assert_eq!(
            unsafe { validation.assume_init() }.flags & ACCEPTED_FLAG,
            ACCEPTED_FLAG
        );

        let mut export = MaybeUninit::<XvRadianceExportMetadata>::uninit();
        // SAFETY: Sizing pass permits null output with zero capacity.
        assert_eq!(
            unsafe {
                xv_scene_radiance_export_section(
                    scene,
                    sensor.as_ptr(),
                    1,
                    core::ptr::null(),
                    0,
                    core::ptr::null(),
                    0,
                    3,
                    core::ptr::null_mut(),
                    0,
                    export.as_mut_ptr(),
                )
            },
            0
        );
        // SAFETY: Successful sizing initialized metadata.
        let required =
            usize::try_from(unsafe { export.assume_init() }.required_bytes).expect("size");
        let mut bytes = vec![0_u8; required];
        // SAFETY: Byte output has the exact queried capacity.
        assert_eq!(
            unsafe {
                xv_scene_radiance_export_section(
                    scene,
                    sensor.as_ptr(),
                    1,
                    core::ptr::null(),
                    0,
                    core::ptr::null(),
                    0,
                    3,
                    bytes.as_mut_ptr(),
                    bytes.len(),
                    export.as_mut_ptr(),
                )
            },
            0
        );
        let manifest = core::str::from_utf8(&bytes[..bytes.len() - 1]).expect("UTF-8");
        assert!(manifest.contains("\"schemaVersion\":\"0.16.0\""));
        assert_eq!(xv_scene_release(scene), 0);
    }
}
