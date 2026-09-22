//! Volumetric isovists, landmark visibility, attribution, and scenario deltas.

use core::f64::consts::{PI, TAU};
use std::collections::BTreeMap;
use xvarna_geometry::Vec3;
use xvarna_scene::{MaterialLibrary, QueryRay, Scene, TransmissionChannel, TransmissionTrace};
use xvarna_types::{InstanceId, MeshId, ObjectId, SensorId, TargetId};

use crate::VisibilityError;

const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
const MINIMUM_3D_SAMPLES: usize = 64;
const MAXIMUM_3D_SAMPLES: usize = 262_144;
const MAXIMUM_ATTRIBUTION_ROWS: usize = 1_000_000;

/// Finite eye position for an omnidirectional three-dimensional isovist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialViewpoint {
    /// Stable viewpoint identifier.
    pub id: SensorId,
    /// Eye position in canonical metres.
    pub position: Vec3,
}

impl SpatialViewpoint {
    /// Creates a finite spatial viewpoint.
    pub const fn try_new(id: SensorId, position: Vec3) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        Ok(Self { id, position })
    }
}

/// Sampling, optical, attribution, and counterfactual policy for a 3D isovist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isovist3dOptions {
    /// Equal-solid-angle spherical sample count per viewpoint.
    pub sample_count: usize,
    /// Finite radial clipping distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Positive ray start offset in canonical metres.
    pub eye_offset_meters: f64,
    /// Included scene category mask.
    pub category_mask: u64,
    /// Maximum ranked objects retained per viewpoint.
    pub top_k: usize,
    /// Number of leading objects evaluated with exact remove-one retracing.
    pub counterfactual_count: usize,
    /// Maximum transparent geometric layers per ray.
    pub maximum_material_layers: usize,
    /// Throughput below this threshold is treated as fully blocked.
    pub minimum_transmission: f64,
}

impl Default for Isovist3dOptions {
    fn default() -> Self {
        Self {
            sample_count: 4_096,
            maximum_distance_meters: 100.0,
            eye_offset_meters: 1.0e-4,
            category_mask: u64::MAX,
            top_k: 10,
            counterfactual_count: 5,
            maximum_material_layers: 8,
            minimum_transmission: 1.0e-4,
        }
    }
}

impl Isovist3dOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !(MINIMUM_3D_SAMPLES..=MAXIMUM_3D_SAMPLES).contains(&self.sample_count) {
            return Err(VisibilityError::InvalidSampleCount);
        }
        if !self.maximum_distance_meters.is_finite() || self.maximum_distance_meters <= 0.0 {
            return Err(VisibilityError::InvalidMaximumDistance);
        }
        if !self.eye_offset_meters.is_finite() || self.eye_offset_meters < 0.0 {
            return Err(VisibilityError::InvalidClearance);
        }
        if self.top_k == 0
            || self.top_k > 1_024
            || self.counterfactual_count > self.top_k
            || !(1..=64).contains(&self.maximum_material_layers)
            || !self.minimum_transmission.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_transmission)
        {
            return Err(VisibilityError::InvalidResourcePolicy);
        }
        Ok(self)
    }
}

/// One equal-solid-angle spatial ray and its optical first-hit attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isovist3dRay {
    /// World-space unit direction.
    pub direction: Vec3,
    /// Endpoint at first hit or the radial limit.
    pub endpoint: Vec3,
    /// First-hit or clipped radial distance.
    pub distance_meters: f64,
    /// Remaining visible transmission after all material layers.
    pub transmission: f64,
    /// First blocking object, or zero for open space.
    pub object_id: ObjectId,
    /// First blocking occurrence.
    pub instance_id: InstanceId,
    /// First blocking mesh.
    pub mesh_id: MeshId,
    /// First blocking triangle.
    pub triangle_id: u32,
    /// Exact category combination of the first occurrence.
    pub category_mask: u64,
}

/// Volumetric and solid-angle metrics for one spatial viewpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isovist3dSummary {
    /// Stable viewpoint identifier.
    pub viewpoint_id: SensorId,
    /// Radial volume integral `sum(omega * r^3 / 3)` in m³.
    pub volume_cubic_meters: f64,
    /// Radial surface measure `sum(omega * r²)` in m²; not triangulated envelope area.
    pub radial_surface_square_meters: f64,
    /// Mean clipped radial length.
    pub mean_radial_meters: f64,
    /// Minimum clipped radial length.
    pub minimum_radial_meters: f64,
    /// Maximum clipped radial length.
    pub maximum_radial_meters: f64,
    /// Transmission-weighted visible solid angle in steradians.
    pub visible_solid_angle_steradians: f64,
    /// Visible solid angle divided by `4*pi`.
    pub openness_ratio: f64,
    /// Full-versus-interleaved-half volume convergence delta.
    pub volume_convergence_delta_cubic_meters: f64,
    /// Object with the largest attributed blocked solid angle.
    pub dominant_occluder_object_id: ObjectId,
    /// Dominant object's share of total blocked solid angle.
    pub dominant_occluder_fraction: f64,
}

