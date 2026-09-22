//! Deterministic protected view-corridor aperture analysis.

use crate::{VisibilityError, VisibilityState};
use core::f64::consts::PI;
use std::collections::BTreeMap;
use xvarna_geometry::Vec3;
use xvarna_scene::{QueryRay, RayQueryExecutionSummary, RayQueryExecutor, Scene};
use xvarna_types::{InstanceId, MeshId, ObjectId};

const MAXIMUM_CORRIDOR_SAMPLES: usize = 1_048_576;
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// A finite cone-like protected corridor ending in a circular target aperture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCorridor {
    /// Stable corridor identifier.
    pub corridor_id: u64,
    /// Observer or corridor apex in canonical metres.
    pub origin: Vec3,
    /// Centre of the protected target aperture.
    pub target: Vec3,
    /// Approximate camera up; orthogonalized against the corridor axis.
    pub up: Vec3,
    /// Radius of the target aperture in canonical metres.
    pub target_radius_meters: f64,
}

impl ViewCorridor {
    /// Creates a finite non-degenerate corridor.
    pub fn try_new(
        corridor_id: u64,
        origin: Vec3,
        target: Vec3,
        up: Vec3,
        target_radius_meters: f64,
    ) -> Result<Self, VisibilityError> {
        if !origin.is_finite()
            || !target.is_finite()
            || !up.is_finite()
            || !target_radius_meters.is_finite()
            || target_radius_meters < 0.0
            || (target - origin).length_squared() <= 1.0e-24
        {
            return Err(VisibilityError::InvalidGeometry);
        }
        let axis = (target - origin)
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        let up = (up - axis * up.dot(axis))
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        Ok(Self {
            corridor_id,
            origin,
            target,
            up,
            target_radius_meters,
        })
    }
}

/// Sampling and scene-query policy for protected corridors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCorridorOptions {
    /// Uniform-area aperture samples per corridor.
    pub sample_count: usize,
    /// Clearance removed from both endpoints.
    pub endpoint_clearance_meters: f64,
    /// Included occluder categories.
    pub category_mask: u64,
}

impl Default for ViewCorridorOptions {
    fn default() -> Self {
        Self {
            sample_count: 512,
            endpoint_clearance_meters: 1.0e-4,
            category_mask: u64::MAX,
        }
    }
}

impl ViewCorridorOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !(1..=MAXIMUM_CORRIDOR_SAMPLES).contains(&self.sample_count) {
            return Err(VisibilityError::InvalidSamplingPolicy);
        }
        if !self.endpoint_clearance_meters.is_finite() || self.endpoint_clearance_meters < 0.0 {
            return Err(VisibilityError::InvalidClearance);
        }
        Ok(self)
    }
}

/// One sampled ray through the protected target aperture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCorridorSample {
    /// Corridor identifier.
    pub corridor_id: u64,
    /// Open or blocked state.
    pub state: VisibilityState,
    /// Sample point on the target aperture.
    pub aperture_point: Vec3,
    /// Unit origin-to-aperture direction.
    pub direction: Vec3,
    /// Distance to the aperture sample.
    pub aperture_distance_meters: f64,
    /// Distance to the blocker or aperture when open.
    pub first_hit_distance_meters: f64,
    /// First blocking source identity.
    pub blocker_object_id: ObjectId,
    /// First blocking instance identity.
    pub blocker_instance_id: InstanceId,
    /// First blocking mesh identity.
    pub blocker_mesh_id: MeshId,
    /// First blocking triangle identity.
    pub blocker_triangle_id: u32,
}

/// Aggregate protected-corridor visibility and conflict attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCorridorSummary {
    /// Corridor identifier.
    pub corridor_id: u64,
    /// Aperture solid angle in steradians for a perpendicular circular disk.
    pub aperture_solid_angle_steradians: f64,
    /// Number of unblocked aperture samples.
    pub open_sample_count: usize,
    /// Number of blocked aperture samples.
    pub blocked_sample_count: usize,
    /// Uniform-area open aperture fraction.
    pub open_fraction: f64,
    /// Estimated unblocked solid angle.
    pub open_solid_angle_steradians: f64,
    /// Full-vs-interleaved-half open-fraction delta.
    pub convergence_delta: f64,
    /// Dominant corridor-conflict object.
    pub dominant_blocker_object_id: ObjectId,
    /// Dominant object's fraction of all aperture samples.
    pub dominant_blocker_fraction: f64,
    /// Nearest blocker distance; infinity when fully open.
    pub nearest_blocker_distance_meters: f64,
}

