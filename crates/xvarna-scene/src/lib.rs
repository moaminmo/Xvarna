//! Canonical scene snapshots, instancing, and CPU ray queries for XVARNA.

#![forbid(unsafe_code)]

mod bvh;
mod document;
mod executor;
mod material;
mod math;
mod portable;

pub use document::{
    SCENE_DOCUMENT_SCHEMA, SCENE_DOCUMENT_VERSION, SceneDocument, SceneDocumentBuildOptions,
    SceneDocumentError, SceneDocumentInstance, SceneDocumentLayer, SceneDocumentMesh,
};
pub use executor::{
    RayQueryBackend, RayQueryBatch, RayQueryBatchStats, RayQueryContext, RayQueryExecutionError,
    RayQueryExecutionSummary, RayQueryExecutor,
};
pub use material::{
    AnalysisMaterial, MaterialError, MaterialId, MaterialLibrary, TransmissionChannel,
    TransmissionLayer, TransmissionTrace,
};

use bvh::{Bvh, Primitive};
use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
use std::{collections::HashSet, fmt, mem, sync::Arc, time::Instant};
use xvarna_geometry::{Aabb, Mesh, MeshAuditOptions, Ray, Vec3, audit_mesh};
use xvarna_types::{InstanceId, MeshId, ObjectId};

pub use math::{Transform, TransformError};
pub use portable::{
    PortableBvhNode, PortableSceneError, PortableSceneOptions, PortableSceneSnapshot,
    PortableSceneStats, PortableTriangle,
};

const ALL_CATEGORIES: u64 = u64::MAX;

/// Configuration used to compile an immutable CPU scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneBuildOptions {
    /// Source model-unit scale in metres.
    pub unit_scale_to_meters: f64,
    /// Absolute canonical tolerance in metres.
    pub absolute_tolerance_meters: f64,
    /// Maximum primitives per BVH leaf.
    pub maximum_leaf_size: usize,
    /// Worker threads; zero selects the Rayon default.
    pub thread_count: usize,
}

impl Default for SceneBuildOptions {
    fn default() -> Self {
        Self {
            unit_scale_to_meters: 1.0,
            absolute_tolerance_meters: 1.0e-6,
            maximum_leaf_size: 4,
            thread_count: 0,
        }
    }
}

/// Acceleration layer controlling which hierarchy may be updated interactively.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u32)]
pub enum SceneLayer {
    /// Geometry expected to remain stable across design variants.
    Static = 0,
    /// Geometry whose transform, metadata, or membership may change interactively.
    Dynamic = 1,
}

/// Policy deciding when a cheap TLAS refit must be replaced by a full rebuild.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneUpdatePolicy {
    /// Maximum new/old surface-area cost accepted after refit.
    pub maximum_refit_quality_ratio: f64,
    /// Maximum consecutive refits before a deterministic rebuild.
    pub maximum_consecutive_refits: u32,
}

impl Default for SceneUpdatePolicy {
    fn default() -> Self {
        Self {
            maximum_refit_quality_ratio: 1.35,
            maximum_consecutive_refits: 32,
        }
    }
}

/// Hierarchy action selected for one scene layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HierarchyUpdateKind {
    /// Existing hierarchy and bounds remained valid.
    Reused = 0,
    /// Topology was preserved and node bounds were refitted bottom-up.
    Refit = 1,
    /// A fresh binned-SAH hierarchy was built.
    Rebuilt = 2,
}

/// One atomic lifecycle change. Successful application always produces a new immutable snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneDelta {
    /// Changes selected fields of one stable instance.
    UpdateInstance {
        /// Stable occurrence identifier.
        instance_id: InstanceId,
        /// Optional source-model transform replacement.
        transform: Option<Transform>,
        /// Optional object identifier replacement.
        object_id: Option<ObjectId>,
        /// Optional category replacement; zero is normalized to all categories.
        category_mask: Option<u64>,
        /// Optional static/dynamic layer migration.
        layer: Option<SceneLayer>,
    },
    /// Adds an occurrence of an existing shared mesh resource.
    AddInstance {
        /// Existing mesh resource.
        mesh_id: MeshId,
        /// Source-model affine transform.
        transform: Transform,
        /// Stable object identifier.
        object_id: ObjectId,
        /// Stable occurrence identifier.
        instance_id: InstanceId,
        /// Category bits; zero means all.
        category_mask: u64,
        /// Static or dynamic hierarchy.
        layer: SceneLayer,
    },
    /// Removes one occurrence by stable identifier.
    RemoveInstance {
        /// Stable occurrence identifier.
        instance_id: InstanceId,
    },
    /// Replaces one shared mesh and rebuilds only that resource's BLAS.
    ReplaceMesh {
        /// Stable resource identifier.
        mesh_id: MeshId,
        /// Replacement mesh in source model units.
        mesh: Mesh,
    },
}

/// Complete provenance for an incremental scene transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneUpdateReport {
    /// Number of requested delta records.
    pub delta_count: usize,
    /// Static occurrences whose data or membership changed.
    pub static_change_count: usize,
    /// Dynamic occurrences whose data or membership changed.
    pub dynamic_change_count: usize,
    /// BLAS resources reused without rebuilding.
    pub reused_blas_count: usize,
    /// BLAS resources rebuilt due to geometry replacement.
    pub rebuilt_blas_count: usize,
    /// Static TLAS action.
    pub static_tlas_update: HierarchyUpdateKind,
    /// Dynamic TLAS action.
    pub dynamic_tlas_update: HierarchyUpdateKind,
    /// Worst accepted refit quality ratio; one for pure reuse/rebuild.
    pub maximum_refit_quality_ratio: f64,
    /// Total transition time in microseconds.
    pub update_time_microseconds: u64,
    /// Content identity before the transition.
    pub previous_hash: [u8; 32],
    /// Content identity after the transition.
    pub current_hash: [u8; 32],
}

/// Failure while compiling or querying a scene.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneError {
    /// Unit scale is non-finite or not positive.
    InvalidUnitScale,
    /// Canonical tolerance is non-finite or not positive.
    InvalidTolerance,
    /// BVH leaf size is outside the supported range 1..=64.
    InvalidLeafSize,
    /// The mesh requires repair before scene ingest.
    MeshRequiresRepair {
        /// Number of source faces requiring removal.
        face_count: usize,
    },
    /// A referenced mesh resource does not exist.
    UnknownMesh(MeshId),
    /// No mesh resource was added.
    EmptyResources,
    /// No instance was added.
    EmptyInstances,
    /// The worker pool could not be created.
    ThreadPool(String),
    /// The query ray is invalid.
    InvalidRay,
    /// An instance transform is non-finite, projective, or singular.
    InvalidTransform,
    /// More than one occurrence uses the same stable instance identifier.
    DuplicateInstance(InstanceId),
    /// A lifecycle delta references an occurrence that does not exist.
    UnknownInstance(InstanceId),
    /// The scene update policy is non-finite or outside its supported range.
    InvalidUpdatePolicy,
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUnitScale => formatter.write_str("unit scale must be finite and positive"),
            Self::InvalidTolerance => formatter.write_str("tolerance must be finite and positive"),
            Self::InvalidLeafSize => formatter.write_str("BVH leaf size must be between 1 and 64"),
            Self::MeshRequiresRepair { face_count } => {
                write!(
                    formatter,
                    "mesh contains {face_count} face(s) requiring repair"
                )
            }
            Self::UnknownMesh(id) => write!(formatter, "mesh resource {id} does not exist"),
            Self::EmptyResources => formatter.write_str("scene contains no mesh resources"),
            Self::EmptyInstances => formatter.write_str("scene contains no instances"),
            Self::ThreadPool(message) => {
                write!(formatter, "worker pool creation failed: {message}")
            }
            Self::InvalidRay => {
                formatter.write_str("ray is non-finite, zero-length, or has invalid bounds")
            }
            Self::InvalidTransform => {
                formatter.write_str("instance transform must be finite, affine, and invertible")
            }
            Self::DuplicateInstance(id) => {
                write!(formatter, "instance identifier {id} is duplicated")
            }
            Self::UnknownInstance(id) => {
                write!(formatter, "instance identifier {id} does not exist")
            }
            Self::InvalidUpdatePolicy => formatter.write_str(
                "refit quality ratio must be finite and at least one; refit limit must be positive",
            ),
        }
    }
}

