//! Deterministic optional welding, winding repair, and exact box clipping.

use crate::{
    Aabb, Mesh, MeshAuditError, MeshAuditOptions, Vec3, audit_mesh, repair_mesh_conservative,
};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
};

/// Explicit opt-in mesh healing policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshHealOptions {
    /// Audit and degeneracy tolerance.
    pub audit: MeshAuditOptions,
    /// Maximum Euclidean weld distance; zero disables welding.
    pub weld_tolerance: f64,
    /// Make adjacent manifold faces use opposite directions along shared edges.
    pub repair_winding: bool,
    /// Flip closed consistently-wound components to positive signed volume.
    pub orient_closed_components_outward: bool,
}

impl Default for MeshHealOptions {
    fn default() -> Self {
        Self {
            audit: MeshAuditOptions::default(),
            weld_tolerance: 0.0,
            repair_winding: true,
            orient_closed_components_outward: true,
        }
    }
}

/// Complete provenance for opt-in mesh healing.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshHealReport {
    /// Healed canonical triangle mesh.
    pub mesh: Mesh,
    /// Invalid/duplicate/degenerate source faces removed before and after welding.
    pub removed_face_count: usize,
    /// Unreferenced or invalid vertices removed during compaction.
    pub removed_vertex_count: usize,
    /// Vertices merged into an earlier deterministic representative.
    pub welded_vertex_count: usize,
    /// Faces reversed to make manifold adjacency consistent.
    pub consistency_flip_count: usize,
    /// Closed components reversed to positive signed volume.
    pub outward_component_flip_count: usize,
    /// Non-manifold or contradictory adjacency relations not used for propagation.
    pub unresolved_winding_edge_count: usize,
    /// Source content identity.
    pub before_hash: [u8; 32],
    /// Healed content identity.
    pub after_hash: [u8; 32],
}

/// Invalid healing policy or geometry range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshHealError {
    /// Audit policy is invalid.
    Audit(MeshAuditError),
    /// Weld distance is negative or non-finite.
    InvalidWeldTolerance,
    /// Coordinates cannot be represented by the deterministic spatial grid.
    CoordinateRangeExceeded,
}

impl fmt::Display for MeshHealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Audit(_) => "mesh audit policy is invalid",
            Self::InvalidWeldTolerance => "weld tolerance must be finite and non-negative",
            Self::CoordinateRangeExceeded => "mesh coordinates exceed the weld-grid range",
        })
    }
}

impl std::error::Error for MeshHealError {}

impl From<MeshAuditError> for MeshHealError {
    fn from(value: MeshAuditError) -> Self {
        Self::Audit(value)
    }
}

/// Removes unsafe faces, optionally welds vertices, and repairs manifold winding.
pub fn heal_mesh(mesh: &Mesh, options: MeshHealOptions) -> Result<MeshHealReport, MeshHealError> {
    if !options.weld_tolerance.is_finite() || options.weld_tolerance < 0.0 {
        return Err(MeshHealError::InvalidWeldTolerance);
    }
    let before = audit_mesh(mesh, options.audit)?;
    let first = repair_mesh_conservative(mesh, options.audit)?;
    let initial_removed_faces = first.removed_face_count;
    let initial_removed_vertices = first.removed_vertex_count;
    let (mut current, welded_vertex_count) = if options.weld_tolerance > 0.0 {
        weld_mesh(&first.mesh, options.weld_tolerance)?
    } else {
        (first.mesh, 0)
    };
    let after_weld = repair_mesh_conservative(&current, options.audit)?;
    let weld_removed_faces = after_weld.removed_face_count;
    let weld_removed_vertices = after_weld.removed_vertex_count;
    current = after_weld.mesh;

    let winding = if options.repair_winding {
        repair_winding(&mut current, options.orient_closed_components_outward)
    } else {
        WindingReport::default()
    };
    let final_audit = audit_mesh(&current, options.audit)?;
    Ok(MeshHealReport {
        mesh: current,
        removed_face_count: initial_removed_faces + weld_removed_faces,
        removed_vertex_count: initial_removed_vertices + weld_removed_vertices,
        welded_vertex_count,
        consistency_flip_count: winding.consistency_flips,
        outward_component_flip_count: winding.outward_component_flips,
        unresolved_winding_edge_count: winding.unresolved_edges,
        before_hash: before.content_hash,
        after_hash: final_audit.content_hash,
    })
}

