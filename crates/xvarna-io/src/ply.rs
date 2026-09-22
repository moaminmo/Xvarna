//! Auditable ASCII and little-endian binary PLY mesh conversion.

use std::fmt;
use xvarna_geometry::{Mesh, Vec3};

/// Supported PLY source/writer encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlyEncoding {
    /// Whitespace-delimited PLY 1.0.
    Text,
    /// Little-endian binary PLY 1.0.
    BinaryLittleEndian,
}

/// Successful PLY conversion statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct PlyImportReport {
    /// Canonical triangle mesh.
    pub mesh: Mesh,
    /// Source encoding.
    pub encoding: PlyEncoding,
    /// Source polygon count.
    pub source_face_count: usize,
    /// Quad polygons split using the deterministic 0–2 diagonal.
    pub triangulated_quad_count: usize,
    /// Extra vertex scalar properties consumed and ignored.
    pub ignored_vertex_property_count: usize,
}

/// PLY header, schema, payload, or numeric failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlyError {
    /// Magic/header framing is invalid.
    InvalidHeader,
    /// Source encoding is unsupported.
    UnsupportedEncoding,
    /// Required x/y/z or vertex-index properties are missing.
    MissingProperty(&'static str),
    /// A property scalar/list type is unsupported.
    UnsupportedPropertyType(String),
    /// Text or binary payload ended before all declared elements.
    TruncatedPayload,
    /// Vertex coordinate is malformed or non-finite.
    InvalidVertex,
    /// Face indices are malformed or outside the vertex buffer.
    InvalidFace,
    /// Polygons other than triangles/quads require an explicit triangulator.
    UnsupportedPolygon(usize),
    /// Canonical index range exceeded.
    TooManyVertices,
}

impl fmt::Display for PlyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => formatter.write_str("invalid PLY 1.0 header"),
            Self::UnsupportedEncoding => formatter.write_str("unsupported PLY encoding"),
            Self::MissingProperty(name) => {
                write!(formatter, "PLY is missing required property {name}")
            }
            Self::UnsupportedPropertyType(kind) => {
                write!(formatter, "unsupported PLY property type {kind}")
            }
            Self::TruncatedPayload => formatter.write_str("PLY payload is truncated"),
            Self::InvalidVertex => formatter.write_str("PLY contains an invalid vertex"),
            Self::InvalidFace => formatter.write_str("PLY contains an invalid face index"),
            Self::UnsupportedPolygon(count) => write!(formatter, "PLY {count}-gon is unsupported"),
            Self::TooManyVertices => {
                formatter.write_str("PLY exceeds the canonical u32 vertex range")
            }
        }
    }
}

impl std::error::Error for PlyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl ScalarType {
    fn parse(value: &str) -> Result<Self, PlyError> {
        match value {
            "char" | "int8" => Ok(Self::I8),
            "uchar" | "uint8" => Ok(Self::U8),
            "short" | "int16" => Ok(Self::I16),
            "ushort" | "uint16" => Ok(Self::U16),
            "int" | "int32" => Ok(Self::I32),
            "uint" | "uint32" => Ok(Self::U32),
            "float" | "float32" => Ok(Self::F32),
            "double" | "float64" => Ok(Self::F64),
            other => Err(PlyError::UnsupportedPropertyType(other.to_owned())),
        }
    }

