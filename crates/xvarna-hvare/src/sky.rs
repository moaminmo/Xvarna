//! Deterministic hemispherical visibility, sky-view, and shadow-mask analysis.

use super::{SolarError, SolarSensor};
use core::{f64::consts::TAU, fmt};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use xvarna_geometry::Vec3;
use xvarna_scene::{QueryRay, Scene};
use xvarna_types::{InstanceId, MeshId, ObjectId, SensorId};

const MINIMUM_SAMPLE_COUNT: usize = 16;
const MAXIMUM_SAMPLE_COUNT: usize = 262_144;
const RAY_CHUNK_SIZE: usize = 8_192;
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653_5;

/// Deterministic equal-solid-angle hemisphere sampling and query policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyViewOptions {
    /// Number of Fibonacci directions per sensor.
    pub sample_count: usize,
    /// Stable azimuthal rotation seed.
    pub seed: u64,
    /// Normal offset preventing self-intersection, in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum obstruction distance in canonical metres; infinity is supported.
    pub maximum_distance_meters: f64,
    /// Instance category mask included as opaque context.
    pub category_mask: u64,
}

impl Default for SkyViewOptions {
    fn default() -> Self {
        Self {
            sample_count: 2_048,
            seed: 0,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
        }
    }
}

impl SkyViewOptions {
    fn validate(self) -> Result<Self, SkyViewError> {
        if !(MINIMUM_SAMPLE_COUNT..=MAXIMUM_SAMPLE_COUNT).contains(&self.sample_count) {
            return Err(SkyViewError::InvalidSampleCount);
        }
        if !self.sensor_offset_meters.is_finite() || self.sensor_offset_meters < 0.0 {
            return Err(SkyViewError::InvalidSensorOffset);
        }
        if self.maximum_distance_meters.is_nan() || self.maximum_distance_meters <= 0.0 {
            return Err(SkyViewError::InvalidMaximumDistance);
        }
        Ok(self)
    }
}

/// Binary state for one hemispherical shadow-mask direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SkyRayState {
    /// Direction reaches open sky within the configured distance.
    Visible = 0,
    /// Direction is blocked by scene geometry.
    Blocked = 1,
}

/// One world-space shadow-mask direction and its first-hit attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyRayEntry {
    /// World-space unit direction in the oriented sensor hemisphere.
    pub direction: Vec3,
    /// Open-sky or blocked state.
    pub state: SkyRayState,
    /// First blocking object, or zero when visible.
    pub object_id: ObjectId,
    /// First blocking instance, or zero when visible.
    pub instance_id: InstanceId,
    /// First blocking mesh resource, or zero when visible.
    pub mesh_id: MeshId,
    /// First blocking local triangle, or `u32::MAX` when visible.
    pub triangle_id: u32,
    /// First-hit distance in canonical metres, or infinity when visible.
    pub distance_meters: f64,
}

/// Sky-view metrics, convergence diagnostics, and dominant attribution for one sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensorSkySummary {
    /// Stable source sensor identifier.
    pub sensor_id: SensorId,
    /// Unweighted visible-direction count divided by total samples.
    pub visible_hemisphere_fraction: f64,
    /// Lambert cosine-weighted visible hemispherical fraction.
    pub cosine_weighted_svf: f64,
    /// Approximate unweighted visible solid angle in steradians.
    pub visible_solid_angle_steradians: f64,
    /// Absolute SVF difference between interleaved even and odd sample subsets.
    pub cosine_convergence_delta: f64,
    /// Absolute visible-fraction difference between interleaved subsets.
    pub unweighted_convergence_delta: f64,
    /// Number of open-sky directions.
    pub visible_count: usize,
    /// Number of blocked directions.
    pub blocked_count: usize,
    /// Object with the greatest cosine-weighted blocked contribution.
    pub dominant_occluder_object_id: ObjectId,
    /// Approximate unweighted solid angle blocked by the dominant object.
    pub dominant_occluder_solid_angle_steradians: f64,
    /// Cosine-weighted blocked fraction attributed to the dominant object.
    pub dominant_occluder_projected_fraction: f64,
}

/// Complete sensor-major Sky View and Shadow Mask result.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyViewResult {
    /// One metric aggregate per source sensor.
    pub summaries: Vec<SensorSkySummary>,
    /// Sensor-major directions: `sensor_index * sample_count + sample_index`.
    pub timeline: Vec<SkyRayEntry>,
    /// Number of hemisphere samples per sensor.
    pub sample_count: usize,
    /// Deterministic identity of scene, sensors, sampling, policy, and hits.
    pub content_hash: [u8; 32],
}

