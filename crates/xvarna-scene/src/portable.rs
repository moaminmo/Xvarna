//! Portable, chunkable two-level f32 snapshots for compute backends.

use super::{Instance, Scene, bvh};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use xvarna_geometry::Vec3;

/// Maximum TLAS or BLAS depth supported by the portable shader stacks.
pub const PORTABLE_MAXIMUM_BVH_DEPTH: usize = 96;

/// Controls conversion of the canonical f64 scene into bounded portable chunks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableSceneOptions {
    /// Maximum permitted f64-to-f32 coordinate error in canonical metres.
    pub maximum_precision_error_meters: f64,
    /// Hard cap on effective triangles represented by the complete scene.
    pub maximum_expanded_triangle_count: usize,
    /// Maximum triangles stored in a portable BLAS/TLAS leaf.
    pub maximum_leaf_size: usize,
    /// Maximum static payload bytes in one uploadable geometry chunk.
    pub maximum_chunk_payload_bytes: usize,
}

impl Default for PortableSceneOptions {
    fn default() -> Self {
        Self {
            maximum_precision_error_meters: 0.000_25,
            maximum_expanded_triangle_count: 20_000_000,
            maximum_leaf_size: 4,
            maximum_chunk_payload_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Failure while creating a portable execution snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum PortableSceneError {
    /// The precision budget is non-finite or not positive.
    InvalidPrecisionBudget,
    /// Triangle, leaf, or chunk policy is invalid.
    InvalidResourcePolicy,
    /// Effective triangle count exceeds the explicit whole-scene cap.
    TriangleLimitExceeded {
        /// Required effective triangle count.
        required: usize,
        /// Configured hard cap.
        limit: usize,
    },
    /// One indivisible mesh instance cannot fit inside the per-chunk payload budget.
    GeometryChunkTooLarge {
        /// Required approximate bytes for the smallest valid chunk.
        required_bytes: usize,
        /// Configured per-chunk byte budget.
        limit_bytes: usize,
    },
    /// f64 to f32 conversion would exceed the accepted metric error.
    PrecisionBudgetExceeded {
        /// Largest observed coordinate conversion error in metres.
        estimated_error_meters: f64,
        /// User-selected error limit in metres.
        allowed_error_meters: f64,
    },
    /// A hierarchy cannot be traversed by a bounded portable shader stack.
    HierarchyDepthExceeded {
        /// Required BVH depth.
        required: usize,
        /// Portable shader stack capacity.
        limit: usize,
    },
    /// A caller requested a single snapshot for a scene requiring chunked execution.
    ChunkingRequired {
        /// Number of bounded chunks required.
        chunk_count: usize,
    },
}

impl fmt::Display for PortableSceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPrecisionBudget => {
                formatter.write_str("portable precision budget must be finite and positive")
            }
            Self::InvalidResourcePolicy => formatter.write_str(
                "portable triangle limit and chunk budget must be positive and leaf size must be between 1 and 64",
            ),
            Self::TriangleLimitExceeded { required, limit } => write!(
                formatter,
                "portable scene represents {required} effective triangles, above the cap of {limit}"
            ),
            Self::GeometryChunkTooLarge {
                required_bytes,
                limit_bytes,
            } => write!(
                formatter,
                "one portable geometry unit needs {required_bytes} bytes, above the per-chunk budget of {limit_bytes} bytes"
            ),
            Self::PrecisionBudgetExceeded {
                estimated_error_meters,
                allowed_error_meters,
            } => write!(
                formatter,
                "portable f32 coordinate error {estimated_error_meters:.6e} m exceeds the {allowed_error_meters:.6e} m budget"
            ),
            Self::HierarchyDepthExceeded { required, limit } => write!(
                formatter,
                "portable BVH depth {required} exceeds the shader stack limit {limit}"
            ),
            Self::ChunkingRequired { chunk_count } => write!(
                formatter,
                "portable scene needs {chunk_count} geometry chunks; use portable_snapshots"
            ),
        }
    }
}

impl std::error::Error for PortableSceneError {}

