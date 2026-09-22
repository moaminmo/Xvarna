//! Dynamic observer-path sampling built on the solid-angle target-view engine.

use crate::{
    TargetViewOptions, ViewObserver, ViewTargetPatch, VisibilityError,
    analyze_target_view_with_executor,
};
use xvarna_geometry::Vec3;
use xvarna_scene::{RayQueryExecutionSummary, RayQueryExecutor, Scene};
use xvarna_types::{SensorId, TargetId};

const MAXIMUM_PATH_POINTS: usize = 1_000_000;

/// A polyline followed by a tangent-facing dynamic observer.
#[derive(Clone, Debug, PartialEq)]
pub struct ObserverPath {
    /// Stable path identifier.
    pub path_id: u64,
    /// Ordered finite polyline vertices in canonical metres.
    pub vertices: Vec<Vec3>,
    /// Camera up direction, re-orthogonalized at every sample.
    pub up: Vec3,
    /// Observer importance in the inclusive range zero through one.
    pub weight: f64,
}

impl ObserverPath {
    /// Creates a finite path with at least two distinct consecutive vertices.
    pub fn try_new(
        path_id: u64,
        vertices: Vec<Vec3>,
        up: Vec3,
        weight: f64,
    ) -> Result<Self, VisibilityError> {
        if vertices.len() < 2
            || vertices.len() > MAXIMUM_PATH_POINTS
            || vertices.iter().any(|point| !point.is_finite())
            || !up.is_finite()
            || up.length_squared() <= 1.0e-24
            || !weight.is_finite()
            || !(0.0..=1.0).contains(&weight)
            || !vertices
                .windows(2)
                .any(|pair| (pair[1] - pair[0]).length_squared() > 1.0e-24)
        {
            return Err(VisibilityError::InvalidGeometry);
        }
        Ok(Self {
            path_id,
            vertices,
            up,
            weight,
        })
    }
}

/// Dynamic path sampling and target-view policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPathOptions {
    /// Uniform maximum distance between path samples.
    pub spacing_meters: f64,
    /// Target/Weighted/Green View policy evaluated at every sample.
    pub view: TargetViewOptions,
}

impl Default for ObserverPathOptions {
    fn default() -> Self {
        Self {
            spacing_meters: 1.0,
            view: TargetViewOptions::default(),
        }
    }
}

/// One time/distance-ordered observer sample and its principal view metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPathSample {
    /// Source path identifier.
    pub path_id: u64,
    /// Zero-based sample index within the path.
    pub sample_index: usize,
    /// Accumulated path distance in canonical metres.
    pub distance_along_path_meters: f64,
    /// Sample position.
    pub position: Vec3,
    /// Tangent camera direction.
    pub forward: Vec3,
    /// Raw target share of camera FOV.
    pub target_view_fraction: f64,
    /// Weighted view score.
    pub weighted_view_score: f64,
    /// Green View Index.
    pub green_view_index: f64,
    /// Most valuable visible target at this sample.
    pub dominant_target_id: TargetId,
    /// Sampling convergence delta in steradians.
    pub convergence_delta_steradians: f64,
}

/// Aggregate dynamic-path quality and hotspot indices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPathSummary {
    /// Source path identifier.
    pub path_id: u64,
    /// Polyline length in canonical metres.
    pub path_length_meters: f64,
    /// Number of evaluated observer samples.
    pub sample_count: usize,
    /// Distance-weighted mean Target View.
    pub mean_target_view_fraction: f64,
    /// Distance-weighted mean Weighted View.
    pub mean_weighted_view_score: f64,
    /// Distance-weighted mean Green View Index.
    pub mean_green_view_index: f64,
    /// Smallest weighted score on the path.
    pub minimum_weighted_view_score: f64,
    /// Largest weighted score on the path.
    pub maximum_weighted_view_score: f64,
    /// Index of the smallest weighted score.
    pub worst_sample_index: usize,
    /// Index of the largest weighted score.
    pub best_sample_index: usize,
}

/// Immutable path-major dynamic observer analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct ObserverPathResult {
    /// One aggregate per source path.
    pub summaries: Vec<ObserverPathSummary>,
    /// Path-major ordered samples.
    pub samples: Vec<ObserverPathSample>,
    /// Stable scene/input/result identity.
    pub content_hash: [u8; 32],
    /// Backend, adapter, batching, transfer, precision, and fallback provenance.
    pub execution: RayQueryExecutionSummary,
}

/// Samples tangent-facing observer paths and evaluates Target/Weighted/Green View.
#[allow(clippy::too_many_lines)]
pub fn analyze_observer_paths(
    scene: &Scene,
    paths: &[ObserverPath],
    patches: &[ViewTargetPatch],
    options: ObserverPathOptions,
) -> Result<ObserverPathResult, VisibilityError> {
    analyze_observer_paths_with_executor(scene, paths, patches, options)
}

