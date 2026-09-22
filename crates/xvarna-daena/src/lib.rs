//! DAENA deterministic spatial-visibility intelligence.
//!
//! The crate contains independently usable isovist, intervisibility/privacy,
//! visibility-graph, target-view, view-corridor, and observer-path analyses.

#![forbid(unsafe_code)]

use core::{f64::consts::TAU, fmt};
use std::collections::{BTreeMap, HashMap, VecDeque};
use xvarna_geometry::Vec3;
use xvarna_scene::{QueryRay, Scene};
use xvarna_types::{InstanceId, MeshId, ObjectId, SensorId, TargetId};

mod corridor;
mod intelligence;
mod path;
mod target;

pub use corridor::*;
pub use intelligence::*;
pub use path::*;
pub use target::*;

const MINIMUM_ISOVIST_SAMPLES: usize = 32;
const MAXIMUM_ISOVIST_SAMPLES: usize = 65_536;
const MAXIMUM_MATRIX_ENTRIES: usize = 16_000_000;
const MAXIMUM_GRAPH_NODES: usize = 4_096;
const MAXIMUM_SPARSE_GRAPH_NODES: usize = 100_000;

/// A validated eye position and oriented analysis plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewpoint {
    /// Stable viewpoint identifier.
    pub id: SensorId,
    /// Eye position in canonical metres.
    pub position: Vec3,
    /// Unit normal of the isovist plane.
    pub plane_normal: Vec3,
    /// Unit forward direction projected onto the plane.
    pub forward: Vec3,
}

impl Viewpoint {
    /// Validates a viewpoint and orthonormalizes its frame.
    pub fn try_new(
        id: SensorId,
        position: Vec3,
        plane_normal: Vec3,
        forward: Vec3,
    ) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        let plane_normal = plane_normal
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        let projected = forward - plane_normal * forward.dot(plane_normal);
        let forward = projected
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        Ok(Self {
            id,
            position,
            plane_normal,
            forward,
        })
    }
}

/// Sampling and scene-query policy for a planar isovist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsovistOptions {
    /// Angular ray count per viewpoint.
    pub sample_count: usize,
    /// Field of view in radians, greater than zero and at most a full turn.
    pub field_of_view_radians: f64,
    /// Finite radial clipping distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Offset along the plane normal to avoid coplanar self-intersections.
    pub eye_offset_meters: f64,
    /// Included scene categories; zero and all-bits both mean all categories.
    pub category_mask: u64,
}

impl Default for IsovistOptions {
    fn default() -> Self {
        Self {
            sample_count: 720,
            field_of_view_radians: TAU,
            maximum_distance_meters: 100.0,
            eye_offset_meters: 1.0e-4,
            category_mask: u64::MAX,
        }
    }
}

impl IsovistOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !(MINIMUM_ISOVIST_SAMPLES..=MAXIMUM_ISOVIST_SAMPLES).contains(&self.sample_count) {
            return Err(VisibilityError::InvalidSampleCount);
        }
        if !self.field_of_view_radians.is_finite()
            || self.field_of_view_radians <= 0.0
            || self.field_of_view_radians > TAU
        {
            return Err(VisibilityError::InvalidFieldOfView);
        }
        if !self.maximum_distance_meters.is_finite() || self.maximum_distance_meters <= 0.0 {
            return Err(VisibilityError::InvalidMaximumDistance);
        }
        if !self.eye_offset_meters.is_finite() || self.eye_offset_meters < 0.0 {
            return Err(VisibilityError::InvalidClearance);
        }
        Ok(self)
    }
}

/// State of one sampled isovist direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum IsovistRayState {
    /// Scene geometry was hit before the radial limit.
    Occluded = 0,
    /// No geometry was hit before the configured radial limit.
    OpenAtLimit = 1,
}

/// One ordered boundary sample with first-hit attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsovistRay {
    /// World-space unit direction.
    pub direction: Vec3,
    /// Sampled boundary point.
    pub endpoint: Vec3,
    /// Eye-to-boundary distance in canonical metres.
    pub distance_meters: f64,
    /// Occluded or open-at-limit state.
    pub state: IsovistRayState,
    /// First blocking object, or zero for an open ray.
    pub object_id: ObjectId,
    /// First blocking occurrence, or zero for an open ray.
    pub instance_id: InstanceId,
    /// First blocking mesh resource, or zero for an open ray.
    pub mesh_id: MeshId,
    /// First blocking triangle, or `u32::MAX` for an open ray.
    pub triangle_id: u32,
}

/// Spatial, radial, convergence, and occluder metrics for one isovist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsovistSummary {
    /// Stable viewpoint identifier.
    pub viewpoint_id: SensorId,
    /// Sampled polygon area in square canonical metres.
    pub area_square_meters: f64,
    /// Sampled polygon perimeter in canonical metres.
    pub perimeter_meters: f64,
    /// Distance from eye to sampled polygon centroid.
    pub centroid_distance_meters: f64,
    /// Mean radial length.
    pub mean_radial_meters: f64,
    /// Minimum radial length.
    pub minimum_radial_meters: f64,
    /// Maximum radial length.
    pub maximum_radial_meters: f64,
    /// Population standard deviation of radial length.
    pub radial_standard_deviation_meters: f64,
    /// Dimensionless radial skewness; zero for a constant-radius field.
    pub radial_skewness: f64,
    /// `4*pi*area/perimeter^2`, clamped to zero for a degenerate polygon.
    pub compactness: f64,
    /// Absolute area delta between all samples and the interleaved half sample set.
    pub area_convergence_delta_square_meters: f64,
    /// Occluded-ray count.
    pub occluded_count: usize,
    /// Open-at-limit ray count.
    pub open_count: usize,
    /// Object responsible for the largest angular share of occluded rays.
    pub dominant_occluder_object_id: ObjectId,
    /// Dominant object's share of all directions.
    pub dominant_occluder_fraction: f64,
}

/// Complete viewpoint-major isovist result.
#[derive(Clone, Debug, PartialEq)]
pub struct IsovistResult {
    /// One aggregate per viewpoint.
    pub summaries: Vec<IsovistSummary>,
    /// Viewpoint-major ordered boundary rays.
    pub rays: Vec<IsovistRay>,
    /// Rays per viewpoint.
    pub sample_count: usize,
    /// Stable identity of the scene, inputs, policy, and hits.
    pub content_hash: [u8; 32],
}

