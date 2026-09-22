//! Deterministic mesh auditing and conservative repair.

use crate::{Mesh, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Axis-aligned bounds in canonical double-precision coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// Minimum coordinate on every axis.
    pub min: Vec3,
    /// Maximum coordinate on every axis.
    pub max: Vec3,
}

impl Aabb {
    /// Creates bounds from explicit minimum and maximum coordinates.
    #[must_use]
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Creates zero-volume bounds containing one point.
    #[must_use]
    pub const fn from_point(point: Vec3) -> Self {
        Self {
            min: point,
            max: point,
        }
    }

    /// Expands these bounds to include a point.
    pub const fn include(&mut self, point: Vec3) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Returns the union of two bounds.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Returns the center point.
    #[must_use]
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Returns the axis extents.
    #[must_use]
    pub fn extent(self) -> Vec3 {
        self.max - self.min
    }

    /// Returns surface area, including zero for flat bounds.
    #[must_use]
    pub fn surface_area(self) -> f64 {
        let extent = self.extent();
        2.0 * extent
            .x
            .mul_add(extent.y, extent.y.mul_add(extent.z, extent.z * extent.x))
    }
}

/// Configuration for deterministic mesh auditing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshAuditOptions {
    /// Absolute model-space length tolerance.
    pub absolute_tolerance: f64,
    /// Fraction of the absolute tolerance allowed for estimated f32 rounding.
    pub maximum_f32_error_ratio: f64,
}

impl MeshAuditOptions {
    /// Validates and creates mesh-audit options.
    pub fn try_new(
        absolute_tolerance: f64,
        maximum_f32_error_ratio: f64,
    ) -> Result<Self, MeshAuditError> {
        if !absolute_tolerance.is_finite() || absolute_tolerance <= 0.0 {
            return Err(MeshAuditError::InvalidTolerance);
        }
        if !maximum_f32_error_ratio.is_finite() || maximum_f32_error_ratio <= 0.0 {
            return Err(MeshAuditError::InvalidPrecisionBudget);
        }
        Ok(Self {
            absolute_tolerance,
            maximum_f32_error_ratio,
        })
    }
}

impl Default for MeshAuditOptions {
    fn default() -> Self {
        Self {
            absolute_tolerance: 1.0e-6,
            maximum_f32_error_ratio: 0.25,
        }
    }
}

/// Invalid mesh-audit configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshAuditError {
    /// The absolute tolerance is non-finite or not positive.
    InvalidTolerance,
    /// The floating-point precision budget is non-finite or not positive.
    InvalidPrecisionBudget,
    /// A repaired mesh would exceed the canonical u32 vertex-index range.
    TooManyVertices,
}

/// Per-face findings represented as stable ABI-compatible bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct MeshFaceFlags(u32);

impl MeshFaceFlags {
    /// The face references a vertex outside the position buffer.
    pub const INVALID_INDEX: Self = Self(1 << 0);
    /// At least one referenced vertex is non-finite.
    pub const NON_FINITE_VERTEX: Self = Self(1 << 1);
    /// The face repeats an index or has negligible area.
    pub const DEGENERATE: Self = Self(1 << 2);
    /// The face duplicates an earlier face, independent of winding.
    pub const DUPLICATE: Self = Self(1 << 3);

    /// Returns the raw stable bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true when every bit from `other` is present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns true when conservative repair may remove this face.
    #[must_use]
    pub const fn is_repairable(self) -> bool {
        self.0 != 0
    }

    const fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// Per-vertex findings represented as stable ABI-compatible bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct MeshVertexFlags(u32);

impl MeshVertexFlags {
    /// The vertex contains NaN or infinity.
    pub const NON_FINITE: Self = Self(1 << 0);
    /// The vertex is not referenced by an accepted face.
    pub const ISOLATED: Self = Self(1 << 1);