fn weld_mesh(mesh: &Mesh, tolerance: f64) -> Result<(Mesh, usize), MeshHealError> {
    let mut buckets = BTreeMap::<(i64, i64, i64), Vec<u32>>::new();
    let mut positions = Vec::<Vec3>::with_capacity(mesh.positions.len());
    let mut remap = Vec::<u32>::with_capacity(mesh.positions.len());
    let tolerance_squared = tolerance * tolerance;
    let mut welded = 0_usize;

    for point in &mesh.positions {
        let cell = grid_cell(*point, tolerance)?;
        let mut selected: Option<u32> = None;
        'neighbors: for dx in -1_i64..=1 {
            for dy in -1_i64..=1 {
                for dz in -1_i64..=1 {
                    let key = (
                        cell.0.saturating_add(dx),
                        cell.1.saturating_add(dy),
                        cell.2.saturating_add(dz),
                    );
                    if let Some(candidates) = buckets.get(&key) {
                        for &candidate in candidates {
                            if (positions[candidate as usize] - *point).length_squared()
                                <= tolerance_squared
                            {
                                selected = Some(candidate);
                                break 'neighbors;
                            }
                        }
                    }
                }
            }
        }
        let index = if let Some(index) = selected {
            welded += 1;
            index
        } else {
            let index = u32::try_from(positions.len())
                .map_err(|_| MeshHealError::Audit(MeshAuditError::TooManyVertices))?;
            positions.push(*point);
            buckets.entry(cell).or_default().push(index);
            index
        };
        remap.push(index);
    }
    let triangles = mesh
        .triangles
        .iter()
        .map(|triangle| triangle.map(|index| remap[index as usize]))
        .collect();
    Ok((
        Mesh {
            positions,
            triangles,
        },
        welded,
    ))
}

fn grid_cell(point: Vec3, tolerance: f64) -> Result<(i64, i64, i64), MeshHealError> {
    // The range check is the proof obligation for this deliberately quantising cast.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn coordinate(value: f64, tolerance: f64) -> Result<i64, MeshHealError> {
        let scaled = (value / tolerance).floor();
        if scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
            return Err(MeshHealError::CoordinateRangeExceeded);
        }
        Ok(scaled as i64)
    }
    Ok((
        coordinate(point.x, tolerance)?,
        coordinate(point.y, tolerance)?,
        coordinate(point.z, tolerance)?,
    ))
}

#[derive(Clone, Copy, Debug, Default)]
struct WindingReport {
    consistency_flips: usize,
    outward_component_flips: usize,
    unresolved_edges: usize,
}

#[derive(Clone, Copy, Debug)]
struct EdgeUse {
    face: usize,
    forward: bool,
}

fn repair_winding(mesh: &mut Mesh, orient_outward: bool) -> WindingReport {
    let mut edges = BTreeMap::<(u32, u32), Vec<EdgeUse>>::new();
    for (face, triangle) in mesh.triangles.iter().copied().enumerate() {
        for (first, second) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let (key, forward) = if first < second {
                ((first, second), true)
            } else {
                ((second, first), false)
            };
            edges
                .entry(key)
                .or_default()
                .push(EdgeUse { face, forward });
        }
    }

    let mut adjacency = vec![Vec::<(usize, bool)>::new(); mesh.triangles.len()];
    let mut report = WindingReport::default();
    for uses in edges.values() {
        if let [first, second] = uses.as_slice() {
            let same_direction = first.forward == second.forward;
            adjacency[first.face].push((second.face, same_direction));
            adjacency[second.face].push((first.face, same_direction));
        } else {
            report.unresolved_edges += 1;
        }
    }

    let mut parity = vec![None::<bool>; mesh.triangles.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for root in 0..mesh.triangles.len() {
        if parity[root].is_some() {
            continue;
        }
        parity[root] = Some(false);
        let mut queue = VecDeque::from([root]);
        let mut component = Vec::new();
        while let Some(face) = queue.pop_front() {
            component.push(face);
            let current = parity[face].expect("queued face has parity");
            for &(neighbor, must_differ) in &adjacency[face] {
                let expected = current ^ must_differ;
                match parity[neighbor] {
                    None => {
                        parity[neighbor] = Some(expected);
                        queue.push_back(neighbor);
                    }
                    Some(actual) if actual != expected => report.unresolved_edges += 1,
                    Some(_) => {}
                }
            }
        }
        components.push(component);
    }

    for (face, flip) in parity.iter().copied().enumerate() {
        if flip.unwrap_or(false) {
            mesh.triangles[face].swap(1, 2);
            report.consistency_flips += 1;
        }
    }

    if orient_outward {
        for component in components {
            if component_is_closed(&component, &edges)
                && signed_component_volume(mesh, &component) < 0.0
            {
                for face in component {
                    mesh.triangles[face].swap(1, 2);
                }
                report.outward_component_flips += 1;
            }
        }
    }
    report
}