/// Computes deterministic sampled planar isovists for one or more viewpoints.
pub fn analyze_isovists(
    scene: &Scene,
    viewpoints: &[Viewpoint],
    options: IsovistOptions,
) -> Result<IsovistResult, VisibilityError> {
    if viewpoints.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let options = options.validate()?;
    let total = viewpoints
        .len()
        .checked_mul(options.sample_count)
        .filter(|count| *count <= MAXIMUM_MATRIX_ENTRIES)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let mut rays = Vec::with_capacity(total);
    let mut summaries = Vec::with_capacity(viewpoints.len());
    for viewpoint in viewpoints {
        let right = viewpoint
            .forward
            .cross(viewpoint.plane_normal)
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        let directions = sample_planar_directions(*viewpoint, right, options);
        let origin = viewpoint.position + viewpoint.plane_normal * options.eye_offset_meters;
        let queries = directions
            .iter()
            .map(|direction| {
                QueryRay::try_new(
                    origin,
                    *direction,
                    0.0,
                    options.maximum_distance_meters,
                    options.category_mask,
                )
                .map_err(|_| VisibilityError::InvalidQuery)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let start = rays.len();
        rays.extend(
            directions
                .into_iter()
                .zip(scene.trace_closest_batch(&queries))
                .map(|(direction, hit)| {
                    let distance = if hit.hit {
                        hit.distance
                    } else {
                        options.maximum_distance_meters
                    };
                    IsovistRay {
                        direction,
                        endpoint: viewpoint.position + direction * distance,
                        distance_meters: distance,
                        state: if hit.hit {
                            IsovistRayState::Occluded
                        } else {
                            IsovistRayState::OpenAtLimit
                        },
                        object_id: hit.object_id,
                        instance_id: hit.instance_id,
                        mesh_id: hit.mesh_id,
                        triangle_id: hit.triangle_id,
                    }
                }),
        );
        summaries.push(summarize_isovist(
            *viewpoint,
            &rays[start..],
            options.field_of_view_radians,
        ));
    }
    let content_hash = isovist_hash(scene, viewpoints, options, &rays);
    Ok(IsovistResult {
        summaries,
        rays,
        sample_count: options.sample_count,
        content_hash,
    })
}

fn sample_planar_directions(
    viewpoint: Viewpoint,
    right: Vec3,
    options: IsovistOptions,
) -> Vec<Vec3> {
    let full_circle = (options.field_of_view_radians - TAU).abs() <= 1.0e-12;
    let denominator = if full_circle {
        usize_to_f64(options.sample_count)
    } else {
        usize_to_f64(options.sample_count - 1)
    };
    (0..options.sample_count)
        .map(|index| {
            let fraction = usize_to_f64(index) / denominator;
            let angle = if full_circle {
                fraction * TAU
            } else {
                (-options.field_of_view_radians)
                    .mul_add(0.5, fraction * options.field_of_view_radians)
            };
            (viewpoint.forward * angle.cos() + right * angle.sin())
                .normalized()
                .expect("orthonormal frame preserves unit length")
        })
        .collect()
}

fn summarize_isovist(
    viewpoint: Viewpoint,
    rays: &[IsovistRay],
    field_of_view: f64,
) -> IsovistSummary {
    let full_circle = (field_of_view - TAU).abs() <= 1.0e-12;
    let points = rays.iter().map(|ray| ray.endpoint).collect::<Vec<_>>();
    let (area, perimeter, centroid) = planar_polygon_metrics(
        viewpoint.position,
        viewpoint.plane_normal,
        &points,
        full_circle,
    );
    let reduced = points.iter().step_by(2).copied().collect::<Vec<_>>();
    let (reduced_area, _, _) = planar_polygon_metrics(
        viewpoint.position,
        viewpoint.plane_normal,
        &reduced,
        full_circle,
    );
    let count = usize_to_f64(rays.len());
    let mean = rays.iter().map(|ray| ray.distance_meters).sum::<f64>() / count;
    let variance = rays
        .iter()
        .map(|ray| (ray.distance_meters - mean).powi(2))
        .sum::<f64>()
        / count;
    let standard_deviation = variance.sqrt();
    let skewness = if standard_deviation > 0.0 {
        rays.iter()
            .map(|ray| ((ray.distance_meters - mean) / standard_deviation).powi(3))
            .sum::<f64>()
            / count
    } else {
        0.0
    };
    let mut occluders = BTreeMap::<ObjectId, usize>::new();
    let mut occluded_count = 0;
    for ray in rays {
        if ray.state == IsovistRayState::Occluded {
            occluded_count += 1;
            *occluders.entry(ray.object_id).or_default() += 1;
        }
    }
    let (dominant, dominant_count) = occluders
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .unwrap_or((ObjectId::new(0), 0));
    IsovistSummary {
        viewpoint_id: viewpoint.id,
        area_square_meters: area,
        perimeter_meters: perimeter,
        centroid_distance_meters: (centroid - viewpoint.position).length_squared().sqrt(),
        mean_radial_meters: mean,
        minimum_radial_meters: rays
            .iter()
            .map(|ray| ray.distance_meters)
            .fold(f64::INFINITY, f64::min),
        maximum_radial_meters: rays
            .iter()
            .map(|ray| ray.distance_meters)
            .fold(0.0, f64::max),
        radial_standard_deviation_meters: standard_deviation,
        radial_skewness: skewness,
        compactness: if perimeter > 0.0 {
            (4.0 * core::f64::consts::PI * area / perimeter.powi(2)).clamp(0.0, 1.0)
        } else {
            0.0
        },
        area_convergence_delta_square_meters: (area - reduced_area).abs(),
        occluded_count,
        open_count: rays.len() - occluded_count,
        dominant_occluder_object_id: dominant,
        dominant_occluder_fraction: usize_to_f64(dominant_count) / count,
    }
}

fn planar_polygon_metrics(
    eye: Vec3,
    normal: Vec3,
    points: &[Vec3],
    full_circle: bool,
) -> (f64, f64, Vec3) {
    if points.len() < 2 {
        return (0.0, 0.0, eye);
    }
    let vertices = if full_circle {
        points.to_vec()
    } else {
        let mut values = Vec::with_capacity(points.len() + 1);
        values.push(eye);
        values.extend_from_slice(points);
        values
    };
    let mut signed_double_area = 0.0;
    let mut centroid_numerator = Vec3::ZERO;
    let mut perimeter = 0.0;
    for index in 0..vertices.len() {
        let first = vertices[index];
        let second = vertices[(index + 1) % vertices.len()];
        let cross = (first - eye).cross(second - eye).dot(normal);
        signed_double_area += cross;
        centroid_numerator = centroid_numerator + ((first - eye) + (second - eye)) * cross;
        perimeter += (second - first).length_squared().sqrt();
    }
    let area = signed_double_area.abs() * 0.5;
    let centroid = if signed_double_area.abs() > 1.0e-15 {
        eye + centroid_numerator * (1.0 / (3.0 * signed_double_area))
    } else {
        eye
    };
    (area, perimeter, centroid)
}

/// A weighted observer used by directed visibility analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityObserver {
    /// Stable observer identifier.
    pub id: SensorId,
    /// Eye position in canonical metres.
    pub position: Vec3,
    /// Importance in the inclusive range zero through one.
    pub weight: f64,
}

impl VisibilityObserver {
    /// Creates a validated observer.
    pub fn try_new(id: SensorId, position: Vec3, weight: f64) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            return Err(VisibilityError::InvalidWeight);
        }
        Ok(Self {
            id,
            position,
            weight,
        })
    }
}

/// A directionally sensitive privacy or view target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityTarget {
    /// Stable target identifier.
    pub id: TargetId,
    /// Target position in canonical metres.
    pub position: Vec3,
    /// Optional unit outward/front direction. `None` is omnidirectional.
    pub facing: Option<Vec3>,
    /// Privacy sensitivity in the inclusive range zero through one.
    pub sensitivity: f64,
}

impl VisibilityTarget {
    /// Creates a validated target and normalizes its optional facing direction.
    pub fn try_new(
        id: TargetId,
        position: Vec3,
        facing: Option<Vec3>,
        sensitivity: f64,
    ) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        if !sensitivity.is_finite() || !(0.0..=1.0).contains(&sensitivity) {
            return Err(VisibilityError::InvalidWeight);
        }
        let facing = facing
            .map(|value| value.normalized().ok_or(VisibilityError::InvalidDirection))
            .transpose()?;
        Ok(Self {
            id,
            position,
            facing,
            sensitivity,
        })
    }
}