/// Ranked, material-aware obstruction attribution reusable across DAENA analyses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributionEntry {
    /// Source observer/viewpoint.
    pub observer_id: SensorId,
    /// Optional target; zero denotes an omnidirectional isovist scope.
    pub target_id: TargetId,
    /// One-based rank within the observer/target scope.
    pub rank: usize,
    /// Source object receiving the attribution.
    pub object_id: ObjectId,
    /// Union of exact occurrence categories contributing to the object.
    pub category_mask: u64,
    /// Blocked solid angle or normalized sample weight.
    pub blocked_weight: f64,
    /// Object weight divided by all attributed blocked weight in the scope.
    pub fraction: f64,
    /// Loss-weighted mean first-interaction distance.
    pub mean_distance_meters: f64,
    /// Exact remove-one recovered visible weight; zero when not evaluated.
    pub counterfactual_recovered_weight: f64,
    /// Exact remove-one radial volume delta; zero outside 3D-isovist counterfactuals.
    pub counterfactual_volume_delta_cubic_meters: f64,
}

/// Partitioned attribution by exact category-mask combination.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CategoryBreakdown {
    /// Source observer/viewpoint.
    pub observer_id: SensorId,
    /// Optional target; zero denotes the whole viewpoint.
    pub target_id: TargetId,
    /// Exact category-mask combination; rows do not overlap.
    pub category_mask: u64,
    /// Attributed blocked weight.
    pub blocked_weight: f64,
    /// Category weight divided by all blocked weight in scope.
    pub fraction: f64,
}

/// Complete viewpoint-major volumetric result.
#[derive(Clone, Debug, PartialEq)]
pub struct Isovist3dResult {
    /// One aggregate per viewpoint.
    pub summaries: Vec<Isovist3dSummary>,
    /// Viewpoint-major spherical ray matrix.
    pub rays: Vec<Isovist3dRay>,
    /// Ranked object attribution.
    pub attribution: Vec<AttributionEntry>,
    /// Exact category partitions.
    pub categories: Vec<CategoryBreakdown>,
    /// Samples per viewpoint.
    pub sample_count: usize,
    /// Material, scene, input, policy, ray, and attribution identity.
    pub content_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Default)]
struct AttributionAccumulator {
    weight: f64,
    weighted_distance: f64,
    category_mask: u64,
}