    const fn bytes(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

#[derive(Clone, Debug)]
enum Property {
    Scalar {
        kind: ScalarType,
        name: String,
    },
    List {
        count: ScalarType,
        item: ScalarType,
        name: String,
    },
}

#[derive(Clone, Debug)]
struct Header {
    encoding: PlyEncoding,
    vertex_count: usize,
    face_count: usize,
    vertex_properties: Vec<Property>,
    face_properties: Vec<Property>,
    payload_offset: usize,
}

/// Parses PLY 1.0 ASCII or little-endian binary meshes.
pub fn parse_ply(bytes: &[u8]) -> Result<PlyImportReport, PlyError> {
    let header = parse_header(bytes)?;
    match header.encoding {
        PlyEncoding::Text => parse_text(bytes, &header),
        PlyEncoding::BinaryLittleEndian => parse_binary(bytes, &header),
    }
}

fn parse_header(bytes: &[u8]) -> Result<Header, PlyError> {
    let marker = b"end_header";
    let marker_start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or(PlyError::InvalidHeader)?;
    let after_marker = marker_start + marker.len();
    let payload_offset = match bytes.get(after_marker..after_marker + 2) {
        Some(b"\r\n") => after_marker + 2,
        _ if bytes.get(after_marker) == Some(&b'\n') => after_marker + 1,
        _ => return Err(PlyError::InvalidHeader),
    };
    let header_text =
        std::str::from_utf8(&bytes[..marker_start]).map_err(|_| PlyError::InvalidHeader)?;
    let mut lines = header_text.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err(PlyError::InvalidHeader);
    }
    let format = lines.next().ok_or(PlyError::InvalidHeader)?;
    let encoding = match format.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["format", "ascii", "1.0"] => PlyEncoding::Text,
        ["format", "binary_little_endian", "1.0"] => PlyEncoding::BinaryLittleEndian,
        ["format", _, "1.0"] => return Err(PlyError::UnsupportedEncoding),
        _ => return Err(PlyError::InvalidHeader),
    };
    let mut vertex_count = None;
    let mut face_count = None;
    let mut active = "";
    let mut vertex_properties = Vec::new();
    let mut face_properties = Vec::new();
    for line in lines {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.as_slice() {
            ["element", "vertex", count] => {
                vertex_count = count.parse::<usize>().ok();
                active = "vertex";
            }
            ["element", "face", count] => {
                face_count = count.parse::<usize>().ok();
                active = "face";
            }
            ["element", _, _] => active = "other",
            ["property", kind, name] if active == "vertex" || active == "face" => {
                let property = Property::Scalar {
                    kind: ScalarType::parse(kind)?,
                    name: (*name).to_owned(),
                };
                if active == "vertex" {
                    vertex_properties.push(property);
                } else {
                    face_properties.push(property);
                }
            }
            ["property", "list", count, item, name] if active == "face" => {
                face_properties.push(Property::List {
                    count: ScalarType::parse(count)?,
                    item: ScalarType::parse(item)?,
                    name: (*name).to_owned(),
                });
            }
            _ => {}
        }
    }
    let vertex_count = vertex_count.ok_or(PlyError::InvalidHeader)?;
    let face_count = face_count.ok_or(PlyError::InvalidHeader)?;
    for required in ["x", "y", "z"] {
        if !vertex_properties
            .iter()
            .any(|property| matches!(property, Property::Scalar { name, .. } if name == required))
        {
            return Err(PlyError::MissingProperty(required));
        }
    }
    if !face_properties.iter().any(|property| matches!(property, Property::List { name, .. } if name == "vertex_indices" || name == "vertex_index")) {
        return Err(PlyError::MissingProperty("vertex_indices"));
    }
    Ok(Header {
        encoding,
        vertex_count,
        face_count,
        vertex_properties,
        face_properties,
        payload_offset,
    })
}

fn parse_text(bytes: &[u8], header: &Header) -> Result<PlyImportReport, PlyError> {
    let payload = std::str::from_utf8(&bytes[header.payload_offset..])
        .map_err(|_| PlyError::TruncatedPayload)?;
    let mut lines = payload.lines();
    let mut positions = Vec::with_capacity(header.vertex_count);
    for _ in 0..header.vertex_count {
        let fields = lines
            .next()
            .ok_or(PlyError::TruncatedPayload)?
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() < header.vertex_properties.len() {
            return Err(PlyError::TruncatedPayload);
        }
        let mut point = [None; 3];
        for (index, property) in header.vertex_properties.iter().enumerate() {
            if let Property::Scalar { name, .. } = property {
                let slot = match name.as_str() {
                    "x" => Some(0),
                    "y" => Some(1),
                    "z" => Some(2),
                    _ => None,
                };
                if let Some(slot) = slot {
                    point[slot] = fields[index].parse::<f64>().ok();
                }
            }
        }
        let point = Vec3::new(
            point[0].ok_or(PlyError::InvalidVertex)?,
            point[1].ok_or(PlyError::InvalidVertex)?,
            point[2].ok_or(PlyError::InvalidVertex)?,
        );
        if !point.is_finite() {
            return Err(PlyError::InvalidVertex);
        }
        positions.push(point);
    }
    let mut triangles = Vec::with_capacity(header.face_count);
    let mut quads = 0_usize;
    for _ in 0..header.face_count {
        let fields = lines
            .next()
            .ok_or(PlyError::TruncatedPayload)?
            .split_whitespace()
            .collect::<Vec<_>>();
        let indices = text_face_indices(&fields, &header.face_properties)?;
        push_face(&indices, positions.len(), &mut triangles, &mut quads)?;
    }
    Ok(report(header, positions, triangles, quads))
}