/// Sky-view analysis errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkyViewError {
    /// At least one sensor is required.
    EmptySensors,
    /// Sample count is outside the supported range.
    InvalidSampleCount,
    /// Sensor offset is negative or non-finite.
    InvalidSensorOffset,
    /// Maximum distance is invalid.
    InvalidMaximumDistance,
    /// Sensor position or normal is invalid.
    InvalidSensor,
    /// Sensor/sample matrix length overflowed.
    ResultTooLarge,
    /// A production ray could not be constructed.
    InvalidQuery,
    /// The caller requested cancellation.
    Cancelled,
}

impl fmt::Display for SkyViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptySensors => "sky-view analysis requires at least one sensor",
            Self::InvalidSampleCount => "sky sample count must be between 16 and 262144",
            Self::InvalidSensorOffset => "sky sensor offset must be finite and non-negative",
            Self::InvalidMaximumDistance => "sky maximum distance must be positive and not NaN",
            Self::InvalidSensor => "sky sensor position and normal must be finite and non-zero",
            Self::ResultTooLarge => "sky sensor/sample result matrix is too large",
            Self::InvalidQuery => "sky-view ray construction failed",
            Self::Cancelled => "sky-view analysis was cancelled",
        })
    }
}

impl std::error::Error for SkyViewError {}

impl From<SolarError> for SkyViewError {
    fn from(_: SolarError) -> Self {
        Self::InvalidSensor
    }
}

/// Computes a complete deterministic Sky View and Shadow Mask result.
pub fn analyze_sky_view(
    scene: &Scene,
    sensors: &[SolarSensor],
    options: SkyViewOptions,
) -> Result<SkyViewResult, SkyViewError> {
    let cancelled = AtomicBool::new(false);
    let completed_rays = AtomicU64::new(0);
    analyze_sky_view_controlled(scene, sensors, options, &cancelled, &completed_rays)
}