/// Computes a material-aware equal-solid-angle 3D isovist with exact remove-one attribution.
#[allow(clippy::too_many_lines)]
pub fn analyze_isovist_3d(
    scene: &Scene,
    viewpoints: &[SpatialViewpoint],
    materials: &MaterialLibrary,
    options: Isovist3dOptions,
) -> Result<Isovist3dResult, VisibilityError> {
    if viewpoints.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let options = options.validate()?;
    let total = viewpoints
        .len()
        .checked_mul(options.sample_count)
        .filter(|value| *value <= MAXIMUM_ATTRIBUTION_ROWS)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let directions = fibonacci_sphere(options.sample_count);
    let solid_angle = 4.0 * PI / usize_to_f64(options.sample_count);
    let mut rays = Vec::with_capacity(total);
    let mut summaries = Vec::with_capacity(viewpoints.len());
    let mut attribution = Vec::new();
    let mut categories = Vec::new();

    for viewpoint in viewpoints {
        let start = rays.len();
        let mut objects = BTreeMap::<ObjectId, AttributionAccumulator>::new();
        let mut category_weights = BTreeMap::<u64, f64>::new();
        for direction in &directions {
            let query = QueryRay::try_new(
                viewpoint.position + *direction * options.eye_offset_meters,
                *direction,
                0.0,
                options.maximum_distance_meters,
                options.category_mask,
            )
            .map_err(|_| VisibilityError::InvalidQuery)?;
            let trace = scene.trace_transmission(
                query,
                materials,
                TransmissionChannel::Visible,
                options.maximum_material_layers,
                options.minimum_transmission,
            );
            accumulate_trace(
                scene,
                &trace,
                solid_angle,
                &mut objects,
                &mut category_weights,
            );
            let first = trace.layers.first().map(|layer| layer.hit);
            let distance = first.map_or(options.maximum_distance_meters, |hit| hit.distance);
            let category_mask = first
                .and_then(|hit| scene.instance_category_mask(hit.instance_id))
                .unwrap_or(0);
            rays.push(Isovist3dRay {
                direction: *direction,
                endpoint: viewpoint.position + *direction * distance,
                distance_meters: distance,
                transmission: trace.transmission,
                object_id: first.map_or(ObjectId::new(0), |hit| hit.object_id),
                instance_id: first.map_or(InstanceId::new(0), |hit| hit.instance_id),
                mesh_id: first.map_or(MeshId::new(0), |hit| hit.mesh_id),
                triangle_id: first.map_or(u32::MAX, |hit| hit.triangle_id),
                category_mask,
            });
        }
        let row = &rays[start..];
        let total_blocked = objects.values().map(|value| value.weight).sum::<f64>();
        let mut ranked = objects.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .1
                .weight
                .total_cmp(&left.1.weight)
                .then_with(|| left.0.cmp(&right.0))
        });
        let volume = radial_volume(row, solid_angle);
        let reduced_volume = radial_volume_stride(row, solid_angle * 2.0, 2);
        let visible_solid_angle = row
            .iter()
            .map(|ray| ray.transmission * solid_angle)
            .sum::<f64>();
        let dominant = ranked.first().map_or(ObjectId::new(0), |value| value.0);
        let dominant_weight = ranked.first().map_or(0.0, |value| value.1.weight);
        let mut rows = ranked
            .iter()
            .take(options.top_k)
            .enumerate()
            .map(|(index, (object_id, value))| AttributionEntry {
                observer_id: viewpoint.id,
                target_id: TargetId::new(0),
                rank: index + 1,
                object_id: *object_id,
                category_mask: value.category_mask,
                blocked_weight: value.weight,
                fraction: safe_ratio(value.weight, total_blocked),
                mean_distance_meters: safe_ratio(value.weighted_distance, value.weight),
                counterfactual_recovered_weight: 0.0,
                counterfactual_volume_delta_cubic_meters: 0.0,
            })
            .collect::<Vec<_>>();
        for entry in rows.iter_mut().take(options.counterfactual_count) {
            let (recovered, counterfactual_volume) = counterfactual_isovist(
                scene,
                *viewpoint,
                &directions,
                materials,
                options,
                entry.object_id,
                visible_solid_angle,
                volume,
                solid_angle,
            )?;
            entry.counterfactual_recovered_weight = recovered;
            entry.counterfactual_volume_delta_cubic_meters = counterfactual_volume;
        }
        attribution.extend(rows);
        categories.extend(category_weights.into_iter().map(|(category_mask, weight)| {
            CategoryBreakdown {
                observer_id: viewpoint.id,
                target_id: TargetId::new(0),
                category_mask,
                blocked_weight: weight,
                fraction: safe_ratio(weight, total_blocked),
            }
        }));
        summaries.push(Isovist3dSummary {
            viewpoint_id: viewpoint.id,
            volume_cubic_meters: volume,
            radial_surface_square_meters: row
                .iter()
                .map(|ray| solid_angle * ray.distance_meters.powi(2))
                .sum(),
            mean_radial_meters: row.iter().map(|ray| ray.distance_meters).sum::<f64>()
                / usize_to_f64(row.len()),
            minimum_radial_meters: row
                .iter()
                .map(|ray| ray.distance_meters)
                .fold(f64::INFINITY, f64::min),
            maximum_radial_meters: row
                .iter()
                .map(|ray| ray.distance_meters)
                .fold(0.0, f64::max),
            visible_solid_angle_steradians: visible_solid_angle,
            openness_ratio: visible_solid_angle / (4.0 * PI),
            volume_convergence_delta_cubic_meters: (volume - reduced_volume).abs(),
            dominant_occluder_object_id: dominant,
            dominant_occluder_fraction: safe_ratio(dominant_weight, total_blocked),
        });
    }
    let content_hash = isovist_3d_hash(scene, viewpoints, materials, options, &rays, &attribution);
    Ok(Isovist3dResult {
        summaries,
        rays,
        attribution,
        categories,
        sample_count: options.sample_count,
        content_hash,
    })
}

#[allow(clippy::too_many_arguments)]
fn counterfactual_isovist(
    scene: &Scene,
    viewpoint: SpatialViewpoint,
    directions: &[Vec3],
    materials: &MaterialLibrary,
    options: Isovist3dOptions,
    excluded: ObjectId,
    baseline_visible: f64,
    baseline_volume: f64,
    solid_angle: f64,
) -> Result<(f64, f64), VisibilityError> {
    let mut visible = 0.0;
    let mut volume = 0.0;
    for direction in directions {
        let query = QueryRay::try_new(
            viewpoint.position + *direction * options.eye_offset_meters,
            *direction,
            0.0,
            options.maximum_distance_meters,
            options.category_mask,
        )
        .map_err(|_| VisibilityError::InvalidQuery)?;
        let trace = scene.trace_transmission_excluding_object(
            query,
            materials,
            TransmissionChannel::Visible,
            options.maximum_material_layers,
            options.minimum_transmission,
            excluded,
        );
        let distance = trace
            .layers
            .first()
            .map_or(options.maximum_distance_meters, |layer| layer.hit.distance);
        visible = trace.transmission.mul_add(solid_angle, visible);
        volume += solid_angle * distance.powi(3) / 3.0;
    }
    Ok((
        (visible - baseline_visible).max(0.0),
        volume - baseline_volume,
    ))
}