/// One flattened BVH node using a shader-compatible logical layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableBvhNode {
    /// Conservative minimum bounds.
    pub minimum: [f32; 3],
    /// Left child for an internal node.
    pub left: u32,
    /// Conservative maximum bounds.
    pub maximum: [f32; 3],
    /// Right child for an internal node.
    pub right: u32,
    /// Offset in the matching primitive-index buffer for a leaf.
    pub start: u32,
    /// Primitive count; zero marks an internal node.
    pub count: u32,
}

/// One unique local-space triangle stored once regardless of instance count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableTriangle {
    /// First local-space vertex.
    pub first: [f32; 3],
    /// First local-space triangle edge.
    pub edge_one: [f32; 3],
    /// Second local-space triangle edge.
    pub edge_two: [f32; 3],
    /// Stable triangle index inside the shared source mesh.
    pub triangle_id: u32,
}

/// One two-level occurrence with a local-ray transform and exact source identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableInstance {
    /// First three rows of the inverse affine transform from rebased world to local space.
    pub inverse_transform: [[f32; 4]; 3],
    /// Stable source object identifier.
    pub object_id: u64,
    /// Stable occurrence identifier.
    pub instance_id: u64,
    /// Stable shared mesh identifier.
    pub mesh_id: u64,
    /// Instance category bits.
    pub category_mask: u64,
    /// Root node of the shared BLAS in the concatenated node array.
    pub blas_root_node: u32,
    /// True when the world transform reverses orientation.
    pub mirrored: bool,
    /// Zero static, one dynamic.
    pub layer: u32,
}

/// Resource, hierarchy, chunking, precision, and identity metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableSceneStats {
    /// Unique triangles uploaded in this chunk.
    pub triangle_count: usize,
    /// Effective triangles after applying the chunk's instances.
    pub effective_triangle_count: usize,
    /// Occurrences represented by this chunk.
    pub instance_count: usize,
    /// Total TLAS plus BLAS nodes.
    pub node_count: usize,
    /// TLAS nodes.
    pub tlas_node_count: usize,
    /// Shared BLAS nodes.
    pub blas_node_count: usize,
    /// Maximum hierarchy depth across TLAS and BLAS.
    pub maximum_bvh_depth: usize,
    /// Largest observed f64-to-f32 coordinate error in metres.
    pub maximum_precision_error_meters: f64,
    /// Approximate bytes before backend-specific alignment.
    pub approximate_payload_bytes: usize,
    /// Zero-based chunk index.
    pub chunk_index: usize,
    /// Total chunks in the scene execution plan.
    pub chunk_count: usize,
    /// Canonical whole-scene content identity.
    pub scene_hash: [u8; 32],
}

/// Immutable two-level portable snapshot for one bounded geometry chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableSceneSnapshot {
    /// World origin subtracted before portable execution.
    pub rebase_origin_meters: Vec3,
    /// Top-level instance hierarchy.
    pub tlas_nodes: Vec<PortableBvhNode>,
    /// TLAS leaf-to-instance indirection.
    pub tlas_primitive_indices: Vec<u32>,
    /// Concatenated shared bottom-level hierarchies.
    pub blas_nodes: Vec<PortableBvhNode>,
    /// BLAS leaf-to-triangle indirection.
    pub blas_primitive_indices: Vec<u32>,
    /// Unique local triangles.
    pub triangles: Vec<PortableTriangle>,
    /// Occurrences referencing shared BLAS roots.
    pub instances: Vec<PortableInstance>,
    /// Snapshot metadata.
    pub stats: PortableSceneStats,
}

impl Scene {
    /// Builds exactly one portable snapshot or reports that chunked execution is required.
    pub fn portable_snapshot(
        &self,
        options: PortableSceneOptions,
    ) -> Result<PortableSceneSnapshot, PortableSceneError> {
        let mut snapshots = self.portable_snapshots(options)?;
        if snapshots.len() != 1 {
            return Err(PortableSceneError::ChunkingRequired {
                chunk_count: snapshots.len(),
            });
        }
        Ok(snapshots.remove(0))
    }