/// Evaluates dynamic observer paths through a backend-neutral ray executor.
#[allow(clippy::too_many_lines)]
pub fn analyze_observer_paths_with_executor<E: RayQueryExecutor + ?Sized>(
    executor: &E,
    paths: &[ObserverPath],
    patches: &[ViewTargetPatch],
    options: ObserverPathOptions,
) -> Result<ObserverPathResult, VisibilityError> {
    if paths.is_empty() || patches.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    if !options.spacing_meters.is_finite() || options.spacing_meters <= 0.0 {
        return Err(VisibilityError::InvalidSamplingPolicy);
    }
    let mut all_samples = Vec::new();
    let mut summaries = Vec::with_capacity(paths.len());
    let mut result_hashes = Vec::with_capacity(paths.len());
    let mut execution = RayQueryExecutionSummary::from_context(executor.context());
    for path in paths {
        let (sampled, length) = sample_polyline(&path.vertices, options.spacing_meters)?;
        all_samples
            .len()
            .checked_add(sampled.len())
            .filter(|count| *count <= MAXIMUM_PATH_POINTS)
            .ok_or(VisibilityError::ResultTooLarge)?;
        let observers = sampled
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                ViewObserver::try_new(
                    SensorId::new(
                        path.path_id
                            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            .wrapping_add(u64::try_from(index).unwrap_or(u64::MAX))
                            .wrapping_add(1),
                    ),
                    sample.position,
                    sample.forward,
                    path.up,
                    path.weight,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let view_result =
            analyze_target_view_with_executor(executor, &observers, patches, options.view)?;
        execution.merge(&view_result.execution);
        result_hashes.push(view_result.content_hash);
        let start = all_samples.len();
        all_samples.extend(
            sampled
                .iter()
                .zip(view_result.summaries.iter())
                .enumerate()
                .map(|(index, (sample, view))| ObserverPathSample {
                    path_id: path.path_id,
                    sample_index: index,
                    distance_along_path_meters: sample.distance,
                    position: sample.position,
                    forward: sample.forward,
                    target_view_fraction: view.target_view_fraction,
                    weighted_view_score: view.weighted_view_score,
                    green_view_index: view.green_view_index,
                    dominant_target_id: view.dominant_target_id,
                    convergence_delta_steradians: view.convergence_delta_steradians,
                }),
        );
        let values = &all_samples[start..];
        let weights = path_sample_weights(values, length);
        let weighted_mean = |extract: fn(&ObserverPathSample) -> f64| {
            if length <= f64::EPSILON {
                values.first().map_or(0.0, extract)
            } else {
                values
                    .iter()
                    .zip(&weights)
                    .map(|(sample, weight)| extract(sample) * weight)
                    .sum::<f64>()
                    / weights.iter().sum::<f64>()
            }
        };
        let (worst_sample_index, minimum_weighted_view_score) = values
            .iter()
            .enumerate()
            .min_by(|left, right| {
                left.1
                    .weighted_view_score
                    .total_cmp(&right.1.weighted_view_score)
            })
            .map_or((0, 0.0), |(index, sample)| {
                (index, sample.weighted_view_score)
            });
        let (best_sample_index, maximum_weighted_view_score) = values
            .iter()
            .enumerate()
            .max_by(|left, right| {
                left.1
                    .weighted_view_score
                    .total_cmp(&right.1.weighted_view_score)
            })
            .map_or((0, 0.0), |(index, sample)| {
                (index, sample.weighted_view_score)
            });
        summaries.push(ObserverPathSummary {
            path_id: path.path_id,
            path_length_meters: length,
            sample_count: values.len(),
            mean_target_view_fraction: weighted_mean(|sample| sample.target_view_fraction),
            mean_weighted_view_score: weighted_mean(|sample| sample.weighted_view_score),
            mean_green_view_index: weighted_mean(|sample| sample.green_view_index),
            minimum_weighted_view_score,
            maximum_weighted_view_score,
            worst_sample_index,
            best_sample_index,
        });
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_OBSERVER_PATH_V1\0");
    hasher.update(&executor.canonical_scene().stats().content_hash);
    hasher.update(&options.spacing_meters.to_bits().to_le_bytes());
    for hash in result_hashes {
        hasher.update(&hash);
    }
    for sample in &all_samples {
        hasher.update(&sample.path_id.to_le_bytes());
        hasher.update(&sample.distance_along_path_meters.to_bits().to_le_bytes());
        hasher.update(&sample.weighted_view_score.to_bits().to_le_bytes());
    }
    Ok(ObserverPathResult {
        summaries,
        samples: all_samples,
        content_hash: *hasher.finalize().as_bytes(),
        execution,
    })
}

#[derive(Clone, Copy)]
struct SampledPoint {
    position: Vec3,
    forward: Vec3,
    distance: f64,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn sample_polyline(
    vertices: &[Vec3],
    spacing: f64,
) -> Result<(Vec<SampledPoint>, f64), VisibilityError> {
    let segments = vertices
        .windows(2)
        .filter_map(|pair| {
            let delta = pair[1] - pair[0];
            let length = delta.length_squared().sqrt();
            (length > 1.0e-12).then_some((pair[0], pair[1], delta * length.recip(), length))
        })
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(VisibilityError::InvalidGeometry);
    }
    let total_length: f64 = segments.iter().map(|segment| segment.3).sum();
    let intervals = (total_length / spacing).ceil() as usize;
    let sample_count = intervals.saturating_add(1).max(2);
    if sample_count > MAXIMUM_PATH_POINTS {
        return Err(VisibilityError::ResultTooLarge);
    }
    let mut output = Vec::with_capacity(sample_count);
    let mut segment_index = 0;
    let mut segment_start_distance = 0.0;
    for index in 0..sample_count {
        let distance = if index + 1 == sample_count {
            total_length
        } else {
            f64::from(u32::try_from(index).map_err(|_| VisibilityError::ResultTooLarge)?)
                * total_length
                / f64::from(
                    u32::try_from(sample_count - 1).map_err(|_| VisibilityError::ResultTooLarge)?,
                )
        };
        while segment_index + 1 < segments.len()
            && distance > segment_start_distance + segments[segment_index].3
        {
            segment_start_distance += segments[segment_index].3;
            segment_index += 1;
        }
        let segment = segments[segment_index];
        let local = ((distance - segment_start_distance) / segment.3).clamp(0.0, 1.0);
        output.push(SampledPoint {
            position: segment.0 + (segment.1 - segment.0) * local,
            forward: segment.2,
            distance,
        });
    }
    Ok((output, total_length))
}

fn path_sample_weights(samples: &[ObserverPathSample], length: f64) -> Vec<f64> {
    if samples.len() == 1 || length <= f64::EPSILON {
        return vec![1.0; samples.len()];
    }
    (0..samples.len())
        .map(|index| {
            let previous = if index == 0 {
                samples[index].distance_along_path_meters
            } else {
                samples[index - 1].distance_along_path_meters
            };
            let next = if index + 1 == samples.len() {
                samples[index].distance_along_path_meters
            } else {
                samples[index + 1].distance_along_path_meters
            };
            if index == 0 {
                (next - samples[index].distance_along_path_meters) * 0.5
            } else if index + 1 == samples.len() {
                (samples[index].distance_along_path_meters - previous) * 0.5
            } else {
                (next - previous) * 0.5
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::{InstanceId, ObjectId};

    fn distant_scene() -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).unwrap();
        let mesh = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(-100.0, -100.0, -100.0),
                    Vec3::new(-100.0, 100.0, -100.0),
                    Vec3::new(-100.0, 0.0, 100.0),
                ],
                triangles: vec![[0, 1, 2]],
            })
            .unwrap();
        builder
            .add_instance(
                mesh,
                Transform::IDENTITY,
                ObjectId::new(99),
                InstanceId::new(1),
                1,
            )
            .unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn dynamic_path_samples_endpoints_and_reports_distance_weighted_metrics() {
        let scene = distant_scene();
        let path = ObserverPath::try_new(
            5,
            vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
        )
        .unwrap();
        let target = ViewTargetPatch::try_new(
            TargetId::new(7),
            [
                Vec3::new(15.0, -1.0, -1.0),
                Vec3::new(15.0, 0.0, 1.0),
                Vec3::new(15.0, 1.0, -1.0),
            ],
            2,
            1.0,
        )
        .unwrap();
        let result = analyze_observer_paths(
            &scene,
            &[path],
            &[target],
            ObserverPathOptions {
                spacing_meters: 2.0,
                view: TargetViewOptions {
                    green_category_mask: 2,
                    two_sided_targets: true,
                    ..TargetViewOptions::default()
                },
            },
        )
        .unwrap();
        assert_eq!(result.samples.len(), 6);
        assert!((result.summaries[0].path_length_meters - 10.0).abs() < 1.0e-12);
        assert!(result.summaries[0].mean_green_view_index > 0.0);
        assert!(
            result.summaries[0].maximum_weighted_view_score
                > result.summaries[0].minimum_weighted_view_score
        );
    }
}