/// Policy for directed intervisibility and privacy-risk calculation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervisibilityOptions {
    /// Clearance removed from both endpoints in canonical metres.
    pub endpoint_clearance_meters: f64,
    /// Maximum observer-target distance; positive infinity is supported.
    pub maximum_distance_meters: f64,
    /// Distance where the inverse-square-like privacy multiplier equals one half.
    pub privacy_reference_distance_meters: f64,
    /// Exponent applied to the directional facing cosine.
    pub facing_exponent: f64,
    /// Included scene categories.
    pub category_mask: u64,
}

impl Default for IntervisibilityOptions {
    fn default() -> Self {
        Self {
            endpoint_clearance_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            privacy_reference_distance_meters: 10.0,
            facing_exponent: 1.0,
            category_mask: u64::MAX,
        }
    }
}

impl IntervisibilityOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !self.endpoint_clearance_meters.is_finite() || self.endpoint_clearance_meters < 0.0 {
            return Err(VisibilityError::InvalidClearance);
        }
        if self.maximum_distance_meters.is_nan() || self.maximum_distance_meters <= 0.0 {
            return Err(VisibilityError::InvalidMaximumDistance);
        }
        if !self.privacy_reference_distance_meters.is_finite()
            || self.privacy_reference_distance_meters <= 0.0
        {
            return Err(VisibilityError::InvalidReferenceDistance);
        }
        if !self.facing_exponent.is_finite() || self.facing_exponent < 0.0 {
            return Err(VisibilityError::InvalidFacingExponent);
        }
        Ok(self)
    }
}

/// Classification of one observer-target pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VisibilityState {
    /// Direct segment is unobstructed.
    Visible = 0,
    /// Direct segment is blocked by scene geometry.
    Blocked = 1,
    /// Pair exceeds the configured analysis distance.
    OutOfRange = 2,
    /// Endpoints are coincident within numeric precision.
    Coincident = 3,
}

/// One row-major observer-target matrix entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervisibilityEntry {
    /// Pair state.
    pub state: VisibilityState,
    /// Endpoint distance in canonical metres.
    pub distance_meters: f64,
    /// Observer-to-target unit direction, or zero for a coincident pair.
    pub direction: Vec3,
    /// Transparent privacy risk in the inclusive range zero through one.
    pub privacy_risk: f64,
    /// Directional facing multiplier in the inclusive range zero through one.
    pub facing_factor: f64,
    /// Distance multiplier in the inclusive range zero through one.
    pub distance_factor: f64,
    /// First blocking object, or zero when no blocker exists.
    pub blocker_object_id: ObjectId,
    /// First blocking occurrence, or zero when no blocker exists.
    pub blocker_instance_id: InstanceId,
    /// First blocking mesh resource, or zero when no blocker exists.
    pub blocker_mesh_id: MeshId,
    /// First blocking triangle, or `u32::MAX` when no blocker exists.
    pub blocker_triangle_id: u32,
}

/// Aggregate for one observer matrix row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverVisibilitySummary {
    /// Stable observer identifier.
    pub observer_id: SensorId,
    /// Number of directly visible targets.
    pub visible_count: usize,
    /// Visible targets divided by all non-coincident targets.
    pub visible_fraction: f64,
    /// Sum of pair privacy risks.
    pub total_privacy_risk: f64,
    /// Largest single pair privacy risk.
    pub peak_privacy_risk: f64,
}

/// Aggregate for one target matrix column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetVisibilitySummary {
    /// Stable target identifier.
    pub target_id: TargetId,
    /// Number of observers with direct visibility.
    pub visible_observer_count: usize,
    /// Visible observers divided by all non-coincident observers.
    pub exposure_fraction: f64,
    /// Sum of pair privacy risks received by the target.
    pub cumulative_privacy_risk: f64,
    /// Complement-product probability proxy `1-product(1-risk_i)`.
    pub combined_privacy_risk: f64,
}

/// Complete directed intervisibility and privacy result.
#[derive(Clone, Debug, PartialEq)]
pub struct IntervisibilityResult {
    /// Row-major entries: `observer_index * target_count + target_index`.
    pub entries: Vec<IntervisibilityEntry>,
    /// One aggregate per observer.
    pub observer_summaries: Vec<ObserverVisibilitySummary>,
    /// One aggregate per target.
    pub target_summaries: Vec<TargetVisibilitySummary>,
    /// Number of target columns.
    pub target_count: usize,
    /// Stable identity of scene, endpoints, policy, states, and blockers.
    pub content_hash: [u8; 32],
}

/// Computes a directed visibility matrix and transparent privacy-risk model.
///
/// For visible pairs, `risk = observer_weight * target_sensitivity *
/// facing_factor * 1/(1 + (distance/reference_distance)^2)`.
pub fn analyze_intervisibility(
    scene: &Scene,
    observers: &[VisibilityObserver],
    targets: &[VisibilityTarget],
    options: IntervisibilityOptions,
) -> Result<IntervisibilityResult, VisibilityError> {
    if observers.is_empty() || targets.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let options = options.validate()?;
    observers
        .len()
        .checked_mul(targets.len())
        .filter(|count| *count <= MAXIMUM_MATRIX_ENTRIES)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let mut entries =
        vec![empty_entry(VisibilityState::OutOfRange, 0.0); observers.len() * targets.len()];
    let mut ray_indices = Vec::new();
    let mut rays = Vec::new();
    for (observer_index, observer) in observers.iter().enumerate() {
        for (target_index, target) in targets.iter().enumerate() {
            let index = observer_index * targets.len() + target_index;
            let delta = target.position - observer.position;
            let distance = delta.length_squared().sqrt();
            if distance <= f64::EPSILON {
                entries[index] = empty_entry(VisibilityState::Coincident, distance);
                continue;
            }
            let direction = delta * distance.recip();
            if distance > options.maximum_distance_meters {
                entries[index] = IntervisibilityEntry {
                    direction,
                    ..empty_entry(VisibilityState::OutOfRange, distance)
                };
                continue;
            }
            if distance <= options.endpoint_clearance_meters * 2.0 {
                entries[index] = visible_entry(*observer, *target, direction, distance, options);
                continue;
            }
            let query = QueryRay::try_new(
                observer.position,
                direction,
                options.endpoint_clearance_meters,
                distance - options.endpoint_clearance_meters,
                options.category_mask,
            )
            .map_err(|_| VisibilityError::InvalidQuery)?;
            entries[index] = IntervisibilityEntry {
                direction,
                ..empty_entry(VisibilityState::Blocked, distance)
            };
            ray_indices.push((index, *observer, *target, direction, distance));
            rays.push(query);
        }
    }
    for ((index, observer, target, direction, distance), hit) in ray_indices
        .into_iter()
        .zip(scene.trace_closest_batch(&rays))
    {
        entries[index] = if hit.hit {
            IntervisibilityEntry {
                blocker_object_id: hit.object_id,
                blocker_instance_id: hit.instance_id,
                blocker_mesh_id: hit.mesh_id,
                blocker_triangle_id: hit.triangle_id,
                direction,
                ..empty_entry(VisibilityState::Blocked, distance)
            }
        } else {
            visible_entry(observer, target, direction, distance, options)
        };
    }
    let observer_summaries = summarize_observers(observers, targets.len(), &entries);
    let target_summaries = summarize_targets(targets, observers.len(), &entries);
    let content_hash = intervisibility_hash(scene, observers, targets, options, &entries);
    Ok(IntervisibilityResult {
        entries,
        observer_summaries,
        target_summaries,
        target_count: targets.len(),
        content_hash,
    })
}

