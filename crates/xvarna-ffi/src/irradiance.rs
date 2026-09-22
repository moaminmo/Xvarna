//! Fixed-layout EPW and annual-irradiance ABI.

use super::{XvStatus, hash_word, input_slice, job::get_job, output_slice};
use crate::solar::XvSolarSensor;
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::Ordering,
    time::Instant,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{
    AnnualIrradianceOptions, IrradianceError, SolarSensor, analyze_annual_irradiance_controlled,
};
use xvarna_types::SensorId;
use xvarna_zurvan::{SolarOptions, parse_epw};

const USE_WEATHER_ALBEDO_FLAG: u32 = 1 << 0;
const SUPPORTED_OPTIONS_FLAGS: u32 = USE_WEATHER_ALBEDO_FLAG;
const STATIC_RAYS_PER_SENSOR: usize = 168;

/// Fixed-layout EPW dimensions, location, diagnostics, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvEpwMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Number of source records per hour.
    pub records_per_hour: u32,
    /// Parsed weather-record count.
    pub weather_count: u64,
    /// Missing or invalid GHI values sanitized to zero.
    pub missing_global_horizontal_count: u64,
    /// Missing or invalid DNI values sanitized to zero.
    pub missing_direct_normal_count: u64,
    /// Missing or invalid DHI values sanitized to zero.
    pub missing_diffuse_horizontal_count: u64,
    /// EPW station latitude in degrees.
    pub latitude_degrees: f64,
    /// EPW station longitude in degrees.
    pub longitude_degrees: f64,
    /// EPW local standard-time offset from UTC, in hours.
    pub time_zone_hours: f64,
    /// EPW station elevation above sea level, in metres.
    pub elevation_meters: f64,
    /// First little-endian word of the semantic BLAKE3 hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Fixed-layout annual irradiance, solar-position, and obstruction policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualIrradianceOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Bit zero prefers valid EPW albedo over the configured fallback.
    pub flags: u32,
    /// Sensor normal offset in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Opaque scene instance-category mask.
    pub category_mask: u64,
    /// Fallback ground albedo from zero through one.
    pub ground_albedo: f64,
    /// Terrestrial Time minus Universal Time in seconds.
    pub delta_t_seconds: f64,
    /// Atmospheric pressure in millibars used by SPA refraction.
    pub pressure_millibars: f64,
    /// Ambient temperature in degrees Celsius used by SPA refraction.
    pub temperature_celsius: f64,
    /// Counter-clockwise model rotation of true north about +Z.
    pub north_rotation_degrees: f64,
    /// Minimum apparent solar altitude included in direct/circumsolar tracing.
    pub minimum_altitude_degrees: f64,
}

/// Fixed-layout annual aggregate and dominant obstruction for one sensor.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIrradianceSummary {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Stable sensor identifier.
    pub sensor_id: u64,
    /// Annual received beam energy, Wh/m².
    pub direct_wh_m2: f64,
    /// Annual received sky-diffuse energy, Wh/m².
    pub diffuse_sky_wh_m2: f64,
    /// Annual ground-reflected energy, Wh/m².
    pub ground_reflected_wh_m2: f64,
    /// Annual plane-of-array energy, Wh/m².
    pub global_wh_m2: f64,
    /// Peak interval-average irradiance, W/m².
    pub peak_global_w_m2: f64,
    /// UTC midpoint of the first peak interval.
    pub peak_unix_seconds_utc: i64,
    /// Cosine-weighted visible fraction of the 144-patch sky dome.
    pub dome_visibility_ratio: f64,
    /// Projected visible fraction of the 24-sector horizon ring.
    pub horizon_visibility_ratio: f64,
    /// Unobstructed front-facing daytime interval count.
    pub visible_count: u64,
    /// Obstructed front-facing daytime interval count.
    pub blocked_count: u64,
    /// Active back-facing interval count.
    pub back_facing_count: u64,
    /// Object causing the greatest beam plus circumsolar loss.
    pub dominant_solar_occluder_object_id: u64,
    /// Energy assigned to the dominant solar blocker, Wh/m².
    pub dominant_solar_occluder_loss_wh_m2: f64,
    /// Object blocking the greatest projected static sky weight.
    pub dominant_sky_occluder_object_id: u64,
    /// Combined normalized dome/horizon weight assigned to that object.
    pub dominant_sky_occluder_projected_fraction: f64,
}