impl std::error::Error for SceneError {}

/// Normalized ray query in canonical metre coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryRay {
    /// Origin in canonical metres.
    pub origin: Vec3,
    /// Unit direction.
    pub direction: Vec3,
    /// Inclusive minimum distance in metres.
    pub t_min: f64,
    /// Inclusive maximum distance in metres.
    pub t_max: f64,
    /// Instance category bit mask.
    pub category_mask: u64,
}

impl QueryRay {
    /// Validates and creates a query ray.
    pub fn try_new(
        origin: Vec3,
        direction: Vec3,
        t_min: f64,
        t_max: f64,
        category_mask: u64,
    ) -> Result<Self, SceneError> {
        let ray =
            Ray::try_new(origin, direction, t_min, t_max).map_err(|_| SceneError::InvalidRay)?;
        Ok(Self {
            origin: ray.origin,
            direction: ray.direction,
            t_min: ray.t_min,
            t_max: ray.t_max,
            category_mask: if category_mask == 0 {
                ALL_CATEGORIES
            } else {
                category_mask
            },
        })
    }
}

/// Closest-hit record with stable source attribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// True when geometry was hit.
    pub hit: bool,
    /// Canonical distance in metres, or infinity for a miss.
    pub distance: f64,
    /// First barycentric coordinate.
    pub barycentric_u: f64,
    /// Second barycentric coordinate.
    pub barycentric_v: f64,
    /// Stable source object identifier.
    pub object_id: ObjectId,
    /// Stable occurrence identifier.
    pub instance_id: InstanceId,
    /// Shared mesh resource identifier.
    pub mesh_id: MeshId,
    /// Local triangle index.
    pub triangle_id: u32,
    /// True when the world-space ray hit the front side.
    pub front_face: bool,
}

impl Hit {
    /// Miss sentinel.
    #[must_use]
    pub const fn miss() -> Self {
        Self {
            hit: false,
            distance: f64::INFINITY,
            barycentric_u: 0.0,
            barycentric_v: 0.0,
            object_id: ObjectId::new(0),
            instance_id: InstanceId::new(0),
            mesh_id: MeshId::new(0),
            triangle_id: u32::MAX,
            front_face: false,
        }
    }
}

/// Build and memory statistics for an immutable scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneStats {
    /// Unique mesh resources after content deduplication.
    pub mesh_resource_count: usize,
    /// Number of geometry instances.
    pub instance_count: usize,
    /// Unique triangles stored once across resources.
    pub unique_triangle_count: usize,
    /// Effective triangles after applying instances.
    pub instanced_triangle_count: usize,
    /// Total BLAS node count.
    pub blas_node_count: usize,
    /// TLAS node count.
    pub tlas_node_count: usize,
    /// Maximum depth among all acceleration structures.
    pub maximum_bvh_depth: usize,
    /// Approximate owned CPU bytes.
    pub approximate_memory_bytes: usize,
    /// Total compile time in microseconds.
    pub build_time_microseconds: u64,
    /// Worker threads assigned to batch queries.
    pub thread_count: usize,
    /// Canonical origin subtracted internally from world coordinates.
    pub rebase_origin_meters: Vec3,
    /// Full deterministic scene hash.
    pub content_hash: [u8; 32],
}

/// One canonical world-space triangle exported for interoperable analysis engines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTriangle {
    /// First vertex in canonical metres.
    pub first: Vec3,
    /// Second vertex in canonical metres.
    pub second: Vec3,
    /// Third vertex in canonical metres.
    pub third: Vec3,
    /// Stable source object identifier.
    pub object_id: ObjectId,
    /// Stable source occurrence identifier.
    pub instance_id: InstanceId,
    /// Stable source mesh identifier.
    pub mesh_id: MeshId,
    /// Triangle index inside the source mesh.
    pub triangle_id: u32,
}

#[derive(Clone, Debug)]
struct MeshResource {
    id: MeshId,
    mesh: Mesh,
    bounds: Aabb,
    content_hash: [u8; 32],
    bvh: Bvh,
}

#[derive(Clone, Copy, Debug)]
struct InstanceDraft {
    mesh_index: usize,
    object_id: ObjectId,
    instance_id: InstanceId,
    transform: Transform,
    category_mask: u64,
    layer: SceneLayer,
}

#[derive(Clone, Copy, Debug)]
struct Instance {
    mesh_index: usize,
    object_id: ObjectId,
    id: InstanceId,
    inverse_transform: Transform,
    mirrored: bool,
    category_mask: u64,
    bounds: Aabb,
    layer: SceneLayer,
}

/// Mutable description used to compile an immutable [`Scene`].
pub struct SceneBuilder {
    options: SceneBuildOptions,
    resources: Vec<Arc<MeshResource>>,
    instances: Vec<InstanceDraft>,
    started: Instant,
}

impl SceneBuilder {
    /// Creates a validated scene builder.
    pub fn new(options: SceneBuildOptions) -> Result<Self, SceneError> {
        if !options.unit_scale_to_meters.is_finite() || options.unit_scale_to_meters <= 0.0 {
            return Err(SceneError::InvalidUnitScale);
        }
        if !options.absolute_tolerance_meters.is_finite()
            || options.absolute_tolerance_meters <= 0.0
        {
            return Err(SceneError::InvalidTolerance);
        }
        if !(1..=64).contains(&options.maximum_leaf_size) {
            return Err(SceneError::InvalidLeafSize);
        }
        Ok(Self {
            options,
            resources: Vec::new(),
            instances: Vec::new(),
            started: Instant::now(),
        })
    }

    /// Adds or deduplicates a mesh resource supplied in source model units.
    pub fn add_mesh(&mut self, mesh: Mesh) -> Result<MeshId, SceneError> {
        let next_id = MeshId::new(
            u64::try_from(self.resources.len()).expect("mesh resource count fits u64") + 1,
        );
        let resource = compile_resource(mesh, self.options, next_id)?;
        if let Some(existing) = self.resources.iter().find(|existing| {
            existing.content_hash == resource.content_hash && existing.mesh == resource.mesh
        }) {
            return Ok(existing.id);
        }
        let id = resource.id;
        self.resources.push(Arc::new(resource));
        Ok(id)
    }

