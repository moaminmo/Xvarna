//! Perez annual irradiance with ray-traced direct and diffuse obstruction.

use super::{SolarSensor, SunState};
use core::{f64::consts::PI, fmt};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use xvarna_geometry::Vec3;
use xvarna_scene::{Hit, QueryRay, Scene};
use xvarna_types::{InstanceId, MeshId, ObjectId, SensorId};
use xvarna_zurvan::{
    EpwMissingCounts, EpwRecord, EpwWeather, SolarOptions, SunSample, TimeError, calculate_sun_set,
};

const SKY_AZIMUTH_COUNT: usize = 24;
const SKY_ALTITUDE_COUNT: usize = 6;
const SKY_DOME_RAY_COUNT: usize = SKY_AZIMUTH_COUNT * SKY_ALTITUDE_COUNT;
const HORIZON_RAY_COUNT: usize = SKY_AZIMUTH_COUNT;
const STATIC_RAY_COUNT: usize = SKY_DOME_RAY_COUNT + HORIZON_RAY_COUNT;
const WEATHER_CHUNK_SIZE: usize = 512;
const SOLAR_CONSTANT_W_M2: f64 = 1_353.0;

const PEREZ_EPSILON_LIMITS: [f64; 7] = [1.065, 1.230, 1.500, 1.950, 2.800, 4.500, 6.200];
const PEREZ_F11: [f64; 8] = [
    -0.008_311_7,
    0.129_945_7,
    0.329_695_8,
    0.568_205_3,
    0.873_028_0,
    1.132_607_7,
    1.060_159_1,
    0.677_747_0,
];
const PEREZ_F12: [f64; 8] = [
    0.587_728_5,
    0.682_595_4,
    0.486_873_5,
    0.187_452_5,
    -0.392_040_3,
    -1.236_728_4,
    -1.599_913_7,
    -0.327_258_8,
];
const PEREZ_F13: [f64; 8] = [
    -0.062_063_6,
    -0.151_375_2,
    -0.221_095_8,
    -0.295_129_0,
    -0.361_614_9,
    -0.411_849_4,
    -0.358_922_1,
    -0.250_428_6,
];
const PEREZ_F21: [f64; 8] = [
    -0.059_601_2,
    -0.018_932_5,
    0.055_414_0,
    0.108_863_1,
    0.225_564_7,
    0.287_781_3,
    0.264_212_4,
    0.156_131_3,
];
const PEREZ_F22: [f64; 8] = [
    0.072_124_9,
    0.065_965_0,
    -0.063_958_8,
    -0.151_922_9,
    -0.462_044_2,
    -0.823_035_7,
    -1.127_234_0,
    -1.376_503_1,
];
const PEREZ_F23: [f64; 8] = [
    -0.022_021_6,
    -0.028_874_8,
    -0.026_054_2,
    -0.013_975_4,
    0.001_244_8,
    0.055_865_1,
    0.131_069_4,
    0.250_621_2,
];

/// Annual weather, surface, ray-query, and ground-reflection policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualIrradianceOptions {
    /// NREL SPA calculation and true-north policy.
    pub solar: SolarOptions,
    /// Normal offset preventing self-intersection, in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in canonical metres; infinity is supported.
    pub maximum_distance_meters: f64,
    /// Instance category mask included as opaque context.
    pub category_mask: u64,
    /// Fallback diffuse ground-reflectance fraction.
    pub ground_albedo: f64,
    /// Prefer each valid EPW albedo value over the fallback.
    pub use_weather_albedo: bool,
}

impl Default for AnnualIrradianceOptions {
    fn default() -> Self {
        Self {
            solar: SolarOptions::default(),
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
            ground_albedo: 0.2,
            use_weather_albedo: true,
        }
    }
}

impl AnnualIrradianceOptions {
    fn validate(self) -> Result<Self, IrradianceError> {
        if !self.sensor_offset_meters.is_finite() || self.sensor_offset_meters < 0.0 {
            return Err(IrradianceError::InvalidSensorOffset);
        }
        if self.maximum_distance_meters.is_nan() || self.maximum_distance_meters <= 0.0 {
            return Err(IrradianceError::InvalidMaximumDistance);
        }
        if !self.ground_albedo.is_finite() || !(0.0..=1.0).contains(&self.ground_albedo) {
            return Err(IrradianceError::InvalidGroundAlbedo);
        }
        Ok(self)
    }
}

/// Unshaded Perez sky components incident on one tilted surface, in W/m².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerezDiffuseComponents {
    /// Isotropic sky-dome contribution.
    pub dome_w_m2: f64,
    /// Circumsolar contribution concentrated around the sun direction.
    pub circumsolar_w_m2: f64,
    /// Horizon brightening or darkening correction; it may be negative.
    pub horizon_w_m2: f64,
    /// Non-negative sum of all three unshaded components.
    pub total_w_m2: f64,
    /// Perez sky clearness parameter.
    pub clearness: f64,
    /// Perez sky brightness parameter.
    pub brightness: f64,
}

