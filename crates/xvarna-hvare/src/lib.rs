//! Direct sun and annual irradiance with spatial and temporal attribution.

#![forbid(unsafe_code)]

mod daylight;
mod envelope;
mod irradiance;
mod pv;
mod radiance;
mod sky;

pub use daylight::*;
pub use envelope::*;

pub use irradiance::{
    AnnualIrradianceOptions, AnnualIrradianceResult, IrradianceError, IrradianceTimelineEntry,
    PerezDiffuseComponents, SensorIrradianceSummary, analyze_annual_irradiance,
    analyze_annual_irradiance_controlled, perez_diffuse_components,
};
pub use pv::{
    PvPotentialCell, PvPotentialError, PvPotentialOptions, PvPotentialRegion, PvPotentialResult,
    analyze_pv_potential,
};
pub use radiance::*;

pub use sky::{
    SensorSkySummary, SkyRayEntry, SkyRayState, SkyViewError, SkyViewOptions, SkyViewResult,
    analyze_sky_view, analyze_sky_view_controlled,
};

use core::fmt;
use std::collections::BTreeMap;
use xvarna_geometry::Vec3;
use xvarna_scene::{QueryRay, Scene};
use xvarna_types::{InstanceId, MeshId, ObjectId, SensorId};
use xvarna_zurvan::SunSet;

/// One oriented analysis point in canonical metre coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarSensor {
    /// Stable sensor identifier.
    pub id: SensorId,
    /// Sensor position in canonical metres.
    pub position: Vec3,
    /// Unit surface normal.
    pub normal: Vec3,
}

impl SolarSensor {
    /// Creates and normalizes a finite sensor.
    pub fn try_new(id: SensorId, position: Vec3, normal: Vec3) -> Result<Self, SolarError> {
        if !position.is_finite() {
            return Err(SolarError::InvalidSensorPosition);
        }
        let normal = normal.normalized().ok_or(SolarError::InvalidSensorNormal)?;
        Ok(Self {
            id,
            position,
            normal,
        })
    }
}

/// Direct-sun query and surface-incidence policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectSunOptions {
    /// Normal offset preventing a sensor surface from hitting itself, in metres.
    pub sensor_offset_meters: f64,
    /// Maximum occluder distance in metres; infinity is supported.
    pub maximum_distance_meters: f64,
    /// Minimum accepted normal-to-sun cosine; zero means front hemisphere.
    pub minimum_incidence_cosine: f64,
    /// Instance category mask included as opaque context.
    pub category_mask: u64,
}

impl Default for DirectSunOptions {
    fn default() -> Self {
        Self {
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            minimum_incidence_cosine: 0.0,
            category_mask: u64::MAX,
        }
    }
}

impl DirectSunOptions {
    fn validate(self) -> Result<Self, SolarError> {
        if !self.sensor_offset_meters.is_finite() || self.sensor_offset_meters < 0.0 {
            return Err(SolarError::InvalidSensorOffset);
        }
        if self.maximum_distance_meters.is_nan() || self.maximum_distance_meters <= 0.0 {
            return Err(SolarError::InvalidMaximumDistance);
        }
        if !self.minimum_incidence_cosine.is_finite()
            || !(-1.0..=1.0).contains(&self.minimum_incidence_cosine)
        {
            return Err(SolarError::InvalidIncidenceCosine);
        }
        Ok(self)
    }
}

/// Classification of one sensor/time pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SunState {
    /// Sun was below the configured altitude or had zero schedule weight.
    Inactive = 0,
    /// Sun was behind the oriented sensor surface.
    BackFacing = 1,
    /// Eligible sun direction was unobstructed.
    Visible = 2,
    /// Eligible sun direction was obstructed.
    Blocked = 3,
}