fn component_is_closed(component: &[usize], edges: &BTreeMap<(u32, u32), Vec<EdgeUse>>) -> bool {
    let mut membership = vec![
        false;
        edges
            .values()
            .flatten()
            .map(|usage| usage.face)
            .max()
            .unwrap_or(0)
            + 1
    ];
    for &face in component {
        membership[face] = true;
    }
    edges.values().all(|uses| {
        let incident = uses.iter().filter(|usage| membership[usage.face]).count();
        incident == 0 || incident == 2
    })
}

fn signed_component_volume(mesh: &Mesh, component: &[usize]) -> f64 {
    component.iter().fold(0.0, |volume, &face| {
        let triangle = mesh.triangles[face];
        let [a, b, c] = triangle.map(|index| mesh.positions[index as usize]);
        a.dot(b.cross(c)).mul_add(1.0 / 6.0, volume)
    })
}

/// Exact clipping statistics for one axis-aligned region of interest.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshClipReport {
    /// Clipped and triangulated output.
    pub mesh: Mesh,
    /// Source triangles entirely outside the ROI.
    pub rejected_triangle_count: usize,
    /// Source triangles intersected by one or more ROI planes.
    pub clipped_triangle_count: usize,
    /// Final triangles after polygon clipping and fan triangulation.
    pub output_triangle_count: usize,
}

/// Invalid ROI or post-clip healing failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshClipError {
    /// Bounds contain non-finite coordinates or have inverted axes.
    InvalidBounds,
    /// Clip tolerance is non-finite or not positive.
    InvalidTolerance,
    /// Post-clip canonicalization failed.
    Heal(MeshHealError),
}

impl fmt::Display for MeshClipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBounds => "ROI bounds must be finite and ordered",
            Self::InvalidTolerance => "ROI tolerance must be finite and positive",
            Self::Heal(_) => "clipped mesh canonicalization failed",
        })
    }
}

impl std::error::Error for MeshClipError {}

/// Clips every triangle against all six ROI planes and canonicalizes the result.
pub fn clip_mesh_to_aabb(
    mesh: &Mesh,
    bounds: Aabb,
    tolerance: f64,
) -> Result<MeshClipReport, MeshClipError> {
    if !bounds.min.is_finite()
        || !bounds.max.is_finite()
        || bounds.min.x > bounds.max.x
        || bounds.min.y > bounds.max.y
        || bounds.min.z > bounds.max.z
    {
        return Err(MeshClipError::InvalidBounds);
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(MeshClipError::InvalidTolerance);
    }
    let planes = [
        (0_usize, bounds.min.x, true),
        (0, bounds.max.x, false),
        (1, bounds.min.y, true),
        (1, bounds.max.y, false),
        (2, bounds.min.z, true),
        (2, bounds.max.z, false),
    ];
    let mut raw = Mesh::default();
    let mut rejected = 0_usize;
    let mut clipped = 0_usize;
    for triangle in &mesh.triangles {
        let Some(vertices) = triangle
            .map(|index| mesh.positions.get(index as usize).copied())
            .into_iter()
            .collect::<Option<Vec<_>>>()
        else {
            rejected += 1;
            continue;
        };
        let mut polygon = vertices.clone();
        for plane in planes {
            polygon = clip_polygon(&polygon, plane, tolerance);
            if polygon.is_empty() {
                break;
            }
        }
        if polygon.len() < 3 {
            rejected += 1;
            continue;
        }
        if polygon.len() != 3 || polygon != vertices {
            clipped += 1;
        }
        let base = u32::try_from(raw.positions.len()).map_err(|_| {
            MeshClipError::Heal(MeshHealError::Audit(MeshAuditError::TooManyVertices))
        })?;
        raw.positions.extend_from_slice(&polygon);
        for offset in 1..polygon.len() - 1 {
            raw.triangles.push([
                base,
                base + u32::try_from(offset).expect("polygon index fits u32"),
                base + u32::try_from(offset + 1).expect("polygon index fits u32"),
            ]);
        }
    }
    if raw.triangles.is_empty() {
        return Ok(MeshClipReport {
            mesh: raw,
            rejected_triangle_count: rejected,
            clipped_triangle_count: clipped,
            output_triangle_count: 0,
        });
    }
    let healed = heal_mesh(
        &raw,
        MeshHealOptions {
            audit: MeshAuditOptions::try_new(tolerance, 0.25)
                .map_err(|error| MeshClipError::Heal(error.into()))?,
            weld_tolerance: tolerance,
            repair_winding: false,
            orient_closed_components_outward: false,
        },
    )
    .map_err(MeshClipError::Heal)?;
    let output_triangle_count = healed.mesh.triangles.len();
    Ok(MeshClipReport {
        mesh: healed.mesh,
        rejected_triangle_count: rejected,
        clipped_triangle_count: clipped,
        output_triangle_count,
    })
}