const fn empty_entry(state: VisibilityState, distance: f64) -> IntervisibilityEntry {
    IntervisibilityEntry {
        state,
        distance_meters: distance,
        direction: Vec3::ZERO,
        privacy_risk: 0.0,
        facing_factor: 0.0,
        distance_factor: 0.0,
        blocker_object_id: ObjectId::new(0),
        blocker_instance_id: InstanceId::new(0),
        blocker_mesh_id: MeshId::new(0),
        blocker_triangle_id: u32::MAX,
    }
}

fn visible_entry(
    observer: VisibilityObserver,
    target: VisibilityTarget,
    direction: Vec3,
    distance: f64,
    options: IntervisibilityOptions,
) -> IntervisibilityEntry {
    let target_to_observer = direction * -1.0;
    let facing_factor = target.facing.map_or(1.0, |facing| {
        facing
            .dot(target_to_observer)
            .max(0.0)
            .powf(options.facing_exponent)
    });
    let ratio = distance / options.privacy_reference_distance_meters;
    let distance_factor = 1.0 / ratio.mul_add(ratio, 1.0);
    IntervisibilityEntry {
        state: VisibilityState::Visible,
        distance_meters: distance,
        direction,
        privacy_risk: observer.weight * target.sensitivity * facing_factor * distance_factor,
        facing_factor,
        distance_factor,
        ..empty_entry(VisibilityState::Visible, distance)
    }
}

fn summarize_observers(
    observers: &[VisibilityObserver],
    target_count: usize,
    entries: &[IntervisibilityEntry],
) -> Vec<ObserverVisibilitySummary> {
    observers
        .iter()
        .enumerate()
        .map(|(index, observer)| {
            let row = &entries[index * target_count..(index + 1) * target_count];
            let eligible = row
                .iter()
                .filter(|entry| entry.state != VisibilityState::Coincident)
                .count();
            let visible = row
                .iter()
                .filter(|entry| entry.state == VisibilityState::Visible)
                .count();
            ObserverVisibilitySummary {
                observer_id: observer.id,
                visible_count: visible,
                visible_fraction: if eligible == 0 {
                    0.0
                } else {
                    usize_to_f64(visible) / usize_to_f64(eligible)
                },
                total_privacy_risk: row.iter().map(|entry| entry.privacy_risk).sum(),
                peak_privacy_risk: row
                    .iter()
                    .map(|entry| entry.privacy_risk)
                    .fold(0.0, f64::max),
            }
        })
        .collect()
}

fn summarize_targets(
    targets: &[VisibilityTarget],
    observer_count: usize,
    entries: &[IntervisibilityEntry],
) -> Vec<TargetVisibilitySummary> {
    targets
        .iter()
        .enumerate()
        .map(|(target_index, target)| {
            let column = (0..observer_count)
                .map(|observer_index| entries[observer_index * targets.len() + target_index])
                .collect::<Vec<_>>();
            let eligible = column
                .iter()
                .filter(|entry| entry.state != VisibilityState::Coincident)
                .count();
            let visible = column
                .iter()
                .filter(|entry| entry.state == VisibilityState::Visible)
                .count();
            let product_safe = column
                .iter()
                .fold(1.0, |product, entry| product * (1.0 - entry.privacy_risk));
            TargetVisibilitySummary {
                target_id: target.id,
                visible_observer_count: visible,
                exposure_fraction: if eligible == 0 {
                    0.0
                } else {
                    usize_to_f64(visible) / usize_to_f64(eligible)
                },
                cumulative_privacy_risk: column.iter().map(|entry| entry.privacy_risk).sum(),
                combined_privacy_risk: 1.0 - product_safe,
            }
        })
        .collect()
}

/// A point used as a node in an undirected visibility graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityNode {
    /// Stable node identifier.
    pub id: SensorId,
    /// Node position in canonical metres.
    pub position: Vec3,
}

impl VisibilityNode {
    /// Creates a finite node.
    pub const fn try_new(id: SensorId, position: Vec3) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        Ok(Self { id, position })
    }
}

/// Policy for an undirected all-pairs visibility graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityGraphOptions {
    /// Clearance removed from both line endpoints.
    pub endpoint_clearance_meters: f64,
    /// Maximum edge length; positive infinity is supported.
    pub maximum_distance_meters: f64,
    /// Included scene categories.
    pub category_mask: u64,
    /// Whether to compute harmonic closeness and Brandes betweenness.
    pub compute_centrality: bool,
}

/// Policy for a spatially indexed, degree-bounded visibility graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SparseVisibilityGraphOptions {
    /// Clearance removed from both line endpoints.
    pub endpoint_clearance_meters: f64,
    /// Finite spatial search radius.
    pub maximum_distance_meters: f64,
    /// Included scene categories.
    pub category_mask: u64,
    /// Maximum candidate degree retained before ray tracing.
    pub maximum_neighbors: usize,
    /// Whether to compute harmonic closeness and Brandes betweenness.
    pub compute_centrality: bool,
}

impl Default for SparseVisibilityGraphOptions {
    fn default() -> Self {
        Self {
            endpoint_clearance_meters: 1.0e-4,
            maximum_distance_meters: 25.0,
            category_mask: u64::MAX,
            maximum_neighbors: 16,
            compute_centrality: false,
        }
    }
}

impl Default for VisibilityGraphOptions {
    fn default() -> Self {
        Self {
            endpoint_clearance_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
            compute_centrality: true,
        }
    }
}

/// One unordered node pair with obstruction attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityGraphPair {
    /// Zero-based first node index.
    pub first_index: usize,
    /// Zero-based second node index.
    pub second_index: usize,
    /// Pair state.
    pub state: VisibilityState,
    /// Endpoint distance in canonical metres.
    pub distance_meters: f64,
    /// First blocking object, or zero for a visible/out-of-range pair.
    pub blocker_object_id: ObjectId,
}

/// Per-node graph topology metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityNodeMetrics {
    /// Stable node identifier.
    pub node_id: SensorId,
    /// Number of visible adjacent nodes.
    pub degree: usize,
    /// Degree divided by the maximum possible degree.
    pub degree_centrality: f64,
    /// Connected component label in deterministic discovery order.
    pub component_index: usize,
    /// Normalized harmonic closeness over graph hop distances.
    pub harmonic_closeness: f64,
    /// Normalized Brandes betweenness for an undirected graph.
    pub betweenness_centrality: f64,
}

/// Complete undirected visibility graph.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityGraphResult {
    /// One record for every unordered pair, including blockers and out-of-range pairs.
    pub pairs: Vec<VisibilityGraphPair>,
    /// Per-node topology metrics.
    pub metrics: Vec<VisibilityNodeMetrics>,
    /// Number of directly visible edges.
    pub visible_edge_count: usize,
    /// Number of connected components, including isolated nodes.
    pub connected_component_count: usize,
    /// Stable identity of scene, nodes, policy, and every pair state.
    pub content_hash: [u8; 32],
}