/// Source attribution for one sensor/time pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunTimelineEntry {
    /// Visibility/incidence state.
    pub state: SunState,
    /// First blocking object, or zero when not blocked.
    pub object_id: ObjectId,
    /// First blocking instance, or zero when not blocked.
    pub instance_id: InstanceId,
    /// First blocking mesh resource, or zero when not blocked.
    pub mesh_id: MeshId,
    /// First blocking local triangle, or `u32::MAX` when not blocked.
    pub triangle_id: u32,
    /// First-hit distance in metres, or infinity when not blocked.
    pub distance_meters: f64,
}

impl SunTimelineEntry {
    const fn with_state(state: SunState) -> Self {
        Self {
            state,
            object_id: ObjectId::new(0),
            instance_id: InstanceId::new(0),
            mesh_id: MeshId::new(0),
            triangle_id: u32::MAX,
            distance_meters: f64::INFINITY,
        }
    }
}

/// Aggregated metric and dominant blocking attribution for one sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensorSunSummary {
    /// Stable sensor identifier.
    pub sensor_id: SensorId,
    /// Weighted eligible direct-sun hours received.
    pub direct_sun_hours: f64,
    /// Weighted eligible hours blocked by geometry.
    pub shadow_hours: f64,
    /// Weighted hours after altitude, schedule, and incidence filtering.
    pub eligible_hours: f64,
    /// Direct sun divided by eligible hours, or zero when no interval is eligible.
    pub solar_access_ratio: f64,
    /// Count of visible eligible intervals.
    pub visible_count: usize,
    /// Count of blocked eligible intervals.
    pub blocked_count: usize,
    /// Count excluded by surface incidence.
    pub back_facing_count: usize,
    /// Object responsible for the most weighted shadow hours.
    pub dominant_occluder_object_id: ObjectId,
    /// Weighted hours attributed to the dominant object.
    pub dominant_occluder_hours: f64,
}

/// Complete direct-sun result aligned as `sensor_index * sun_count + sun_index`.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectSunResult {
    /// One aggregate per source sensor.
    pub summaries: Vec<SensorSunSummary>,
    /// Full ordered temporal and spatial attribution matrix.
    pub timeline: Vec<SunTimelineEntry>,
    /// Number of sun samples per sensor row.
    pub sun_count: usize,
    /// Deterministic identity of scene, sun set, sensors, options, and timeline.
    pub content_hash: [u8; 32],
}

/// Computes direct sun, shadow, access ratio, and first-hit attribution.
pub fn analyze_direct_sun(
    scene: &Scene,
    sensors: &[SolarSensor],
    sun_set: &SunSet,
    options: DirectSunOptions,
) -> Result<DirectSunResult, SolarError> {
    if sensors.is_empty() {
        return Err(SolarError::EmptySensors);
    }
    if sun_set.samples.is_empty() {
        return Err(SolarError::EmptySunSet);
    }
    let options = options.validate()?;
    let timeline_count = sensors
        .len()
        .checked_mul(sun_set.samples.len())
        .ok_or(SolarError::ResultTooLarge)?;
    let mut timeline = vec![SunTimelineEntry::with_state(SunState::Inactive); timeline_count];
    let mut ray_indices = Vec::new();
    let mut rays = Vec::new();
    for (sensor_index, sensor) in sensors.iter().enumerate() {
        for (sun_index, sun) in sun_set.samples.iter().enumerate() {
            let timeline_index = sensor_index * sun_set.samples.len() + sun_index;
            if !sun.is_active {
                continue;
            }
            if sensor.normal.dot(sun.direction) <= options.minimum_incidence_cosine {
                timeline[timeline_index] = SunTimelineEntry::with_state(SunState::BackFacing);
                continue;
            }
            let ray = QueryRay::try_new(
                sensor.position + sensor.normal * options.sensor_offset_meters,
                sun.direction,
                0.0,
                options.maximum_distance_meters,
                options.category_mask,
            )
            .map_err(|_| SolarError::InvalidQuery)?;
            ray_indices.push(timeline_index);
            rays.push(ray);
        }
    }
    for (timeline_index, hit) in ray_indices
        .into_iter()
        .zip(scene.trace_closest_batch(&rays))
    {
        timeline[timeline_index] = if hit.hit {
            SunTimelineEntry {
                state: SunState::Blocked,
                object_id: hit.object_id,
                instance_id: hit.instance_id,
                mesh_id: hit.mesh_id,
                triangle_id: hit.triangle_id,
                distance_meters: hit.distance,
            }
        } else {
            SunTimelineEntry::with_state(SunState::Visible)
        };
    }

    let summaries = sensors
        .iter()
        .enumerate()
        .map(|(sensor_index, sensor)| {
            summarize_sensor(
                *sensor,
                &sun_set.samples,
                &timeline[sensor_index * sun_set.samples.len()
                    ..(sensor_index + 1) * sun_set.samples.len()],
            )
        })
        .collect::<Vec<_>>();
    let content_hash = result_hash(scene, sensors, sun_set, options, &timeline);
    Ok(DirectSunResult {
        summaries,
        timeline,
        sun_count: sun_set.samples.len(),
        content_hash,
    })
}