    /// Builds a deterministic sequence of two-level chunks under an explicit byte budget.
    pub fn portable_snapshots(
        &self,
        options: PortableSceneOptions,
    ) -> Result<Vec<PortableSceneSnapshot>, PortableSceneError> {
        validate_options(self, options)?;
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut current = Vec::new();
        for instance_index in 0..self.instances.len() {
            current.push(instance_index);
            let estimate = estimate_payload(self, &current);
            if estimate > options.maximum_chunk_payload_bytes {
                current.pop();
                if current.is_empty() {
                    return Err(PortableSceneError::GeometryChunkTooLarge {
                        required_bytes: estimate,
                        limit_bytes: options.maximum_chunk_payload_bytes,
                    });
                }
                groups.push(std::mem::take(&mut current));
                current.push(instance_index);
                let required = estimate_payload(self, &current);
                if required > options.maximum_chunk_payload_bytes {
                    return Err(PortableSceneError::GeometryChunkTooLarge {
                        required_bytes: required,
                        limit_bytes: options.maximum_chunk_payload_bytes,
                    });
                }
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }
        let chunk_count = groups.len();
        let mut snapshots = Vec::with_capacity(chunk_count);
        for (chunk_index, group) in groups.iter().enumerate() {
            let snapshot = build_snapshot(self, group, options, chunk_index, chunk_count)?;
            if snapshot.stats.approximate_payload_bytes > options.maximum_chunk_payload_bytes {
                return Err(PortableSceneError::GeometryChunkTooLarge {
                    required_bytes: snapshot.stats.approximate_payload_bytes,
                    limit_bytes: options.maximum_chunk_payload_bytes,
                });
            }
            snapshots.push(snapshot);
        }
        Ok(snapshots)
    }
}

fn validate_options(
    scene: &Scene,
    options: PortableSceneOptions,
) -> Result<(), PortableSceneError> {
    if !options.maximum_precision_error_meters.is_finite()
        || options.maximum_precision_error_meters <= 0.0
    {
        return Err(PortableSceneError::InvalidPrecisionBudget);
    }
    if options.maximum_expanded_triangle_count == 0
        || !(1..=64).contains(&options.maximum_leaf_size)
        || options.maximum_chunk_payload_bytes == 0
    {
        return Err(PortableSceneError::InvalidResourcePolicy);
    }
    let required = scene.stats.instanced_triangle_count;
    if required > options.maximum_expanded_triangle_count {
        return Err(PortableSceneError::TriangleLimitExceeded {
            required,
            limit: options.maximum_expanded_triangle_count,
        });
    }
    Ok(())
}

fn estimate_payload(scene: &Scene, instance_indices: &[usize]) -> usize {
    let mut resources = HashSet::new();
    let mut bytes = instance_indices
        .len()
        .saturating_mul(core::mem::size_of::<PortableInstance>())
        .saturating_add(
            instance_indices
                .len()
                .saturating_mul(2)
                .saturating_mul(core::mem::size_of::<PortableBvhNode>()),
        )
        .saturating_add(
            instance_indices
                .len()
                .saturating_mul(core::mem::size_of::<u32>()),
        );
    for &instance_index in instance_indices {
        let resource_index = scene.instances[instance_index].mesh_index;
        if !resources.insert(resource_index) {
            continue;
        }
        let resource = &scene.resources[resource_index];
        bytes = bytes
            .saturating_add(
                resource
                    .mesh
                    .triangles
                    .len()
                    .saturating_mul(core::mem::size_of::<PortableTriangle>()),
            )
            .saturating_add(
                resource
                    .bvh
                    .nodes
                    .len()
                    .saturating_mul(core::mem::size_of::<PortableBvhNode>()),
            )
            .saturating_add(
                resource
                    .bvh
                    .primitive_indices
                    .len()
                    .saturating_mul(core::mem::size_of::<u32>()),
            );
    }
    bytes.saturating_mul(2)
}

#[allow(clippy::too_many_lines)]
fn build_snapshot(
    scene: &Scene,
    instance_indices: &[usize],
    options: PortableSceneOptions,
    chunk_index: usize,
    chunk_count: usize,
) -> Result<PortableSceneSnapshot, PortableSceneError> {
    let mut maximum_error = 0.0_f64;
    let mut resource_offsets: HashMap<usize, u32> = HashMap::new();
    let mut blas_nodes = Vec::new();
    let mut blas_primitive_indices = Vec::new();
    let mut triangles = Vec::new();
    for &instance_index in instance_indices {
        let resource_index = scene.instances[instance_index].mesh_index;
        if resource_offsets.contains_key(&resource_index) {
            continue;
        }
        let resource = &scene.resources[resource_index];
        let node_offset = u32::try_from(blas_nodes.len()).expect("portable node count fits u32");
        let primitive_offset =
            u32::try_from(blas_primitive_indices.len()).expect("portable primitive count fits u32");
        let triangle_offset =
            u32::try_from(triangles.len()).expect("portable triangle count fits u32");
        resource_offsets.insert(resource_index, node_offset);
        for (triangle_id, indices) in resource.mesh.triangles.iter().copied().enumerate() {
            let first = quantize(
                resource.mesh.positions[indices[0] as usize],
                &mut maximum_error,
            );
            let second = quantize(
                resource.mesh.positions[indices[1] as usize],
                &mut maximum_error,
            );
            let third = quantize(
                resource.mesh.positions[indices[2] as usize],
                &mut maximum_error,
            );
            triangles.push(PortableTriangle {
                first,
                edge_one: subtract(second, first),
                edge_two: subtract(third, first),
                triangle_id: u32::try_from(triangle_id).expect("canonical triangle index fits u32"),
            });
        }
        blas_nodes.extend(resource.bvh.nodes.iter().map(|node| {
            portable_node_with_offsets(*node, maximum_error, node_offset, primitive_offset)
        }));
        blas_primitive_indices.extend(
            resource
                .bvh
                .primitive_indices
                .iter()
                .map(|index| triangle_offset.saturating_add(*index)),
        );
    }
    let maximum_blas_depth = instance_indices
        .iter()
        .map(|index| {
            scene.resources[scene.instances[*index].mesh_index]
                .bvh
                .maximum_depth
        })
        .max()
        .unwrap_or(0);
    if maximum_blas_depth > PORTABLE_MAXIMUM_BVH_DEPTH {
        return Err(PortableSceneError::HierarchyDepthExceeded {
            required: maximum_blas_depth,
            limit: PORTABLE_MAXIMUM_BVH_DEPTH,
        });
    }

    let mut instances = Vec::with_capacity(instance_indices.len());
    let mut tlas_primitives = Vec::with_capacity(instance_indices.len());
    let mut effective_triangle_count = 0_usize;
    for (portable_index, &instance_index) in instance_indices.iter().enumerate() {
        let instance = scene.instances[instance_index];
        let resource = &scene.resources[instance.mesh_index];
        effective_triangle_count =
            effective_triangle_count.saturating_add(resource.mesh.triangles.len());
        let inverse_transform = quantize_inverse_transform(instance, &mut maximum_error);
        instances.push(PortableInstance {
            inverse_transform,
            object_id: instance.object_id.get(),
            instance_id: instance.id.get(),
            mesh_id: resource.id.get(),
            category_mask: instance.category_mask,
            blas_root_node: resource_offsets[&instance.mesh_index],
            mirrored: instance.mirrored,
            layer: instance.layer as u32,
        });
        tlas_primitives.push(bvh::Primitive::new(
            u32::try_from(portable_index).expect("portable instance count fits u32"),
            instance.bounds,
        ));
    }
    let tlas = bvh::Bvh::build(tlas_primitives, options.maximum_leaf_size);
    if tlas.maximum_depth > PORTABLE_MAXIMUM_BVH_DEPTH {
        return Err(PortableSceneError::HierarchyDepthExceeded {
            required: tlas.maximum_depth,
            limit: PORTABLE_MAXIMUM_BVH_DEPTH,
        });
    }
    if maximum_error > options.maximum_precision_error_meters {
        return Err(PortableSceneError::PrecisionBudgetExceeded {
            estimated_error_meters: maximum_error,
            allowed_error_meters: options.maximum_precision_error_meters,
        });
    }
    let tlas_nodes = tlas
        .nodes
        .iter()
        .map(|node| portable_node_with_offsets(*node, maximum_error, 0, 0))
        .collect::<Vec<_>>();
    let approximate_payload_bytes = (tlas_nodes.len() * core::mem::size_of::<PortableBvhNode>()
        + tlas.primitive_indices.len() * core::mem::size_of::<u32>()
        + blas_nodes.len() * core::mem::size_of::<PortableBvhNode>()
        + blas_primitive_indices.len() * core::mem::size_of::<u32>()
        + triangles.len() * core::mem::size_of::<PortableTriangle>()
        + instances.len() * core::mem::size_of::<PortableInstance>())
    .saturating_mul(2);
    let node_count = tlas_nodes.len() + blas_nodes.len();
    let maximum_bvh_depth = tlas.maximum_depth.max(maximum_blas_depth);
    Ok(PortableSceneSnapshot {
        rebase_origin_meters: scene.stats.rebase_origin_meters,
        tlas_nodes,
        tlas_primitive_indices: tlas.primitive_indices,
        blas_nodes,
        blas_primitive_indices,
        triangles,
        instances,
        stats: PortableSceneStats {
            triangle_count: effective_unique_triangle_count(scene, instance_indices),
            effective_triangle_count,
            instance_count: instance_indices.len(),
            node_count,
            tlas_node_count: tlas.nodes.len(),
            blas_node_count: node_count.saturating_sub(tlas.nodes.len()),
            maximum_bvh_depth,
            maximum_precision_error_meters: maximum_error,
            approximate_payload_bytes,
            chunk_index,
            chunk_count,
            scene_hash: scene.stats.content_hash,
        },
    })
}

fn effective_unique_triangle_count(scene: &Scene, instance_indices: &[usize]) -> usize {
    let mut resources = HashSet::new();
    instance_indices
        .iter()
        .filter_map(|index| {
            let resource_index = scene.instances[*index].mesh_index;
            resources
                .insert(resource_index)
                .then_some(scene.resources[resource_index].mesh.triangles.len())
        })
        .sum()
}

fn quantize_inverse_transform(instance: Instance, maximum_error: &mut f64) -> [[f32; 4]; 3] {
    let matrix = instance.inverse_transform.to_row_major();
    let mut output = [[0.0_f32; 4]; 3];
    for row in 0..3 {
        for column in 0..4 {
            let value = matrix[row * 4 + column];
            #[allow(clippy::cast_possible_truncation)]
            let portable = value as f32;
            output[row][column] = portable;
        }
    }
    for x in [instance.bounds.min.x, instance.bounds.max.x] {
        for y in [instance.bounds.min.y, instance.bounds.max.y] {
            for z in [instance.bounds.min.z, instance.bounds.max.z] {
                let point = Vec3::new(x, y, z);
                let exact = instance.inverse_transform.transform_point(point);
                let portable = transform_point_f32(output, point);
                *maximum_error = maximum_error
                    .max((exact.x - portable.x).abs())
                    .max((exact.y - portable.y).abs())
                    .max((exact.z - portable.z).abs());
            }
        }
    }
    output
}

#[allow(clippy::cast_possible_truncation)]
fn transform_point_f32(matrix: [[f32; 4]; 3], point: Vec3) -> Vec3 {
    let point = [point.x as f32, point.y as f32, point.z as f32];
    Vec3::new(
        f64::from(matrix[0][0].mul_add(
            point[0],
            matrix[0][1].mul_add(point[1], matrix[0][2].mul_add(point[2], matrix[0][3])),
        )),
        f64::from(matrix[1][0].mul_add(
            point[0],
            matrix[1][1].mul_add(point[1], matrix[1][2].mul_add(point[2], matrix[1][3])),
        )),
        f64::from(matrix[2][0].mul_add(
            point[0],
            matrix[2][1].mul_add(point[1], matrix[2][2].mul_add(point[2], matrix[2][3])),
        )),
    )
}

#[allow(clippy::cast_possible_truncation)]
fn quantize(value: Vec3, maximum_error: &mut f64) -> [f32; 3] {
    let converted = [value.x as f32, value.y as f32, value.z as f32];
    for (source, portable) in [value.x, value.y, value.z].into_iter().zip(converted) {
        *maximum_error = maximum_error.max((source - f64::from(portable)).abs());
    }
    converted
}

const fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

#[allow(clippy::cast_possible_truncation)]
fn portable_node_with_offsets(
    node: bvh::Node,
    padding: f64,
    node_offset: u32,
    primitive_offset: u32,
) -> PortableBvhNode {
    PortableBvhNode {
        minimum: [
            (node.bounds.min.x - padding) as f32,
            (node.bounds.min.y - padding) as f32,
            (node.bounds.min.z - padding) as f32,
        ],
        left: if node.is_leaf() {
            0
        } else {
            node.left.saturating_add(node_offset)
        },
        maximum: [
            (node.bounds.max.x + padding) as f32,
            (node.bounds.max.y + padding) as f32,
            (node.bounds.max.z + padding) as f32,
        ],
        right: if node.is_leaf() {
            0
        } else {
            node.right.saturating_add(node_offset)
        },
        start: if node.is_leaf() {
            node.start.saturating_add(primitive_offset)
        } else {
            0
        },
        count: node.count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SceneBuildOptions, SceneBuilder, SceneLayer, Transform};
    use xvarna_geometry::Mesh;
    use xvarna_types::{InstanceId, ObjectId};

    fn triangle() -> Mesh {
        Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        }
    }