fn accumulate_trace(
    scene: &Scene,
    trace: &TransmissionTrace,
    sample_weight: f64,
    objects: &mut BTreeMap<ObjectId, AttributionAccumulator>,
    categories: &mut BTreeMap<u64, f64>,
) {
    for layer in &trace.layers {
        if layer.attributed_loss <= 0.0 {
            continue;
        }
        let weight = layer.attributed_loss * sample_weight;
        let category = scene
            .instance_category_mask(layer.hit.instance_id)
            .unwrap_or(0);
        let entry = objects.entry(layer.hit.object_id).or_default();
        entry.weight += weight;
        entry.weighted_distance = weight.mul_add(layer.hit.distance, entry.weighted_distance);
        entry.category_mask |= category;
        *categories.entry(category).or_default() += weight;
    }
}

fn fibonacci_sphere(count: usize) -> Vec<Vec3> {
    (0..count)
        .map(|index| {
            let fraction = (usize_to_f64(index) + 0.5) / usize_to_f64(count);
            let z = 2.0_f64.mul_add(-fraction, 1.0);
            let radius = (1.0 - z * z).max(0.0).sqrt();
            let angle = usize_to_f64(index) * GOLDEN_ANGLE;
            Vec3::new(radius * angle.cos(), radius * angle.sin(), z)
        })
        .collect()
}

fn radial_volume(rays: &[Isovist3dRay], solid_angle: f64) -> f64 {
    rays.iter()
        .map(|ray| solid_angle * ray.distance_meters.powi(3) / 3.0)
        .sum()
}

fn radial_volume_stride(rays: &[Isovist3dRay], solid_angle: f64, stride: usize) -> f64 {
    rays.iter()
        .step_by(stride)
        .map(|ray| solid_angle * ray.distance_meters.powi(3) / 3.0)
        .sum()
}

/// Oriented landmark observer with a complete camera frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandmarkObserver {
    /// Stable observer identifier.
    pub id: SensorId,
    /// Eye position in canonical metres.
    pub position: Vec3,
    /// Unit forward direction.
    pub forward: Vec3,
    /// Unit up direction orthogonal to forward.
    pub up: Vec3,
}

impl LandmarkObserver {
    /// Validates and orthonormalizes a landmark observer.
    pub fn try_new(
        id: SensorId,
        position: Vec3,
        forward: Vec3,
        up: Vec3,
    ) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        let forward = forward
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        let up = (up - forward * up.dot(forward))
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        Ok(Self {
            id,
            position,
            forward,
            up,
        })
    }
}

/// Spherical landmark proxy with declared importance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landmark {
    /// Stable landmark identifier.
    pub id: TargetId,
    /// Landmark centre in canonical metres.
    pub position: Vec3,
    /// Positive proxy radius in metres.
    pub radius_meters: f64,
    /// Importance multiplier from zero through one.
    pub weight: f64,
}

impl Landmark {
    /// Creates a finite positive-radius landmark.
    pub const fn try_new(
        id: TargetId,
        position: Vec3,
        radius_meters: f64,
        weight: f64,
    ) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        if !radius_meters.is_finite() || radius_meters <= 0.0 {
            return Err(VisibilityError::InvalidMaximumDistance);
        }
        if !weight.is_finite() || weight < 0.0 || weight > 1.0 {
            return Err(VisibilityError::InvalidWeight);
        }
        Ok(Self {
            id,
            position,
            radius_meters,
            weight,
        })
    }
}

/// Camera, sampling, and material policy for landmark visibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandmarkVisibilityOptions {
    /// Uniform-area disk samples per observer/landmark pair.
    pub sample_count: usize,
    /// Horizontal camera field of view in radians.
    pub horizontal_field_of_view_radians: f64,
    /// Vertical camera field of view in radians.
    pub vertical_field_of_view_radians: f64,
    /// Maximum centre distance in metres.
    pub maximum_distance_meters: f64,
    /// Endpoint clearance in metres.
    pub endpoint_clearance_meters: f64,
    /// Included occluder categories.
    pub category_mask: u64,
    /// Maximum top object rows per pair.
    pub top_k: usize,
    /// Maximum transparent layers.
    pub maximum_material_layers: usize,
    /// Minimum retained visible throughput.
    pub minimum_transmission: f64,
}