    /// Returns the raw stable bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true when every bit from `other` is present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// Complete deterministic mesh-health report.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshAuditReport {
    /// Number of source vertices.
    pub vertex_count: usize,
    /// Number of source triangle faces.
    pub face_count: usize,
    /// Number of unique, finite, non-degenerate faces accepted for metrics.
    pub accepted_face_count: usize,
    /// Number of vertices with NaN or infinity.
    pub non_finite_vertex_count: usize,
    /// Number of faces containing an invalid index.
    pub invalid_index_face_count: usize,
    /// Number of faces referencing a non-finite vertex.
    pub non_finite_face_count: usize,
    /// Number of repeated-index or negligible-area faces.
    pub degenerate_face_count: usize,
    /// Number of faces duplicating an earlier face.
    pub duplicate_face_count: usize,
    /// Number of finite vertices unused by accepted faces.
    pub isolated_vertex_count: usize,
    /// Number of edges referenced by exactly one accepted face.
    pub boundary_edge_count: usize,
    /// Number of edges referenced by more than two accepted faces.
    pub non_manifold_edge_count: usize,
    /// Number of two-face edges whose incident winding is equal rather than opposite.
    pub inconsistent_winding_edge_count: usize,
    /// Number of connected components among accepted faces.
    pub connected_component_count: usize,
    /// Sum of accepted triangle areas in squared model units.
    pub surface_area: f64,
    /// Signed tetrahedral volume in cubed model units.
    pub signed_volume: f64,
    /// Bounds of all finite source vertices.
    pub bounds: Option<Aabb>,
    /// Maximum absolute coordinate among finite vertices.
    pub maximum_coordinate_magnitude: f64,
    /// Conservative absolute rounding estimate for an f32 compute path.
    pub estimated_f32_error: f64,
    /// True when estimated f32 error exceeds the declared precision budget.
    pub exceeds_f32_precision_budget: bool,
    /// True when the mesh contains usable faces and no face needs conservative removal.
    pub is_analysis_ready: bool,
    /// True when accepted topology is closed, manifold, consistently wound, and clean.
    pub is_watertight: bool,
    /// BLAKE3 digest of canonical source geometry bytes.
    pub content_hash: [u8; 32],
    /// Stable flags corresponding one-to-one with source faces.
    pub face_flags: Vec<MeshFaceFlags>,
    /// Stable flags corresponding one-to-one with source vertices.
    pub vertex_flags: Vec<MeshVertexFlags>,
}

/// Result of conservative non-welding mesh repair.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshRepairReport {
    /// Repaired mesh containing only accepted faces and referenced vertices.
    pub mesh: Mesh,
    /// Number of removed faces.
    pub removed_face_count: usize,
    /// Number of removed vertices.
    pub removed_vertex_count: usize,
    /// Content hash before repair.
    pub before_hash: [u8; 32],
    /// Content hash after repair.
    pub after_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Default)]
struct EdgeStats {
    incident_faces: usize,
    orientation_balance: i32,
}

#[derive(Debug)]
struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(length: usize) -> Self {
        Self {
            parent: (0..length).collect(),
            rank: vec![0; length],
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            self.parent[value] = self.find(self.parent[value]);
        }
        self.parent[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }

        match self.rank[left_root].cmp(&self.rank[right_root]) {
            std::cmp::Ordering::Less => self.parent[left_root] = right_root,
            std::cmp::Ordering::Greater => self.parent[right_root] = left_root,
            std::cmp::Ordering::Equal => {
                self.parent[right_root] = left_root;
                self.rank[left_root] += 1;
            }
        }
    }
}

