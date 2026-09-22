//! Deterministic surface tessellation for area-aware analysis sensors.

use crate::{Mesh, Vec3};
use xvarna_types::SensorId;

const MAXIMUM_SUBDIVISION_DEPTH: u32 = 48;

/// Configuration for deterministic longest-edge surface sampling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceGridOptions {
    /// Maximum accepted analysis-cell edge length in model units.
    pub target_edge_length: f64,
    /// Distance applied along each cell normal to produce the sensor position.
    pub sensor_offset: f64,
    /// First deterministic sensor identifier assigned to the first accepted cell.
    pub first_sensor_id: u64,
    /// Hard safety ceiling for generated cells.
    pub maximum_cell_count: usize,
}

impl SurfaceGridOptions {
    /// Validates and creates surface-grid options.
    pub fn try_new(
        target_edge_length: f64,
        sensor_offset: f64,
        first_sensor_id: u64,
        maximum_cell_count: usize,
    ) -> Result<Self, SurfaceGridError> {
        if !target_edge_length.is_finite() || target_edge_length <= 0.0 {
            return Err(SurfaceGridError::InvalidTargetEdgeLength);
        }
        if !sensor_offset.is_finite() || sensor_offset < 0.0 {
            return Err(SurfaceGridError::InvalidSensorOffset);
        }
        if first_sensor_id == 0 {
            return Err(SurfaceGridError::InvalidFirstSensorId);
        }
        if maximum_cell_count == 0 {
            return Err(SurfaceGridError::InvalidMaximumCellCount);
        }
        Ok(Self {
            target_edge_length,
            sensor_offset,
            first_sensor_id,
            maximum_cell_count,
        })
    }
}

impl Default for SurfaceGridOptions {
    fn default() -> Self {
        Self {
            target_edge_length: 1.0,
            sensor_offset: 1.0e-4,
            first_sensor_id: 1,
            maximum_cell_count: 1_000_000,
        }
    }
}

/// One analysis triangle and its stable area-aware sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceCell {
    /// Stable sensor identifier.
    pub sensor_id: SensorId,
    /// Index of the source triangle before subdivision.
    pub source_face_index: u32,
    /// Longest-edge subdivision depth from the source face.
    pub subdivision_depth: u32,
    /// First analysis-cell corner.
    pub a: Vec3,
    /// Second analysis-cell corner.
    pub b: Vec3,
    /// Third analysis-cell corner.
    pub c: Vec3,
    /// Offset analysis position at the triangle centroid.
    pub position: Vec3,
    /// Unit face normal.
    pub normal: Vec3,
    /// Exact triangle area in squared model units.
    pub area: f64,
}

impl SurfaceCell {
    /// Returns the longest analysis-cell edge length.
    #[must_use]
    pub fn maximum_edge_length(self) -> f64 {
        maximum_edge_squared(self.a, self.b, self.c).sqrt()
    }
}

/// Complete deterministic surface-grid result.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceGridResult {
    /// Validated generation policy.
    pub options: SurfaceGridOptions,
    /// Number of source triangle records.
    pub source_face_count: usize,
    /// Invalid, non-finite, or degenerate source faces skipped explicitly.
    pub skipped_face_count: usize,
    /// Sum of valid source-face areas.
    pub source_area: f64,
    /// Sum of generated cell areas.
    pub sampled_area: f64,
    /// Longest generated cell edge.
    pub maximum_cell_edge_length: f64,
    /// Greatest generated subdivision depth.
    pub maximum_subdivision_depth: u32,
    /// Deterministically ordered analysis cells.
    pub cells: Vec<SurfaceCell>,
    /// BLAKE3 identity of source geometry, options, and generated cells.
    pub content_hash: [u8; 32],
}

/// Surface-grid validation and resource failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceGridError {
    /// Target edge length is non-finite or not positive.
    InvalidTargetEdgeLength,
    /// Sensor offset is non-finite or negative.
    InvalidSensorOffset,
    /// Stable sensor identifiers reserve zero as the absent value.
    InvalidFirstSensorId,
    /// Maximum cell count must be positive.
    InvalidMaximumCellCount,
    /// Generated cells would exceed the configured resource ceiling.
    CellLimitExceeded,
    /// Stable sensor identifiers would overflow `u64`.
    SensorIdOverflow,
    /// A source face index cannot be represented by the stable contract.
    SourceFaceIndexOverflow,
    /// Subdivision failed to converge within the defensive depth ceiling.
    SubdivisionDepthExceeded,
}