/// Builds an exact all-pairs line-of-sight graph and topology metrics.
#[allow(clippy::too_many_lines)]
pub fn analyze_visibility_graph(
    scene: &Scene,
    nodes: &[VisibilityNode],
    options: VisibilityGraphOptions,
) -> Result<VisibilityGraphResult, VisibilityError> {
    if nodes.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    if nodes.len() > MAXIMUM_GRAPH_NODES {
        return Err(VisibilityError::ResultTooLarge);
    }
    let query_options = IntervisibilityOptions {
        endpoint_clearance_meters: options.endpoint_clearance_meters,
        maximum_distance_meters: options.maximum_distance_meters,
        ..IntervisibilityOptions::default()
    }
    .validate()?;
    let pair_count = nodes
        .len()
        .checked_mul(nodes.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .filter(|count| *count <= MAXIMUM_MATRIX_ENTRIES)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let mut pairs = Vec::with_capacity(pair_count);
    let mut ray_pair_indices = Vec::new();
    let mut rays = Vec::new();
    for first in 0..nodes.len() {
        for second in first + 1..nodes.len() {
            let delta = nodes[second].position - nodes[first].position;
            let distance = delta.length_squared().sqrt();
            let state = if distance <= f64::EPSILON {
                VisibilityState::Coincident
            } else if distance > query_options.maximum_distance_meters {
                VisibilityState::OutOfRange
            } else {
                VisibilityState::Visible
            };
            let pair_index = pairs.len();
            pairs.push(VisibilityGraphPair {
                first_index: first,
                second_index: second,
                state,
                distance_meters: distance,
                blocker_object_id: ObjectId::new(0),
            });
            if state == VisibilityState::Visible
                && distance > query_options.endpoint_clearance_meters * 2.0
            {
                let direction = delta * distance.recip();
                rays.push(
                    QueryRay::try_new(
                        nodes[first].position,
                        direction,
                        query_options.endpoint_clearance_meters,
                        distance - query_options.endpoint_clearance_meters,
                        options.category_mask,
                    )
                    .map_err(|_| VisibilityError::InvalidQuery)?,
                );
                ray_pair_indices.push(pair_index);
            }
        }
    }
    for (pair_index, hit) in ray_pair_indices
        .into_iter()
        .zip(scene.trace_closest_batch(&rays))
    {
        if hit.hit {
            pairs[pair_index].state = VisibilityState::Blocked;
            pairs[pair_index].blocker_object_id = hit.object_id;
        }
    }
    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    for pair in &pairs {
        if pair.state == VisibilityState::Visible {
            adjacency[pair.first_index].push(pair.second_index);
            adjacency[pair.second_index].push(pair.first_index);
        }
    }
    let (components, component_count) = connected_components(&adjacency);
    let (harmonic, betweenness) = if options.compute_centrality {
        graph_centralities(&adjacency)
    } else {
        (vec![0.0; nodes.len()], vec![0.0; nodes.len()])
    };
    let metrics = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| VisibilityNodeMetrics {
            node_id: node.id,
            degree: adjacency[index].len(),
            degree_centrality: if nodes.len() <= 1 {
                0.0
            } else {
                usize_to_f64(adjacency[index].len()) / usize_to_f64(nodes.len() - 1)
            },
            component_index: components[index],
            harmonic_closeness: harmonic[index],
            betweenness_centrality: betweenness[index],
        })
        .collect::<Vec<_>>();
    let visible_edge_count = pairs
        .iter()
        .filter(|pair| pair.state == VisibilityState::Visible)
        .count();
    let content_hash = visibility_graph_hash(scene, nodes, options, &pairs);
    Ok(VisibilityGraphResult {
        pairs,
        metrics,
        visible_edge_count,
        connected_component_count: component_count,
        content_hash,
    })
}