/// Direct-sun analysis errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolarError {
    /// Sensor position contains NaN or infinity.
    InvalidSensorPosition,
    /// Sensor normal is zero or non-finite.
    InvalidSensorNormal,
    /// Sensor offset is negative or non-finite.
    InvalidSensorOffset,
    /// Maximum distance is invalid.
    InvalidMaximumDistance,
    /// Incidence cosine is outside -1..=1.
    InvalidIncidenceCosine,
    /// At least one sensor is required.
    EmptySensors,
    /// At least one sun sample is required.
    EmptySunSet,
    /// Sensor/time matrix length overflowed.
    ResultTooLarge,
    /// A production ray could not be constructed.
    InvalidQuery,
}

impl fmt::Display for SolarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSensorPosition => "sensor position must be finite",
            Self::InvalidSensorNormal => "sensor normal must be finite and non-zero",
            Self::InvalidSensorOffset => "sensor offset must be finite and non-negative",
            Self::InvalidMaximumDistance => "maximum distance must be positive and not NaN",
            Self::InvalidIncidenceCosine => "minimum incidence cosine must be between -1 and 1",
            Self::EmptySensors => "direct-sun analysis requires at least one sensor",
            Self::EmptySunSet => "direct-sun analysis requires at least one sun sample",
            Self::ResultTooLarge => "sensor/time result matrix is too large",
            Self::InvalidQuery => "direct-sun ray construction failed",
        })
    }
}

impl std::error::Error for SolarError {}

fn summarize_sensor(
    sensor: SolarSensor,
    suns: &[xvarna_zurvan::SunSample],
    timeline: &[SunTimelineEntry],
) -> SensorSunSummary {
    let mut direct_sun_hours = 0.0;
    let mut shadow_hours = 0.0;
    let mut visible_count = 0;
    let mut blocked_count = 0;
    let mut back_facing_count = 0;
    let mut occluder_hours = BTreeMap::<ObjectId, f64>::new();
    for (sun, entry) in suns.iter().zip(timeline) {
        let contribution = sun.duration_hours * sun.weight;
        match entry.state {
            SunState::Visible => {
                direct_sun_hours += contribution;
                visible_count += 1;
            }
            SunState::Blocked => {
                shadow_hours += contribution;
                blocked_count += 1;
                *occluder_hours.entry(entry.object_id).or_default() += contribution;
            }
            SunState::BackFacing => back_facing_count += 1,
            SunState::Inactive => {}
        }
    }
    let eligible_hours = direct_sun_hours + shadow_hours;
    let (dominant_occluder_object_id, dominant_occluder_hours) = occluder_hours
        .into_iter()
        .max_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| right.0.cmp(&left.0))
        })
        .unwrap_or((ObjectId::new(0), 0.0));
    SensorSunSummary {
        sensor_id: sensor.id,
        direct_sun_hours,
        shadow_hours,
        eligible_hours,
        solar_access_ratio: if eligible_hours > 0.0 {
            direct_sun_hours / eligible_hours
        } else {
            0.0
        },
        visible_count,
        blocked_count,
        back_facing_count,
        dominant_occluder_object_id,
        dominant_occluder_hours,
    }
}