impl core::fmt::Display for SurfaceGridError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTargetEdgeLength => "target edge length must be finite and positive",
            Self::InvalidSensorOffset => "sensor offset must be finite and non-negative",
            Self::InvalidFirstSensorId => "first sensor ID must be non-zero",
            Self::InvalidMaximumCellCount => "maximum cell count must be positive",
            Self::CellLimitExceeded => "surface grid exceeded its configured cell limit",
            Self::SensorIdOverflow => "surface-grid sensor IDs overflowed u64",
            Self::SourceFaceIndexOverflow => "source face index overflowed u32",
            Self::SubdivisionDepthExceeded => "surface-grid subdivision exceeded its depth limit",
        })
    }
}

impl std::error::Error for SurfaceGridError {}

#[derive(Clone, Copy)]
struct PendingTriangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    source_face_index: u32,
    depth: u32,
}

/// Builds a deterministic area-preserving analysis grid by recursively bisecting
/// the longest edge of every source triangle.
#[allow(clippy::too_many_lines)]
pub fn generate_surface_grid(
    mesh: &Mesh,
    options: SurfaceGridOptions,
) -> Result<SurfaceGridResult, SurfaceGridError> {
    // Revalidate public fields for callers constructing the non-exhaustive policy directly.
    let options = SurfaceGridOptions::try_new(
        options.target_edge_length,
        options.sensor_offset,
        options.first_sensor_id,
        options.maximum_cell_count,
    )?;
    let target_squared = options.target_edge_length * options.target_edge_length;
    let mut pending = Vec::new();
    let mut cells = Vec::new();
    let mut skipped_face_count = 0_usize;
    let mut source_area = 0.0;
    let mut sampled_area = 0.0;
    let mut maximum_cell_edge_length = 0.0_f64;
    let mut maximum_subdivision_depth = 0_u32;

    for (face_index, indices) in mesh.triangles.iter().copied().enumerate() {
        let source_face_index =
            u32::try_from(face_index).map_err(|_| SurfaceGridError::SourceFaceIndexOverflow)?;
        let Some([a, b, c]) = indices
            .map(|index| mesh.positions.get(index as usize).copied())
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .and_then(|vertices| <[Vec3; 3]>::try_from(vertices).ok())
        else {
            skipped_face_count += 1;
            continue;
        };
        let cross = (b - a).cross(c - a);
        let doubled_area = cross.length_squared().sqrt();
        if !a.is_finite()
            || !b.is_finite()
            || !c.is_finite()
            || !doubled_area.is_finite()
            || doubled_area <= 0.0
        {
            skipped_face_count += 1;
            continue;
        }
        source_area = doubled_area.mul_add(0.5, source_area);
        pending.push(PendingTriangle {
            a,
            b,
            c,
            source_face_index,
            depth: 0,
        });

        while let Some(triangle) = pending.pop() {
            let longest = maximum_edge_squared(triangle.a, triangle.b, triangle.c);
            if longest > target_squared {
                if triangle.depth >= MAXIMUM_SUBDIVISION_DEPTH {
                    return Err(SurfaceGridError::SubdivisionDepthExceeded);
                }
                if cells.len().saturating_add(pending.len()) >= options.maximum_cell_count {
                    return Err(SurfaceGridError::CellLimitExceeded);
                }
                let [first, second] = split_longest_edge(triangle);
                // LIFO reversal preserves the documented deterministic first-child order.
                pending.push(second);
                pending.push(first);
                continue;
            }

            if cells.len() >= options.maximum_cell_count {
                return Err(SurfaceGridError::CellLimitExceeded);
            }
            let cross = (triangle.b - triangle.a).cross(triangle.c - triangle.a);
            let doubled_area = cross.length_squared().sqrt();
            let Some(normal) = cross.normalized() else {
                skipped_face_count += 1;
                continue;
            };
            let area = doubled_area * 0.5;
            let centroid = (triangle.a + triangle.b + triangle.c) * (1.0 / 3.0);
            let sensor_index =
                u64::try_from(cells.len()).map_err(|_| SurfaceGridError::SensorIdOverflow)?;
            let sensor_id = options
                .first_sensor_id
                .checked_add(sensor_index)
                .ok_or(SurfaceGridError::SensorIdOverflow)?;
            let edge_length = longest.sqrt();
            maximum_cell_edge_length = maximum_cell_edge_length.max(edge_length);
            maximum_subdivision_depth = maximum_subdivision_depth.max(triangle.depth);
            sampled_area += area;
            cells.push(SurfaceCell {
                sensor_id: SensorId::new(sensor_id),
                source_face_index: triangle.source_face_index,
                subdivision_depth: triangle.depth,
                a: triangle.a,
                b: triangle.b,
                c: triangle.c,
                position: centroid + normal * options.sensor_offset,
                normal,
                area,
            });
        }
    }

    let content_hash = hash_surface_grid(mesh, options, &cells, skipped_face_count);
    Ok(SurfaceGridResult {
        options,
        source_face_count: mesh.triangles.len(),
        skipped_face_count,
        source_area,
        sampled_area,
        maximum_cell_edge_length,
        maximum_subdivision_depth,
        cells,
        content_hash,
    })
}