fn text_face_indices(fields: &[&str], properties: &[Property]) -> Result<Vec<u32>, PlyError> {
    let mut cursor = 0_usize;
    let mut result = None;
    for property in properties {
        match property {
            Property::Scalar { .. } => {
                cursor = cursor.checked_add(1).ok_or(PlyError::InvalidFace)?;
            }
            Property::List { name, .. } => {
                let count = fields
                    .get(cursor)
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or(PlyError::InvalidFace)?;
                cursor += 1;
                let end = cursor.checked_add(count).ok_or(PlyError::InvalidFace)?;
                let values = fields.get(cursor..end).ok_or(PlyError::TruncatedPayload)?;
                if name == "vertex_indices" || name == "vertex_index" {
                    result = Some(
                        values
                            .iter()
                            .map(|value| value.parse::<u32>().map_err(|_| PlyError::InvalidFace))
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                cursor = end;
            }
        }
    }
    result.ok_or(PlyError::MissingProperty("vertex_indices"))
}

fn parse_binary(bytes: &[u8], header: &Header) -> Result<PlyImportReport, PlyError> {
    let mut cursor = header.payload_offset;
    let mut positions = Vec::with_capacity(header.vertex_count);
    for _ in 0..header.vertex_count {
        let mut point = [None; 3];
        for property in &header.vertex_properties {
            let Property::Scalar { kind, name } = property else {
                return Err(PlyError::InvalidHeader);
            };
            let value = read_scalar(bytes, &mut cursor, *kind)?;
            match name.as_str() {
                "x" => point[0] = Some(value),
                "y" => point[1] = Some(value),
                "z" => point[2] = Some(value),
                _ => {}
            }
        }
        let point = Vec3::new(
            point[0].ok_or(PlyError::InvalidVertex)?,
            point[1].ok_or(PlyError::InvalidVertex)?,
            point[2].ok_or(PlyError::InvalidVertex)?,
        );
        if !point.is_finite() {
            return Err(PlyError::InvalidVertex);
        }
        positions.push(point);
    }
    let mut triangles = Vec::with_capacity(header.face_count);
    let mut quads = 0_usize;
    for _ in 0..header.face_count {
        let mut indices = None;
        for property in &header.face_properties {
            match property {
                Property::Scalar { kind, .. } => {
                    read_scalar(bytes, &mut cursor, *kind)?;
                }
                Property::List { count, item, name } => {
                    let length = read_unsigned(bytes, &mut cursor, *count)?;
                    let length = usize::try_from(length).map_err(|_| PlyError::InvalidFace)?;
                    let mut values = Vec::with_capacity(length);
                    for _ in 0..length {
                        values.push(
                            read_unsigned(bytes, &mut cursor, *item)?
                                .try_into()
                                .map_err(|_| PlyError::InvalidFace)?,
                        );
                    }
                    if name == "vertex_indices" || name == "vertex_index" {
                        indices = Some(values);
                    }
                }
            }
        }
        push_face(
            &indices.ok_or(PlyError::MissingProperty("vertex_indices"))?,
            positions.len(),
            &mut triangles,
            &mut quads,
        )?;
    }
    Ok(report(header, positions, triangles, quads))
}

fn read_scalar(bytes: &[u8], cursor: &mut usize, kind: ScalarType) -> Result<f64, PlyError> {
    let end = cursor
        .checked_add(kind.bytes())
        .ok_or(PlyError::TruncatedPayload)?;
    let data = bytes.get(*cursor..end).ok_or(PlyError::TruncatedPayload)?;
    *cursor = end;
    Ok(match kind {
        ScalarType::I8 => f64::from(i8::from_le_bytes([data[0]])),
        ScalarType::U8 => f64::from(data[0]),
        ScalarType::I16 => f64::from(i16::from_le_bytes(data.try_into().expect("two-byte slice"))),
        ScalarType::U16 => f64::from(u16::from_le_bytes(data.try_into().expect("two-byte slice"))),
        ScalarType::I32 => f64::from(i32::from_le_bytes(
            data.try_into().expect("four-byte slice"),
        )),
        ScalarType::U32 => f64::from(u32::from_le_bytes(
            data.try_into().expect("four-byte slice"),
        )),
        ScalarType::F32 => f64::from(f32::from_le_bytes(
            data.try_into().expect("four-byte slice"),
        )),
        ScalarType::F64 => f64::from_le_bytes(data.try_into().expect("eight-byte slice")),
    })
}

// PLY permits numeric scalar declarations for list counts and indices. The
// finite, non-negative, integral and range checks make this conversion exact.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn read_unsigned(bytes: &[u8], cursor: &mut usize, kind: ScalarType) -> Result<u64, PlyError> {
    let value = read_scalar(bytes, cursor, kind)?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return Err(PlyError::InvalidFace);
    }
    Ok(value as u64)
}