/// Immutable corridor-major aperture samples and summaries.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewCorridorResult {
    /// One summary per corridor.
    pub summaries: Vec<ViewCorridorSummary>,
    /// Corridor-major samples.
    pub samples: Vec<ViewCorridorSample>,
    /// Samples per corridor.
    pub samples_per_corridor: usize,
    /// Stable scene/input/result identity.
    pub content_hash: [u8; 32],
    /// Backend, adapter, batching, transfer, precision, and fallback provenance.
    pub execution: RayQueryExecutionSummary,
}

/// Tests protected view corridors by tracing a deterministic disk aperture.
#[allow(clippy::too_many_lines)]
pub fn analyze_view_corridors(
    scene: &Scene,
    corridors: &[ViewCorridor],
    options: ViewCorridorOptions,
) -> Result<ViewCorridorResult, VisibilityError> {
    analyze_view_corridors_with_executor(scene, corridors, options)
}

/// Tests protected view corridors through a backend-neutral ray executor.
#[allow(clippy::too_many_lines)]
pub fn analyze_view_corridors_with_executor<E: RayQueryExecutor + ?Sized>(
    executor: &E,
    corridors: &[ViewCorridor],
    options: ViewCorridorOptions,
) -> Result<ViewCorridorResult, VisibilityError> {
    if corridors.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let scene = executor.canonical_scene();
    let mut execution = RayQueryExecutionSummary::from_context(executor.context());
    let options = options.validate()?;
    corridors
        .len()
        .checked_mul(options.sample_count)
        .filter(|count| *count <= crate::MAXIMUM_MATRIX_ENTRIES)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let mut samples = Vec::with_capacity(corridors.len() * options.sample_count);
    let mut summaries = Vec::with_capacity(corridors.len());

    for corridor in corridors {
        let axis_delta = corridor.target - corridor.origin;
        let axis_distance = axis_delta.length_squared().sqrt();
        let axis = axis_delta * axis_distance.recip();
        let right = axis
            .cross(corridor.up)
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        let aperture_points = (0..options.sample_count)
            .map(|index| {
                if index == 0 || corridor.target_radius_meters == 0.0 {
                    corridor.target
                } else {
                    let denominator = f64::from(
                        u32::try_from(options.sample_count.saturating_sub(1).max(1))
                            .expect("corridor sample cap fits u32"),
                    );
                    let radius = corridor.target_radius_meters
                        * (f64::from(u32::try_from(index).expect("sample cap fits u32"))
                            / denominator)
                            .sqrt();
                    let angle = f64::from(u32::try_from(index).expect("sample cap fits u32"))
                        * GOLDEN_ANGLE;
                    corridor.target
                        + right * (radius * angle.cos())
                        + corridor.up * (radius * angle.sin())
                }
            })
            .collect::<Vec<_>>();
        let rays = aperture_points
            .iter()
            .map(|point| {
                let delta = *point - corridor.origin;
                let distance = delta.length_squared().sqrt();
                let direction = delta * distance.recip();
                QueryRay::try_new(
                    corridor.origin,
                    direction,
                    options.endpoint_clearance_meters.min(distance * 0.49),
                    distance - options.endpoint_clearance_meters.min(distance * 0.49),
                    options.category_mask,
                )
                .map_err(|_| VisibilityError::InvalidQuery)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch = executor
            .trace_closest(&rays)
            .map_err(|_| VisibilityError::ExecutionFailed)?;
        execution.record(&batch.stats);
        let start = samples.len();
        samples.extend(
            aperture_points
                .into_iter()
                .zip(rays.iter())
                .zip(batch.hits)
                .map(|((aperture_point, ray), hit)| ViewCorridorSample {
                    corridor_id: corridor.corridor_id,
                    state: if hit.hit {
                        VisibilityState::Blocked
                    } else {
                        VisibilityState::Visible
                    },
                    aperture_point,
                    direction: ray.direction,
                    aperture_distance_meters: ray.t_max
                        + options
                            .endpoint_clearance_meters
                            .min((ray.t_max + ray.t_min) * 0.49),
                    first_hit_distance_meters: if hit.hit { hit.distance } else { ray.t_max },
                    blocker_object_id: hit.object_id,
                    blocker_instance_id: hit.instance_id,
                    blocker_mesh_id: hit.mesh_id,
                    blocker_triangle_id: hit.triangle_id,
                }),
        );
        let corridor_samples = &samples[start..];
        let open = corridor_samples
            .iter()
            .filter(|sample| sample.state == VisibilityState::Visible)
            .count();
        let half_open = corridor_samples
            .iter()
            .step_by(2)
            .filter(|sample| sample.state == VisibilityState::Visible)
            .count();
        let full_fraction = ratio(open, options.sample_count);
        let half_fraction = ratio(half_open, options.sample_count.div_ceil(2));
        let mut blockers = BTreeMap::<ObjectId, usize>::new();
        for sample in corridor_samples
            .iter()
            .filter(|sample| sample.state == VisibilityState::Blocked)
        {
            *blockers.entry(sample.blocker_object_id).or_default() += 1;
        }
        let (dominant_blocker_object_id, dominant_count) = blockers
            .into_iter()
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
            .unwrap_or((ObjectId::new(0), 0));
        let aperture_solid_angle = if corridor.target_radius_meters == 0.0 {
            0.0
        } else {
            2.0 * PI
                * (1.0
                    - axis_distance
                        / axis_distance
                            .mul_add(axis_distance, corridor.target_radius_meters.powi(2))
                            .sqrt())
        };
        summaries.push(ViewCorridorSummary {
            corridor_id: corridor.corridor_id,
            aperture_solid_angle_steradians: aperture_solid_angle,
            open_sample_count: open,
            blocked_sample_count: options.sample_count - open,
            open_fraction: full_fraction,
            open_solid_angle_steradians: aperture_solid_angle * full_fraction,
            convergence_delta: (full_fraction - half_fraction).abs(),
            dominant_blocker_object_id,
            dominant_blocker_fraction: ratio(dominant_count, options.sample_count),
            nearest_blocker_distance_meters: corridor_samples
                .iter()
                .filter(|sample| sample.state == VisibilityState::Blocked)
                .map(|sample| sample.first_hit_distance_meters)
                .fold(f64::INFINITY, f64::min),
        });
    }
    let content_hash = corridor_hash(scene, corridors, options, &samples);
    Ok(ViewCorridorResult {
        summaries,
        samples,
        samples_per_corridor: options.sample_count,
        content_hash,
        execution,
    })
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        f64::from(u32::try_from(numerator).expect("sample cap fits u32"))
            / f64::from(u32::try_from(denominator).expect("sample cap fits u32"))
    }
}

fn corridor_hash(
    scene: &Scene,
    corridors: &[ViewCorridor],
    options: ViewCorridorOptions,
    samples: &[ViewCorridorSample],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_VIEW_CORRIDOR_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(
        &u64::try_from(options.sample_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&options.endpoint_clearance_meters.to_bits().to_le_bytes());
    hasher.update(&options.category_mask.to_le_bytes());
    for corridor in corridors {
        hasher.update(&corridor.corridor_id.to_le_bytes());
        for point in [corridor.origin, corridor.target, corridor.up] {
            for value in [point.x, point.y, point.z] {
                hasher.update(&value.to_bits().to_le_bytes());
            }
        }
        hasher.update(&corridor.target_radius_meters.to_bits().to_le_bytes());
    }
    for sample in samples {
        hasher.update(&[sample.state as u8]);
        hasher.update(&sample.blocker_object_id.get().to_le_bytes());
        hasher.update(&sample.first_hit_distance_meters.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};

    fn scene_with_panel() -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).unwrap();
        let mesh = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(2.0, -0.5, -0.5),
                    Vec3::new(2.0, 0.5, -0.5),
                    Vec3::new(2.0, 0.5, 0.5),
                    Vec3::new(2.0, -0.5, 0.5),
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            })
            .unwrap();
        builder
            .add_instance(
                mesh,
                Transform::IDENTITY,
                ObjectId::new(42),
                InstanceId::new(1),
                1,
            )
            .unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn corridor_detects_partial_aperture_conflict_and_blocker() {
        let scene = scene_with_panel();
        let corridor = ViewCorridor::try_new(
            9,
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.5,
        )
        .unwrap();
        let result = analyze_view_corridors(
            &scene,
            &[corridor],
            ViewCorridorOptions {
                sample_count: 2_048,
                ..ViewCorridorOptions::default()
            },
        )
        .unwrap();
        let summary = result.summaries[0];
        assert!(summary.open_fraction > 0.0 && summary.open_fraction < 1.0);
        assert_eq!(summary.dominant_blocker_object_id, ObjectId::new(42));
        assert!(summary.aperture_solid_angle_steradians > 0.0);
    }
}