/// One sensor/weather interval with component energy and first-hit attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IrradianceTimelineEntry {
    /// Visibility/incidence state of the solar direction.
    pub state: SunState,
    /// UTC midpoint inherited from the EPW interval.
    pub unix_seconds_utc: i64,
    /// Received beam energy, Wh/m².
    pub direct_wh_m2: f64,
    /// Received Perez isotropic-dome energy, Wh/m².
    pub diffuse_dome_wh_m2: f64,
    /// Received Perez circumsolar energy, Wh/m².
    pub diffuse_circumsolar_wh_m2: f64,
    /// Received Perez horizon correction, Wh/m²; it may be negative.
    pub diffuse_horizon_wh_m2: f64,
    /// Non-negative received total sky-diffuse energy, Wh/m².
    pub diffuse_sky_wh_m2: f64,
    /// Ground-reflected energy from the isotropic ground model, Wh/m².
    pub ground_reflected_wh_m2: f64,
    /// Total plane-of-array energy, Wh/m².
    pub global_wh_m2: f64,
    /// Average total plane-of-array irradiance during the interval, W/m².
    pub global_w_m2: f64,
    /// Direct plus circumsolar energy removed by the first blocker, Wh/m².
    pub attributed_loss_wh_m2: f64,
    /// First blocking object, or zero when the solar ray is not blocked.
    pub object_id: ObjectId,
    /// First blocking instance, or zero when the solar ray is not blocked.
    pub instance_id: InstanceId,
    /// First blocking mesh, or zero when the solar ray is not blocked.
    pub mesh_id: MeshId,
    /// First blocking local triangle, or `u32::MAX` when not blocked.
    pub triangle_id: u32,
    /// First-hit distance in canonical metres, or infinity when not blocked.
    pub distance_meters: f64,
}

/// Annual totals, peak load, visibility diagnostics, and dominant obstruction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensorIrradianceSummary {
    /// Stable source sensor identifier.
    pub sensor_id: SensorId,
    /// Annual received beam energy, Wh/m².
    pub direct_wh_m2: f64,
    /// Annual received sky-diffuse energy, Wh/m².
    pub diffuse_sky_wh_m2: f64,
    /// Annual ground-reflected energy, Wh/m².
    pub ground_reflected_wh_m2: f64,
    /// Annual plane-of-array energy, Wh/m².
    pub global_wh_m2: f64,
    /// Highest interval-average plane-of-array irradiance, W/m².
    pub peak_global_w_m2: f64,
    /// UTC midpoint at which the peak first occurred.
    pub peak_unix_seconds_utc: i64,
    /// Cosine-weighted visible fraction of the 144-patch sky dome.
    pub dome_visibility_ratio: f64,
    /// Projected visible fraction of the 24-sector horizon ring.
    pub horizon_visibility_ratio: f64,
    /// Count of unobstructed front-facing daytime intervals.
    pub visible_count: usize,
    /// Count of obstructed front-facing daytime intervals.
    pub blocked_count: usize,
    /// Count of active sun intervals behind the surface.
    pub back_facing_count: usize,
    /// Object responsible for the greatest direct plus circumsolar energy loss.
    pub dominant_solar_occluder_object_id: ObjectId,
    /// Energy loss assigned to the dominant solar occluder, Wh/m².
    pub dominant_solar_occluder_loss_wh_m2: f64,
    /// Object blocking the greatest projected static sky weight.
    pub dominant_sky_occluder_object_id: ObjectId,
    /// Combined normalized dome/horizon weight assigned to that object.
    pub dominant_sky_occluder_projected_fraction: f64,
}

/// Complete annual result aligned as `sensor_index * weather_count + weather_index`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnualIrradianceResult {
    /// One annual aggregate per source sensor.
    pub summaries: Vec<SensorIrradianceSummary>,
    /// Full sensor-major weather timeline.
    pub timeline: Vec<IrradianceTimelineEntry>,
    /// Number of weather records in every sensor row.
    pub weather_count: usize,
    /// Radiation sanitization counts inherited from EPW parsing.
    pub missing_weather_counts: EpwMissingCounts,
    /// Deterministic identity of scene, weather, sun, sensors, options, and results.
    pub content_hash: [u8; 32],
}

/// Annual-irradiance validation or execution error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrradianceError {
    /// At least one sensor is required.
    EmptySensors,
    /// The EPW weather timeline is empty.
    EmptyWeather,
    /// Sensor offset is negative or non-finite.
    InvalidSensorOffset,
    /// Maximum ray distance is invalid.
    InvalidMaximumDistance,
    /// Ground albedo is outside 0..=1.
    InvalidGroundAlbedo,
    /// Sensor/weather matrix length overflowed.
    ResultTooLarge,
    /// A production ray could not be constructed.
    InvalidQuery,
    /// Solar-position calculation failed.
    Solar(TimeError),
    /// The caller requested cancellation.
    Cancelled,
}