    /// Adds an occurrence of a mesh resource using a source-model transform.
    pub fn add_instance(
        &mut self,
        mesh_id: MeshId,
        transform: Transform,
        object_id: ObjectId,
        instance_id: InstanceId,
        category_mask: u64,
    ) -> Result<(), SceneError> {
        self.add_instance_in_layer(
            mesh_id,
            transform,
            object_id,
            instance_id,
            category_mask,
            SceneLayer::Static,
        )
    }

    /// Adds an occurrence to the explicit static or dynamic acceleration layer.
    pub fn add_instance_in_layer(
        &mut self,
        mesh_id: MeshId,
        transform: Transform,
        object_id: ObjectId,
        instance_id: InstanceId,
        category_mask: u64,
        layer: SceneLayer,
    ) -> Result<(), SceneError> {
        let mesh_index = self
            .resources
            .iter()
            .position(|resource| resource.id == mesh_id)
            .ok_or(SceneError::UnknownMesh(mesh_id))?;
        if self
            .instances
            .iter()
            .any(|instance| instance.instance_id == instance_id)
        {
            return Err(SceneError::DuplicateInstance(instance_id));
        }
        self.instances.push(InstanceDraft {
            mesh_index,
            object_id,
            instance_id,
            transform: transform.with_translation_scale(self.options.unit_scale_to_meters),
            category_mask: if category_mask == 0 {
                ALL_CATEGORIES
            } else {
                category_mask
            },
            layer,
        });
        Ok(())
    }

    /// Compiles BLAS/TLAS data and freezes the scene snapshot.
    pub fn build(self) -> Result<Scene, SceneError> {
        if self.resources.is_empty() {
            return Err(SceneError::EmptyResources);
        }
        if self.instances.is_empty() {
            return Err(SceneError::EmptyInstances);
        }

        compile_scene(
            self.options,
            self.resources,
            self.instances,
            None,
            u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX),
        )
        .map(|value| value.scene)
    }
}

/// Immutable, thread-safe CPU scene snapshot.
pub struct Scene {
    options: SceneBuildOptions,
    resources: Vec<Arc<MeshResource>>,
    source_instances: Vec<InstanceDraft>,
    instances: Vec<Instance>,
    static_tlas: Bvh,
    dynamic_tlas: Bvh,
    static_refit_count: u32,
    dynamic_refit_count: u32,
    thread_pool: ThreadPool,
    stats: SceneStats,
}

impl Scene {
    /// Returns immutable build and memory statistics.
    #[must_use]
    pub const fn stats(&self) -> SceneStats {
        self.stats
    }

    /// Expands immutable instances into deterministic canonical world-space triangles.
    ///
    /// This is an interoperability path for exporters such as Radiance. The ray engine keeps
    /// shared BLAS geometry and does not use this expanded representation for traversal.
    #[must_use]
    pub fn world_triangles(&self) -> Vec<WorldTriangle> {
        let mut output = Vec::with_capacity(self.stats.instanced_triangle_count);
        for instance in &self.source_instances {
            let resource = &self.resources[instance.mesh_index];
            for (triangle_index, triangle) in resource.mesh.triangles.iter().copied().enumerate() {
                output.push(WorldTriangle {
                    first: instance
                        .transform
                        .transform_point(resource.mesh.positions[triangle[0] as usize]),
                    second: instance
                        .transform
                        .transform_point(resource.mesh.positions[triangle[1] as usize]),
                    third: instance
                        .transform
                        .transform_point(resource.mesh.positions[triangle[2] as usize]),
                    object_id: instance.object_id,
                    instance_id: instance.instance_id,
                    mesh_id: resource.id,
                    triangle_id: u32::try_from(triangle_index).unwrap_or(u32::MAX),
                });
            }
        }
        output
    }

    /// Applies an ordered delta transaction and returns a new immutable snapshot plus provenance.
    #[allow(clippy::too_many_lines)]
    pub fn apply_deltas(
        &self,
        deltas: &[SceneDelta],
        policy: SceneUpdatePolicy,
    ) -> Result<(Self, SceneUpdateReport), SceneError> {
        if !policy.maximum_refit_quality_ratio.is_finite()
            || policy.maximum_refit_quality_ratio < 1.0
            || policy.maximum_consecutive_refits == 0
        {
            return Err(SceneError::InvalidUpdatePolicy);
        }
        let started = Instant::now();
        let mut resources = self.resources.clone();
        let mut drafts = self.source_instances.clone();
        let mut static_change_count = 0_usize;
        let mut dynamic_change_count = 0_usize;
        let mut static_change = LayerChange::default();
        let mut dynamic_change = LayerChange::default();
        let mut rebuilt_resources = HashSet::new();

        for delta in deltas {
            match delta {
                SceneDelta::UpdateInstance {
                    instance_id,
                    transform,
                    object_id,
                    category_mask,
                    layer,
                } => {
                    let draft = drafts
                        .iter_mut()
                        .find(|draft| draft.instance_id == *instance_id)
                        .ok_or(SceneError::UnknownInstance(*instance_id))?;
                    let previous_layer = draft.layer;
                    let mut bounds_changed = false;
                    let mut changed = false;
                    if let Some(value) = transform {
                        let value = value.with_translation_scale(self.options.unit_scale_to_meters);
                        bounds_changed |= draft.transform != value;
                        changed |= draft.transform != value;
                        draft.transform = value;
                    }
                    if let Some(value) = object_id {
                        changed |= draft.object_id != *value;
                        draft.object_id = *value;
                    }
                    if let Some(value) = category_mask {
                        let value = normalize_category(*value);
                        changed |= draft.category_mask != value;
                        draft.category_mask = value;
                    }
                    if let Some(value) = layer {
                        changed |= draft.layer != *value;
                        draft.layer = *value;
                    }
                    if changed {
                        mark_layer_change(
                            previous_layer,
                            bounds_changed,
                            previous_layer != draft.layer,
                            &mut static_change,
                            &mut dynamic_change,
                            &mut static_change_count,
                            &mut dynamic_change_count,
                        );
                        if previous_layer != draft.layer {
                            mark_layer_change(
                                draft.layer,
                                true,
                                true,
                                &mut static_change,
                                &mut dynamic_change,
                                &mut static_change_count,
                                &mut dynamic_change_count,
                            );
                        }
                    }
                }
                SceneDelta::AddInstance {
                    mesh_id,
                    transform,
                    object_id,
                    instance_id,
                    category_mask,
                    layer,
                } => {
                    if drafts.iter().any(|draft| draft.instance_id == *instance_id) {
                        return Err(SceneError::DuplicateInstance(*instance_id));
                    }
                    let mesh_index = resources
                        .iter()
                        .position(|resource| resource.id == *mesh_id)
                        .ok_or(SceneError::UnknownMesh(*mesh_id))?;
                    drafts.push(InstanceDraft {
                        mesh_index,
                        object_id: *object_id,
                        instance_id: *instance_id,
                        transform: transform
                            .with_translation_scale(self.options.unit_scale_to_meters),
                        category_mask: normalize_category(*category_mask),
                        layer: *layer,
                    });
                    mark_layer_change(
                        *layer,
                        true,
                        true,
                        &mut static_change,
                        &mut dynamic_change,
                        &mut static_change_count,
                        &mut dynamic_change_count,
                    );
                }
                SceneDelta::RemoveInstance { instance_id } => {
                    let index = drafts
                        .iter()
                        .position(|draft| draft.instance_id == *instance_id)
                        .ok_or(SceneError::UnknownInstance(*instance_id))?;
                    let layer = drafts[index].layer;
                    drafts.remove(index);
                    mark_layer_change(
                        layer,
                        true,
                        true,
                        &mut static_change,
                        &mut dynamic_change,
                        &mut static_change_count,
                        &mut dynamic_change_count,
                    );
                }
                SceneDelta::ReplaceMesh { mesh_id, mesh } => {
                    let resource_index = resources
                        .iter()
                        .position(|resource| resource.id == *mesh_id)
                        .ok_or(SceneError::UnknownMesh(*mesh_id))?;
                    let replacement =
                        Arc::new(compile_resource(mesh.clone(), self.options, *mesh_id)?);
                    if replacement.content_hash == resources[resource_index].content_hash
                        && replacement.mesh == resources[resource_index].mesh
                    {
                        continue;
                    }
                    resources[resource_index] = replacement;
                    rebuilt_resources.insert(resource_index);
                    for draft in drafts
                        .iter()
                        .filter(|draft| draft.mesh_index == resource_index)
                    {
                        mark_layer_change(
                            draft.layer,
                            true,
                            false,
                            &mut static_change,
                            &mut dynamic_change,
                            &mut static_change_count,
                            &mut dynamic_change_count,
                        );
                    }
                }
            }
        }
        if drafts.is_empty() {
            return Err(SceneError::EmptyInstances);
        }

        let build_time = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let outcome = compile_scene(
            self.options,
            resources,
            drafts,
            Some(PreviousCompile {
                scene: self,
                policy,
                static_change,
                dynamic_change,
            }),
            build_time,
        )?;
        let update_time_microseconds =
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let report = SceneUpdateReport {
            delta_count: deltas.len(),
            static_change_count,
            dynamic_change_count,
            reused_blas_count: outcome
                .scene
                .resources
                .len()
                .saturating_sub(rebuilt_resources.len()),
            rebuilt_blas_count: rebuilt_resources.len(),
            static_tlas_update: outcome.static_kind,
            dynamic_tlas_update: outcome.dynamic_kind,
            maximum_refit_quality_ratio: outcome.maximum_quality_ratio,
            update_time_microseconds,
            previous_hash: self.stats.content_hash,
            current_hash: outcome.scene.stats.content_hash,
        };
        Ok((outcome.scene, report))
    }