/// Audits a mesh without mutating source geometry.
#[allow(clippy::too_many_lines)]
pub fn audit_mesh(
    mesh: &Mesh,
    options: MeshAuditOptions,
) -> Result<MeshAuditReport, MeshAuditError> {
    let options =
        MeshAuditOptions::try_new(options.absolute_tolerance, options.maximum_f32_error_ratio)?;
    let vertex_count = mesh.positions.len();
    let face_count = mesh.triangles.len();
    let mut vertex_flags = vec![MeshVertexFlags::default(); vertex_count];
    let mut face_flags = vec![MeshFaceFlags::default(); face_count];
    let mut non_finite_vertex_count = 0;
    let mut bounds: Option<Aabb> = None;
    let mut maximum_coordinate_magnitude = 0.0_f64;

    for (index, position) in mesh.positions.iter().copied().enumerate() {
        if position.is_finite() {
            if let Some(bounds) = &mut bounds {
                bounds.include(position);
            } else {
                bounds = Some(Aabb::from_point(position));
            }
            maximum_coordinate_magnitude = maximum_coordinate_magnitude
                .max(position.x.abs())
                .max(position.y.abs())
                .max(position.z.abs());
        } else {
            vertex_flags[index].insert(MeshVertexFlags::NON_FINITE);
            non_finite_vertex_count += 1;
        }
    }

    let mut invalid_index_face_count = 0;
    let mut non_finite_face_count = 0;
    let mut degenerate_face_count = 0;
    let mut duplicate_face_count = 0;
    let mut accepted_face_count = 0;
    let mut surface_area = 0.0;
    let mut signed_volume = 0.0;
    let mut used_vertices = vec![false; vertex_count];
    let mut topology = BTreeMap::<(u32, u32), EdgeStats>::new();
    let mut unique_faces = BTreeSet::<[u32; 3]>::new();
    let mut components = DisjointSet::new(vertex_count);
    let area_threshold = 2.0 * options.absolute_tolerance.powi(2);

    for (face_index, indices) in mesh.triangles.iter().copied().enumerate() {
        if indices.iter().any(|&index| index as usize >= vertex_count) {
            face_flags[face_index].insert(MeshFaceFlags::INVALID_INDEX);
            invalid_index_face_count += 1;
            continue;
        }

        let [first, second, third] = indices.map(|index| mesh.positions[index as usize]);
        if !first.is_finite() || !second.is_finite() || !third.is_finite() {
            face_flags[face_index].insert(MeshFaceFlags::NON_FINITE_VERTEX);
            non_finite_face_count += 1;
            continue;
        }

        let doubled_area = (second - first)
            .cross(third - first)
            .length_squared()
            .sqrt();
        if indices[0] == indices[1]
            || indices[1] == indices[2]
            || indices[2] == indices[0]
            || doubled_area <= area_threshold
        {
            face_flags[face_index].insert(MeshFaceFlags::DEGENERATE);
            degenerate_face_count += 1;
            continue;
        }

        let mut canonical_face = indices;
        canonical_face.sort_unstable();
        if !unique_faces.insert(canonical_face) {
            face_flags[face_index].insert(MeshFaceFlags::DUPLICATE);
            duplicate_face_count += 1;
            continue;
        }

        accepted_face_count += 1;
        surface_area = 0.5_f64.mul_add(doubled_area, surface_area);
        signed_volume += first.dot(second.cross(third)) / 6.0;
        for &index in &indices {
            used_vertices[index as usize] = true;
        }
        components.union(indices[0] as usize, indices[1] as usize);
        components.union(indices[1] as usize, indices[2] as usize);

        for (start, end) in [
            (indices[0], indices[1]),
            (indices[1], indices[2]),
            (indices[2], indices[0]),
        ] {
            let (key, orientation) = if start < end {
                ((start, end), 1)
            } else {
                ((end, start), -1)
            };
            let statistics = topology.entry(key).or_default();
            statistics.incident_faces += 1;
            statistics.orientation_balance += orientation;
        }
    }

    let boundary_edge_count = topology
        .values()
        .filter(|statistics| statistics.incident_faces == 1)
        .count();
    let non_manifold_edge_count = topology
        .values()
        .filter(|statistics| statistics.incident_faces > 2)
        .count();
    let inconsistent_winding_edge_count = topology
        .values()
        .filter(|statistics| statistics.incident_faces == 2 && statistics.orientation_balance != 0)
        .count();

    let mut isolated_vertex_count = 0;
    for (index, used) in used_vertices.iter().copied().enumerate() {
        if !used && mesh.positions[index].is_finite() {
            vertex_flags[index].insert(MeshVertexFlags::ISOLATED);
            isolated_vertex_count += 1;
        }
    }

    let mut component_roots = BTreeSet::new();
    for (index, used) in used_vertices.iter().copied().enumerate() {
        if used {
            component_roots.insert(components.find(index));
        }
    }
    let connected_component_count = component_roots.len();
    let estimated_f32_error = maximum_coordinate_magnitude * f64::from(f32::EPSILON);
    let exceeds_f32_precision_budget =
        estimated_f32_error > options.absolute_tolerance * options.maximum_f32_error_ratio;
    let requires_face_repair = invalid_index_face_count > 0
        || non_finite_face_count > 0
        || degenerate_face_count > 0
        || duplicate_face_count > 0;
    let is_analysis_ready = accepted_face_count > 0 && !requires_face_repair;
    let is_watertight = is_analysis_ready
        && boundary_edge_count == 0
        && non_manifold_edge_count == 0
        && inconsistent_winding_edge_count == 0;

    Ok(MeshAuditReport {
        vertex_count,
        face_count,
        accepted_face_count,
        non_finite_vertex_count,
        invalid_index_face_count,
        non_finite_face_count,
        degenerate_face_count,
        duplicate_face_count,
        isolated_vertex_count,
        boundary_edge_count,
        non_manifold_edge_count,
        inconsistent_winding_edge_count,
        connected_component_count,
        surface_area,
        signed_volume,
        bounds,
        maximum_coordinate_magnitude,
        estimated_f32_error,
        exceeds_f32_precision_budget,
        is_analysis_ready,
        is_watertight,
        content_hash: hash_mesh(mesh),
        face_flags,
        vertex_flags,
    })
}