/// Fixed-layout energy components and first-hit attribution for one interval.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvIrradianceTimelineEntry {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// [`xvarna_hvare::SunState`] numeric value.
    pub state: u32,
    /// EPW interval midpoint in UTC Unix seconds.
    pub unix_seconds_utc: i64,
    /// Received beam energy, Wh/m².
    pub direct_wh_m2: f64,
    /// Received Perez isotropic-dome energy, Wh/m².
    pub diffuse_dome_wh_m2: f64,
    /// Received Perez circumsolar energy, Wh/m².
    pub diffuse_circumsolar_wh_m2: f64,
    /// Received Perez horizon correction, Wh/m².
    pub diffuse_horizon_wh_m2: f64,
    /// Non-negative received total sky-diffuse energy, Wh/m².
    pub diffuse_sky_wh_m2: f64,
    /// Ground-reflected energy, Wh/m².
    pub ground_reflected_wh_m2: f64,
    /// Total plane-of-array energy, Wh/m².
    pub global_wh_m2: f64,
    /// Interval-average plane-of-array irradiance, W/m².
    pub global_w_m2: f64,
    /// Direct plus circumsolar energy removed by the first blocker, Wh/m².
    pub attributed_loss_wh_m2: f64,
    /// First blocking object, or zero.
    pub object_id: u64,
    /// First blocking instance, or zero.
    pub instance_id: u64,
    /// First blocking mesh, or zero.
    pub mesh_id: u64,
    /// First blocking local triangle, or `UINT32_MAX`.
    pub triangle_id: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// First-hit distance in canonical metres, or infinity.
    pub distance_meters: f64,
}

/// Fixed-layout annual result dimensions, diagnostics, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAnnualIrradianceMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Source sensor count.
    pub sensor_count: u64,
    /// Source EPW weather-record count.
    pub weather_count: u64,
    /// Sensor-major interval result count.
    pub timeline_count: u64,
    /// Native analysis duration in microseconds.
    pub analysis_time_microseconds: u64,
    /// Sanitized GHI count.
    pub missing_global_horizontal_count: u64,
    /// Sanitized DNI count.
    pub missing_direct_normal_count: u64,
    /// Sanitized DHI count.
    pub missing_diffuse_horizontal_count: u64,
    /// First little-endian word of the result BLAKE3 hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Parses and validates EPW text without running an analysis.
///
/// # Safety
///
/// `epw_utf8` references `epw_length` readable bytes and `output_metadata` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_epw_inspect(
    epw_utf8: *const u8,
    epw_length: usize,
    output_metadata: *mut XvEpwMetadata,
) -> i32 {
    ffi_status(|| {
        if output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let weather = parse_weather(epw_utf8, epw_length)?;
        let metadata = epw_metadata(&weather);
        // SAFETY: Output pointer is non-null and caller-owned writable storage.
        unsafe { output_metadata.write(metadata) };
        Ok(())
    })
}