fn maximum_edge_squared(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    (b - a)
        .length_squared()
        .max((c - b).length_squared())
        .max((a - c).length_squared())
}

fn split_longest_edge(triangle: PendingTriangle) -> [PendingTriangle; 2] {
    let ab = (triangle.b - triangle.a).length_squared();
    let bc = (triangle.c - triangle.b).length_squared();
    let ca = (triangle.a - triangle.c).length_squared();
    let depth = triangle.depth + 1;
    if ab >= bc && ab >= ca {
        let midpoint = (triangle.a + triangle.b) * 0.5;
        [
            PendingTriangle {
                a: triangle.a,
                b: midpoint,
                c: triangle.c,
                depth,
                ..triangle
            },
            PendingTriangle {
                a: midpoint,
                b: triangle.b,
                c: triangle.c,
                depth,
                ..triangle
            },
        ]
    } else if bc >= ca {
        let midpoint = (triangle.b + triangle.c) * 0.5;
        [
            PendingTriangle {
                a: triangle.a,
                b: triangle.b,
                c: midpoint,
                depth,
                ..triangle
            },
            PendingTriangle {
                a: triangle.a,
                b: midpoint,
                c: triangle.c,
                depth,
                ..triangle
            },
        ]
    } else {
        let midpoint = (triangle.c + triangle.a) * 0.5;
        [
            PendingTriangle {
                a: triangle.a,
                b: triangle.b,
                c: midpoint,
                depth,
                ..triangle
            },
            PendingTriangle {
                a: midpoint,
                b: triangle.b,
                c: triangle.c,
                depth,
                ..triangle
            },
        ]
    }
}

fn hash_surface_grid(
    mesh: &Mesh,
    options: SurfaceGridOptions,
    cells: &[SurfaceCell],
    skipped_face_count: usize,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"xvarna.surface-grid.v1\0");
    hasher.update(&options.target_edge_length.to_bits().to_le_bytes());
    hasher.update(&options.sensor_offset.to_bits().to_le_bytes());
    hasher.update(&options.first_sensor_id.to_le_bytes());
    hasher.update(&(options.maximum_cell_count as u64).to_le_bytes());
    hasher.update(&(mesh.positions.len() as u64).to_le_bytes());
    for point in &mesh.positions {
        for value in [point.x, point.y, point.z] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    hasher.update(&(mesh.triangles.len() as u64).to_le_bytes());
    for triangle in &mesh.triangles {
        for index in triangle {
            hasher.update(&index.to_le_bytes());
        }
    }
    hasher.update(&(skipped_face_count as u64).to_le_bytes());
    hasher.update(&(cells.len() as u64).to_le_bytes());
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_quad() -> Mesh {
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
    fn surface_grid_preserves_area_and_edge_limit() {
        let result = generate_surface_grid(
            &unit_quad(),
            SurfaceGridOptions::try_new(0.36, 0.01, 100, 10_000).expect("valid options"),
        )
        .expect("grid succeeds");
        assert!(result.cells.len() > 2);
        assert!((result.source_area - 1.0).abs() < 1.0e-12);
        assert!((result.sampled_area - 1.0).abs() < 1.0e-12);
        assert!(result.maximum_cell_edge_length <= 0.36 + 1.0e-12);
        assert_eq!(result.cells[0].sensor_id, SensorId::new(100));
        assert_eq!(
            result.cells.last().expect("cell").sensor_id.get(),
            99 + result.cells.len() as u64
        );
        assert!(
            result
                .cells
                .iter()
                .all(|cell| (cell.position.z - 0.01).abs() < 1.0e-12)
        );
    }

    #[test]
    fn surface_grid_is_bitwise_deterministic() {
        let options = SurfaceGridOptions::try_new(0.5, 0.0, 1, 1_000).expect("valid options");
        let first = generate_surface_grid(&unit_quad(), options).expect("first grid");
        let second = generate_surface_grid(&unit_quad(), options).expect("second grid");
        assert_eq!(first, second);
    }

    #[test]
    fn surface_grid_skips_invalid_faces_and_enforces_limit() {
        let mut mesh = unit_quad();
        mesh.triangles.push([0, 20, 1]);
        let result = generate_surface_grid(
            &mesh,
            SurfaceGridOptions::try_new(2.0, 0.0, 1, 10).expect("valid options"),
        )
        .expect("invalid source face is reported, not fatal");
        assert_eq!(result.skipped_face_count, 1);

        let error = generate_surface_grid(
            &unit_quad(),
            SurfaceGridOptions::try_new(0.01, 0.0, 1, 4).expect("valid options"),
        )
        .expect_err("limit must stop excessive subdivision");
        assert_eq!(error, SurfaceGridError::CellLimitExceeded);
    }
}