fn result_hash(
    scene: &Scene,
    sensors: &[SolarSensor],
    sun_set: &SunSet,
    options: DirectSunOptions,
    timeline: &[SunTimelineEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_HVARE_DIRECT_SUN_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&sun_set.content_hash);
    for value in [
        options.sensor_offset_meters,
        options.maximum_distance_meters,
        options.minimum_incidence_cosine,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&options.category_mask.to_le_bytes());
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
    for entry in timeline {
        hasher.update(&[entry.state as u8]);
        hasher.update(&entry.object_id.get().to_le_bytes());
        hasher.update(&entry.instance_id.get().to_le_bytes());
        hasher.update(&entry.mesh_id.get().to_le_bytes());
        hasher.update(&entry.triangle_id.to_le_bytes());
        hasher.update(&entry.distance_meters.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_zurvan::{SolarLocation, SolarOptions, SunSample, TimeSample, calculate_sun_set};

    fn horizontal_occluder() -> Scene {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(-1.0, -1.0, 1.0),
                Vec3::new(2.0, -1.0, 1.0),
                Vec3::new(-1.0, 2.0, 1.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(mesh).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(99),
                InstanceId::new(7),
                1,
            )
            .expect("instance valid");
        builder.build().expect("scene valid")
    }

    fn manual_sun(direction: Vec3, weight: f64) -> SunSample {
        SunSample {
            unix_seconds_utc: 0,
            direction: direction.normalized().expect("direction valid"),
            altitude_degrees: direction.z.asin().to_degrees(),
            azimuth_degrees: 0.0,
            duration_hours: 1.0,
            weight,
            is_active: true,
        }
    }

    #[test]
    fn direct_sun_reports_timeline_and_dominant_occluder() {
        let scene = horizontal_occluder();
        let sensor = SolarSensor::try_new(
            SensorId::new(5),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .expect("sensor valid");
        let sun_set = SunSet {
            location: SolarLocation::try_new(0.0, 0.0, 0.0).expect("location valid"),
            options: SolarOptions::default(),
            samples: vec![
                manual_sun(Vec3::new(0.0, 0.0, 1.0), 1.0),
                manual_sun(Vec3::new(2.0, 0.0, 1.0), 0.5),
            ],
            content_hash: [1; 32],
        };
        let result = analyze_direct_sun(
            &scene,
            &[sensor],
            &sun_set,
            DirectSunOptions {
                category_mask: 1,
                ..DirectSunOptions::default()
            },
        )
        .expect("analysis succeeds");
        assert_eq!(result.timeline[0].state, SunState::Blocked);
        assert_eq!(result.timeline[0].object_id, ObjectId::new(99));
        assert_eq!(result.timeline[1].state, SunState::Visible);
        assert!((result.summaries[0].shadow_hours - 1.0).abs() < 1.0e-12);
        assert!((result.summaries[0].direct_sun_hours - 0.5).abs() < 1.0e-12);
        assert_eq!(
            result.summaries[0].dominant_occluder_object_id,
            ObjectId::new(99)
        );
    }

    #[test]
    fn back_facing_sun_is_excluded_from_eligible_hours() {
        let scene = horizontal_occluder();
        let sensor = SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0))
            .expect("sensor valid");
        let times = [TimeSample::try_new(1_687_348_800, 1.0, 1.0).expect("time valid")];
        let sun_set = calculate_sun_set(
            SolarLocation::try_new(35.0, 51.0, 1_200.0).expect("location valid"),
            &times,
            SolarOptions::default(),
        )
        .expect("sun set valid");
        let result = analyze_direct_sun(&scene, &[sensor], &sun_set, DirectSunOptions::default())
            .expect("analysis succeeds");
        assert_eq!(result.timeline[0].state, SunState::BackFacing);
        assert!(result.summaries[0].eligible_hours.abs() < 1.0e-12);
        assert_eq!(result.summaries[0].back_facing_count, 1);
    }
}