/// Computes Perez annual irradiance from UTF-8 EPW text and a compiled scene.
///
/// # Safety
///
/// All input pointers reference their declared counts. Output capacities cover
/// `sensor_count` summaries and `sensor_count * EPW weather_count` entries.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_scene_annual_irradiance(
    scene_handle: u64,
    sensors: *const XvSolarSensor,
    sensor_count: usize,
    epw_utf8: *const u8,
    epw_length: usize,
    options: *const XvAnnualIrradianceOptions,
    job_handle: u64,
    output_metadata: *mut XvAnnualIrradianceMetadata,
    output_summaries: *mut XvIrradianceSummary,
    summary_capacity: usize,
    output_timeline: *mut XvIrradianceTimelineEntry,
    timeline_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Options pointer is non-null and readable for this call.
        let native_options = unsafe { options.read() };
        if native_options.structure_size != structure_size::<XvAnnualIrradianceOptions>()
            || native_options.flags & !SUPPORTED_OPTIONS_FLAGS != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let weather = parse_weather(epw_utf8, epw_length)?;
        let required_timeline = sensor_count
            .checked_mul(weather.records.len())
            .ok_or(XvStatus::InvalidLength)?;
        if summary_capacity < sensor_count || timeline_capacity < required_timeline {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Input pointers and counts are validated by shared helpers.
        let sensors = unsafe { input_slice(sensors, sensor_count)? };
        // SAFETY: Capacities were validated above.
        let output_summaries = unsafe { output_slice(output_summaries, sensor_count)? };
        // SAFETY: Capacity and multiplication were validated above.
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
        let job = get_job(job_handle)?;
        let work_per_sensor = STATIC_RAYS_PER_SENSOR
            .checked_add(weather.records.len())
            .ok_or(XvStatus::InvalidLength)?;
        let total_units = sensor_count
            .checked_mul(work_per_sensor)
            .ok_or(XvStatus::InvalidLength)?;
        job.total_units.store(
            u64::try_from(total_units).map_err(|_| XvStatus::InvalidLength)?,
            Ordering::Relaxed,
        );
        job.completed_units.store(0, Ordering::Relaxed);
        let scene = super::scene::get_ready_scene(scene_handle)?;
        let started = Instant::now();
        let result = analyze_annual_irradiance_controlled(
            &scene,
            &sensors,
            &weather,
            AnnualIrradianceOptions {
                solar: SolarOptions {
                    delta_t_seconds: native_options.delta_t_seconds,
                    pressure_millibars: native_options.pressure_millibars,
                    temperature_celsius: native_options.temperature_celsius,
                    north_rotation_degrees: native_options.north_rotation_degrees,
                    minimum_altitude_degrees: native_options.minimum_altitude_degrees,
                },
                sensor_offset_meters: native_options.sensor_offset_meters,
                maximum_distance_meters: native_options.maximum_distance_meters,
                category_mask: native_options.category_mask,
                ground_albedo: native_options.ground_albedo,
                use_weather_albedo: native_options.flags & USE_WEATHER_ALBEDO_FLAG != 0,
            },
            &job.cancelled,
            &job.completed_units,
        )
        .map_err(irradiance_error_status)?;
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        for (destination, summary) in output_summaries.iter_mut().zip(&result.summaries) {
            *destination = XvIrradianceSummary {
                structure_size: structure_size::<XvIrradianceSummary>(),
                reserved: 0,
                sensor_id: summary.sensor_id.get(),
                direct_wh_m2: summary.direct_wh_m2,
                diffuse_sky_wh_m2: summary.diffuse_sky_wh_m2,
                ground_reflected_wh_m2: summary.ground_reflected_wh_m2,
                global_wh_m2: summary.global_wh_m2,
                peak_global_w_m2: summary.peak_global_w_m2,
                peak_unix_seconds_utc: summary.peak_unix_seconds_utc,
                dome_visibility_ratio: summary.dome_visibility_ratio,
                horizon_visibility_ratio: summary.horizon_visibility_ratio,
                visible_count: u64::try_from(summary.visible_count).unwrap_or(u64::MAX),
                blocked_count: u64::try_from(summary.blocked_count).unwrap_or(u64::MAX),
                back_facing_count: u64::try_from(summary.back_facing_count).unwrap_or(u64::MAX),
                dominant_solar_occluder_object_id: summary.dominant_solar_occluder_object_id.get(),
                dominant_solar_occluder_loss_wh_m2: summary.dominant_solar_occluder_loss_wh_m2,
                dominant_sky_occluder_object_id: summary.dominant_sky_occluder_object_id.get(),
                dominant_sky_occluder_projected_fraction: summary
                    .dominant_sky_occluder_projected_fraction,
            };
        }
        for (destination, entry) in output_timeline.iter_mut().zip(&result.timeline) {
            *destination = XvIrradianceTimelineEntry {
                structure_size: structure_size::<XvIrradianceTimelineEntry>(),
                state: u32::from(entry.state as u8),
                unix_seconds_utc: entry.unix_seconds_utc,
                direct_wh_m2: entry.direct_wh_m2,
                diffuse_dome_wh_m2: entry.diffuse_dome_wh_m2,
                diffuse_circumsolar_wh_m2: entry.diffuse_circumsolar_wh_m2,
                diffuse_horizon_wh_m2: entry.diffuse_horizon_wh_m2,
                diffuse_sky_wh_m2: entry.diffuse_sky_wh_m2,
                ground_reflected_wh_m2: entry.ground_reflected_wh_m2,
                global_wh_m2: entry.global_wh_m2,
                global_w_m2: entry.global_w_m2,
                attributed_loss_wh_m2: entry.attributed_loss_wh_m2,
                object_id: entry.object_id.get(),
                instance_id: entry.instance_id.get(),
                mesh_id: entry.mesh_id.get(),
                triangle_id: entry.triangle_id,
                reserved: 0,
                distance_meters: entry.distance_meters,
            };
        }
        let missing = result.missing_weather_counts;
        let metadata = XvAnnualIrradianceMetadata {
            structure_size: structure_size::<XvAnnualIrradianceMetadata>(),
            reserved: 0,
            sensor_count: u64::try_from(sensor_count).unwrap_or(u64::MAX),
            weather_count: u64::try_from(result.weather_count).unwrap_or(u64::MAX),
            timeline_count: u64::try_from(required_timeline).unwrap_or(u64::MAX),
            analysis_time_microseconds: elapsed,
            missing_global_horizontal_count: u64::try_from(missing.global_horizontal)
                .unwrap_or(u64::MAX),
            missing_direct_normal_count: u64::try_from(missing.direct_normal).unwrap_or(u64::MAX),
            missing_diffuse_horizontal_count: u64::try_from(missing.diffuse_horizontal)
                .unwrap_or(u64::MAX),
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

fn parse_weather(
    epw_utf8: *const u8,
    epw_length: usize,
) -> Result<xvarna_zurvan::EpwWeather, XvStatus> {
    // SAFETY: Caller declares this input buffer readable for the duration of the call.
    let bytes = unsafe { input_slice(epw_utf8, epw_length)? };
    let text = core::str::from_utf8(bytes).map_err(|_| XvStatus::InvalidArgument)?;
    parse_epw(text).map_err(|_| XvStatus::InvalidArgument)
}

fn epw_metadata(weather: &xvarna_zurvan::EpwWeather) -> XvEpwMetadata {
    XvEpwMetadata {
        structure_size: structure_size::<XvEpwMetadata>(),
        records_per_hour: u32::from(weather.data_period.records_per_hour),
        weather_count: u64::try_from(weather.records.len()).unwrap_or(u64::MAX),
        missing_global_horizontal_count: u64::try_from(weather.missing_counts.global_horizontal)
            .unwrap_or(u64::MAX),
        missing_direct_normal_count: u64::try_from(weather.missing_counts.direct_normal)
            .unwrap_or(u64::MAX),
        missing_diffuse_horizontal_count: u64::try_from(weather.missing_counts.diffuse_horizontal)
            .unwrap_or(u64::MAX),
        latitude_degrees: weather.location.solar.latitude_degrees,
        longitude_degrees: weather.location.solar.longitude_degrees,
        time_zone_hours: weather.location.time_zone_hours,
        elevation_meters: weather.location.solar.elevation_meters,
        content_hash_0: hash_word(&weather.content_hash, 0),
        content_hash_1: hash_word(&weather.content_hash, 1),
        content_hash_2: hash_word(&weather.content_hash, 2),
        content_hash_3: hash_word(&weather.content_hash, 3),
    }
}

const fn irradiance_error_status(error: IrradianceError) -> XvStatus {
    match error {
        IrradianceError::Cancelled => XvStatus::Cancelled,
        IrradianceError::EmptySensors
        | IrradianceError::EmptyWeather
        | IrradianceError::InvalidSensorOffset
        | IrradianceError::InvalidMaximumDistance
        | IrradianceError::InvalidGroundAlbedo
        | IrradianceError::ResultTooLarge
        | IrradianceError::InvalidQuery
        | IrradianceError::Solar(_) => XvStatus::InvalidArgument,
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
    use core::mem::MaybeUninit;

    const EPW: &str = concat!(
        "LOCATION,Tehran,Tehran,IRN,IWEC,407540,35.68,51.32,3.5,1191\n",
        "DESIGN CONDITIONS,0\n",
        "TYPICAL/EXTREME PERIODS,0\n",
        "GROUND TEMPERATURES,0\n",
        "HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0\n",
        "COMMENTS 1,XVARNA ABI fixture\n",
        "COMMENTS 2,one interval\n",
        "DATA PERIODS,1,1,Data,Monday,1/1,12/31\n",
        "2024,6,21,12,60,A,25,0,0,90000,0,0,333,800,700,100,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.2,0,0\n",
    );

    #[test]
    fn annual_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvEpwMetadata>(), 104);
        assert_eq!(mem::size_of::<XvAnnualIrradianceOptions>(), 80);
        assert_eq!(mem::size_of::<XvIrradianceSummary>(), 136);
        assert_eq!(mem::size_of::<XvIrradianceTimelineEntry>(), 128);
        assert_eq!(mem::size_of::<XvAnnualIrradianceMetadata>(), 96);
    }

    #[test]
    fn epw_inspection_crosses_utf8_abi() {
        let mut metadata = MaybeUninit::<XvEpwMetadata>::uninit();
        // SAFETY: EPW bytes are readable and metadata storage is writable.
        assert_eq!(
            unsafe { xv_epw_inspect(EPW.as_ptr(), EPW.len(), metadata.as_mut_ptr()) },
            XvStatus::Success as i32
        );
        // SAFETY: Successful call initialized metadata.
        let metadata = unsafe { metadata.assume_init() };
        assert_eq!(metadata.structure_size, 104);
        assert_eq!(metadata.weather_count, 1);
        assert_eq!(metadata.records_per_hour, 1);
        assert!((metadata.latitude_degrees - 35.68).abs() < 1.0e-12);
    }
}