/// Computes Sky View while observing cooperative cancellation between short ray chunks.
pub fn analyze_sky_view_controlled(
    scene: &Scene,
    sensors: &[SolarSensor],
    options: SkyViewOptions,
    cancelled: &AtomicBool,
    completed_rays: &AtomicU64,
) -> Result<SkyViewResult, SkyViewError> {
    if sensors.is_empty() {
        return Err(SkyViewError::EmptySensors);
    }
    let options = options.validate()?;
    let timeline_count = sensors
        .len()
        .checked_mul(options.sample_count)
        .ok_or(SkyViewError::ResultTooLarge)?;
    let local_directions = fibonacci_hemisphere(options.sample_count, options.seed);
    let mut timeline = Vec::with_capacity(timeline_count);
    completed_rays.store(0, Ordering::Relaxed);

    for sensor in sensors {
        if cancelled.load(Ordering::Relaxed) {
            return Err(SkyViewError::Cancelled);
        }
        let (tangent, bitangent) = stable_basis(sensor.normal)?;
        for local_chunk in local_directions.chunks(RAY_CHUNK_SIZE) {
            if cancelled.load(Ordering::Relaxed) {
                return Err(SkyViewError::Cancelled);
            }
            let world_directions = local_chunk
                .iter()
                .map(|local| {
                    (tangent * local.x + bitangent * local.y + sensor.normal * local.z)
                        .normalized()
                        .expect("orthonormal basis preserves finite unit directions")
                })
                .collect::<Vec<_>>();
            let rays = world_directions
                .iter()
                .map(|direction| {
                    QueryRay::try_new(
                        sensor.position + sensor.normal * options.sensor_offset_meters,
                        *direction,
                        0.0,
                        options.maximum_distance_meters,
                        options.category_mask,
                    )
                    .map_err(|_| SkyViewError::InvalidQuery)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let hits = scene.trace_closest_batch(&rays);
            timeline.extend(
                world_directions
                    .into_iter()
                    .zip(hits)
                    .map(|(direction, hit)| {
                        if hit.hit {
                            SkyRayEntry {
                                direction,
                                state: SkyRayState::Blocked,
                                object_id: hit.object_id,
                                instance_id: hit.instance_id,
                                mesh_id: hit.mesh_id,
                                triangle_id: hit.triangle_id,
                                distance_meters: hit.distance,
                            }
                        } else {
                            SkyRayEntry {
                                direction,
                                state: SkyRayState::Visible,
                                object_id: ObjectId::new(0),
                                instance_id: InstanceId::new(0),
                                mesh_id: MeshId::new(0),
                                triangle_id: u32::MAX,
                                distance_meters: f64::INFINITY,
                            }
                        }
                    }),
            );
            completed_rays.fetch_add(local_chunk.len() as u64, Ordering::Relaxed);
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(SkyViewError::Cancelled);
    }

    let summaries = sensors
        .iter()
        .enumerate()
        .map(|(sensor_index, sensor)| {
            summarize_sky_sensor(
                *sensor,
                &timeline[sensor_index * options.sample_count
                    ..(sensor_index + 1) * options.sample_count],
            )
        })
        .collect();
    let content_hash = sky_result_hash(scene, sensors, options, &timeline);
    Ok(SkyViewResult {
        summaries,
        timeline,
        sample_count: options.sample_count,
        content_hash,
    })
}

fn fibonacci_hemisphere(sample_count: usize, seed: u64) -> Vec<Vec3> {
    let count = bounded_count_to_f64(sample_count);
    let phase = seed_phase(seed);
    (0..sample_count)
        .map(|index| {
            let index = bounded_count_to_f64(index);
            let z = (index + 0.5) / count;
            let radius = (1.0 - z * z).max(0.0).sqrt();
            let azimuth = index.mul_add(GOLDEN_ANGLE, phase).rem_euclid(TAU);
            let (sine, cosine) = azimuth.sin_cos();
            Vec3::new(radius * cosine, radius * sine, z)
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn seed_phase(seed: u64) -> f64 {
    let mut value = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    // The shift leaves at most 53 bits, which f64 represents exactly.
    let unit = (value >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
    unit * TAU
}

fn bounded_count_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("sky sample counts are bounded below u32::MAX"))
}

fn stable_basis(normal: Vec3) -> Result<(Vec3, Vec3), SkyViewError> {
    let normal = normal.normalized().ok_or(SkyViewError::InvalidSensor)?;
    let helper = if normal.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let tangent = helper
        .cross(normal)
        .normalized()
        .ok_or(SkyViewError::InvalidSensor)?;
    let bitangent = normal
        .cross(tangent)
        .normalized()
        .ok_or(SkyViewError::InvalidSensor)?;
    Ok((tangent, bitangent))
}

fn summarize_sky_sensor(sensor: SolarSensor, entries: &[SkyRayEntry]) -> SensorSkySummary {
    let solid_angle_per_sample = TAU / bounded_count_to_f64(entries.len());
    let mut visible_count = 0;
    let mut total_cosine = 0.0;
    let mut visible_cosine = 0.0;
    let mut subset_counts = [0_usize; 2];
    let mut subset_visible = [0_usize; 2];
    let mut subset_cosine = [0.0; 2];
    let mut subset_visible_cosine = [0.0; 2];
    let mut occluders = BTreeMap::<ObjectId, (usize, f64)>::new();
    for (index, entry) in entries.iter().enumerate() {
        let subset = index & 1;
        let cosine = sensor.normal.dot(entry.direction).max(0.0);
        total_cosine += cosine;
        subset_counts[subset] += 1;
        subset_cosine[subset] += cosine;
        match entry.state {
            SkyRayState::Visible => {
                visible_count += 1;
                visible_cosine += cosine;
                subset_visible[subset] += 1;
                subset_visible_cosine[subset] += cosine;
            }
            SkyRayState::Blocked => {
                let aggregate = occluders.entry(entry.object_id).or_default();
                aggregate.0 += 1;
                aggregate.1 += cosine;
            }
        }
    }
    let cosine_weighted_svf = visible_cosine / total_cosine;
    let subset_svf = [
        subset_visible_cosine[0] / subset_cosine[0],
        subset_visible_cosine[1] / subset_cosine[1],
    ];
    let subset_unweighted = [
        bounded_count_to_f64(subset_visible[0]) / bounded_count_to_f64(subset_counts[0]),
        bounded_count_to_f64(subset_visible[1]) / bounded_count_to_f64(subset_counts[1]),
    ];
    let (dominant_occluder_object_id, (dominant_count, dominant_cosine)) = occluders
        .into_iter()
        .max_by(|left, right| {
            left.1
                .1
                .total_cmp(&right.1.1)
                .then_with(|| right.0.cmp(&left.0))
        })
        .unwrap_or((ObjectId::new(0), (0, 0.0)));
    SensorSkySummary {
        sensor_id: sensor.id,
        visible_hemisphere_fraction: bounded_count_to_f64(visible_count)
            / bounded_count_to_f64(entries.len()),
        cosine_weighted_svf,
        visible_solid_angle_steradians: bounded_count_to_f64(visible_count)
            * solid_angle_per_sample,
        cosine_convergence_delta: (subset_svf[0] - subset_svf[1]).abs(),
        unweighted_convergence_delta: (subset_unweighted[0] - subset_unweighted[1]).abs(),
        visible_count,
        blocked_count: entries.len() - visible_count,
        dominant_occluder_object_id,
        dominant_occluder_solid_angle_steradians: bounded_count_to_f64(dominant_count)
            * solid_angle_per_sample,
        dominant_occluder_projected_fraction: dominant_cosine / total_cosine,
    }
}

fn sky_result_hash(
    scene: &Scene,
    sensors: &[SolarSensor],
    options: SkyViewOptions,
    timeline: &[SkyRayEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_ASMAN_SKY_VIEW_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&(options.sample_count as u64).to_le_bytes());
    hasher.update(&options.seed.to_le_bytes());
    hasher.update(&options.sensor_offset_meters.to_bits().to_le_bytes());
    hasher.update(&options.maximum_distance_meters.to_bits().to_le_bytes());
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
        for value in [entry.direction.x, entry.direction.y, entry.direction.z] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
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

    fn scene_with_mesh(positions: Vec<Vec3>, triangles: Vec<[u32; 3]>, object_id: u64) -> Scene {
        let mesh = Mesh {
            positions,
            triangles,
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(mesh).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(object_id),
                InstanceId::new(7),
                1,
            )
            .expect("instance valid");
        builder.build().expect("scene valid")
    }

    fn sensor() -> SolarSensor {
        SolarSensor::try_new(SensorId::new(5), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
            .expect("sensor valid")
    }

    #[test]
    fn fibonacci_samples_are_unit_upper_hemisphere_and_seeded() {
        let first = fibonacci_hemisphere(2_048, 42);
        let repeat = fibonacci_hemisphere(2_048, 42);
        let different = fibonacci_hemisphere(2_048, 43);
        assert_eq!(first, repeat);
        assert_ne!(first, different);
        assert!(first.iter().all(|direction| {
            direction.z > 0.0 && (direction.length_squared() - 1.0).abs() < 1.0e-12
        }));
    }

    #[test]
    fn open_and_fully_blocked_hemispheres_have_exact_limits() {
        let open_scene = scene_with_mesh(
            vec![
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(0.0, 1.0, -1.0),
            ],
            vec![[0, 1, 2]],
            1,
        );
        let options = SkyViewOptions {
            sample_count: 1_024,
            ..SkyViewOptions::default()
        };
        let open = analyze_sky_view(&open_scene, &[sensor()], options).expect("analysis succeeds");
        assert!((open.summaries[0].cosine_weighted_svf - 1.0).abs() < f64::EPSILON);
        assert!((open.summaries[0].visible_hemisphere_fraction - 1.0).abs() < f64::EPSILON);

        let extent = 1_000_000.0;
        let blocked_scene = scene_with_mesh(
            vec![
                Vec3::new(-extent, -extent, 1.0),
                Vec3::new(extent, -extent, 1.0),
                Vec3::new(extent, extent, 1.0),
                Vec3::new(-extent, extent, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            99,
        );
        let blocked =
            analyze_sky_view(&blocked_scene, &[sensor()], options).expect("analysis succeeds");
        assert!(blocked.summaries[0].cosine_weighted_svf.abs() < f64::EPSILON);
        assert_eq!(
            blocked.summaries[0].dominant_occluder_object_id,
            ObjectId::new(99)
        );
        assert!(
            blocked
                .timeline
                .iter()
                .all(|entry| entry.state == SkyRayState::Blocked)
        );
    }

    #[test]
    fn vertical_half_screen_converges_to_half_sky_with_attribution() {
        let extent = 1_000_000.0;
        let scene = scene_with_mesh(
            vec![
                Vec3::new(1.0, -extent, 0.0),
                Vec3::new(1.0, extent, 0.0),
                Vec3::new(1.0, extent, extent),
                Vec3::new(1.0, -extent, extent),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            77,
        );
        let result = analyze_sky_view(
            &scene,
            &[sensor()],
            SkyViewOptions {
                sample_count: 8_192,
                ..SkyViewOptions::default()
            },
        )
        .expect("analysis succeeds");
        let summary = result.summaries[0];
        assert!((summary.cosine_weighted_svf - 0.5).abs() < 0.01);
        assert!((summary.visible_hemisphere_fraction - 0.5).abs() < 0.01);
        assert_eq!(summary.dominant_occluder_object_id, ObjectId::new(77));
        assert!(summary.cosine_convergence_delta < 0.02);
    }

    #[test]
    fn cancellation_is_observed_before_ray_generation() {
        let scene = scene_with_mesh(
            vec![
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(0.0, 1.0, -1.0),
            ],
            vec![[0, 1, 2]],
            1,
        );
        let cancelled = AtomicBool::new(true);
        let progress = AtomicU64::new(0);
        let error = analyze_sky_view_controlled(
            &scene,
            &[sensor()],
            SkyViewOptions::default(),
            &cancelled,
            &progress,
        )
        .expect_err("analysis must cancel");
        assert_eq!(error, SkyViewError::Cancelled);
        assert_eq!(progress.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn cancellation_interrupts_a_running_multi_chunk_analysis() {
        let scene = scene_with_mesh(
            vec![
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(0.0, 1.0, -1.0),
            ],
            vec![[0, 1, 2]],
            1,
        );
        let cancelled = AtomicBool::new(false);
        let progress = AtomicU64::new(0);
        let options = SkyViewOptions {
            sample_count: MAXIMUM_SAMPLE_COUNT,
            ..SkyViewOptions::default()
        };

        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                analyze_sky_view_controlled(&scene, &[sensor()], options, &cancelled, &progress)
            });
            while progress.load(Ordering::Relaxed) == 0 && !worker.is_finished() {
                std::thread::yield_now();
            }
            cancelled.store(true, Ordering::Relaxed);
            worker.join().expect("sky worker must not panic")
        });

        assert_eq!(result, Err(SkyViewError::Cancelled));
        let completed = progress.load(Ordering::Relaxed);
        assert!(completed >= RAY_CHUNK_SIZE as u64);
        assert!(completed < MAXIMUM_SAMPLE_COUNT as u64);
    }

    #[test]
    fn category_and_distance_policies_can_exclude_a_full_blocker() {
        let extent = 1_000_000.0;
        let scene = scene_with_mesh(
            vec![
                Vec3::new(-extent, -extent, 1.0),
                Vec3::new(extent, -extent, 1.0),
                Vec3::new(extent, extent, 1.0),
                Vec3::new(-extent, extent, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            99,
        );
        let filtered = analyze_sky_view(
            &scene,
            &[sensor()],
            SkyViewOptions {
                sample_count: 128,
                category_mask: 2,
                ..SkyViewOptions::default()
            },
        )
        .expect("category-filtered analysis succeeds");
        let distance_limited = analyze_sky_view(
            &scene,
            &[sensor()],
            SkyViewOptions {
                sample_count: 128,
                maximum_distance_meters: 0.5,
                ..SkyViewOptions::default()
            },
        )
        .expect("distance-limited analysis succeeds");

        assert!((filtered.summaries[0].cosine_weighted_svf - 1.0).abs() < f64::EPSILON);
        assert!((distance_limited.summaries[0].cosine_weighted_svf - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn oriented_sensor_directions_stay_in_its_world_hemisphere() {
        let scene = scene_with_mesh(
            vec![
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(-1.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2]],
            1,
        );
        let normal = Vec3::new(1.0, 2.0, 3.0)
            .normalized()
            .expect("normal is non-zero");
        let oriented =
            SolarSensor::try_new(SensorId::new(8), Vec3::ZERO, normal).expect("sensor valid");
        let result = analyze_sky_view(
            &scene,
            &[oriented],
            SkyViewOptions {
                sample_count: 2_048,
                ..SkyViewOptions::default()
            },
        )
        .expect("analysis succeeds");

        assert!(
            result
                .timeline
                .iter()
                .all(|entry| entry.direction.dot(normal) > 0.0)
        );
    }
}