fn clip_polygon(polygon: &[Vec3], plane: (usize, f64, bool), tolerance: f64) -> Vec<Vec3> {
    if polygon.is_empty() {
        return Vec::new();
    }
    let mut output = Vec::with_capacity(polygon.len() + 1);
    let mut previous = *polygon.last().expect("non-empty polygon");
    let mut previous_inside = inside(previous, plane, tolerance);
    for &current in polygon {
        let current_inside = inside(current, plane, tolerance);
        if current_inside != previous_inside {
            output.push(plane_intersection(previous, current, plane));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output
}

fn inside(point: Vec3, plane: (usize, f64, bool), tolerance: f64) -> bool {
    let coordinate = component(point, plane.0);
    if plane.2 {
        coordinate >= plane.1 - tolerance
    } else {
        coordinate <= plane.1 + tolerance
    }
}

fn plane_intersection(first: Vec3, second: Vec3, plane: (usize, f64, bool)) -> Vec3 {
    let first_coordinate = component(first, plane.0);
    let denominator = component(second, plane.0) - first_coordinate;
    let parameter = if denominator.abs() <= f64::EPSILON {
        0.0
    } else {
        ((plane.1 - first_coordinate) / denominator).clamp(0.0, 1.0)
    };
    first + (second - first) * parameter
}

const fn component(point: Vec3, axis: usize) -> f64 {
    match axis {
        0 => point.x,
        1 => point.y,
        _ => point.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welding_collapses_near_vertices_and_removes_collapsed_face() {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.000_01, 0.0, 0.0),
            ],
            triangles: vec![[0, 1, 2], [3, 1, 2]],
        };
        let report = heal_mesh(
            &mesh,
            MeshHealOptions {
                weld_tolerance: 0.001,
                ..MeshHealOptions::default()
            },
        )
        .expect("healing succeeds");
        assert_eq!(report.welded_vertex_count, 1);
        assert_eq!(report.mesh.triangles.len(), 1);
        assert_eq!(report.mesh.positions.len(), 3);
    }

    #[test]
    fn shared_edge_winding_is_repaired() {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2], [0, 3, 2]],
        };
        let report = heal_mesh(&mesh, MeshHealOptions::default()).expect("healing succeeds");
        assert_eq!(report.consistency_flip_count, 1);
        let audit = audit_mesh(&report.mesh, MeshAuditOptions::default()).expect("audit succeeds");
        assert_eq!(audit.inconsistent_winding_edge_count, 0);
    }

    #[test]
    fn triangle_is_exactly_clipped_to_box() {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let report = clip_mesh_to_aabb(
            &mesh,
            Aabb::new(Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            1.0e-9,
        )
        .expect("clip succeeds");
        assert_eq!(report.clipped_triangle_count, 1);
        assert!(report.output_triangle_count >= 2);
        assert!(
            report
                .mesh
                .positions
                .iter()
                .all(|point| { (0.0..=1.0).contains(&point.x) && (0.0..=1.0).contains(&point.y) })
        );
    }
}