impl fmt::Display for IrradianceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySensors => {
                formatter.write_str("annual irradiance requires at least one sensor")
            }
            Self::EmptyWeather => formatter.write_str("annual irradiance requires weather records"),
            Self::InvalidSensorOffset => {
                formatter.write_str("sensor offset must be finite and non-negative")
            }
            Self::InvalidMaximumDistance => {
                formatter.write_str("maximum distance must be positive and not NaN")
            }
            Self::InvalidGroundAlbedo => {
                formatter.write_str("ground albedo must be between zero and one")
            }
            Self::ResultTooLarge => {
                formatter.write_str("sensor/weather result matrix is too large")
            }
            Self::InvalidQuery => formatter.write_str("annual irradiance ray construction failed"),
            Self::Solar(error) => write!(formatter, "solar-position calculation failed: {error}"),
            Self::Cancelled => formatter.write_str("annual irradiance was cancelled"),
        }
    }
}

impl std::error::Error for IrradianceError {}

impl From<TimeError> for IrradianceError {
    fn from(error: TimeError) -> Self {
        Self::Solar(error)
    }
}

/// Evaluates the EnergyPlus-style Perez 1990 tilted-surface sky model.
#[must_use]
pub fn perez_diffuse_components(
    surface_normal: Vec3,
    sun_direction: Vec3,
    diffuse_horizontal_w_m2: f64,
    direct_normal_w_m2: f64,
    site_elevation_meters: f64,
) -> PerezDiffuseComponents {
    let normal = surface_normal
        .normalized()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    let sun = sun_direction
        .normalized()
        .unwrap_or(Vec3::new(0.0, 0.0, -1.0));
    let diffuse = diffuse_horizontal_w_m2.max(0.0);
    let direct = direct_normal_w_m2.max(0.0);
    let cosine_tilt = normal.z.clamp(-1.0, 1.0);
    let tilt = cosine_tilt.acos();
    let cosine_zenith = sun.z.clamp(-1.0, 1.0);
    if diffuse <= f64::EPSILON || cosine_zenith <= 0.0 {
        let dome = diffuse * (1.0 + cosine_tilt) * 0.5;
        return PerezDiffuseComponents {
            dome_w_m2: dome,
            circumsolar_w_m2: 0.0,
            horizon_w_m2: 0.0,
            total_w_m2: dome,
            clearness: 1.0,
            brightness: 0.0,
        };
    }
    let zenith = cosine_zenith.acos();
    let zenith_cubed = zenith * zenith * zenith;
    let clearness = 1.041_f64.mul_add(zenith_cubed, (diffuse + direct) / diffuse)
        / 1.041_f64.mul_add(zenith_cubed, 1.0);
    let zenith_degrees = zenith.to_degrees();
    let height_factor = 1.0 - 0.1 * site_elevation_meters / 1_000.0;
    let relative_air_mass = if zenith_degrees <= 75.0 {
        height_factor / cosine_zenith
    } else {
        height_factor
            / 0.15_f64.mul_add((93.9 - zenith_degrees).max(0.1).powf(-1.253), cosine_zenith)
    };
    let brightness = diffuse * relative_air_mass.max(0.0) / SOLAR_CONSTANT_W_M2;
    let bin = PEREZ_EPSILON_LIMITS
        .iter()
        .position(|limit| clearness < *limit)
        .unwrap_or(7);
    let first = PEREZ_F13[bin]
        .mul_add(zenith, PEREZ_F12[bin].mul_add(brightness, PEREZ_F11[bin]))
        .max(0.0);
    let second = PEREZ_F23[bin].mul_add(zenith, PEREZ_F22[bin].mul_add(brightness, PEREZ_F21[bin]));
    let incidence = normal.dot(sun).max(0.0);
    let circumsolar_denominator = cosine_zenith.max(0.087);
    let dome = diffuse * (1.0 - first) * (1.0 + cosine_tilt) * 0.5;
    let circumsolar = diffuse * first * incidence / circumsolar_denominator;
    let horizon = diffuse * second * tilt.sin();
    PerezDiffuseComponents {
        dome_w_m2: dome,
        circumsolar_w_m2: circumsolar,
        horizon_w_m2: horizon,
        total_w_m2: (dome + circumsolar + horizon).max(0.0),
        clearness,
        brightness,
    }
}

/// Computes a deterministic complete annual irradiance result.
pub fn analyze_annual_irradiance(
    scene: &Scene,
    sensors: &[SolarSensor],
    weather: &EpwWeather,
    options: AnnualIrradianceOptions,
) -> Result<AnnualIrradianceResult, IrradianceError> {
    let cancelled = AtomicBool::new(false);
    let completed_units = AtomicU64::new(0);
    analyze_annual_irradiance_controlled(
        scene,
        sensors,
        weather,
        options,
        &cancelled,
        &completed_units,
    )
}

