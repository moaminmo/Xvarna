//! Strict ASCII/binary STL conversion with deterministic vertex deduplication.

use std::{collections::BTreeMap, fmt, fmt::Write};
use xvarna_geometry::{Mesh, Vec3};

const BINARY_HEADER_BYTES: usize = 84;
const BINARY_TRIANGLE_BYTES: usize = 50;

/// Successful STL conversion statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct StlImportReport {
    /// Canonical triangle mesh.
    pub mesh: Mesh,
    /// True when the source used binary STL framing.
    pub binary: bool,
    /// Triangle facets consumed from the source.
    pub source_triangle_count: usize,
    /// Exact duplicate positions eliminated across facets.
    pub deduplicated_vertex_count: usize,
}

/// Invalid STL syntax, framing, or numeric data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StlError {
    /// Input is neither valid UTF-8 ASCII STL nor a complete binary STL.
    InvalidEncoding,
    /// Binary count/length framing is inconsistent.
    InvalidBinaryLength,
    /// A vertex is absent, malformed, or non-finite.
    InvalidVertex,
    /// ASCII facets do not contain exactly three vertices.
    InvalidFacet,
    /// Canonical vertex indices exceed u32.
    TooManyVertices,
    /// Binary triangle count exceeds u32.
    TooManyTriangles,
}

impl fmt::Display for StlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEncoding => "STL is neither valid ASCII nor complete binary data",
            Self::InvalidBinaryLength => "binary STL triangle count does not match file length",
            Self::InvalidVertex => "STL contains a missing, malformed, or non-finite vertex",
            Self::InvalidFacet => "ASCII STL facet does not contain exactly three vertices",
            Self::TooManyVertices => "STL exceeds the canonical u32 vertex range",
            Self::TooManyTriangles => "STL exceeds the binary u32 triangle range",
        })
    }
}

impl std::error::Error for StlError {}

/// Auto-detects and parses binary or ASCII STL.
pub fn parse_stl(bytes: &[u8]) -> Result<StlImportReport, StlError> {
    if let Some(count) = binary_triangle_count(bytes)? {
        return parse_binary(bytes, count);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| StlError::InvalidEncoding)?;
    parse_ascii(text)
}

fn binary_triangle_count(bytes: &[u8]) -> Result<Option<usize>, StlError> {
    if bytes.len() < BINARY_HEADER_BYTES {
        return Ok(None);
    }
    let count = u32::from_le_bytes(bytes[80..84].try_into().expect("fixed count slice"));
    let count = usize::try_from(count).map_err(|_| StlError::TooManyTriangles)?;
    let expected = BINARY_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(BINARY_TRIANGLE_BYTES)
                .ok_or(StlError::InvalidBinaryLength)?,
        )
        .ok_or(StlError::InvalidBinaryLength)?;
    if expected == bytes.len() {
        Ok(Some(count))
    } else {
        Ok(None)
    }
}

fn parse_binary(bytes: &[u8], triangle_count: usize) -> Result<StlImportReport, StlError> {
    let mut mesh = Mesh {
        positions: Vec::with_capacity(triangle_count.saturating_mul(3)),
        triangles: Vec::with_capacity(triangle_count),
    };
    let mut lookup = BTreeMap::new();
    for triangle in 0..triangle_count {
        let offset = BINARY_HEADER_BYTES + triangle * BINARY_TRIANGLE_BYTES + 12;
        let mut indices = [0_u32; 3];
        for (vertex, index) in indices.iter_mut().enumerate() {
            let start = offset + vertex * 12;
            let point = Vec3::new(
                f64::from(read_f32(bytes, start)),
                f64::from(read_f32(bytes, start + 4)),
                f64::from(read_f32(bytes, start + 8)),
            );
            if !point.is_finite() {
                return Err(StlError::InvalidVertex);
            }
            *index = intern(point, &mut mesh.positions, &mut lookup)?;
        }
        mesh.triangles.push(indices);
    }
    let deduplicated_vertex_count = triangle_count
        .saturating_mul(3)
        .saturating_sub(mesh.positions.len());
    Ok(StlImportReport {
        mesh,
        binary: true,
        source_triangle_count: triangle_count,
        deduplicated_vertex_count,
    })
}