    fn translated(x: f64) -> Transform {
        Transform::try_from_row_major([
            1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
        .expect("transform valid")
    }

    #[test]
    fn snapshot_keeps_shared_geometry_once_and_preserves_instance_identity() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(triangle()).expect("mesh valid");
        for (id, x, layer) in [
            (7_u64, 0.0, SceneLayer::Static),
            (8, 5.0, SceneLayer::Dynamic),
        ] {
            builder
                .add_instance_in_layer(
                    mesh_id,
                    translated(x),
                    ObjectId::new(id),
                    InstanceId::new(id + 10),
                    1_u64 << id,
                    layer,
                )
                .expect("instance valid");
        }
        let scene = builder.build().expect("scene valid");
        let snapshot = scene
            .portable_snapshot(PortableSceneOptions::default())
            .expect("snapshot valid");
        assert_eq!(snapshot.triangles.len(), 1);
        assert_eq!(snapshot.instances.len(), 2);
        assert_eq!(snapshot.stats.effective_triangle_count, 2);
        assert_eq!(snapshot.instances[0].object_id, 7);
        assert_eq!(snapshot.instances[1].instance_id, 18);
        assert_eq!(snapshot.instances[1].category_mask, 1_u64 << 8);
        assert_eq!(snapshot.instances[1].layer, SceneLayer::Dynamic as u32);
        assert!(!snapshot.tlas_nodes.is_empty());
        assert!(!snapshot.blas_nodes.is_empty());
    }

    #[test]
    fn payload_budget_splits_instances_into_ordered_chunks() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(triangle()).expect("mesh valid");
        for index in 0..20_u64 {
            builder
                .add_instance_in_layer(
                    mesh_id,
                    translated(f64::from(u32::try_from(index).expect("small test index")) * 2.0),
                    ObjectId::new(index + 1),
                    InstanceId::new(index + 1),
                    1,
                    SceneLayer::Dynamic,
                )
                .expect("instance valid");
        }
        let scene = builder.build().expect("scene valid");
        let snapshots = scene
            .portable_snapshots(PortableSceneOptions {
                maximum_chunk_payload_bytes: 900,
                ..PortableSceneOptions::default()
            })
            .expect("chunking succeeds");
        assert!(snapshots.len() > 1);
        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| snapshot.instances.len())
                .sum::<usize>(),
            20
        );
        assert!(
            snapshots
                .iter()
                .all(|snapshot| snapshot.stats.approximate_payload_bytes <= 900)
        );
        assert!(matches!(
            scene.portable_snapshot(PortableSceneOptions {
                maximum_chunk_payload_bytes: 900,
                ..PortableSceneOptions::default()
            }),
            Err(PortableSceneError::ChunkingRequired { .. })
        ));
    }

    #[test]
    fn snapshot_enforces_effective_triangle_limit_before_allocating() {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder.add_mesh(triangle()).expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                InstanceId::new(1),
                1,
            )
            .expect("instance valid");
        let scene = builder.build().expect("scene valid");
        assert_eq!(
            scene
                .portable_snapshots(PortableSceneOptions {
                    maximum_expanded_triangle_count: 0,
                    ..PortableSceneOptions::default()
                })
                .expect_err("zero cap rejected"),
            PortableSceneError::InvalidResourcePolicy
        );
    }
}