fn push_face(
    indices: &[u32],
    vertex_count: usize,
    output: &mut Vec<[u32; 3]>,
    quads: &mut usize,
) -> Result<(), PlyError> {
    if indices.iter().any(|index| *index as usize >= vertex_count) {
        return Err(PlyError::InvalidFace);
    }
    match indices {
        [a, b, c] => output.push([*a, *b, *c]),
        [a, b, c, d] => {
            output.push([*a, *b, *c]);
            output.push([*a, *c, *d]);
            *quads += 1;
        }
        polygon => return Err(PlyError::UnsupportedPolygon(polygon.len())),
    }
    Ok(())
}

const fn report(
    header: &Header,
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    quads: usize,
) -> PlyImportReport {
    PlyImportReport {
        mesh: Mesh {
            positions,
            triangles,
        },
        encoding: header.encoding,
        source_face_count: header.face_count,
        triangulated_quad_count: quads,
        ignored_vertex_property_count: header.vertex_properties.len().saturating_sub(3),
    }
}

/// Writes deterministic little-endian binary PLY 1.0.
pub fn write_ply_binary(mesh: &Mesh) -> Result<Vec<u8>, PlyError> {
    let vertex_count =
        u32::try_from(mesh.positions.len()).map_err(|_| PlyError::TooManyVertices)?;
    let mut bytes = format!("ply\nformat binary_little_endian 1.0\ncomment XVARNA canonical mesh\nelement vertex {}\nproperty double x\nproperty double y\nproperty double z\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n", mesh.positions.len(), mesh.triangles.len()).into_bytes();
    for point in &mesh.positions {
        if !point.is_finite() {
            return Err(PlyError::InvalidVertex);
        }
        for value in [point.x, point.y, point.z] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for triangle in &mesh.triangles {
        if triangle.iter().any(|index| *index >= vertex_count) {
            return Err(PlyError::InvalidFace);
        }
        bytes.push(3);
        for index in triangle {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Writes deterministic ASCII PLY 1.0.
pub fn write_ply_text(mesh: &Mesh) -> Result<String, PlyError> {
    use std::fmt::Write as _;
    let mut output = format!(
        "ply\nformat ascii 1.0\ncomment XVARNA canonical mesh\nelement vertex {}\nproperty double x\nproperty double y\nproperty double z\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        mesh.positions.len(),
        mesh.triangles.len()
    );
    for point in &mesh.positions {
        if !point.is_finite() {
            return Err(PlyError::InvalidVertex);
        }
        writeln!(output, "{:.17} {:.17} {:.17}", point.x, point.y, point.z)
            .expect("String write succeeds");
    }
    for triangle in &mesh.triangles {
        if triangle
            .iter()
            .any(|index| *index as usize >= mesh.positions.len())
        {
            return Err(PlyError::InvalidFace);
        }
        writeln!(output, "3 {} {} {}", triangle[0], triangle[1], triangle[2])
            .expect("String write succeeds");
    }
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
    fn binary_and_text_round_trip() {
        let binary = write_ply_binary(&mesh()).expect("binary writes");
        let binary_report = parse_ply(&binary).expect("binary parses");
        assert_eq!(binary_report.encoding, PlyEncoding::BinaryLittleEndian);
        assert_eq!(binary_report.mesh, mesh());
        let text = write_ply_text(&mesh()).expect("text writes");
        let text_report = parse_ply(text.as_bytes()).expect("text parses");
        assert_eq!(text_report.encoding, PlyEncoding::Text);
        assert_eq!(text_report.mesh, mesh());
    }

    #[test]
    fn text_quad_uses_deterministic_diagonal() {
        let source = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        let report = parse_ply(source.as_bytes()).expect("quad parses");
        assert_eq!(report.triangulated_quad_count, 1);
        assert_eq!(report.mesh.triangles, vec![[0, 1, 2], [0, 2, 3]]);
    }
}