/// Computes a deterministic BLAKE3 hash over canonical mesh bytes.
#[must_use]
pub fn hash_mesh(mesh: &Mesh) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_MESH_V1\0");
    hasher.update(&(mesh.positions.len() as u64).to_le_bytes());
    for position in &mesh.positions {
        for coordinate in [position.x, position.y, position.z] {
            let normalized = if coordinate == 0.0 { 0.0 } else { coordinate };
            hasher.update(&normalized.to_bits().to_le_bytes());
        }
    }
    hasher.update(&(mesh.triangles.len() as u64).to_le_bytes());
    for triangle in &mesh.triangles {
        for index in triangle {
            hasher.update(&index.to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

/// Removes invalid, non-finite, degenerate, and duplicate faces and compacts vertices.
///
/// This policy never welds vertices, fills holes, or changes source geometry.
pub fn repair_mesh_conservative(
    mesh: &Mesh,
    options: MeshAuditOptions,
) -> Result<MeshRepairReport, MeshAuditError> {
    let audit = audit_mesh(mesh, options)?;
    let before_hash = audit.content_hash;
    let mut old_to_new = vec![None; mesh.positions.len()];
    let mut positions = Vec::new();
    let mut triangles = Vec::with_capacity(audit.accepted_face_count);

    for (indices, flags) in mesh.triangles.iter().zip(&audit.face_flags) {
        if flags.is_repairable() {
            continue;
        }

        let mut repaired = [0_u32; 3];
        for (slot, old_index) in indices.iter().copied().enumerate() {
            repaired[slot] = remap_vertex(old_index, mesh, &mut old_to_new, &mut positions)?;
        }
        triangles.push(repaired);
    }

    let repaired_mesh = Mesh {
        positions,
        triangles,
    };
    let after_hash = hash_mesh(&repaired_mesh);
    Ok(MeshRepairReport {
        removed_face_count: mesh.triangles.len() - repaired_mesh.triangles.len(),
        removed_vertex_count: mesh.positions.len() - repaired_mesh.positions.len(),
        mesh: repaired_mesh,
        before_hash,
        after_hash,
    })
}

fn remap_vertex(
    old_index: u32,
    mesh: &Mesh,
    old_to_new: &mut [Option<u32>],
    positions: &mut Vec<Vec3>,
) -> Result<u32, MeshAuditError> {
    if let Some(mapped) = old_to_new[old_index as usize] {
        return Ok(mapped);
    }

    let mapped = u32::try_from(positions.len()).map_err(|_| MeshAuditError::TooManyVertices)?;
    positions.push(mesh.positions[old_index as usize]);
    old_to_new[old_index as usize] = Some(mapped);
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> MeshAuditOptions {
        MeshAuditOptions::try_new(1.0e-9, 0.25).expect("valid test options")
    }

    fn open_quad() -> Mesh {
        Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    #[test]
    fn open_quad_has_four_boundary_edges_and_unit_area() {
        let report = audit_mesh(&open_quad(), options()).expect("audit succeeds");
        assert!(report.is_analysis_ready);
        assert!(!report.is_watertight);
        assert_eq!(report.boundary_edge_count, 4);
        assert_eq!(report.inconsistent_winding_edge_count, 0);
        assert_eq!(report.connected_component_count, 1);
        assert!((report.surface_area - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn closed_tetrahedron_reports_volume_and_watertight_topology() {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        };
        let report = audit_mesh(&mesh, options()).expect("audit succeeds");
        assert!(report.is_watertight);
        assert_eq!(report.boundary_edge_count, 0);
        assert_eq!(report.non_manifold_edge_count, 0);
        assert!((report.signed_volume - (1.0 / 6.0)).abs() < 1.0e-12);
    }

    #[test]
    fn duplicate_and_degenerate_faces_are_repairable() {
        let mut mesh = open_quad();
        mesh.positions.push(Vec3::new(9.0, 9.0, 9.0));
        mesh.triangles.push([2, 1, 0]);
        mesh.triangles.push([0, 0, 1]);

        let report = audit_mesh(&mesh, options()).expect("audit succeeds");
        assert_eq!(report.duplicate_face_count, 1);
        assert_eq!(report.degenerate_face_count, 1);
        assert_eq!(report.isolated_vertex_count, 1);
        assert!(!report.is_analysis_ready);

        let repair = repair_mesh_conservative(&mesh, options()).expect("repair succeeds");
        assert_eq!(repair.removed_face_count, 2);
        assert_eq!(repair.removed_vertex_count, 1);
        let repaired = audit_mesh(&repair.mesh, options()).expect("repaired audit succeeds");
        assert!(repaired.is_analysis_ready);
        assert!((repaired.surface_area - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn same_direction_shared_edge_is_reported() {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, -1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2], [0, 1, 3]],
        };
        let report = audit_mesh(&mesh, options()).expect("audit succeeds");
        assert_eq!(report.inconsistent_winding_edge_count, 1);
    }

    #[test]
    fn hash_normalizes_negative_zero_and_changes_with_topology() {
        let positive = open_quad();
        let mut negative = positive.clone();
        negative.positions[0].x = -0.0;
        assert_eq!(hash_mesh(&positive), hash_mesh(&negative));
        negative.triangles.swap(0, 1);
        assert_ne!(hash_mesh(&positive), hash_mesh(&negative));
    }

    #[test]
    fn large_coordinates_trigger_precision_budget_warning() {
        let mut mesh = open_quad();
        for point in &mut mesh.positions {
            point.x += 10_000_000.0;
        }
        let report = audit_mesh(
            &mesh,
            MeshAuditOptions::try_new(1.0e-3, 0.25).expect("valid options"),
        )
        .expect("audit succeeds");
        assert!(report.exceeds_f32_precision_budget);
        assert!(report.estimated_f32_error > 1.0);
    }
}