    /// Traces one closest-hit query.
    #[must_use]
    pub fn trace_closest(&self, ray: QueryRay) -> Hit {
        self.trace_internal(ray, false, None)
    }

    /// Traces one visibility/any-hit query.
    #[must_use]
    pub fn trace_any(&self, ray: QueryRay) -> bool {
        self.trace_internal(ray, true, None).hit
    }

    /// Traces an ordered closest-hit batch using the scene worker pool.
    #[must_use]
    pub fn trace_closest_batch(&self, rays: &[QueryRay]) -> Vec<Hit> {
        if rays.len() < 256 || self.thread_pool.current_num_threads() == 1 {
            return rays.iter().map(|ray| self.trace_closest(*ray)).collect();
        }
        self.thread_pool.install(|| {
            rays.par_iter()
                .map(|ray| self.trace_closest(*ray))
                .collect()
        })
    }

    /// Traces an ordered any-hit batch using the scene worker pool.
    #[must_use]
    pub fn trace_any_batch(&self, rays: &[QueryRay]) -> Vec<bool> {
        if rays.len() < 256 || self.thread_pool.current_num_threads() == 1 {
            return rays.iter().map(|ray| self.trace_any(*ray)).collect();
        }
        self.thread_pool
            .install(|| rays.par_iter().map(|ray| self.trace_any(*ray)).collect())
    }

    /// Traces one closest hit while treating one source object as absent.
    #[must_use]
    pub fn trace_closest_excluding_object(&self, ray: QueryRay, excluded: ObjectId) -> Hit {
        self.trace_internal(ray, false, Some(excluded))
    }

    /// Returns the category mask of a scene occurrence.
    #[must_use]
    pub fn instance_category_mask(&self, instance_id: InstanceId) -> Option<u64> {
        self.instances
            .iter()
            .find(|instance| instance.id == instance_id)
            .map(|instance| instance.category_mask)
    }

    /// Traces ordered transparent layers under a validated material catalog.
    #[must_use]
    pub fn trace_transmission(
        &self,
        ray: QueryRay,
        materials: &MaterialLibrary,
        channel: TransmissionChannel,
        maximum_layers: usize,
        minimum_transmission: f64,
    ) -> TransmissionTrace {
        self.trace_transmission_internal(
            ray,
            materials,
            channel,
            maximum_layers,
            minimum_transmission,
            None,
        )
    }

    /// Traces material transmission while treating one source object as absent.
    #[must_use]
    pub fn trace_transmission_excluding_object(
        &self,
        ray: QueryRay,
        materials: &MaterialLibrary,
        channel: TransmissionChannel,
        maximum_layers: usize,
        minimum_transmission: f64,
        excluded_object: ObjectId,
    ) -> TransmissionTrace {
        self.trace_transmission_internal(
            ray,
            materials,
            channel,
            maximum_layers,
            minimum_transmission,
            Some(excluded_object),
        )
    }

    fn trace_transmission_internal(
        &self,
        ray: QueryRay,
        materials: &MaterialLibrary,
        channel: TransmissionChannel,
        maximum_layers: usize,
        minimum_transmission: f64,
        excluded_object: Option<ObjectId>,
    ) -> TransmissionTrace {
        let layer_cap = maximum_layers.clamp(1, 64);
        let cutoff = if minimum_transmission.is_finite() {
            minimum_transmission.clamp(0.0, 1.0)
        } else {
            1.0e-6
        };
        let mut transmission = 1.0;
        let mut travelled = 0.0;
        let mut origin = ray.origin;
        let mut layers = Vec::new();
        let mut layer_limit_reached = false;
        for layer_index in 0..layer_cap {
            let remaining = ray.t_max - travelled;
            if remaining <= 0.0 || transmission <= cutoff {
                break;
            }
            let minimum = if layer_index == 0 { ray.t_min } else { 0.0 };
            let query = QueryRay {
                origin,
                direction: ray.direction,
                t_min: minimum,
                t_max: remaining,
                category_mask: ray.category_mask,
            };
            let mut hit = excluded_object.map_or_else(
                || self.trace_closest(query),
                |excluded| self.trace_closest_excluding_object(query, excluded),
            );
            if !hit.hit {
                break;
            }
            let local_distance = hit.distance;
            hit.distance += travelled;
            let material = materials.material_for_object(hit.object_id);
            let coefficient = material.map_or(0.0, |value| match channel {
                TransmissionChannel::Visible => value.visible_transmittance,
                TransmissionChannel::Solar => value.solar_transmittance,
            });
            let attributed_loss = transmission * (1.0 - coefficient);
            layers.push(TransmissionLayer {
                hit,
                incident_transmission: transmission,
                attributed_loss,
                material_id: material.map_or(0, |value| value.id),
            });
            transmission *= coefficient;
            if coefficient <= 0.0 || transmission <= cutoff {
                break;
            }
            let epsilon = self.options.absolute_tolerance_meters.max(1.0e-7);
            let advance = local_distance + epsilon;
            travelled += advance;
            origin = origin + ray.direction * advance;
            if layer_index + 1 == layer_cap {
                layer_limit_reached = true;
            }
        }
        TransmissionTrace {
            transmission,
            layer_limit_reached,
            layers,
        }
    }

