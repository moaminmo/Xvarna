//! Solid-angle target, weighted, and green-view analysis.

use crate::VisibilityError;
use core::f64::consts::PI;
use std::collections::BTreeMap;
use xvarna_geometry::Vec3;
use xvarna_scene::{QueryRay, RayQueryExecutionSummary, RayQueryExecutor, Scene};
use xvarna_types::{ObjectId, SensorId, TargetId};

const MAXIMUM_PATCH_SAMPLES: usize = 4_096;
const MAXIMUM_TARGET_RAYS: usize = 16_000_000;

/// An oriented observer camera used by Target/Weighted/Green View.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewObserver {
    /// Stable observer identifier.
    pub id: SensorId,
    /// Eye position in canonical metres.
    pub position: Vec3,
    /// Unit camera-forward direction.
    pub forward: Vec3,
    /// Unit camera-up direction, orthogonal to `forward`.
    pub up: Vec3,
    /// Observer importance in the inclusive range zero through one.
    pub weight: f64,
}

impl ViewObserver {
    /// Validates and orthonormalizes an observer camera frame.
    pub fn try_new(
        id: SensorId,
        position: Vec3,
        forward: Vec3,
        up: Vec3,
        weight: f64,
    ) -> Result<Self, VisibilityError> {
        if !position.is_finite() {
            return Err(VisibilityError::InvalidPoint);
        }
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            return Err(VisibilityError::InvalidWeight);
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
            weight,
        })
    }
}

/// One triangular target patch. Multiple patches may share a `target_id`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTargetPatch {
    /// Stable logical target identifier.
    pub target_id: TargetId,
    /// Triangle vertices in canonical metres.
    pub vertices: [Vec3; 3],
    /// Target category bits; at least one bit must be set.
    pub category_mask: u64,
    /// Explicit category/desirability weight in the inclusive range zero through one.
    pub category_weight: f64,
}

impl ViewTargetPatch {
    /// Creates a finite, non-degenerate target triangle.
    pub fn try_new(
        target_id: TargetId,
        vertices: [Vec3; 3],
        category_mask: u64,
        category_weight: f64,
    ) -> Result<Self, VisibilityError> {
        if vertices.iter().any(|vertex| !vertex.is_finite())
            || (vertices[1] - vertices[0])
                .cross(vertices[2] - vertices[0])
                .length_squared()
                <= 1.0e-24
            || category_mask == 0
        {
            return Err(VisibilityError::InvalidGeometry);
        }
        if !category_weight.is_finite() || !(0.0..=1.0).contains(&category_weight) {
            return Err(VisibilityError::InvalidWeightingPolicy);
        }
        Ok(Self {
            target_id,
            vertices,
            category_mask,
            category_weight,
        })
    }

    fn normal_and_area(self) -> (Vec3, f64) {
        let cross =
            (self.vertices[1] - self.vertices[0]).cross(self.vertices[2] - self.vertices[0]);
        let twice_area = cross.length_squared().sqrt();
        (cross * twice_area.recip(), twice_area * 0.5)
    }
}

/// Transparent policy for sampled solid-angle Target/Weighted/Green View.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetViewOptions {
    /// Deterministic samples per triangular patch.
    pub samples_per_patch: usize,
    /// Rectangular camera horizontal field of view in radians.
    pub horizontal_fov_radians: f64,
    /// Rectangular camera vertical field of view in radians.
    pub vertical_fov_radians: f64,
    /// Maximum eye-to-target distance in canonical metres.
    pub maximum_distance_meters: f64,
    /// Clearance removed at both ray endpoints.
    pub endpoint_clearance_meters: f64,
    /// Included scene categories when tracing occluders.
    pub occluder_category_mask: u64,
    /// Included target patch categories.
    pub target_category_mask: u64,
    /// Categories counted as green in Green View outputs.
    pub green_category_mask: u64,
    /// Distance where `1/(1+(d/reference)^exponent)` equals one half.
    pub distance_reference_meters: f64,
    /// Non-negative distance falloff exponent.
    pub distance_exponent: f64,
    /// Non-negative exponent applied to camera-centre cosine.
    pub direction_exponent: f64,
    /// Whether back-facing target triangles are visible.
    pub two_sided_targets: bool,
}