impl Default for LandmarkVisibilityOptions {
    fn default() -> Self {
        Self {
            sample_count: 64,
            horizontal_field_of_view_radians: PI / 2.0,
            vertical_field_of_view_radians: PI / 2.0,
            maximum_distance_meters: 1_000.0,
            endpoint_clearance_meters: 1.0e-4,
            category_mask: u64::MAX,
            top_k: 10,
            maximum_material_layers: 8,
            minimum_transmission: 1.0e-4,
        }
    }
}

impl LandmarkVisibilityOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !(4..=4_096).contains(&self.sample_count) {
            return Err(VisibilityError::InvalidSampleCount);
        }
        if !self.horizontal_field_of_view_radians.is_finite()
            || self.horizontal_field_of_view_radians <= 0.0
            || self.horizontal_field_of_view_radians > TAU
            || !self.vertical_field_of_view_radians.is_finite()
            || self.vertical_field_of_view_radians <= 0.0
            || self.vertical_field_of_view_radians > PI
        {
            return Err(VisibilityError::InvalidFieldOfView);
        }
        if !self.maximum_distance_meters.is_finite()
            || self.maximum_distance_meters <= 0.0
            || !self.endpoint_clearance_meters.is_finite()
            || self.endpoint_clearance_meters < 0.0
            || self.top_k == 0
            || self.top_k > 1_024
            || !(1..=64).contains(&self.maximum_material_layers)
            || !self.minimum_transmission.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_transmission)
        {
            return Err(VisibilityError::InvalidResourcePolicy);
        }
        Ok(self)
    }
}

/// One observer-landmark visibility entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandmarkVisibilityEntry {
    /// Observer identifier.
    pub observer_id: SensorId,
    /// Landmark identifier.
    pub landmark_id: TargetId,
    /// Centre-to-centre distance.
    pub distance_meters: f64,
    /// Geometric apparent solid angle of the spherical proxy.
    pub apparent_solid_angle_steradians: f64,
    /// Mean material transmission over landmark disk samples.
    pub visible_fraction: f64,
    /// Visible apparent solid angle.
    pub visible_solid_angle_steradians: f64,
    /// Importance-weighted visible solid angle normalized by camera solid angle.
    pub weighted_visibility_score: f64,
    /// True when landmark centre lies inside both camera FOV limits.
    pub inside_field_of_view: bool,
    /// Dominant blocking object.
    pub dominant_blocker_object_id: ObjectId,
    /// Dominant share of attributed blocked sample weight.
    pub dominant_blocker_fraction: f64,
}

/// Complete row-major landmark visibility result.
#[derive(Clone, Debug, PartialEq)]
pub struct LandmarkVisibilityResult {
    /// `observer_index * landmark_count + landmark_index` entries.
    pub entries: Vec<LandmarkVisibilityEntry>,
    /// Ranked per-pair obstruction attribution.
    pub attribution: Vec<AttributionEntry>,
    /// Per-pair exact category partitions.
    pub categories: Vec<CategoryBreakdown>,
    /// Number of landmark columns.
    pub landmark_count: usize,
    /// Complete deterministic result identity.
    pub content_hash: [u8; 32],
}