    fn trace_internal(
        &self,
        ray: QueryRay,
        stop_at_first: bool,
        excluded_object: Option<ObjectId>,
    ) -> Hit {
        let world_ray = RawRay {
            origin: ray.origin - self.stats.rebase_origin_meters,
            direction: ray.direction,
            t_min: ray.t_min,
            t_max: ray.t_max,
        };
        let mut best = Hit::miss();
        for tlas in [&self.static_tlas, &self.dynamic_tlas] {
            best = self.trace_tlas(
                tlas,
                ray.category_mask,
                world_ray,
                best,
                stop_at_first,
                excluded_object,
            );
            if stop_at_first && best.hit {
                break;
            }
        }
        best
    }

    fn trace_tlas(
        &self,
        tlas: &Bvh,
        category_mask: u64,
        world_ray: RawRay,
        mut best: Hit,
        stop_at_first: bool,
        excluded_object: Option<ObjectId>,
    ) -> Hit {
        if tlas.nodes.is_empty() {
            return best;
        }
        let mut stack = NodeStack::new();
        stack.push(0);
        while let Some(node_index) = stack.pop() {
            let node = tlas.nodes[node_index as usize];
            let maximum = best.distance.min(world_ray.t_max);
            if intersect_aabb(world_ray, node.bounds, maximum).is_none() {
                continue;
            }
            if node.is_leaf() {
                let start = node.start as usize;
                let end = start + node.count as usize;
                for &instance_index in &tlas.primitive_indices[start..end] {
                    let instance = self.instances[instance_index as usize];
                    if instance.category_mask & category_mask == 0
                        || excluded_object == Some(instance.object_id)
                    {
                        continue;
                    }
                    let maximum = best.distance.min(world_ray.t_max);
                    if intersect_aabb(world_ray, instance.bounds, maximum).is_none() {
                        continue;
                    }
                    let local_ray = RawRay {
                        origin: instance.inverse_transform.transform_point(world_ray.origin),
                        direction: instance
                            .inverse_transform
                            .transform_direction(world_ray.direction),
                        t_min: world_ray.t_min,
                        t_max: maximum,
                    };
                    if let Some(local_hit) = self.trace_mesh(instance.mesh_index, local_ray) {
                        let resource = &self.resources[instance.mesh_index];
                        best = Hit {
                            hit: true,
                            distance: local_hit.distance,
                            barycentric_u: local_hit.u,
                            barycentric_v: local_hit.v,
                            object_id: instance.object_id,
                            instance_id: instance.id,
                            mesh_id: resource.id,
                            triangle_id: local_hit.triangle_id,
                            front_face: if instance.mirrored {
                                !local_hit.front_face
                            } else {
                                local_hit.front_face
                            },
                        };
                        if stop_at_first {
                            return best;
                        }
                    }
                }
            } else {
                push_children_near_first(&mut stack, tlas, node, world_ray, best.distance);
            }
        }
        best
    }