impl Default for TargetViewOptions {
    fn default() -> Self {
        Self {
            samples_per_patch: 16,
            horizontal_fov_radians: 90.0_f64.to_radians(),
            vertical_fov_radians: 60.0_f64.to_radians(),
            maximum_distance_meters: 1_000.0,
            endpoint_clearance_meters: 1.0e-4,
            occluder_category_mask: u64::MAX,
            target_category_mask: u64::MAX,
            green_category_mask: 0,
            distance_reference_meters: 25.0,
            distance_exponent: 2.0,
            direction_exponent: 1.0,
            two_sided_targets: false,
        }
    }
}

impl TargetViewOptions {
    fn validate(self) -> Result<Self, VisibilityError> {
        if !(1..=MAXIMUM_PATCH_SAMPLES).contains(&self.samples_per_patch) {
            return Err(VisibilityError::InvalidSampleCount);
        }
        if !valid_camera_fov(self.horizontal_fov_radians)
            || !valid_camera_fov(self.vertical_fov_radians)
        {
            return Err(VisibilityError::InvalidCameraFieldOfView);
        }
        if !self.maximum_distance_meters.is_finite() || self.maximum_distance_meters <= 0.0 {
            return Err(VisibilityError::InvalidMaximumDistance);
        }
        if !self.endpoint_clearance_meters.is_finite() || self.endpoint_clearance_meters < 0.0 {
            return Err(VisibilityError::InvalidClearance);
        }
        if !self.distance_reference_meters.is_finite()
            || self.distance_reference_meters <= 0.0
            || !self.distance_exponent.is_finite()
            || self.distance_exponent < 0.0
            || !self.direction_exponent.is_finite()
            || self.direction_exponent < 0.0
        {
            return Err(VisibilityError::InvalidWeightingPolicy);
        }
        Ok(self)
    }
}

/// Aggregated result for one observer/logical-target pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetViewEntry {
    /// Observer identifier.
    pub observer_id: SensorId,
    /// Logical target identifier.
    pub target_id: TargetId,
    /// Target category union over contributing patches.
    pub category_mask: u64,
    /// Potential in-FOV solid angle before scene occlusion.
    pub potential_solid_angle_steradians: f64,
    /// Unoccluded in-FOV solid angle.
    pub visible_solid_angle_steradians: f64,
    /// Visible divided by potential target solid angle.
    pub visibility_fraction: f64,
    /// Visible target solid angle divided by camera FOV solid angle.
    pub fov_fraction: f64,
    /// Weighted contribution divided by camera FOV solid angle.
    pub weighted_fov_score: f64,
    /// Interleaved half-sample delta for visible solid angle.
    pub convergence_delta_steradians: f64,
    /// Potential samples in range and inside FOV.
    pub eligible_sample_count: usize,
    /// Directly visible samples.
    pub visible_sample_count: usize,
    /// Object responsible for the greatest blocked solid angle.
    pub dominant_blocker_object_id: ObjectId,
    /// Dominant blocked solid angle.
    pub dominant_blocked_solid_angle_steradians: f64,
}

/// Target, weighted-quality, and green-view summary for one observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetViewSummary {
    /// Observer identifier.
    pub observer_id: SensorId,
    /// Exact rectangular camera FOV solid angle.
    pub fov_solid_angle_steradians: f64,
    /// Potential target solid angle inside the FOV.
    pub potential_target_solid_angle_steradians: f64,
    /// Visible target solid angle inside the FOV.
    pub visible_target_solid_angle_steradians: f64,
    /// Raw visible target share of the FOV.
    pub target_view_fraction: f64,
    /// Raw visibility relative only to the potential target universe.
    pub target_universe_visibility_fraction: f64,
    /// Explicit category/distance/direction weighted score normalized by FOV.
    pub weighted_view_score: f64,
    /// Visible green solid angle normalized by FOV.
    pub green_view_index: f64,
    /// Green share of all visible target solid angle.
    pub green_share_of_visible_targets: f64,
    /// Largest per-target weighted contribution.
    pub dominant_target_id: TargetId,
    /// Full-vs-interleaved-half convergence delta.
    pub convergence_delta_steradians: f64,
}