/// Computes material-aware partial visibility and solid angle for landmarks.
#[allow(clippy::too_many_lines)]
pub fn analyze_landmark_visibility(
    scene: &Scene,
    observers: &[LandmarkObserver],
    landmarks: &[Landmark],
    materials: &MaterialLibrary,
    options: LandmarkVisibilityOptions,
) -> Result<LandmarkVisibilityResult, VisibilityError> {
    if observers.is_empty() || landmarks.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let options = options.validate()?;
    observers
        .len()
        .checked_mul(landmarks.len())
        .and_then(|value| value.checked_mul(options.sample_count))
        .filter(|value| *value <= MAXIMUM_ATTRIBUTION_ROWS)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let mut entries = Vec::with_capacity(observers.len() * landmarks.len());
    let mut attribution = Vec::new();
    let mut categories = Vec::new();
    for observer in observers {
        let right = observer
            .forward
            .cross(observer.up)
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        for landmark in landmarks {
            let delta = landmark.position - observer.position;
            let distance = delta.length_squared().sqrt();
            let direction = if distance > f64::EPSILON {
                delta * distance.recip()
            } else {
                Vec3::ZERO
            };
            let horizontal = direction.dot(right).atan2(direction.dot(observer.forward));
            let vertical = direction.dot(observer.up).clamp(-1.0, 1.0).asin();
            let inside = distance > landmark.radius_meters
                && distance <= options.maximum_distance_meters
                && horizontal.abs() <= options.horizontal_field_of_view_radians * 0.5
                && vertical.abs() <= options.vertical_field_of_view_radians * 0.5;
            let apparent = if distance > landmark.radius_meters {
                let half_angle = (landmark.radius_meters / distance).clamp(0.0, 1.0).asin();
                2.0 * PI * (1.0 - half_angle.cos())
            } else {
                4.0 * PI
            };
            let mut visible_sum = 0.0;
            let mut objects = BTreeMap::<ObjectId, AttributionAccumulator>::new();
            let mut category_weights = BTreeMap::<u64, f64>::new();
            if inside {
                let disk_right = direction.cross(observer.up).normalized().unwrap_or(right);
                let disk_up = disk_right
                    .cross(direction)
                    .normalized()
                    .ok_or(VisibilityError::InvalidDirection)?;
                for index in 0..options.sample_count {
                    let radius = ((usize_to_f64(index) + 0.5) / usize_to_f64(options.sample_count))
                        .sqrt()
                        * landmark.radius_meters;
                    let angle = usize_to_f64(index) * GOLDEN_ANGLE;
                    let point = landmark.position
                        + disk_right * (radius * angle.cos())
                        + disk_up * (radius * angle.sin());
                    let sample_delta = point - observer.position;
                    let sample_distance = sample_delta.length_squared().sqrt();
                    let sample_direction = sample_delta * sample_distance.recip();
                    let query = QueryRay::try_new(
                        observer.position,
                        sample_direction,
                        options.endpoint_clearance_meters,
                        (sample_distance - options.endpoint_clearance_meters)
                            .max(options.endpoint_clearance_meters),
                        options.category_mask,
                    )
                    .map_err(|_| VisibilityError::InvalidQuery)?;
                    let trace = scene.trace_transmission(
                        query,
                        materials,
                        TransmissionChannel::Visible,
                        options.maximum_material_layers,
                        options.minimum_transmission,
                    );
                    visible_sum += trace.transmission;
                    accumulate_trace(
                        scene,
                        &trace,
                        1.0 / usize_to_f64(options.sample_count),
                        &mut objects,
                        &mut category_weights,
                    );
                }
            }
            let visible_fraction = if inside {
                visible_sum / usize_to_f64(options.sample_count)
            } else {
                0.0
            };
            let total_blocked = objects.values().map(|value| value.weight).sum::<f64>();
            let mut ranked = objects.into_iter().collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                right
                    .1
                    .weight
                    .total_cmp(&left.1.weight)
                    .then_with(|| left.0.cmp(&right.0))
            });
            let dominant = ranked.first().map_or(ObjectId::new(0), |value| value.0);
            let dominant_weight = ranked.first().map_or(0.0, |value| value.1.weight);
            attribution.extend(ranked.iter().take(options.top_k).enumerate().map(
                |(index, (object_id, value))| AttributionEntry {
                    observer_id: observer.id,
                    target_id: landmark.id,
                    rank: index + 1,
                    object_id: *object_id,
                    category_mask: value.category_mask,
                    blocked_weight: value.weight,
                    fraction: safe_ratio(value.weight, total_blocked),
                    mean_distance_meters: safe_ratio(value.weighted_distance, value.weight),
                    counterfactual_recovered_weight: 0.0,
                    counterfactual_volume_delta_cubic_meters: 0.0,
                },
            ));
            categories.extend(category_weights.into_iter().map(|(category_mask, weight)| {
                CategoryBreakdown {
                    observer_id: observer.id,
                    target_id: landmark.id,
                    category_mask,
                    blocked_weight: weight,
                    fraction: safe_ratio(weight, total_blocked),
                }
            }));
            let camera_solid_angle = 4.0
                * (options.horizontal_field_of_view_radians * 0.5).sin()
                * (options.vertical_field_of_view_radians * 0.5).sin();
            entries.push(LandmarkVisibilityEntry {
                observer_id: observer.id,
                landmark_id: landmark.id,
                distance_meters: distance,
                apparent_solid_angle_steradians: apparent,
                visible_fraction,
                visible_solid_angle_steradians: apparent * visible_fraction,
                weighted_visibility_score: safe_ratio(
                    apparent * visible_fraction * landmark.weight,
                    camera_solid_angle,
                ),
                inside_field_of_view: inside,
                dominant_blocker_object_id: dominant,
                dominant_blocker_fraction: safe_ratio(dominant_weight, total_blocked),
            });
        }
    }
    let content_hash = landmark_hash(scene, observers, landmarks, materials, options, &entries);
    Ok(LandmarkVisibilityResult {
        entries,
        attribution,
        categories,
        landmark_count: landmarks.len(),
        content_hash,
    })
}