    fn trace_mesh(&self, mesh_index: usize, ray: RawRay) -> Option<LocalHit> {
        let resource = &self.resources[mesh_index];
        if resource.bvh.nodes.is_empty() {
            return None;
        }
        let mut best: Option<LocalHit> = None;
        let mut stack = NodeStack::new();
        stack.push(0);
        while let Some(node_index) = stack.pop() {
            let node = resource.bvh.nodes[node_index as usize];
            let maximum = best.map_or(ray.t_max, |hit| hit.distance);
            if intersect_aabb(ray, node.bounds, maximum).is_none() {
                continue;
            }
            if node.is_leaf() {
                let start = node.start as usize;
                let end = start + node.count as usize;
                for &triangle_id in &resource.bvh.primitive_indices[start..end] {
                    let triangle = resource.mesh.triangles[triangle_id as usize];
                    let maximum = best.map_or(ray.t_max, |hit| hit.distance);
                    if let Some(hit) = intersect_triangle(
                        ray,
                        resource.mesh.positions[triangle[0] as usize],
                        resource.mesh.positions[triangle[1] as usize],
                        resource.mesh.positions[triangle[2] as usize],
                        triangle_id,
                        maximum,
                    ) {
                        best = Some(hit);
                    }
                }
            } else {
                push_children_near_first(&mut stack, &resource.bvh, node, ray, maximum);
            }
        }
        best
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LayerChange {
    changed: bool,
    bounds_changed: bool,
    topology_changed: bool,
}

struct PreviousCompile<'a> {
    scene: &'a Scene,
    policy: SceneUpdatePolicy,
    static_change: LayerChange,
    dynamic_change: LayerChange,
}

struct CompileOutcome {
    scene: Scene,
    static_kind: HierarchyUpdateKind,
    dynamic_kind: HierarchyUpdateKind,
    maximum_quality_ratio: f64,
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
fn compile_scene(
    options: SceneBuildOptions,
    resources: Vec<Arc<MeshResource>>,
    mut drafts: Vec<InstanceDraft>,
    previous: Option<PreviousCompile<'_>>,
    build_time_microseconds: u64,
) -> Result<CompileOutcome, SceneError> {
    if resources.is_empty() {
        return Err(SceneError::EmptyResources);
    }
    if drafts.is_empty() {
        return Err(SceneError::EmptyInstances);
    }
    let mut identifiers = HashSet::with_capacity(drafts.len());
    for draft in &drafts {
        if !identifiers.insert(draft.instance_id) {
            return Err(SceneError::DuplicateInstance(draft.instance_id));
        }
    }
    drafts.sort_by_key(|draft| match draft.layer {
        SceneLayer::Static => 0_u8,
        SceneLayer::Dynamic => 1_u8,
    });
    let world_bounds = drafts
        .iter()
        .map(|draft| {
            draft
                .transform
                .transform_aabb(resources[draft.mesh_index].bounds)
        })
        .reduce(Aabb::union)
        .expect("non-empty instances");
    let rebase_origin = previous.as_ref().map_or_else(
        || world_bounds.center(),
        |state| state.scene.stats.rebase_origin_meters,
    );
    let mut instances = Vec::with_capacity(drafts.len());
    let mut static_primitives = Vec::new();
    let mut dynamic_primitives = Vec::new();
    let mut instance_bounds = Vec::with_capacity(drafts.len());
    for (index, draft) in drafts.iter().copied().enumerate() {
        let transform = draft.transform.rebased(rebase_origin);
        let bounds = transform.transform_aabb(resources[draft.mesh_index].bounds);
        instances.push(Instance {
            mesh_index: draft.mesh_index,
            object_id: draft.object_id,
            id: draft.instance_id,
            inverse_transform: transform
                .inverse()
                .map_err(|_| SceneError::InvalidTransform)?,
            mirrored: transform.is_mirrored(),
            category_mask: draft.category_mask,
            bounds,
            layer: draft.layer,
        });
        instance_bounds.push(bounds);
        let primitive = Primitive::new(
            u32::try_from(index).expect("instance count fits u32"),
            bounds,
        );
        match draft.layer {
            SceneLayer::Static => static_primitives.push(primitive),
            SceneLayer::Dynamic => dynamic_primitives.push(primitive),
        }
    }

    let (static_tlas, static_kind, static_refits, static_ratio) = update_hierarchy(
        previous.as_ref().map(|state| &state.scene.static_tlas),
        &static_primitives,
        &instance_bounds,
        previous.as_ref().map_or(
            LayerChange {
                changed: true,
                bounds_changed: true,
                topology_changed: true,
            },
            |state| state.static_change,
        ),
        previous
            .as_ref()
            .map_or(0, |state| state.scene.static_refit_count),
        previous
            .as_ref()
            .map_or_else(SceneUpdatePolicy::default, |state| state.policy),
        options.maximum_leaf_size,
    );
    let (dynamic_tlas, dynamic_kind, dynamic_refits, dynamic_ratio) = update_hierarchy(
        previous.as_ref().map(|state| &state.scene.dynamic_tlas),
        &dynamic_primitives,
        &instance_bounds,
        previous.as_ref().map_or(
            LayerChange {
                changed: true,
                bounds_changed: true,
                topology_changed: true,
            },
            |state| state.dynamic_change,
        ),
        previous
            .as_ref()
            .map_or(0, |state| state.scene.dynamic_refit_count),
        previous
            .as_ref()
            .map_or_else(SceneUpdatePolicy::default, |state| state.policy),
        options.maximum_leaf_size,
    );
    let mut pool_builder = ThreadPoolBuilder::new();
    if options.thread_count > 0 {
        pool_builder = pool_builder.num_threads(options.thread_count);
    }
    let thread_pool = pool_builder
        .build()
        .map_err(|error| SceneError::ThreadPool(error.to_string()))?;
    let content_hash = scene_hash(options, &resources, &drafts, rebase_origin);
    let stats = calculate_stats(
        &resources,
        &instances,
        &static_tlas,
        &dynamic_tlas,
        &thread_pool,
        rebase_origin,
        content_hash,
        build_time_microseconds,
    );
    Ok(CompileOutcome {
        scene: Scene {
            options,
            resources,
            source_instances: drafts,
            instances,
            static_tlas,
            dynamic_tlas,
            static_refit_count: static_refits,
            dynamic_refit_count: dynamic_refits,
            thread_pool,
            stats,
        },
        static_kind,
        dynamic_kind,
        maximum_quality_ratio: static_ratio.max(dynamic_ratio),
    })
}

fn update_hierarchy(
    previous: Option<&Bvh>,
    primitives: &[Primitive],
    primitive_bounds: &[Aabb],
    change: LayerChange,
    consecutive_refits: u32,
    policy: SceneUpdatePolicy,
    maximum_leaf_size: usize,
) -> (Bvh, HierarchyUpdateKind, u32, f64) {
    let Some(previous) = previous else {
        return (
            Bvh::build(primitives.to_vec(), maximum_leaf_size),
            HierarchyUpdateKind::Rebuilt,
            0,
            1.0,
        );
    };
    if !change.changed {
        return (
            previous.clone(),
            HierarchyUpdateKind::Reused,
            consecutive_refits,
            1.0,
        );
    }
    if change.topology_changed || !change.bounds_changed || primitives.is_empty() {
        let kind = if !change.bounds_changed && !change.topology_changed {
            HierarchyUpdateKind::Reused
        } else {
            HierarchyUpdateKind::Rebuilt
        };
        return (
            if kind == HierarchyUpdateKind::Reused {
                previous.clone()
            } else {
                Bvh::build(primitives.to_vec(), maximum_leaf_size)
            },
            kind,
            if kind == HierarchyUpdateKind::Reused {
                consecutive_refits
            } else {
                0
            },
            1.0,
        );
    }
    let mut refitted = previous.clone();
    let old_cost = previous.quality_cost().max(f64::EPSILON);
    let valid = refitted.refit(primitive_bounds);
    let quality_ratio = if valid {
        refitted.quality_cost() / old_cost
    } else {
        f64::INFINITY
    };
    if !valid
        || quality_ratio > policy.maximum_refit_quality_ratio
        || consecutive_refits >= policy.maximum_consecutive_refits
    {
        return (
            Bvh::build(primitives.to_vec(), maximum_leaf_size),
            HierarchyUpdateKind::Rebuilt,
            0,
            quality_ratio,
        );
    }
    (
        refitted,
        HierarchyUpdateKind::Refit,
        consecutive_refits.saturating_add(1),
        quality_ratio,
    )
}

const fn mark_layer_change(
    layer: SceneLayer,
    bounds_changed: bool,
    topology_changed: bool,
    static_change: &mut LayerChange,
    dynamic_change: &mut LayerChange,
    static_count: &mut usize,
    dynamic_count: &mut usize,
) {
    let (change, count) = match layer {
        SceneLayer::Static => (static_change, static_count),
        SceneLayer::Dynamic => (dynamic_change, dynamic_count),
    };
    change.changed = true;
    change.bounds_changed |= bounds_changed;
    change.topology_changed |= topology_changed;
    *count = count.saturating_add(1);
}

const fn normalize_category(category_mask: u64) -> u64 {
    if category_mask == 0 {
        ALL_CATEGORIES
    } else {
        category_mask
    }
}

fn compile_resource(
    mut mesh: Mesh,
    options: SceneBuildOptions,
    id: MeshId,
) -> Result<MeshResource, SceneError> {
    for position in &mut mesh.positions {
        *position = *position * options.unit_scale_to_meters;
    }
    let audit = audit_mesh(
        &mesh,
        MeshAuditOptions::try_new(options.absolute_tolerance_meters, 0.25)
            .expect("builder validates tolerance"),
    )
    .expect("builder validates tolerance");
    if !audit.is_analysis_ready {
        let face_count = audit
            .face_flags
            .iter()
            .filter(|flags| flags.is_repairable())
            .count();
        return Err(SceneError::MeshRequiresRepair { face_count });
    }
    let bounds = audit
        .bounds
        .ok_or(SceneError::MeshRequiresRepair { face_count: 0 })?;
    let primitives = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(index, triangle)| {
            let first = mesh.positions[triangle[0] as usize];
            let second = mesh.positions[triangle[1] as usize];
            let third = mesh.positions[triangle[2] as usize];
            let mut triangle_bounds = Aabb::from_point(first);
            triangle_bounds.include(second);
            triangle_bounds.include(third);
            Primitive::new(
                u32::try_from(index).expect("canonical triangle count fits u32"),
                triangle_bounds,
            )
        })
        .collect();
    Ok(MeshResource {
        id,
        mesh,
        bounds,
        content_hash: audit.content_hash,
        bvh: Bvh::build(primitives, options.maximum_leaf_size),
    })
}

#[derive(Clone, Copy, Debug)]
struct RawRay {
    origin: Vec3,
    direction: Vec3,
    t_min: f64,
    t_max: f64,
}

#[derive(Clone, Copy, Debug)]
struct LocalHit {
    distance: f64,
    u: f64,
    v: f64,
    triangle_id: u32,
    front_face: bool,
}

struct NodeStack {
    inline: [u32; 128],
    length: usize,
    overflow: Vec<u32>,
}

impl NodeStack {
    const fn new() -> Self {
        Self {
            inline: [0; 128],
            length: 0,
            overflow: Vec::new(),
        }
    }

    fn push(&mut self, value: u32) {
        if self.length < self.inline.len() {
            self.inline[self.length] = value;
            self.length += 1;
        } else {
            self.overflow.push(value);
        }
    }