/// Builds a deterministic radius graph without materializing the all-pairs matrix.
///
/// Nodes are assigned to a uniform three-dimensional grid whose cell size equals the search
/// radius. Each node examines only its 27 neighboring cells, proposes its nearest candidates,
/// and a global distance-ordered pass enforces the requested maximum degree at both endpoints.
/// The returned pair list therefore contains only traced candidate edges, not out-of-range pairs.
#[allow(clippy::too_many_lines)]
pub fn analyze_sparse_visibility_graph(
    scene: &Scene,
    nodes: &[VisibilityNode],
    options: SparseVisibilityGraphOptions,
) -> Result<VisibilityGraphResult, VisibilityError> {
    if nodes.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    if nodes.len() > MAXIMUM_SPARSE_GRAPH_NODES
        || options.maximum_neighbors == 0
        || options.maximum_neighbors > 1_024
        || !options.maximum_distance_meters.is_finite()
        || options.maximum_distance_meters <= 0.0
        || !options.endpoint_clearance_meters.is_finite()
        || options.endpoint_clearance_meters < 0.0
        || options.endpoint_clearance_meters * 2.0 >= options.maximum_distance_meters
        || (options.compute_centrality && nodes.len() > MAXIMUM_GRAPH_NODES)
    {
        return Err(VisibilityError::InvalidResourcePolicy);
    }
    let radius = options.maximum_distance_meters;
    let mut grid = HashMap::<(i64, i64, i64), Vec<usize>>::new();
    let mut cells = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let cell = spatial_cell(node.position, radius)?;
        grid.entry(cell).or_default().push(index);
        cells.push(cell);
    }
    let mut proposed = BTreeMap::<(usize, usize), f64>::new();
    for (first, &(cell_x, cell_y, cell_z)) in cells.iter().enumerate() {
        let mut local = Vec::<(f64, usize)>::new();
        for offset_x in -1_i64..=1 {
            for offset_y in -1_i64..=1 {
                for offset_z in -1_i64..=1 {
                    let Some(key) = cell_x
                        .checked_add(offset_x)
                        .zip(cell_y.checked_add(offset_y))
                        .zip(cell_z.checked_add(offset_z))
                        .map(|((x, y), z)| (x, y, z))
                    else {
                        continue;
                    };
                    let Some(bucket) = grid.get(&key) else {
                        continue;
                    };
                    for &second in bucket {
                        if first == second {
                            continue;
                        }
                        let distance = (nodes[second].position - nodes[first].position)
                            .length_squared()
                            .sqrt();
                        if distance <= radius {
                            local.push((distance, second));
                        }
                    }
                }
            }
        }
        local.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        local.truncate(options.maximum_neighbors);
        for (distance, second) in local {
            let key = if first < second {
                (first, second)
            } else {
                (second, first)
            };
            proposed.entry(key).or_insert(distance);
        }
    }
    let mut candidates = proposed
        .into_iter()
        .map(|((first, second), distance)| (distance, first, second))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let mut candidate_degree = vec![0_usize; nodes.len()];
    let mut pairs = Vec::with_capacity(nodes.len().saturating_mul(options.maximum_neighbors) / 2);
    let mut rays = Vec::new();
    let mut ray_pair_indices = Vec::new();
    for (distance, first, second) in candidates {
        if candidate_degree[first] >= options.maximum_neighbors
            || candidate_degree[second] >= options.maximum_neighbors
        {
            continue;
        }
        candidate_degree[first] += 1;
        candidate_degree[second] += 1;
        let state = if distance <= f64::EPSILON {
            VisibilityState::Coincident
        } else {
            VisibilityState::Visible
        };
        let pair_index = pairs.len();
        pairs.push(VisibilityGraphPair {
            first_index: first,
            second_index: second,
            state,
            distance_meters: distance,
            blocker_object_id: ObjectId::new(0),
        });
        if state == VisibilityState::Visible && distance > options.endpoint_clearance_meters * 2.0 {
            let direction = (nodes[second].position - nodes[first].position) * distance.recip();
            rays.push(
                QueryRay::try_new(
                    nodes[first].position,
                    direction,
                    options.endpoint_clearance_meters,
                    distance - options.endpoint_clearance_meters,
                    options.category_mask,
                )
                .map_err(|_| VisibilityError::InvalidQuery)?,
            );
            ray_pair_indices.push(pair_index);
        }
    }
    if pairs.len() > MAXIMUM_MATRIX_ENTRIES {
        return Err(VisibilityError::ResultTooLarge);
    }
    for (pair_index, hit) in ray_pair_indices
        .into_iter()
        .zip(scene.trace_closest_batch(&rays))
    {
        if hit.hit {
            pairs[pair_index].state = VisibilityState::Blocked;
            pairs[pair_index].blocker_object_id = hit.object_id;
        }
    }
    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    for pair in &pairs {
        if pair.state == VisibilityState::Visible {
            adjacency[pair.first_index].push(pair.second_index);
            adjacency[pair.second_index].push(pair.first_index);
        }
    }
    let (components, component_count) = connected_components(&adjacency);
    let (harmonic, betweenness) = if options.compute_centrality {
        graph_centralities(&adjacency)
    } else {
        (vec![0.0; nodes.len()], vec![0.0; nodes.len()])
    };
    let metrics = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| VisibilityNodeMetrics {
            node_id: node.id,
            degree: adjacency[index].len(),
            degree_centrality: if nodes.len() <= 1 {
                0.0
            } else {
                usize_to_f64(adjacency[index].len()) / usize_to_f64(nodes.len() - 1)
            },
            component_index: components[index],
            harmonic_closeness: harmonic[index],
            betweenness_centrality: betweenness[index],
        })
        .collect::<Vec<_>>();
    let visible_edge_count = pairs
        .iter()
        .filter(|pair| pair.state == VisibilityState::Visible)
        .count();
    let content_hash = sparse_visibility_graph_hash(scene, nodes, options, &pairs);
    Ok(VisibilityGraphResult {
        pairs,
        metrics,
        visible_edge_count,
        connected_component_count: component_count,
        content_hash,
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn spatial_cell(position: Vec3, cell_size: f64) -> Result<(i64, i64, i64), VisibilityError> {
    fn coordinate(value: f64, cell_size: f64) -> Result<i64, VisibilityError> {
        let scaled = (value / cell_size).floor();
        if scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
            Err(VisibilityError::InvalidPoint)
        } else {
            Ok(scaled as i64)
        }
    }
    Ok((
        coordinate(position.x, cell_size)?,
        coordinate(position.y, cell_size)?,
        coordinate(position.z, cell_size)?,
    ))
}

fn connected_components(adjacency: &[Vec<usize>]) -> (Vec<usize>, usize) {
    let mut labels = vec![usize::MAX; adjacency.len()];
    let mut component = 0;
    for start in 0..adjacency.len() {
        if labels[start] != usize::MAX {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        labels[start] = component;
        while let Some(node) = queue.pop_front() {
            for &neighbor in &adjacency[node] {
                if labels[neighbor] == usize::MAX {
                    labels[neighbor] = component;
                    queue.push_back(neighbor);
                }
            }
        }
        component += 1;
    }
    (labels, component)
}

fn graph_centralities(adjacency: &[Vec<usize>]) -> (Vec<f64>, Vec<f64>) {
    let node_count = adjacency.len();
    let mut harmonic = vec![0.0; node_count];
    let mut betweenness = vec![0.0; node_count];
    for source in 0..node_count {
        let mut stack = Vec::with_capacity(node_count);
        let mut predecessors = vec![Vec::<usize>::new(); node_count];
        let mut paths = vec![0.0_f64; node_count];
        let mut distance = vec![-1_i32; node_count];
        paths[source] = 1.0;
        distance[source] = 0;
        let mut queue = VecDeque::from([source]);
        while let Some(vertex) = queue.pop_front() {
            stack.push(vertex);
            for &neighbor in &adjacency[vertex] {
                if distance[neighbor] < 0 {
                    distance[neighbor] = distance[vertex] + 1;
                    queue.push_back(neighbor);
                }
                if distance[neighbor] == distance[vertex] + 1 {
                    paths[neighbor] += paths[vertex];
                    predecessors[neighbor].push(vertex);
                }
            }
        }
        if node_count > 1 {
            harmonic[source] = distance
                .iter()
                .enumerate()
                .filter(|(index, value)| *index != source && **value > 0)
                .map(|(_, value)| 1.0 / f64::from(*value))
                .sum::<f64>()
                / usize_to_f64(node_count - 1);
        }
        let mut dependency = vec![0.0_f64; node_count];
        while let Some(vertex) = stack.pop() {
            for &predecessor in &predecessors[vertex] {
                if paths[vertex] > 0.0 {
                    dependency[predecessor] = (paths[predecessor] / paths[vertex])
                        .mul_add(1.0 + dependency[vertex], dependency[predecessor]);
                }
            }
            if vertex != source {
                betweenness[vertex] += dependency[vertex];
            }
        }
    }
    let normalization = if node_count > 2 {
        1.0 / usize_to_f64((node_count - 1) * (node_count - 2))
    } else {
        0.0
    };
    for value in &mut betweenness {
        *value *= normalization;
    }
    (harmonic, betweenness)
}

/// Validation or capacity failure in a DAENA analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibilityError {
    /// One or more required endpoint collections are empty.
    EmptyInputs,
    /// A point contains NaN or infinity.
    InvalidPoint,
    /// A direction is zero, non-finite, or parallel to an invalid frame.
    InvalidDirection,
    /// A weight or sensitivity lies outside zero through one.
    InvalidWeight,
    /// Isovist sample count lies outside the supported range.
    InvalidSampleCount,
    /// Field of view is not in the interval `(0, 2*pi]`.
    InvalidFieldOfView,
    /// Maximum distance is invalid.
    InvalidMaximumDistance,
    /// Endpoint or eye clearance is negative or non-finite.
    InvalidClearance,
    /// Privacy reference distance is invalid.
    InvalidReferenceDistance,
    /// Facing exponent is negative or non-finite.
    InvalidFacingExponent,
    /// A triangle, aperture, or polyline is degenerate or non-finite.
    InvalidGeometry,
    /// Camera horizontal or vertical field of view is invalid.
    InvalidCameraFieldOfView,
    /// A distance, direction, or category weighting policy is invalid.
    InvalidWeightingPolicy,
    /// A path spacing or corridor sampling policy is invalid.
    InvalidSamplingPolicy,
    /// A memory, attribution, or material-traversal limit is invalid.
    InvalidResourcePolicy,
    /// Collections that form one comparison do not have matching identities or lengths.
    MismatchedInputs,
    /// Requested matrix or graph exceeds the production safety cap.
    ResultTooLarge,
    /// A validated scene ray could not be constructed.
    InvalidQuery,
    /// The selected ray-query backend failed the analysis.
    ExecutionFailed,
}

impl fmt::Display for VisibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyInputs => "visibility analysis requires non-empty inputs",
            Self::InvalidPoint => "visibility point coordinates must be finite",
            Self::InvalidDirection => {
                "visibility directions must be finite, non-zero, and form a valid frame"
            }
            Self::InvalidWeight => {
                "visibility weights and sensitivities must be between zero and one"
            }
            Self::InvalidSampleCount => "isovist samples must be between 32 and 65536",
            Self::InvalidFieldOfView => {
                "isovist field of view must be greater than zero and no more than 2*pi radians"
            }
            Self::InvalidMaximumDistance => {
                "visibility maximum distance must be positive and not NaN"
            }
            Self::InvalidClearance => {
                "visibility endpoint clearance must be finite and non-negative"
            }
            Self::InvalidReferenceDistance => {
                "privacy reference distance must be finite and positive"
            }
            Self::InvalidFacingExponent => {
                "privacy facing exponent must be finite and non-negative"
            }
            Self::InvalidGeometry => "visibility geometry must be finite and non-degenerate",
            Self::InvalidCameraFieldOfView => {
                "camera field of view must be finite, positive, and less than pi radians"
            }
            Self::InvalidWeightingPolicy => {
                "view weighting values must be finite and within their documented ranges"
            }
            Self::InvalidSamplingPolicy => {
                "visibility sampling policy lies outside the production limits"
            }
            Self::InvalidResourcePolicy => {
                "visibility resource policy lies outside the production limits"
            }
            Self::MismatchedInputs => {
                "visibility comparison inputs must have matching identities and lengths"
            }
            Self::ResultTooLarge => "visibility result exceeds the production resource cap",
            Self::InvalidQuery => "visibility scene query could not be constructed",
            Self::ExecutionFailed => "visibility ray-query backend failed",
        })
    }
}