/// Per-viewpoint delta between two aligned 3D-isovist scenarios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialScenarioDelta {
    /// Viewpoint identifier.
    pub viewpoint_id: SensorId,
    /// Candidate minus baseline volume.
    pub volume_delta_cubic_meters: f64,
    /// Candidate minus baseline openness.
    pub openness_delta: f64,
    /// Candidate minus baseline mean radius.
    pub mean_radial_delta_meters: f64,
}

/// Per-object change in obstruction share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributionDelta {
    /// Viewpoint identifier.
    pub viewpoint_id: SensorId,
    /// Source object.
    pub object_id: ObjectId,
    /// Baseline blocked fraction.
    pub baseline_fraction: f64,
    /// Candidate blocked fraction.
    pub candidate_fraction: f64,
    /// Candidate minus baseline fraction.
    pub delta_fraction: f64,
}

/// Aligned scenario comparison with metric and attribution deltas.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialScenarioComparison {
    /// Baseline scenario label.
    pub baseline_name: String,
    /// Candidate scenario label.
    pub candidate_name: String,
    /// Per-viewpoint metric deltas.
    pub metrics: Vec<SpatialScenarioDelta>,
    /// Union of top-attributed objects in either scenario.
    pub attribution_deltas: Vec<AttributionDelta>,
    /// Deterministic comparison identity.
    pub content_hash: [u8; 32],
}

/// Compares two aligned 3D-isovist results and attributes changed obstruction.
pub fn compare_spatial_scenarios(
    baseline_name: impl Into<String>,
    baseline: &Isovist3dResult,
    candidate_name: impl Into<String>,
    candidate: &Isovist3dResult,
) -> Result<SpatialScenarioComparison, VisibilityError> {
    if baseline.summaries.len() != candidate.summaries.len()
        || baseline
            .summaries
            .iter()
            .zip(&candidate.summaries)
            .any(|(left, right)| left.viewpoint_id != right.viewpoint_id)
    {
        return Err(VisibilityError::MismatchedInputs);
    }
    let baseline_name = baseline_name.into();
    let candidate_name = candidate_name.into();
    let metrics = baseline
        .summaries
        .iter()
        .zip(&candidate.summaries)
        .map(|(left, right)| SpatialScenarioDelta {
            viewpoint_id: left.viewpoint_id,
            volume_delta_cubic_meters: right.volume_cubic_meters - left.volume_cubic_meters,
            openness_delta: right.openness_ratio - left.openness_ratio,
            mean_radial_delta_meters: right.mean_radial_meters - left.mean_radial_meters,
        })
        .collect::<Vec<_>>();
    let mut values = BTreeMap::<(SensorId, ObjectId), (f64, f64)>::new();
    for entry in &baseline.attribution {
        values
            .entry((entry.observer_id, entry.object_id))
            .or_default()
            .0 = entry.fraction;
    }
    for entry in &candidate.attribution {
        values
            .entry((entry.observer_id, entry.object_id))
            .or_default()
            .1 = entry.fraction;
    }
    let attribution_deltas = values
        .into_iter()
        .map(
            |((viewpoint_id, object_id), (baseline_fraction, candidate_fraction))| {
                AttributionDelta {
                    viewpoint_id,
                    object_id,
                    baseline_fraction,
                    candidate_fraction,
                    delta_fraction: candidate_fraction - baseline_fraction,
                }
            },
        )
        .collect::<Vec<_>>();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_SCENARIO_COMPARISON_V1\0");
    hasher.update(baseline_name.as_bytes());
    hasher.update(candidate_name.as_bytes());
    hasher.update(&baseline.content_hash);
    hasher.update(&candidate.content_hash);
    let content_hash = *hasher.finalize().as_bytes();
    Ok(SpatialScenarioComparison {
        baseline_name,
        candidate_name,
        metrics,
        attribution_deltas,
        content_hash,
    })
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

fn usize_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("validated analysis size fits u32"))
}