/// Immutable observer-major Target/Weighted/Green View result.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetViewResult {
    /// One entry per observer and distinct target ID.
    pub entries: Vec<TargetViewEntry>,
    /// One summary per observer.
    pub summaries: Vec<TargetViewSummary>,
    /// Sorted distinct target IDs defining entry columns.
    pub target_ids: Vec<TargetId>,
    /// Stable identity of scene, inputs, policy, and sampled outcomes.
    pub content_hash: [u8; 32],
    /// Backend, adapter, batching, transfer, precision, and fallback provenance.
    pub execution: RayQueryExecutionSummary,
}

#[derive(Clone, Copy)]
struct SampleWork {
    entry_index: usize,
    contribution: f64,
    weighted_contribution: f64,
    is_green: bool,
    parity: usize,
}

/// Computes solid-angle Target View, transparent Weighted View, and Green View.
#[allow(clippy::too_many_lines)]
pub fn analyze_target_view(
    scene: &Scene,
    observers: &[ViewObserver],
    patches: &[ViewTargetPatch],
    options: TargetViewOptions,
) -> Result<TargetViewResult, VisibilityError> {
    analyze_target_view_with_executor(scene, observers, patches, options)
}

/// Computes Target/Weighted/Green View through a backend-neutral ray executor.
#[allow(clippy::too_many_lines)]
pub fn analyze_target_view_with_executor<E: RayQueryExecutor + ?Sized>(
    executor: &E,
    observers: &[ViewObserver],
    patches: &[ViewTargetPatch],
    options: TargetViewOptions,
) -> Result<TargetViewResult, VisibilityError> {
    if observers.is_empty() || patches.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let scene = executor.canonical_scene();
    let mut execution = RayQueryExecutionSummary::from_context(executor.context());
    let options = options.validate()?;
    let mut target_ids = patches
        .iter()
        .filter(|patch| patch.category_mask & options.target_category_mask != 0)
        .map(|patch| patch.target_id)
        .collect::<Vec<_>>();
    target_ids.sort_unstable();
    target_ids.dedup();
    if target_ids.is_empty() {
        return Err(VisibilityError::EmptyInputs);
    }
    let entry_count = observers
        .len()
        .checked_mul(target_ids.len())
        .filter(|count| *count <= crate::MAXIMUM_MATRIX_ENTRIES)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let ray_capacity = observers
        .len()
        .checked_mul(patches.len())
        .and_then(|count| count.checked_mul(options.samples_per_patch))
        .filter(|count| *count <= MAXIMUM_TARGET_RAYS)
        .ok_or(VisibilityError::ResultTooLarge)?;
    let target_columns = target_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect::<BTreeMap<_, _>>();
    let mut entries = observers
        .iter()
        .flat_map(|observer| {
            target_ids.iter().map(move |target_id| TargetViewEntry {
                observer_id: observer.id,
                target_id: *target_id,
                category_mask: 0,
                potential_solid_angle_steradians: 0.0,
                visible_solid_angle_steradians: 0.0,
                visibility_fraction: 0.0,
                fov_fraction: 0.0,
                weighted_fov_score: 0.0,
                convergence_delta_steradians: 0.0,
                eligible_sample_count: 0,
                visible_sample_count: 0,
                dominant_blocker_object_id: ObjectId::new(0),
                dominant_blocked_solid_angle_steradians: 0.0,
            })
        })
        .collect::<Vec<_>>();
    debug_assert_eq!(entries.len(), entry_count);
    let mut rays = Vec::with_capacity(ray_capacity);
    let mut work = Vec::with_capacity(ray_capacity);
    let mut half_visible = vec![0.0; entry_count];
    let mut blocked = vec![BTreeMap::<ObjectId, f64>::new(); entry_count];

    for (observer_index, observer) in observers.iter().enumerate() {
        let right = observer
            .forward
            .cross(observer.up)
            .normalized()
            .ok_or(VisibilityError::InvalidDirection)?;
        for patch in patches {
            if patch.category_mask & options.target_category_mask == 0 {
                continue;
            }
            let target_index = target_columns[&patch.target_id];
            let entry_index = observer_index * target_ids.len() + target_index;
            entries[entry_index].category_mask |= patch.category_mask;
            let (normal, _area) = patch.normal_and_area();
            let centroid =
                (patch.vertices[0] + patch.vertices[1] + patch.vertices[2]) * (1.0 / 3.0);
            let target_to_eye = observer.position - centroid;
            if !options.two_sided_targets && normal.dot(target_to_eye) <= 0.0 {
                continue;
            }
            let exact_solid_angle = triangle_solid_angle(observer.position, patch.vertices);
            if exact_solid_angle <= 0.0 {
                continue;
            }
            let samples = (0..options.samples_per_patch)
                .map(|index| triangle_sample(patch.vertices, index, options.samples_per_patch))
                .collect::<Vec<_>>();
            let raw_weights = samples
                .iter()
                .map(|point| {
                    let delta = *point - observer.position;
                    let distance_squared = delta.length_squared();
                    if distance_squared <= f64::EPSILON {
                        0.0
                    } else {
                        let direction = delta * distance_squared.sqrt().recip();
                        normal.dot(direction * -1.0).abs() / distance_squared
                    }
                })
                .collect::<Vec<_>>();
            let raw_sum: f64 = raw_weights.iter().sum();
            if raw_sum <= f64::EPSILON {
                continue;
            }
            for (sample_index, (point, raw_weight)) in
                samples.into_iter().zip(raw_weights).enumerate()
            {
                let delta = point - observer.position;
                let distance = delta.length_squared().sqrt();
                if distance <= options.endpoint_clearance_meters * 2.0
                    || distance > options.maximum_distance_meters
                {
                    continue;
                }
                let direction = delta * distance.recip();
                if !inside_rectangular_fov(
                    direction,
                    observer.forward,
                    observer.up,
                    right,
                    options.horizontal_fov_radians,
                    options.vertical_fov_radians,
                ) {
                    continue;
                }
                let contribution = exact_solid_angle * raw_weight / raw_sum;
                let distance_ratio = distance / options.distance_reference_meters;
                let distance_factor = 1.0 / (1.0 + distance_ratio.powf(options.distance_exponent));
                let direction_factor = observer
                    .forward
                    .dot(direction)
                    .max(0.0)
                    .powf(options.direction_exponent);
                let weighted = contribution
                    * patch.category_weight
                    * distance_factor
                    * direction_factor
                    * observer.weight;
                entries[entry_index].potential_solid_angle_steradians += contribution;
                entries[entry_index].eligible_sample_count += 1;
                rays.push(
                    QueryRay::try_new(
                        observer.position,
                        direction,
                        options.endpoint_clearance_meters,
                        distance - options.endpoint_clearance_meters,
                        options.occluder_category_mask,
                    )
                    .map_err(|_| VisibilityError::InvalidQuery)?,
                );
                work.push(SampleWork {
                    entry_index,
                    contribution,
                    weighted_contribution: weighted,
                    is_green: patch.category_mask & options.green_category_mask != 0,
                    parity: sample_index & 1,
                });
            }
        }
    }

    let batch = executor
        .trace_closest(&rays)
        .map_err(|_| VisibilityError::ExecutionFailed)?;
    execution.record(&batch.stats);
    let hits = batch.hits;
    let mut green_by_observer = vec![0.0; observers.len()];
    let mut weighted_by_observer = vec![0.0; observers.len()];
    for (sample, hit) in work.into_iter().zip(hits) {
        if hit.hit {
            *blocked[sample.entry_index]
                .entry(hit.object_id)
                .or_default() += sample.contribution;
            continue;
        }
        let entry = &mut entries[sample.entry_index];
        entry.visible_solid_angle_steradians += sample.contribution;
        entry.weighted_fov_score += sample.weighted_contribution;
        entry.visible_sample_count += 1;
        if sample.parity == 0 {
            half_visible[sample.entry_index] = sample
                .contribution
                .mul_add(2.0, half_visible[sample.entry_index]);
        }
        let observer_index = sample.entry_index / target_ids.len();
        weighted_by_observer[observer_index] += sample.weighted_contribution;
        if sample.is_green {
            green_by_observer[observer_index] += sample.contribution;
        }
    }

    let fov_solid_angle =
        rectangular_fov_solid_angle(options.horizontal_fov_radians, options.vertical_fov_radians);
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.visibility_fraction = safe_ratio(
            entry.visible_solid_angle_steradians,
            entry.potential_solid_angle_steradians,
        );
        entry.fov_fraction = entry.visible_solid_angle_steradians / fov_solid_angle;
        entry.weighted_fov_score /= fov_solid_angle;
        entry.convergence_delta_steradians =
            (entry.visible_solid_angle_steradians - half_visible[index]).abs();
        if let Some((id, value)) = blocked[index]
            .iter()
            .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(left.0)))
        {
            entry.dominant_blocker_object_id = *id;
            entry.dominant_blocked_solid_angle_steradians = *value;
        }
    }
    let summaries = observers
        .iter()
        .enumerate()
        .map(|(observer_index, observer)| {
            let row = &entries
                [observer_index * target_ids.len()..(observer_index + 1) * target_ids.len()];
            let potential: f64 = row
                .iter()
                .map(|entry| entry.potential_solid_angle_steradians)
                .sum();
            let visible: f64 = row
                .iter()
                .map(|entry| entry.visible_solid_angle_steradians)
                .sum();
            let dominant_target_id = row
                .iter()
                .max_by(|left, right| {
                    left.weighted_fov_score
                        .total_cmp(&right.weighted_fov_score)
                        .then_with(|| right.target_id.cmp(&left.target_id))
                })
                .map_or(TargetId::new(0), |entry| entry.target_id);
            TargetViewSummary {
                observer_id: observer.id,
                fov_solid_angle_steradians: fov_solid_angle,
                potential_target_solid_angle_steradians: potential,
                visible_target_solid_angle_steradians: visible,
                target_view_fraction: visible / fov_solid_angle,
                target_universe_visibility_fraction: safe_ratio(visible, potential),
                weighted_view_score: weighted_by_observer[observer_index] / fov_solid_angle,
                green_view_index: green_by_observer[observer_index] / fov_solid_angle,
                green_share_of_visible_targets: safe_ratio(
                    green_by_observer[observer_index],
                    visible,
                ),
                dominant_target_id,
                convergence_delta_steradians: row
                    .iter()
                    .map(|entry| entry.convergence_delta_steradians)
                    .sum(),
            }
        })
        .collect::<Vec<_>>();
    let content_hash = target_view_hash(scene, observers, patches, options, &entries);
    Ok(TargetViewResult {
        entries,
        summaries,
        target_ids,
        content_hash,
        execution,
    })
}