impl std::error::Error for VisibilityError {}

fn isovist_hash(
    scene: &Scene,
    viewpoints: &[Viewpoint],
    options: IsovistOptions,
    rays: &[IsovistRay],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_ISOVIST_V1\0");
    hasher.update(&scene.stats().content_hash);
    hash_usize(&mut hasher, options.sample_count);
    for value in [
        options.field_of_view_radians,
        options.maximum_distance_meters,
        options.eye_offset_meters,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&options.category_mask.to_le_bytes());
    for viewpoint in viewpoints {
        hasher.update(&viewpoint.id.get().to_le_bytes());
        hash_vec3(&mut hasher, viewpoint.position);
        hash_vec3(&mut hasher, viewpoint.plane_normal);
        hash_vec3(&mut hasher, viewpoint.forward);
    }
    for ray in rays {
        hasher.update(&[ray.state as u8]);
        hasher.update(&ray.distance_meters.to_bits().to_le_bytes());
        hasher.update(&ray.object_id.get().to_le_bytes());
        hasher.update(&ray.instance_id.get().to_le_bytes());
        hasher.update(&ray.mesh_id.get().to_le_bytes());
        hasher.update(&ray.triangle_id.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn intervisibility_hash(
    scene: &Scene,
    observers: &[VisibilityObserver],
    targets: &[VisibilityTarget],
    options: IntervisibilityOptions,
    entries: &[IntervisibilityEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_INTERVISIBILITY_V1\0");
    hasher.update(&scene.stats().content_hash);
    for value in [
        options.endpoint_clearance_meters,
        options.maximum_distance_meters,
        options.privacy_reference_distance_meters,
        options.facing_exponent,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&options.category_mask.to_le_bytes());
    for observer in observers {
        hasher.update(&observer.id.get().to_le_bytes());
        hash_vec3(&mut hasher, observer.position);
        hasher.update(&observer.weight.to_bits().to_le_bytes());
    }
    for target in targets {
        hasher.update(&target.id.get().to_le_bytes());
        hash_vec3(&mut hasher, target.position);
        hash_vec3(&mut hasher, target.facing.unwrap_or(Vec3::ZERO));
        hasher.update(&target.sensitivity.to_bits().to_le_bytes());
    }
    for entry in entries {
        hasher.update(&[entry.state as u8]);
        hasher.update(&entry.distance_meters.to_bits().to_le_bytes());
        hasher.update(&entry.privacy_risk.to_bits().to_le_bytes());
        hasher.update(&entry.blocker_object_id.get().to_le_bytes());
        hasher.update(&entry.blocker_instance_id.get().to_le_bytes());
        hasher.update(&entry.blocker_mesh_id.get().to_le_bytes());
        hasher.update(&entry.blocker_triangle_id.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn visibility_graph_hash(
    scene: &Scene,
    nodes: &[VisibilityNode],
    options: VisibilityGraphOptions,
    pairs: &[VisibilityGraphPair],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_VISIBILITY_GRAPH_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&options.endpoint_clearance_meters.to_bits().to_le_bytes());
    hasher.update(&options.maximum_distance_meters.to_bits().to_le_bytes());
    hasher.update(&options.category_mask.to_le_bytes());
    hasher.update(&[u8::from(options.compute_centrality)]);
    for node in nodes {
        hasher.update(&node.id.get().to_le_bytes());
        hash_vec3(&mut hasher, node.position);
    }
    for pair in pairs {
        hash_usize(&mut hasher, pair.first_index);
        hash_usize(&mut hasher, pair.second_index);
        hasher.update(&[pair.state as u8]);
        hasher.update(&pair.distance_meters.to_bits().to_le_bytes());
        hasher.update(&pair.blocker_object_id.get().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn sparse_visibility_graph_hash(
    scene: &Scene,
    nodes: &[VisibilityNode],
    options: SparseVisibilityGraphOptions,
    pairs: &[VisibilityGraphPair],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_SPARSE_VISIBILITY_GRAPH_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&options.endpoint_clearance_meters.to_bits().to_le_bytes());
    hasher.update(&options.maximum_distance_meters.to_bits().to_le_bytes());
    hasher.update(&options.category_mask.to_le_bytes());
    hash_usize(&mut hasher, options.maximum_neighbors);
    hasher.update(&[u8::from(options.compute_centrality)]);
    for node in nodes {
        hasher.update(&node.id.get().to_le_bytes());
        hash_vec3(&mut hasher, node.position);
    }
    for pair in pairs {
        hash_usize(&mut hasher, pair.first_index);
        hash_usize(&mut hasher, pair.second_index);
        hasher.update(&[pair.state as u8]);
        hasher.update(&pair.distance_meters.to_bits().to_le_bytes());
        hasher.update(&pair.blocker_object_id.get().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_vec3(hasher: &mut blake3::Hasher, value: Vec3) {
    for component in [value.x, value.y, value.z] {
        hasher.update(&component.to_bits().to_le_bytes());
    }
}

fn hash_usize(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
}

fn usize_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("DAENA production caps keep counts within u32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};

    fn scene_from_mesh(mesh: Mesh, object_id: u64, category: u64) -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(mesh).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(object_id),
                InstanceId::new(1),
                category,
            )
            .expect("instance valid");
        builder.build().expect("scene valid")
    }

    fn square_room() -> Scene {
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        let walls = [
            (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, -1.0)),
            (Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, -1.0)),
            (Vec3::new(1.0, 1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0)),
            (Vec3::new(-1.0, 1.0, -1.0), Vec3::new(-1.0, -1.0, -1.0)),
        ];
        for (first, second) in walls {
            let base = u32::try_from(positions.len()).expect("test mesh index fits u32");
            positions.extend([
                first,
                second,
                Vec3::new(second.x, second.y, 1.0),
                Vec3::new(first.x, first.y, 1.0),
            ]);
            triangles.extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
        }
        scene_from_mesh(
            Mesh {
                positions,
                triangles,
            },
            77,
            1,
        )
    }

    fn divider_scene() -> Scene {
        scene_from_mesh(
            Mesh {
                positions: vec![
                    Vec3::new(0.0, -2.0, -2.0),
                    Vec3::new(0.0, 2.0, -2.0),
                    Vec3::new(0.0, 2.0, 2.0),
                    Vec3::new(0.0, -2.0, 2.0),
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            },
            42,
            1,
        )
    }

    #[test]
    fn square_room_isovist_recovers_area_perimeter_and_attribution() {
        let scene = square_room();
        let viewpoint = Viewpoint::try_new(
            SensorId::new(9),
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
        )
        .expect("viewpoint valid");
        let result = analyze_isovists(
            &scene,
            &[viewpoint],
            IsovistOptions {
                sample_count: 1_440,
                maximum_distance_meters: 10.0,
                eye_offset_meters: 0.0,
                category_mask: 1,
                ..IsovistOptions::default()
            },
        )
        .expect("isovist succeeds");
        let summary = result.summaries[0];
        assert!((summary.area_square_meters - 4.0).abs() < 0.02);
        assert!((summary.perimeter_meters - 8.0).abs() < 0.03);
        assert_eq!(summary.occluded_count, 1_440);
        assert_eq!(summary.dominant_occluder_object_id, ObjectId::new(77));
        assert!(summary.area_convergence_delta_square_meters < 0.02);
    }

    #[test]
    fn intervisibility_reports_blocker_and_transparent_privacy_formula() {
        let scene = divider_scene();
        let observers = [
            VisibilityObserver::try_new(SensorId::new(1), Vec3::new(-1.0, 0.0, 0.0), 1.0)
                .expect("observer valid"),
            VisibilityObserver::try_new(SensorId::new(2), Vec3::new(-1.0, 5.0, 0.0), 0.5)
                .expect("observer valid"),
        ];
        let target = VisibilityTarget::try_new(
            TargetId::new(10),
            Vec3::new(1.0, 0.0, 0.0),
            Some(Vec3::new(-1.0, 0.0, 0.0)),
            0.8,
        )
        .expect("target valid");
        let result = analyze_intervisibility(
            &scene,
            &observers,
            &[target],
            IntervisibilityOptions {
                privacy_reference_distance_meters: 2.0,
                category_mask: 1,
                ..IntervisibilityOptions::default()
            },
        )
        .expect("matrix succeeds");
        assert_eq!(result.entries[0].state, VisibilityState::Blocked);
        assert_eq!(result.entries[0].blocker_object_id, ObjectId::new(42));
        assert_eq!(result.entries[1].state, VisibilityState::Visible);
        assert!(result.entries[1].privacy_risk > 0.0);
        assert!(result.target_summaries[0].combined_privacy_risk <= 1.0);
    }

    #[test]
    fn category_filter_can_make_divider_transparent() {
        let scene = divider_scene();
        let observer =
            VisibilityObserver::try_new(SensorId::new(1), Vec3::new(-1.0, 0.0, 0.0), 1.0)
                .expect("observer valid");
        let target =
            VisibilityTarget::try_new(TargetId::new(1), Vec3::new(1.0, 0.0, 0.0), None, 1.0)
                .expect("target valid");
        let result = analyze_intervisibility(
            &scene,
            &[observer],
            &[target],
            IntervisibilityOptions {
                category_mask: 2,
                ..IntervisibilityOptions::default()
            },
        )
        .expect("matrix succeeds");
        assert_eq!(result.entries[0].state, VisibilityState::Visible);
    }

    #[test]
    fn visibility_graph_finds_two_components_and_bridge_centrality() {
        let scene = divider_scene();
        let nodes = [
            VisibilityNode::try_new(SensorId::new(1), Vec3::new(-2.0, 0.0, 0.0)).unwrap(),
            VisibilityNode::try_new(SensorId::new(2), Vec3::new(-1.0, 0.0, 0.0)).unwrap(),
            VisibilityNode::try_new(SensorId::new(3), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
            VisibilityNode::try_new(SensorId::new(4), Vec3::new(2.0, 0.0, 0.0)).unwrap(),
        ];
        let result = analyze_visibility_graph(
            &scene,
            &nodes,
            VisibilityGraphOptions {
                category_mask: 1,
                ..VisibilityGraphOptions::default()
            },
        )
        .expect("graph succeeds");
        assert_eq!(result.connected_component_count, 2);
        assert_eq!(result.visible_edge_count, 2);
        assert_eq!(
            result
                .metrics
                .iter()
                .map(|value| value.degree)
                .collect::<Vec<_>>(),
            vec![1, 1, 1, 1]
        );
    }

    #[test]
    fn sparse_visibility_graph_uses_radius_and_hard_degree_cap() {
        let scene = divider_scene();
        let nodes = (0_u32..8)
            .map(|index| {
                VisibilityNode::try_new(
                    SensorId::new(u64::from(index) + 1),
                    Vec3::new(f64::from(index), 5.0, 0.0),
                )
                .expect("node valid")
            })
            .collect::<Vec<_>>();
        let result = analyze_sparse_visibility_graph(
            &scene,
            &nodes,
            SparseVisibilityGraphOptions {
                maximum_distance_meters: 1.1,
                maximum_neighbors: 2,
                category_mask: 2,
                ..SparseVisibilityGraphOptions::default()
            },
        )
        .expect("sparse graph succeeds");
        assert_eq!(result.pairs.len(), 7);
        assert_eq!(result.visible_edge_count, 7);
        assert_eq!(result.connected_component_count, 1);
        assert!(result.metrics.iter().all(|metric| metric.degree <= 2));
        assert!(result.pairs.iter().all(|pair| pair.distance_meters <= 1.1));
    }

    #[test]
    fn path_graph_brandes_metrics_are_normalized() {
        let adjacency = vec![vec![1], vec![0, 2], vec![1]];
        let (harmonic, betweenness) = graph_centralities(&adjacency);
        assert!(harmonic[1] > harmonic[0]);
        assert!((betweenness[1] - 1.0).abs() < 1.0e-12);
        assert!(betweenness[0].abs() < 1.0e-12);
    }

    #[test]
    fn invalid_policies_are_rejected_before_querying() {
        assert_eq!(
            IsovistOptions {
                sample_count: 12,
                ..IsovistOptions::default()
            }
            .validate(),
            Err(VisibilityError::InvalidSampleCount)
        );
        assert_eq!(
            IntervisibilityOptions {
                facing_exponent: -1.0,
                ..IntervisibilityOptions::default()
            }
            .validate(),
            Err(VisibilityError::InvalidFacingExponent)
        );
    }

    #[test]
    fn planar_centroid_is_translation_invariant() {
        let eye = Vec3::new(1_000.0, -250.0, 4.0);
        let points = [
            eye + Vec3::new(-1.0, -1.0, 0.0),
            eye + Vec3::new(1.0, -1.0, 0.0),
            eye + Vec3::new(1.0, 1.0, 0.0),
            eye + Vec3::new(-1.0, 1.0, 0.0),
        ];
        let (area, perimeter, centroid) =
            planar_polygon_metrics(eye, Vec3::new(0.0, 0.0, 1.0), &points, true);
        assert!((area - 4.0).abs() < 1.0e-12);
        assert!((perimeter - 8.0).abs() < 1.0e-12);
        assert!((centroid - eye).length_squared() < 1.0e-20);
    }
}