fn parse_ascii(text: &str) -> Result<StlImportReport, StlError> {
    let mut mesh = Mesh::default();
    let mut lookup = BTreeMap::new();
    let mut facet = Vec::with_capacity(3);
    let mut saw_solid = false;
    let mut source_triangle_count = 0_usize;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        match fields
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "solid" => saw_solid = true,
            "vertex" => {
                let point = Vec3::new(
                    parse_coordinate(fields.next())?,
                    parse_coordinate(fields.next())?,
                    parse_coordinate(fields.next())?,
                );
                if fields.next().is_some() || !point.is_finite() {
                    return Err(StlError::InvalidVertex);
                }
                facet.push(intern(point, &mut mesh.positions, &mut lookup)?);
            }
            "endfacet" => {
                if facet.len() != 3 {
                    return Err(StlError::InvalidFacet);
                }
                mesh.triangles.push([facet[0], facet[1], facet[2]]);
                facet.clear();
                source_triangle_count += 1;
            }
            _ => {}
        }
    }
    if !saw_solid || !facet.is_empty() || mesh.triangles.is_empty() {
        return Err(StlError::InvalidEncoding);
    }
    let deduplicated_vertex_count = source_triangle_count
        .saturating_mul(3)
        .saturating_sub(mesh.positions.len());
    Ok(StlImportReport {
        mesh,
        binary: false,
        source_triangle_count,
        deduplicated_vertex_count,
    })
}

fn parse_coordinate(value: Option<&str>) -> Result<f64, StlError> {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .ok_or(StlError::InvalidVertex)
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated STL framing"),
    )
}

fn intern(
    point: Vec3,
    positions: &mut Vec<Vec3>,
    lookup: &mut BTreeMap<(u64, u64, u64), u32>,
) -> Result<u32, StlError> {
    let key = (
        canonical_bits(point.x),
        canonical_bits(point.y),
        canonical_bits(point.z),
    );
    if let Some(index) = lookup.get(&key) {
        return Ok(*index);
    }
    let index = u32::try_from(positions.len()).map_err(|_| StlError::TooManyVertices)?;
    positions.push(point);
    lookup.insert(key, index);
    Ok(index)
}

const fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

/// Writes deterministic binary STL with derived unit normals.
// Binary STL is specified in IEEE-754 f32; callers can use PLY for lossless f64.
#[allow(clippy::cast_possible_truncation)]
pub fn write_stl_binary(mesh: &Mesh) -> Result<Vec<u8>, StlError> {
    let count = u32::try_from(mesh.triangles.len()).map_err(|_| StlError::TooManyTriangles)?;
    let mut bytes = vec![0_u8; BINARY_HEADER_BYTES];
    let label = b"XVARNA canonical binary STL";
    bytes[..label.len()].copy_from_slice(label);
    bytes[80..84].copy_from_slice(&count.to_le_bytes());
    for triangle in &mesh.triangles {
        let vertices = triangle
            .map(|index| mesh.positions.get(index as usize).copied())
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(StlError::InvalidVertex)?;
        let normal = (vertices[1] - vertices[0])
            .cross(vertices[2] - vertices[0])
            .normalized()
            .unwrap_or(Vec3::ZERO);
        for value in [normal.x, normal.y, normal.z] {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        }
        for vertex in vertices {
            for value in [vertex.x, vertex.y, vertex.z] {
                if !value.is_finite() {
                    return Err(StlError::InvalidVertex);
                }
                bytes.extend_from_slice(&(value as f32).to_le_bytes());
            }
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
    }
    Ok(bytes)
}

/// Writes deterministic ASCII STL with derived unit normals.
pub fn write_stl_text(mesh: &Mesh) -> Result<String, StlError> {
    let mut output = String::from("solid xvarna\n");
    for triangle in &mesh.triangles {
        let vertices = triangle
            .map(|index| mesh.positions.get(index as usize).copied())
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(StlError::InvalidVertex)?;
        let normal = (vertices[1] - vertices[0])
            .cross(vertices[2] - vertices[0])
            .normalized()
            .unwrap_or(Vec3::ZERO);
        writeln!(
            output,
            "  facet normal {:.17} {:.17} {:.17}",
            normal.x, normal.y, normal.z
        )
        .expect("writing to String cannot fail");
        output.push_str("    outer loop\n");
        for vertex in vertices {
            writeln!(
                output,
                "      vertex {:.17} {:.17} {:.17}",
                vertex.x, vertex.y, vertex.z
            )
            .expect("writing to String cannot fail");
        }
        output.push_str("    endloop\n  endfacet\n");
    }
    output.push_str("endsolid xvarna\n");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh() -> Mesh {
        Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn binary_and_ascii_round_trip() {
        let binary = write_stl_binary(&mesh()).expect("binary writes");
        let binary_report = parse_stl(&binary).expect("binary parses");
        assert!(binary_report.binary);
        assert_eq!(binary_report.mesh, mesh());

        let text = write_stl_text(&mesh()).expect("ASCII writes");
        let text_report = parse_stl(text.as_bytes()).expect("ASCII parses");
        assert!(!text_report.binary);
        assert_eq!(text_report.mesh, mesh());
    }
}