/// Computes annual irradiance while observing cooperative cancellation at bounded chunks.
pub fn analyze_annual_irradiance_controlled(
    scene: &Scene,
    sensors: &[SolarSensor],
    weather: &EpwWeather,
    options: AnnualIrradianceOptions,
    cancelled: &AtomicBool,
    completed_units: &AtomicU64,
) -> Result<AnnualIrradianceResult, IrradianceError> {
    if sensors.is_empty() {
        return Err(IrradianceError::EmptySensors);
    }
    if weather.records.is_empty() {
        return Err(IrradianceError::EmptyWeather);
    }
    let options = options.validate()?;
    let timeline_count = sensors
        .len()
        .checked_mul(weather.records.len())
        .ok_or(IrradianceError::ResultTooLarge)?;
    completed_units.store(0, Ordering::Relaxed);
    let schedule = weather.time_samples();
    let sun_set = calculate_sun_set(weather.location.solar, &schedule, options.solar)?;
    let mut timeline = Vec::with_capacity(timeline_count);
    let mut summaries = Vec::with_capacity(sensors.len());
    let sky_directions = sky_dome_directions();
    let horizon_directions = horizon_directions();

    for sensor in sensors {
        if cancelled.load(Ordering::Relaxed) {
            return Err(IrradianceError::Cancelled);
        }
        let sky = trace_static_sky(
            scene,
            *sensor,
            options,
            &sky_directions,
            &horizon_directions,
        )?;
        completed_units.fetch_add(STATIC_RAY_COUNT as u64, Ordering::Relaxed);
        let row_start = timeline.len();
        for (weather_chunk, sun_chunk) in weather
            .records
            .chunks(WEATHER_CHUNK_SIZE)
            .zip(sun_set.samples.chunks(WEATHER_CHUNK_SIZE))
        {
            if cancelled.load(Ordering::Relaxed) {
                return Err(IrradianceError::Cancelled);
            }
            analyze_weather_chunk(
                scene,
                *sensor,
                weather_chunk,
                sun_chunk,
                weather.location.solar.elevation_meters,
                options,
                sky,
                &mut timeline,
            )?;
            completed_units.fetch_add(weather_chunk.len() as u64, Ordering::Relaxed);
        }
        summaries.push(summarize_sensor(*sensor, &timeline[row_start..], sky));
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(IrradianceError::Cancelled);
    }
    let content_hash = result_hash(
        scene,
        sensors,
        weather,
        &sun_set.content_hash,
        options,
        &summaries,
        &timeline,
    );
    Ok(AnnualIrradianceResult {
        summaries,
        timeline,
        weather_count: weather.records.len(),
        missing_weather_counts: weather.missing_counts,
        content_hash,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct StaticSkyResult {
    dome_visibility: f64,
    horizon_visibility: f64,
    dominant_object: ObjectId,
    dominant_fraction: f64,
}

fn trace_static_sky(
    scene: &Scene,
    sensor: SolarSensor,
    options: AnnualIrradianceOptions,
    dome_directions: &[Vec3],
    horizon_directions: &[Vec3],
) -> Result<StaticSkyResult, IrradianceError> {
    let directions = dome_directions
        .iter()
        .chain(horizon_directions)
        .copied()
        .collect::<Vec<_>>();
    let rays = directions
        .iter()
        .map(|direction| query_ray(sensor, *direction, options))
        .collect::<Result<Vec<_>, _>>()?;
    let hits = scene.trace_closest_batch(&rays);
    let delta_azimuth = 2.0 * PI / bounded_index_to_f64(SKY_AZIMUTH_COUNT);
    let delta_altitude = (PI * 0.5) / bounded_index_to_f64(SKY_ALTITUDE_COUNT);
    let mut dome_total = 0.0;
    let mut dome_visible = 0.0;
    let mut horizon_total = 0.0;
    let mut horizon_visible = 0.0;
    let mut blocked_weights = BTreeMap::<ObjectId, f64>::new();
    for (index, (direction, hit)) in directions.iter().zip(hits).enumerate() {
        let projected = sensor.normal.dot(*direction).max(0.0);
        let weight = if index < SKY_DOME_RAY_COUNT {
            let solid_angle =
                direction.z.clamp(-1.0, 1.0).asin().cos() * delta_azimuth * delta_altitude;
            let weight = projected * solid_angle;
            dome_total += weight;
            if !hit.hit {
                dome_visible += weight;
            }
            weight
        } else {
            let weight = projected * delta_azimuth;
            horizon_total += weight;
            if !hit.hit {
                horizon_visible += weight;
            }
            weight
        };
        if hit.hit && weight > 0.0 {
            *blocked_weights.entry(hit.object_id).or_default() += weight;
        }
    }
    let combined_total = dome_total + horizon_total;
    let (dominant_object, dominant_weight) = dominant(&blocked_weights);
    Ok(StaticSkyResult {
        dome_visibility: ratio_or_one(dome_visible, dome_total),
        horizon_visibility: ratio_or_one(horizon_visible, horizon_total),
        dominant_object,
        dominant_fraction: if combined_total > 0.0 {
            dominant_weight / combined_total
        } else {
            0.0
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn analyze_weather_chunk(
    scene: &Scene,
    sensor: SolarSensor,
    weather: &[EpwRecord],
    suns: &[SunSample],
    site_elevation_meters: f64,
    options: AnnualIrradianceOptions,
    sky: StaticSkyResult,
    output: &mut Vec<IrradianceTimelineEntry>,
) -> Result<(), IrradianceError> {
    let mut hit_by_index = vec![Hit::miss(); weather.len()];
    let mut ray_indices = Vec::new();
    let mut rays = Vec::new();
    for (index, sun) in suns.iter().enumerate() {
        if sun.is_active && sensor.normal.dot(sun.direction) > 0.0 {
            ray_indices.push(index);
            rays.push(query_ray(sensor, sun.direction, options)?);
        }
    }
    for (index, hit) in ray_indices
        .into_iter()
        .zip(scene.trace_closest_batch(&rays))
    {
        hit_by_index[index] = hit;
    }
    for ((record, sun), hit) in weather.iter().zip(suns).zip(hit_by_index) {
        output.push(interval_entry(
            sensor,
            record,
            sun,
            hit,
            site_elevation_meters,
            options,
            sky,
        ));
    }
    Ok(())
}

fn interval_entry(
    sensor: SolarSensor,
    weather: &EpwRecord,
    sun: &SunSample,
    hit: Hit,
    site_elevation_meters: f64,
    options: AnnualIrradianceOptions,
    sky: StaticSkyResult,
) -> IrradianceTimelineEntry {
    let duration = weather.duration_hours;
    let direct_normal = weather.direct_normal_wh_m2 / duration;
    let diffuse_horizontal = weather.diffuse_horizontal_wh_m2 / duration;
    let global_horizontal = weather.global_horizontal_wh_m2 / duration;
    let incidence = sensor.normal.dot(sun.direction).max(0.0);
    let eligible = sun.is_active && incidence > 0.0;
    let blocked = eligible && hit.hit;
    let visible = eligible && !hit.hit;
    let perez = perez_diffuse_components(
        sensor.normal,
        sun.direction,
        diffuse_horizontal,
        direct_normal,
        site_elevation_meters,
    );
    let direct_w_m2 = if visible {
        direct_normal * incidence
    } else {
        0.0
    };
    let diffuse_dome_w_m2 = perez.dome_w_m2 * sky.dome_visibility;
    let diffuse_circumsolar_w_m2 = if visible { perez.circumsolar_w_m2 } else { 0.0 };
    let diffuse_horizon_w_m2 = perez.horizon_w_m2 * sky.horizon_visibility;
    let diffuse_sky_w_m2 =
        (diffuse_dome_w_m2 + diffuse_circumsolar_w_m2 + diffuse_horizon_w_m2).max(0.0);
    let cosine_tilt = sensor.normal.z.clamp(-1.0, 1.0);
    let albedo = if options.use_weather_albedo {
        weather.albedo.unwrap_or(options.ground_albedo)
    } else {
        options.ground_albedo
    };
    let ground_w_m2 = global_horizontal * albedo * (1.0 - cosine_tilt) * 0.5;
    let global_w_m2 = (direct_w_m2 + diffuse_sky_w_m2 + ground_w_m2).max(0.0);
    let attributed_loss_wh_m2 = if blocked {
        direct_normal.mul_add(incidence, perez.circumsolar_w_m2.max(0.0)) * duration
    } else {
        0.0
    };
    IrradianceTimelineEntry {
        state: if !sun.is_active {
            SunState::Inactive
        } else if incidence <= 0.0 {
            SunState::BackFacing
        } else if blocked {
            SunState::Blocked
        } else {
            SunState::Visible
        },
        unix_seconds_utc: weather.midpoint_unix_seconds_utc,
        direct_wh_m2: direct_w_m2 * duration,
        diffuse_dome_wh_m2: diffuse_dome_w_m2 * duration,
        diffuse_circumsolar_wh_m2: diffuse_circumsolar_w_m2 * duration,
        diffuse_horizon_wh_m2: diffuse_horizon_w_m2 * duration,
        diffuse_sky_wh_m2: diffuse_sky_w_m2 * duration,
        ground_reflected_wh_m2: ground_w_m2 * duration,
        global_wh_m2: global_w_m2 * duration,
        global_w_m2,
        attributed_loss_wh_m2,
        object_id: if blocked {
            hit.object_id
        } else {
            ObjectId::new(0)
        },
        instance_id: if blocked {
            hit.instance_id
        } else {
            InstanceId::new(0)
        },
        mesh_id: if blocked { hit.mesh_id } else { MeshId::new(0) },
        triangle_id: if blocked { hit.triangle_id } else { u32::MAX },
        distance_meters: if blocked { hit.distance } else { f64::INFINITY },
    }
}

fn summarize_sensor(
    sensor: SolarSensor,
    timeline: &[IrradianceTimelineEntry],
    sky: StaticSkyResult,
) -> SensorIrradianceSummary {
    let mut direct = 0.0;
    let mut diffuse = 0.0;
    let mut ground = 0.0;
    let mut global = 0.0;
    let mut peak = f64::NEG_INFINITY;
    let mut peak_time = 0;
    let mut visible_count = 0;
    let mut blocked_count = 0;
    let mut back_facing_count = 0;
    let mut losses = BTreeMap::<ObjectId, f64>::new();
    for entry in timeline {
        direct += entry.direct_wh_m2;
        diffuse += entry.diffuse_sky_wh_m2;
        ground += entry.ground_reflected_wh_m2;
        global += entry.global_wh_m2;
        if entry.global_w_m2 > peak {
            peak = entry.global_w_m2;
            peak_time = entry.unix_seconds_utc;
        }
        match entry.state {
            SunState::Visible => visible_count += 1,
            SunState::Blocked => {
                blocked_count += 1;
                *losses.entry(entry.object_id).or_default() += entry.attributed_loss_wh_m2;
            }
            SunState::BackFacing => back_facing_count += 1,
            SunState::Inactive => {}
        }
    }
    let (dominant_solar_occluder_object_id, dominant_solar_occluder_loss_wh_m2) = dominant(&losses);
    SensorIrradianceSummary {
        sensor_id: sensor.id,
        direct_wh_m2: direct,
        diffuse_sky_wh_m2: diffuse,
        ground_reflected_wh_m2: ground,
        global_wh_m2: global,
        peak_global_w_m2: peak.max(0.0),
        peak_unix_seconds_utc: peak_time,
        dome_visibility_ratio: sky.dome_visibility,
        horizon_visibility_ratio: sky.horizon_visibility,
        visible_count,
        blocked_count,
        back_facing_count,
        dominant_solar_occluder_object_id,
        dominant_solar_occluder_loss_wh_m2,
        dominant_sky_occluder_object_id: sky.dominant_object,
        dominant_sky_occluder_projected_fraction: sky.dominant_fraction,
    }
}

fn sky_dome_directions() -> Vec<Vec3> {
    let delta_azimuth = 2.0 * PI / bounded_index_to_f64(SKY_AZIMUTH_COUNT);
    let delta_altitude = (PI * 0.5) / bounded_index_to_f64(SKY_ALTITUDE_COUNT);
    (0..SKY_ALTITUDE_COUNT)
        .flat_map(|altitude_index| {
            let altitude = (bounded_index_to_f64(altitude_index) + 0.5) * delta_altitude;
            (0..SKY_AZIMUTH_COUNT).map(move |azimuth_index| {
                direction_from_altitude_azimuth(
                    altitude,
                    (bounded_index_to_f64(azimuth_index) + 0.5) * delta_azimuth,
                )
            })
        })
        .collect()
}

fn horizon_directions() -> Vec<Vec3> {
    let delta_azimuth = 2.0 * PI / bounded_index_to_f64(SKY_AZIMUTH_COUNT);
    (0..SKY_AZIMUTH_COUNT)
        .map(|index| {
            direction_from_altitude_azimuth(
                0.0,
                (bounded_index_to_f64(index) + 0.5) * delta_azimuth,
            )
        })
        .collect()
}

fn direction_from_altitude_azimuth(altitude: f64, azimuth: f64) -> Vec3 {
    let horizontal = altitude.cos();
    Vec3::new(
        azimuth.sin() * horizontal,
        azimuth.cos() * horizontal,
        altitude.sin(),
    )
}

fn bounded_index_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("sky grid indices are bounded below u32::MAX"))
}

fn query_ray(
    sensor: SolarSensor,
    direction: Vec3,
    options: AnnualIrradianceOptions,
) -> Result<QueryRay, IrradianceError> {
    QueryRay::try_new(
        sensor.position + sensor.normal * options.sensor_offset_meters,
        direction,
        0.0,
        options.maximum_distance_meters,
        options.category_mask,
    )
    .map_err(|_| IrradianceError::InvalidQuery)
}

fn ratio_or_one(numerator: f64, denominator: f64) -> f64 {
    if denominator > f64::EPSILON {
        (numerator / denominator).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn dominant(values: &BTreeMap<ObjectId, f64>) -> (ObjectId, f64) {
    values
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map_or((ObjectId::new(0), 0.0), |(id, value)| (*id, *value))
}

#[allow(clippy::too_many_arguments)]
fn result_hash(
    scene: &Scene,
    sensors: &[SolarSensor],
    weather: &EpwWeather,
    sun_hash: &[u8; 32],
    options: AnnualIrradianceOptions,
    summaries: &[SensorIrradianceSummary],
    timeline: &[IrradianceTimelineEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_HVARE_ANNUAL_IRRADIANCE_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&weather.content_hash);
    hasher.update(sun_hash);
    for value in [
        options.sensor_offset_meters,
        options.maximum_distance_meters,
        options.ground_albedo,
        options.solar.delta_t_seconds,
        options.solar.pressure_millibars,
        options.solar.temperature_celsius,
        options.solar.north_rotation_degrees,
        options.solar.minimum_altitude_degrees,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&options.category_mask.to_le_bytes());
    hasher.update(&[u8::from(options.use_weather_albedo)]);
    for sensor in sensors {
        hasher.update(&sensor.id.get().to_le_bytes());
        for value in [
            sensor.position.x,
            sensor.position.y,
            sensor.position.z,
            sensor.normal.x,
            sensor.normal.y,
            sensor.normal.z,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    for summary in summaries {
        for value in [
            summary.direct_wh_m2,
            summary.diffuse_sky_wh_m2,
            summary.ground_reflected_wh_m2,
            summary.global_wh_m2,
            summary.peak_global_w_m2,
            summary.dome_visibility_ratio,
            summary.horizon_visibility_ratio,
            summary.dominant_solar_occluder_loss_wh_m2,
            summary.dominant_sky_occluder_projected_fraction,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&summary.peak_unix_seconds_utc.to_le_bytes());
        hasher.update(
            &summary
                .dominant_solar_occluder_object_id
                .get()
                .to_le_bytes(),
        );
        hasher.update(&summary.dominant_sky_occluder_object_id.get().to_le_bytes());
    }
    for entry in timeline {
        hasher.update(&[entry.state as u8]);
        hasher.update(&entry.unix_seconds_utc.to_le_bytes());
        for value in [
            entry.direct_wh_m2,
            entry.diffuse_dome_wh_m2,
            entry.diffuse_circumsolar_wh_m2,
            entry.diffuse_horizon_wh_m2,
            entry.diffuse_sky_wh_m2,
            entry.ground_reflected_wh_m2,
            entry.global_wh_m2,
            entry.global_w_m2,
            entry.attributed_loss_wh_m2,
            entry.distance_meters,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&entry.object_id.get().to_le_bytes());
        hasher.update(&entry.instance_id.get().to_le_bytes());
        hasher.update(&entry.mesh_id.get().to_le_bytes());
        hasher.update(&entry.triangle_id.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};

    fn scene_with_triangle(positions: [Vec3; 3], object_id: u64) -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options");
        let mesh_id = builder
            .add_mesh(Mesh {
                positions: positions.to_vec(),
                triangles: vec![[0, 1, 2]],
            })
            .expect("mesh");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(object_id),
                InstanceId::new(1),
                u64::MAX,
            )
            .expect("instance");
        builder.build().expect("scene")
    }

    fn open_scene() -> Scene {
        scene_with_triangle(
            [
                Vec3::new(-2.0, -2.0, -2.0),
                Vec3::new(2.0, -2.0, -2.0),
                Vec3::new(-2.0, 2.0, -2.0),
            ],
            10,
        )
    }

    fn weather_record() -> EpwRecord {
        EpwRecord {
            year: 2024,
            month: 6,
            day: 21,
            hour: 12,
            minute: 60,
            midpoint_unix_seconds_utc: 1_718_960_400,
            duration_hours: 1.0,
            data_source_and_uncertainty_flags: "A".to_owned(),
            dry_bulb_celsius: Some(25.0),
            atmospheric_pressure_pascals: Some(101_325.0),
            global_horizontal_wh_m2: 800.0,
            direct_normal_wh_m2: 700.0,
            diffuse_horizontal_wh_m2: 100.0,
            albedo: Some(0.2),
            global_horizontal_missing: false,
            direct_normal_missing: false,
            diffuse_horizontal_missing: false,
        }
    }

    fn weather(record: EpwRecord) -> EpwWeather {
        use xvarna_zurvan::{EpwDataPeriod, EpwLocation};
        EpwWeather {
            location: EpwLocation {
                city: "Equator".to_owned(),
                state_or_province: String::new(),
                country: "Test".to_owned(),
                data_source: "Test".to_owned(),
                station_identifier: "0".to_owned(),
                solar: xvarna_zurvan::SolarLocation::try_new(0.0, 0.0, 0.0).expect("location"),
                time_zone_hours: 0.0,
            },
            data_period: EpwDataPeriod {
                records_per_hour: 1,
                name: "Data".to_owned(),
                start_day_of_week: "Monday".to_owned(),
                start_date: "1/1".to_owned(),
                end_date: "12/31".to_owned(),
            },
            records: vec![record],
            missing_counts: EpwMissingCounts::default(),
            content_hash: [3; 32],
        }
    }

    #[test]
    fn horizontal_perez_surface_receives_exact_diffuse_horizontal_total() {
        let sun = Vec3::new(0.3, 0.2, 0.932_737_905)
            .normalized()
            .expect("sun");
        let result = perez_diffuse_components(Vec3::new(0.0, 0.0, 1.0), sun, 120.0, 600.0, 0.0);
        assert!((result.total_w_m2 - 120.0).abs() < 1.0e-9);
        assert!(result.circumsolar_w_m2 > 0.0);
    }

    #[test]
    fn annual_open_sky_totals_components_and_preserves_peak() {
        let result = analyze_annual_irradiance(
            &open_scene(),
            &[
                SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
                    .expect("sensor"),
            ],
            &weather(weather_record()),
            AnnualIrradianceOptions::default(),
        )
        .expect("analysis");
        let entry = result.timeline[0];
        assert_eq!(entry.state, SunState::Visible);
        assert!(entry.direct_wh_m2 > 0.0);
        assert!((entry.diffuse_sky_wh_m2 - 100.0).abs() < 1.0e-8);
        assert!((entry.global_wh_m2 - result.summaries[0].global_wh_m2).abs() < 1.0e-9);
        assert_eq!(result.summaries[0].peak_unix_seconds_utc, 1_718_960_400);
    }

    #[test]
    fn first_hit_receives_direct_and_circumsolar_loss() {
        let blocker = scene_with_triangle(
            [
                Vec3::new(-1_000.0, -1_000.0, 1.0),
                Vec3::new(1_000.0, -1_000.0, 1.0),
                Vec3::new(0.0, 1_000.0, 1.0),
            ],
            77,
        );
        let result = analyze_annual_irradiance(
            &blocker,
            &[
                SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
                    .expect("sensor"),
            ],
            &weather(weather_record()),
            AnnualIrradianceOptions::default(),
        )
        .expect("analysis");
        assert_eq!(result.timeline[0].state, SunState::Blocked);
        assert_eq!(result.timeline[0].object_id, ObjectId::new(77));
        assert!(result.timeline[0].attributed_loss_wh_m2 > 0.0);
        assert_eq!(
            result.summaries[0].dominant_solar_occluder_object_id,
            ObjectId::new(77)
        );
        assert_eq!(
            result.summaries[0].dominant_sky_occluder_object_id,
            ObjectId::new(77)
        );
        assert!(result.summaries[0].dome_visibility_ratio < 0.1);
    }

    #[test]
    fn vertical_surface_uses_isotropic_ground_reflection() {
        let mut record = weather_record();
        record.direct_normal_wh_m2 = 0.0;
        record.diffuse_horizontal_wh_m2 = 0.0;
        let result = analyze_annual_irradiance(
            &open_scene(),
            &[
                SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0))
                    .expect("sensor"),
            ],
            &weather(record),
            AnnualIrradianceOptions::default(),
        )
        .expect("analysis");
        assert!((result.timeline[0].ground_reflected_wh_m2 - 80.0).abs() < 1.0e-9);
    }

    #[test]
    fn cancellation_is_observed_before_tracing() {
        let cancelled = AtomicBool::new(true);
        let progress = AtomicU64::new(99);
        let result = analyze_annual_irradiance_controlled(
            &open_scene(),
            &[
                SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
                    .expect("sensor"),
            ],
            &weather(weather_record()),
            AnnualIrradianceOptions::default(),
            &cancelled,
            &progress,
        );
        assert_eq!(result, Err(IrradianceError::Cancelled));
        assert_eq!(progress.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn full_year_shape_produces_all_8760_aligned_intervals() {
        let source = weather_record();
        let mut annual_weather = weather(source.clone());
        annual_weather.records = (0..8_760)
            .map(|index| {
                let mut record = source.clone();
                record.midpoint_unix_seconds_utc =
                    source.midpoint_unix_seconds_utc + i64::from(index) * 3_600;
                record
            })
            .collect();
        let result = analyze_annual_irradiance(
            &open_scene(),
            &[
                SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
                    .expect("sensor"),
            ],
            &annual_weather,
            AnnualIrradianceOptions::default(),
        )
        .expect("full-year analysis");
        assert_eq!(result.weather_count, 8_760);
        assert_eq!(result.timeline.len(), 8_760);
        assert!(
            result
                .timeline
                .iter()
                .all(|entry| entry.global_wh_m2.is_finite())
        );
        assert!(result.summaries[0].global_wh_m2 > 0.0);
    }
}