fn isovist_3d_hash(
    scene: &Scene,
    viewpoints: &[SpatialViewpoint],
    materials: &MaterialLibrary,
    options: Isovist3dOptions,
    rays: &[Isovist3dRay],
    attribution: &[AttributionEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_ISOVIST_3D_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&materials.content_hash());
    hasher.update(
        &u64::try_from(options.sample_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&options.maximum_distance_meters.to_bits().to_le_bytes());
    hasher.update(&options.category_mask.to_le_bytes());
    for viewpoint in viewpoints {
        hasher.update(&viewpoint.id.get().to_le_bytes());
        hash_vec3(&mut hasher, viewpoint.position);
    }
    for ray in rays {
        hasher.update(&ray.distance_meters.to_bits().to_le_bytes());
        hasher.update(&ray.transmission.to_bits().to_le_bytes());
        hasher.update(&ray.object_id.get().to_le_bytes());
    }
    for entry in attribution {
        hasher.update(&entry.object_id.get().to_le_bytes());
        hasher.update(&entry.blocked_weight.to_bits().to_le_bytes());
        hasher.update(
            &entry
                .counterfactual_recovered_weight
                .to_bits()
                .to_le_bytes(),
        );
    }
    *hasher.finalize().as_bytes()
}

fn landmark_hash(
    scene: &Scene,
    observers: &[LandmarkObserver],
    landmarks: &[Landmark],
    materials: &MaterialLibrary,
    options: LandmarkVisibilityOptions,
    entries: &[LandmarkVisibilityEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_LANDMARK_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&materials.content_hash());
    hasher.update(
        &u64::try_from(options.sample_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for observer in observers {
        hasher.update(&observer.id.get().to_le_bytes());
        hash_vec3(&mut hasher, observer.position);
        hash_vec3(&mut hasher, observer.forward);
        hash_vec3(&mut hasher, observer.up);
    }
    for landmark in landmarks {
        hasher.update(&landmark.id.get().to_le_bytes());
        hash_vec3(&mut hasher, landmark.position);
        hasher.update(&landmark.radius_meters.to_bits().to_le_bytes());
        hasher.update(&landmark.weight.to_bits().to_le_bytes());
    }
    for entry in entries {
        hasher.update(&entry.visible_fraction.to_bits().to_le_bytes());
        hasher.update(&entry.dominant_blocker_object_id.get().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_vec3(hasher: &mut blake3::Hasher, value: Vec3) {
    hasher.update(&value.x.to_bits().to_le_bytes());
    hasher.update(&value.y.to_bits().to_le_bytes());
    hasher.update(&value.z.to_bits().to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{AnalysisMaterial, SceneBuildOptions, SceneBuilder, Transform};

    fn divider() -> Scene {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(1.0, -10.0, -10.0),
                Vec3::new(1.0, 10.0, -10.0),
                Vec3::new(1.0, 10.0, 10.0),
                Vec3::new(1.0, -10.0, 10.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).unwrap();
        let mesh_id = builder.add_mesh(mesh).unwrap();
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(42),
                InstanceId::new(7),
                2,
            )
            .unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn spatial_isovist_partitions_categories_and_computes_remove_one() {
        let scene = divider();
        let result = analyze_isovist_3d(
            &scene,
            &[SpatialViewpoint::try_new(SensorId::new(1), Vec3::ZERO).unwrap()],
            &MaterialLibrary::default(),
            Isovist3dOptions {
                sample_count: 512,
                maximum_distance_meters: 5.0,
                eye_offset_meters: 0.0,
                category_mask: 2,
                top_k: 4,
                counterfactual_count: 1,
                ..Isovist3dOptions::default()
            },
        )
        .unwrap();
        assert_eq!(result.summaries.len(), 1);
        assert_eq!(result.attribution[0].object_id, ObjectId::new(42));
        assert!(result.attribution[0].counterfactual_recovered_weight > 0.0);
        assert_eq!(result.categories[0].category_mask, 2);
        assert!(result.summaries[0].volume_cubic_meters > 0.0);
    }

    #[test]
    fn landmark_material_transmission_and_scenario_delta_are_visible() {
        let scene = divider();
        let glass = AnalysisMaterial::try_new(1, 0.6, 0.4, 0.2).unwrap();
        let materials = MaterialLibrary::try_new([glass], [(ObjectId::new(42), 1)]).unwrap();
        let observer = LandmarkObserver::try_new(
            SensorId::new(1),
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .unwrap();
        let landmark =
            Landmark::try_new(TargetId::new(9), Vec3::new(2.0, 0.0, 0.0), 0.25, 1.0).unwrap();
        let result = analyze_landmark_visibility(
            &scene,
            &[observer],
            &[landmark],
            &materials,
            LandmarkVisibilityOptions::default(),
        )
        .unwrap();
        assert!(result.entries[0].visible_fraction > 0.0);
        assert!(result.entries[0].visible_fraction < 1.0);

        let point = SpatialViewpoint::try_new(SensorId::new(1), Vec3::ZERO).unwrap();
        let baseline = analyze_isovist_3d(
            &scene,
            &[point],
            &MaterialLibrary::default(),
            Isovist3dOptions {
                sample_count: 128,
                counterfactual_count: 0,
                ..Isovist3dOptions::default()
            },
        )
        .unwrap();
        let candidate = analyze_isovist_3d(
            &scene,
            &[point],
            &materials,
            Isovist3dOptions {
                sample_count: 128,
                counterfactual_count: 0,
                ..Isovist3dOptions::default()
            },
        )
        .unwrap();
        let comparison =
            compare_spatial_scenarios("opaque", &baseline, "glass", &candidate).unwrap();
        assert!(comparison.metrics[0].openness_delta > 0.0);
    }
}