fn valid_camera_fov(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < PI
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator <= f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

fn rectangular_fov_solid_angle(horizontal: f64, vertical: f64) -> f64 {
    let horizontal_tangent = (horizontal * 0.5).tan();
    let vertical_tangent = (vertical * 0.5).tan();
    4.0 * (horizontal_tangent * vertical_tangent).atan2(
        (1.0 + horizontal_tangent.mul_add(horizontal_tangent, vertical_tangent * vertical_tangent))
            .sqrt(),
    )
}

fn inside_rectangular_fov(
    direction: Vec3,
    forward: Vec3,
    up: Vec3,
    right: Vec3,
    horizontal: f64,
    vertical: f64,
) -> bool {
    let depth = forward.dot(direction);
    depth > 0.0
        && right.dot(direction).abs() <= depth * (horizontal * 0.5).tan()
        && up.dot(direction).abs() <= depth * (vertical * 0.5).tan()
}

fn triangle_sample(vertices: [Vec3; 3], index: usize, count: usize) -> Vec3 {
    let first = (f64::from(u32::try_from(index).expect("sample cap fits u32")) + 0.5)
        / f64::from(u32::try_from(count).expect("sample cap fits u32"));
    let second = radical_inverse_base_two(u32::try_from(index).expect("sample cap fits u32"));
    let root = first.sqrt();
    let first_weight = 1.0 - root;
    let second_weight = root * (1.0 - second);
    let third_weight = root * second;
    vertices[0] * first_weight + vertices[1] * second_weight + vertices[2] * third_weight
}

fn radical_inverse_base_two(value: u32) -> f64 {
    f64::from(value.reverse_bits()) * (1.0 / 4_294_967_296.0)
}

fn triangle_solid_angle(eye: Vec3, vertices: [Vec3; 3]) -> f64 {
    let a = vertices[0] - eye;
    let b = vertices[1] - eye;
    let c = vertices[2] - eye;
    let la = a.length_squared().sqrt();
    let lb = b.length_squared().sqrt();
    let lc = c.length_squared().sqrt();
    if la <= f64::EPSILON || lb <= f64::EPSILON || lc <= f64::EPSILON {
        return 0.0;
    }
    let numerator = a.dot(b.cross(c)).abs();
    let denominator = c
        .dot(a)
        .mul_add(lb, b.dot(c).mul_add(la, a.dot(b).mul_add(lc, la * lb * lc)));
    2.0 * numerator.atan2(denominator).abs()
}

fn target_view_hash(
    scene: &Scene,
    observers: &[ViewObserver],
    patches: &[ViewTargetPatch],
    options: TargetViewOptions,
    entries: &[TargetViewEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAENA_TARGET_VIEW_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(
        &u64::try_from(options.samples_per_patch)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for value in [
        options.horizontal_fov_radians,
        options.vertical_fov_radians,
        options.maximum_distance_meters,
        options.endpoint_clearance_meters,
        options.distance_reference_meters,
        options.distance_exponent,
        options.direction_exponent,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for value in [
        options.occluder_category_mask,
        options.target_category_mask,
        options.green_category_mask,
    ] {
        hasher.update(&value.to_le_bytes());
    }
    hasher.update(&[u8::from(options.two_sided_targets)]);
    for observer in observers {
        hasher.update(&observer.id.get().to_le_bytes());
        hash_vec3(&mut hasher, observer.position);
        hash_vec3(&mut hasher, observer.forward);
        hash_vec3(&mut hasher, observer.up);
        hasher.update(&observer.weight.to_bits().to_le_bytes());
    }
    for patch in patches {
        hasher.update(&patch.target_id.get().to_le_bytes());
        for vertex in patch.vertices {
            hash_vec3(&mut hasher, vertex);
        }
        hasher.update(&patch.category_mask.to_le_bytes());
        hasher.update(&patch.category_weight.to_bits().to_le_bytes());
    }
    for entry in entries {
        hasher.update(&entry.visible_solid_angle_steradians.to_bits().to_le_bytes());
        hasher.update(&entry.dominant_blocker_object_id.get().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_vec3(hasher: &mut blake3::Hasher, value: Vec3) {
    for component in [value.x, value.y, value.z] {
        hasher.update(&component.to_bits().to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::InstanceId;

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
    fn target_view_reports_solid_angle_weights_and_green_share() {
        let scene = distant_scene();
        let observer = ViewObserver::try_new(
            SensorId::new(1),
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
        )
        .unwrap();
        let patch = ViewTargetPatch::try_new(
            TargetId::new(7),
            [
                Vec3::new(5.0, -1.0, -1.0),
                Vec3::new(5.0, 0.0, 1.0),
                Vec3::new(5.0, 1.0, -1.0),
            ],
            2,
            0.8,
        )
        .unwrap();
        let result = analyze_target_view(
            &scene,
            &[observer],
            &[patch],
            TargetViewOptions {
                samples_per_patch: 64,
                green_category_mask: 2,
                two_sided_targets: true,
                ..TargetViewOptions::default()
            },
        )
        .unwrap();
        let summary = result.summaries[0];
        assert!(summary.target_view_fraction > 0.0);
        assert!((summary.green_share_of_visible_targets - 1.0).abs() < 1.0e-12);
        assert!(summary.weighted_view_score < summary.target_view_fraction);
        assert_eq!(summary.dominant_target_id, TargetId::new(7));
    }

    #[test]
    fn target_view_preserves_raw_visibility_when_weights_change() {
        let scene = distant_scene();
        let observer = ViewObserver::try_new(
            SensorId::new(1),
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
        )
        .unwrap();
        let vertices = [
            Vec3::new(5.0, -1.0, -1.0),
            Vec3::new(5.0, 0.0, 1.0),
            Vec3::new(5.0, 1.0, -1.0),
        ];
        let low = ViewTargetPatch::try_new(TargetId::new(1), vertices, 1, 0.1).unwrap();
        let high = ViewTargetPatch::try_new(TargetId::new(1), vertices, 1, 1.0).unwrap();
        let options = TargetViewOptions {
            two_sided_targets: true,
            ..TargetViewOptions::default()
        };
        let low_result = analyze_target_view(&scene, &[observer], &[low], options).unwrap();
        let high_result = analyze_target_view(&scene, &[observer], &[high], options).unwrap();
        assert!(
            (low_result.summaries[0].target_view_fraction
                - high_result.summaries[0].target_view_fraction)
                .abs()
                < f64::EPSILON
        );
        assert!(
            low_result.summaries[0].weighted_view_score
                < high_result.summaries[0].weighted_view_score
        );
    }
}