    fn pop(&mut self) -> Option<u32> {
        if let Some(value) = self.overflow.pop() {
            return Some(value);
        }
        if self.length == 0 {
            return None;
        }
        self.length -= 1;
        Some(self.inline[self.length])
    }
}

fn push_children_near_first(
    stack: &mut NodeStack,
    bvh: &Bvh,
    node: bvh::Node,
    ray: RawRay,
    maximum: f64,
) {
    let left_distance = intersect_aabb(ray, bvh.nodes[node.left as usize].bounds, maximum);
    let right_distance = intersect_aabb(ray, bvh.nodes[node.right as usize].bounds, maximum);
    match (left_distance, right_distance) {
        (Some(left), Some(right)) if left <= right => {
            stack.push(node.right);
            stack.push(node.left);
        }
        (Some(_), Some(_)) => {
            stack.push(node.left);
            stack.push(node.right);
        }
        (Some(_), None) => stack.push(node.left),
        (None, Some(_)) => stack.push(node.right),
        (None, None) => {}
    }
}

fn intersect_aabb(ray: RawRay, bounds: Aabb, maximum: f64) -> Option<f64> {
    let mut near = ray.t_min;
    let mut far = maximum;
    for (origin, direction, minimum, maximum) in [
        (ray.origin.x, ray.direction.x, bounds.min.x, bounds.max.x),
        (ray.origin.y, ray.direction.y, bounds.min.y, bounds.max.y),
        (ray.origin.z, ray.direction.z, bounds.min.z, bounds.max.z),
    ] {
        if direction.abs() <= 1.0e-18 {
            if origin < minimum || origin > maximum {
                return None;
            }
            continue;
        }
        let inverse = direction.recip();
        let mut first = (minimum - origin) * inverse;
        let mut second = (maximum - origin) * inverse;
        if first > second {
            mem::swap(&mut first, &mut second);
        }
        near = near.max(first);
        far = far.min(second);
        if near > far {
            return None;
        }
    }
    Some(near)
}

fn intersect_triangle(
    ray: RawRay,
    first: Vec3,
    second: Vec3,
    third: Vec3,
    triangle_id: u32,
    maximum: f64,
) -> Option<LocalHit> {
    let edge_one = second - first;
    let edge_two = third - first;
    let p = ray.direction.cross(edge_two);
    let determinant = edge_one.dot(p);
    let epsilon = 1.0e-14 * edge_one.length_squared().sqrt() * edge_two.length_squared().sqrt();
    if determinant.abs() <= epsilon {
        return None;
    }
    let inverse_determinant = determinant.recip();
    let delta = ray.origin - first;
    let u = delta.dot(p) * inverse_determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = delta.cross(edge_one);
    let v = ray.direction.dot(q) * inverse_determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge_two.dot(q) * inverse_determinant;
    if distance < ray.t_min || distance > maximum {
        return None;
    }
    Some(LocalHit {
        distance,
        u,
        v,
        triangle_id,
        front_face: determinant > 0.0,
    })
}

fn scene_hash(
    options: SceneBuildOptions,
    resources: &[Arc<MeshResource>],
    instances: &[InstanceDraft],
    rebase_origin: Vec3,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_SCENE_V1\0");
    hasher.update(&options.unit_scale_to_meters.to_bits().to_le_bytes());
    for coordinate in [rebase_origin.x, rebase_origin.y, rebase_origin.z] {
        hasher.update(&coordinate.to_bits().to_le_bytes());
    }
    hasher.update(&(resources.len() as u64).to_le_bytes());
    for resource in resources {
        hasher.update(&resource.content_hash);
    }
    hasher.update(&(instances.len() as u64).to_le_bytes());
    for instance in instances {
        hasher.update(&instance.object_id.get().to_le_bytes());
        hasher.update(&instance.instance_id.get().to_le_bytes());
        hasher.update(&resources[instance.mesh_index].id.get().to_le_bytes());
        hasher.update(&instance.category_mask.to_le_bytes());
        hasher.update(&(instance.layer as u32).to_le_bytes());
        for value in instance.transform.to_row_major() {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

#[allow(clippy::too_many_arguments)]
fn calculate_stats(
    resources: &[Arc<MeshResource>],
    instances: &[Instance],
    static_tlas: &Bvh,
    dynamic_tlas: &Bvh,
    thread_pool: &ThreadPool,
    rebase_origin: Vec3,
    content_hash: [u8; 32],
    build_time_microseconds: u64,
) -> SceneStats {
    let unique_triangle_count = resources
        .iter()
        .map(|resource| resource.mesh.triangles.len())
        .sum();
    let instanced_triangle_count = instances
        .iter()
        .map(|instance| resources[instance.mesh_index].mesh.triangles.len())
        .sum();
    let blas_node_count = resources
        .iter()
        .map(|resource| resource.bvh.nodes.len())
        .sum();
    let maximum_bvh_depth = resources
        .iter()
        .map(|resource| resource.bvh.maximum_depth)
        .chain([static_tlas.maximum_depth, dynamic_tlas.maximum_depth])
        .max()
        .unwrap_or(0);
    let approximate_memory_bytes = resources
        .iter()
        .map(|resource| {
            resource.mesh.positions.capacity() * mem::size_of::<Vec3>()
                + resource.mesh.triangles.capacity() * mem::size_of::<[u32; 3]>()
                + resource.bvh.nodes.capacity() * mem::size_of::<bvh::Node>()
                + resource.bvh.primitive_indices.capacity() * mem::size_of::<u32>()
        })
        .sum::<usize>()
        + mem::size_of_val(instances)
        + static_tlas.nodes.capacity() * mem::size_of::<bvh::Node>()
        + static_tlas.primitive_indices.capacity() * mem::size_of::<u32>()
        + dynamic_tlas.nodes.capacity() * mem::size_of::<bvh::Node>()
        + dynamic_tlas.primitive_indices.capacity() * mem::size_of::<u32>();

    SceneStats {
        mesh_resource_count: resources.len(),
        instance_count: instances.len(),
        unique_triangle_count,
        instanced_triangle_count,
        blas_node_count,
        tlas_node_count: static_tlas.nodes.len() + dynamic_tlas.nodes.len(),
        maximum_bvh_depth,
        approximate_memory_bytes,
        build_time_microseconds,
        thread_count: thread_pool.current_num_threads(),
        rebase_origin_meters: rebase_origin,
        content_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_triangle() -> Mesh {
        Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        }
    }

    fn translated(x: f64, y: f64, z: f64) -> Transform {
        Transform::try_from_row_major([
            1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z, 0.0, 0.0, 0.0, 1.0,
        ])
        .expect("translation is valid")
    }

    #[test]
    fn closest_hit_preserves_object_instance_mesh_and_triangle_identity() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(unit_triangle()).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(11),
                InstanceId::new(22),
                1,
            )
            .expect("instance valid");
        let scene = builder.build().expect("scene builds");
        let ray = QueryRay::try_new(
            Vec3::new(0.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            10.0,
            1,
        )
        .expect("ray valid");
        let hit = scene.trace_closest(ray);
        assert!(hit.hit);
        assert!((hit.distance - 1.0).abs() < 1.0e-12);
        assert_eq!(hit.object_id, ObjectId::new(11));
        assert_eq!(hit.instance_id, InstanceId::new(22));
        assert_eq!(hit.mesh_id, mesh_id);
        assert_eq!(hit.triangle_id, 0);
    }

    #[test]
    fn shared_mesh_is_stored_once_and_instanced_twice() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let first = builder.add_mesh(unit_triangle()).expect("mesh valid");
        let second = builder.add_mesh(unit_triangle()).expect("dedupe valid");
        assert_eq!(first, second);
        builder
            .add_instance(
                first,
                translated(0.0, 0.0, 0.0),
                ObjectId::new(1),
                InstanceId::new(1),
                1,
            )
            .expect("first instance valid");
        builder
            .add_instance(
                first,
                translated(10.0, 0.0, 0.0),
                ObjectId::new(2),
                InstanceId::new(2),
                2,
            )
            .expect("second instance valid");
        let scene = builder.build().expect("scene builds");
        assert_eq!(scene.stats().mesh_resource_count, 1);
        assert_eq!(scene.stats().instance_count, 2);
        assert_eq!(scene.stats().unique_triangle_count, 1);
        assert_eq!(scene.stats().instanced_triangle_count, 2);

        let masked = QueryRay::try_new(
            Vec3::new(10.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            10.0,
            1,
        )
        .expect("ray valid");
        assert!(!scene.trace_any(masked));
        let visible = QueryRay {
            category_mask: 2,
            ..masked
        };
        assert_eq!(scene.trace_closest(visible).object_id, ObjectId::new(2));
    }

    #[test]
    fn large_world_translation_is_rebased_without_changing_distance() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(unit_triangle()).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                translated(100_000_000.0, -50_000_000.0, 0.0),
                ObjectId::new(1),
                InstanceId::new(1),
                1,
            )
            .expect("instance valid");
        let scene = builder.build().expect("scene builds");
        let ray = QueryRay::try_new(
            Vec3::new(100_000_000.25, -49_999_999.75, 4.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            10.0,
            1,
        )
        .expect("ray valid");
        let hit = scene.trace_closest(ray);
        assert!(hit.hit);
        assert!((hit.distance - 4.0).abs() < 1.0e-9);
        assert!(scene.stats().rebase_origin_meters.x > 99_000_000.0);
    }

    #[test]
    fn bvh_matches_reference_triangle_for_procedural_ray_grid() {
        let mesh = unit_triangle();
        let mut builder = SceneBuilder::new(SceneBuildOptions {
            thread_count: 2,
            ..SceneBuildOptions::default()
        })
        .expect("options valid");
        let mesh_id = builder.add_mesh(mesh.clone()).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                InstanceId::new(1),
                1,
            )
            .expect("instance valid");
        let scene = builder.build().expect("scene builds");
        let rays = (0..1024)
            .map(|index| {
                let x = f64::from(index % 32) / 31.0;
                let y = f64::from(index / 32) / 31.0;
                QueryRay::try_new(Vec3::new(x, y, 1.0), Vec3::new(0.0, 0.0, -1.0), 0.0, 2.0, 1)
                    .expect("ray valid")
            })
            .collect::<Vec<_>>();
        let hits = scene.trace_closest_batch(&rays);
        for (ray, hit) in rays.iter().zip(hits) {
            let reference_ray = Ray::try_new(ray.origin, ray.direction, ray.t_min, ray.t_max)
                .expect("reference ray valid");
            let triangle = xvarna_geometry::Triangle {
                a: mesh.positions[0],
                b: mesh.positions[1],
                c: mesh.positions[2],
                object_id: ObjectId::new(1),
            };
            let reference = xvarna_geometry::intersect_triangle_reference(reference_ray, triangle);
            assert_eq!(hit.hit, reference.is_some());
        }
    }

    #[test]
    fn dynamic_delta_reuses_static_tlas_and_blas_while_refitting_dynamic_tlas() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(unit_triangle()).expect("mesh valid");
        builder
            .add_instance_in_layer(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                InstanceId::new(1),
                1,
                SceneLayer::Static,
            )
            .expect("static valid");
        builder
            .add_instance_in_layer(
                mesh_id,
                translated(2.0, 0.0, 0.0),
                ObjectId::new(2),
                InstanceId::new(2),
                2,
                SceneLayer::Dynamic,
            )
            .expect("dynamic valid");
        let scene = builder.build().expect("scene builds");
        let (updated, report) = scene
            .apply_deltas(
                &[SceneDelta::UpdateInstance {
                    instance_id: InstanceId::new(2),
                    transform: Some(translated(5.0, 0.0, 0.0)),
                    object_id: None,
                    category_mask: None,
                    layer: None,
                }],
                SceneUpdatePolicy::default(),
            )
            .expect("delta applies");
        assert_eq!(report.reused_blas_count, 1);
        assert_eq!(report.rebuilt_blas_count, 0);
        assert_eq!(report.static_tlas_update, HierarchyUpdateKind::Reused);
        assert_eq!(report.dynamic_tlas_update, HierarchyUpdateKind::Refit);
        let ray = QueryRay::try_new(
            Vec3::new(5.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            2.0,
            2,
        )
        .expect("ray valid");
        assert_eq!(updated.trace_closest(ray).instance_id, InstanceId::new(2));
        assert_ne!(report.previous_hash, report.current_hash);
    }

    #[test]
    fn topology_delta_rebuilds_only_the_affected_layer() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(unit_triangle()).expect("mesh valid");
        builder
            .add_instance_in_layer(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                InstanceId::new(1),
                1,
                SceneLayer::Static,
            )
            .expect("static valid");
        builder
            .add_instance_in_layer(
                mesh_id,
                translated(2.0, 0.0, 0.0),
                ObjectId::new(2),
                InstanceId::new(2),
                2,
                SceneLayer::Dynamic,
            )
            .expect("dynamic valid");
        let scene = builder.build().expect("scene builds");
        let (_, report) = scene
            .apply_deltas(
                &[SceneDelta::AddInstance {
                    mesh_id,
                    transform: translated(4.0, 0.0, 0.0),
                    object_id: ObjectId::new(3),
                    instance_id: InstanceId::new(3),
                    category_mask: 2,
                    layer: SceneLayer::Dynamic,
                }],
                SceneUpdatePolicy::default(),
            )
            .expect("delta applies");
        assert_eq!(report.static_tlas_update, HierarchyUpdateKind::Reused);
        assert_eq!(report.dynamic_tlas_update, HierarchyUpdateKind::Rebuilt);
    }

    #[test]
    fn mesh_replacement_rebuilds_one_blas_and_preserves_resource_identity() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(unit_triangle()).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                InstanceId::new(1),
                1,
            )
            .expect("instance valid");
        let scene = builder.build().expect("scene builds");
        let mut replacement = unit_triangle();
        replacement.positions[1].x = 2.0;
        let (updated, report) = scene
            .apply_deltas(
                &[SceneDelta::ReplaceMesh {
                    mesh_id,
                    mesh: replacement,
                }],
                SceneUpdatePolicy::default(),
            )
            .expect("replacement applies");
        assert_eq!(report.rebuilt_blas_count, 1);
        assert_eq!(updated.stats().mesh_resource_count, 1);
        let ray = QueryRay::try_new(
            Vec3::new(1.25, 0.1, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            2.0,
            1,
        )
        .expect("ray valid");
        assert_eq!(updated.trace_closest(ray).mesh_id, mesh_id);
    }
}
