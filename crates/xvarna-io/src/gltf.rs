//! Scene-aware glTF 2.0/GLB mesh conversion without image or network dependencies.

use serde_json::{Value, json};
use std::{collections::HashSet, fmt};
use xvarna_geometry::{Mesh, Vec3};

const GLB_MAGIC: u32 = 0x4654_6C67;
const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;
const MAXIMUM_NODE_DEPTH: usize = 256;

/// Successful glTF scene conversion statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct GltfImportReport {
    /// Canonical expanded triangle mesh in scene coordinates.
    pub mesh: Mesh,
    /// Triangle-mode primitives consumed.
    pub primitive_count: usize,
    /// Node occurrences expanded into the canonical mesh.
    pub instance_count: usize,
    /// Largest conservative f32 coordinate error estimate.
    pub maximum_precision_error: f64,
}

/// Unsupported or malformed glTF 2.0 content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GltfError {
    /// GLB magic, version, chunk, or length framing is invalid.
    InvalidGlb,
    /// JSON syntax or required object structure is invalid.
    InvalidJson,
    /// Asset is not glTF 2.x.
    UnsupportedVersion,
    /// External URI is intentionally not fetched by the byte API.
    ExternalBuffer(String),
    /// Embedded data URI is not valid base64.
    InvalidDataUri,
    /// Buffer view or accessor range exceeds its buffer.
    BufferRange,
    /// Sparse accessors are not supported by this deterministic path.
    SparseAccessor,
    /// Primitive mode or attribute representation is unsupported.
    UnsupportedPrimitive,
    /// Node graph contains a cycle or exceeds the depth budget.
    InvalidNodeGraph,
    /// Node transform contains non-finite or malformed values.
    InvalidTransform,
    /// Mesh exceeds the canonical u32 index range.
    TooManyVertices,
    /// Mesh coordinate or index is invalid.
    InvalidMesh,
}

impl fmt::Display for GltfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGlb => formatter.write_str("invalid GLB 2.0 framing"),
            Self::InvalidJson => formatter.write_str("invalid glTF 2.0 JSON structure"),
            Self::UnsupportedVersion => formatter.write_str("asset version must be glTF 2.x"),
            Self::ExternalBuffer(uri) => write!(
                formatter,
                "external glTF buffer requires path-aware loading: {uri}"
            ),
            Self::InvalidDataUri => formatter.write_str("invalid embedded glTF base64 buffer"),
            Self::BufferRange => {
                formatter.write_str("glTF buffer view or accessor is out of range")
            }
            Self::SparseAccessor => formatter.write_str("sparse glTF accessors are not supported"),
            Self::UnsupportedPrimitive => {
                formatter.write_str("glTF primitive must use TRIANGLES and supported accessors")
            }
            Self::InvalidNodeGraph => formatter.write_str("glTF node graph is cyclic or too deep"),
            Self::InvalidTransform => {
                formatter.write_str("glTF node transform is malformed or non-finite")
            }
            Self::TooManyVertices => {
                formatter.write_str("glTF scene exceeds the canonical u32 vertex range")
            }
            Self::InvalidMesh => {
                formatter.write_str("glTF contains an invalid coordinate or index")
            }
        }
    }
}

impl std::error::Error for GltfError {}

/// Parses a binary GLB 2.0 scene and expands active-scene node instances.
pub fn parse_glb(bytes: &[u8]) -> Result<GltfImportReport, GltfError> {
    if bytes.len() < 20
        || read_u32(bytes, 0)? != GLB_MAGIC
        || read_u32(bytes, 4)? != 2
        || usize::try_from(read_u32(bytes, 8)?).map_err(|_| GltfError::InvalidGlb)? != bytes.len()
    {
        return Err(GltfError::InvalidGlb);
    }
    let mut cursor = 12_usize;
    let mut json_bytes = None;
    let mut binary = None;
    while cursor < bytes.len() {
        let length =
            usize::try_from(read_u32(bytes, cursor)?).map_err(|_| GltfError::InvalidGlb)?;
        let kind = read_u32(bytes, cursor + 4)?;
        cursor = cursor.checked_add(8).ok_or(GltfError::InvalidGlb)?;
        let end = cursor.checked_add(length).ok_or(GltfError::InvalidGlb)?;
        let chunk = bytes.get(cursor..end).ok_or(GltfError::InvalidGlb)?;
        match kind {
            JSON_CHUNK if json_bytes.is_none() => json_bytes = Some(chunk),
            BIN_CHUNK if binary.is_none() => binary = Some(chunk.to_vec()),
            _ => {}
        }
        cursor = end;
    }
    let source = std::str::from_utf8(json_bytes.ok_or(GltfError::InvalidGlb)?)
        .map_err(|_| GltfError::InvalidJson)?;
    parse_document(source, binary.as_deref())
}

/// Parses glTF JSON with embedded data URIs. External files are never fetched implicitly.
pub fn parse_gltf(source: &str) -> Result<GltfImportReport, GltfError> {
    parse_document(source, None)
}

fn parse_document(source: &str, glb_binary: Option<&[u8]>) -> Result<GltfImportReport, GltfError> {
    let root: Value = serde_json::from_str(source).map_err(|_| GltfError::InvalidJson)?;
    let version = root
        .pointer("/asset/version")
        .and_then(Value::as_str)
        .ok_or(GltfError::InvalidJson)?;
    if !version.starts_with("2.") {
        return Err(GltfError::UnsupportedVersion);
    }
    let buffers = load_buffers(&root, glb_binary)?;
    let meshes = parse_meshes(&root, &buffers)?;
    let mut output = Mesh::default();
    let mut primitive_count = 0_usize;
    let mut instance_count = 0_usize;
    let nodes = array(&root, "nodes").unwrap_or(&[]);
    let scenes = array(&root, "scenes").unwrap_or(&[]);
    if nodes.is_empty() || scenes.is_empty() {
        for primitives in &meshes {
            append_primitives(&mut output, primitives, Matrix::IDENTITY)?;
            primitive_count += primitives.len();
            instance_count += 1;
        }
    } else {
        let scene_index = root.get("scene").and_then(Value::as_u64).unwrap_or(0);
        let scene = scenes
            .get(usize::try_from(scene_index).map_err(|_| GltfError::InvalidJson)?)
            .ok_or(GltfError::InvalidJson)?;
        let roots = scene
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or(GltfError::InvalidJson)?;
        let mut path = HashSet::new();
        for node in roots {
            let index = usize::try_from(node.as_u64().ok_or(GltfError::InvalidJson)?)
                .map_err(|_| GltfError::InvalidJson)?;
            visit_node(
                index,
                nodes,
                &meshes,
                Matrix::IDENTITY,
                &mut path,
                0,
                &mut output,
                &mut primitive_count,
                &mut instance_count,
            )?;
        }
    }
    if output.triangles.is_empty() {
        return Err(GltfError::InvalidMesh);
    }
    let maximum_precision_error = output.positions.iter().fold(0.0_f64, |error, point| {
        error.max(point.x.abs().max(point.y.abs()).max(point.z.abs()) * f64::from(f32::EPSILON))
    });
    Ok(GltfImportReport {
        mesh: output,
        primitive_count,
        instance_count,
        maximum_precision_error,
    })
}

fn load_buffers(root: &Value, glb_binary: Option<&[u8]>) -> Result<Vec<Vec<u8>>, GltfError> {
    let declarations = array(root, "buffers").ok_or(GltfError::InvalidJson)?;
    let mut buffers = Vec::with_capacity(declarations.len());
    for (index, declaration) in declarations.iter().enumerate() {
        let declared = declaration
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or(GltfError::InvalidJson)?;
        let declared = usize::try_from(declared).map_err(|_| GltfError::BufferRange)?;
        let data = if let Some(uri) = declaration.get("uri").and_then(Value::as_str) {
            decode_data_uri(uri)?
        } else if index == 0 {
            glb_binary.ok_or(GltfError::BufferRange)?.to_vec()
        } else {
            return Err(GltfError::BufferRange);
        };
        if data.len() < declared {
            return Err(GltfError::BufferRange);
        }
        buffers.push(data);
    }
    Ok(buffers)
}

#[derive(Clone, Debug)]
struct PrimitiveMesh {
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
}

fn parse_meshes(root: &Value, buffers: &[Vec<u8>]) -> Result<Vec<Vec<PrimitiveMesh>>, GltfError> {
    let meshes = array(root, "meshes").ok_or(GltfError::InvalidJson)?;
    let mut output = Vec::with_capacity(meshes.len());
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or(GltfError::InvalidJson)?;
        let mut converted = Vec::with_capacity(primitives.len());
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err(GltfError::UnsupportedPrimitive);
            }
            let position_accessor = primitive
                .pointer("/attributes/POSITION")
                .and_then(Value::as_u64)
                .ok_or(GltfError::UnsupportedPrimitive)?;
            let positions = read_positions(
                root,
                buffers,
                usize::try_from(position_accessor).map_err(|_| GltfError::BufferRange)?,
            )?;
            let indices = if let Some(accessor) = primitive.get("indices").and_then(Value::as_u64) {
                read_indices(
                    root,
                    buffers,
                    usize::try_from(accessor).map_err(|_| GltfError::BufferRange)?,
                )?
            } else {
                (0..positions.len())
                    .map(|index| u32::try_from(index).map_err(|_| GltfError::TooManyVertices))
                    .collect::<Result<Vec<_>, _>>()?
            };
            if indices.len() % 3 != 0
                || indices
                    .iter()
                    .any(|index| *index as usize >= positions.len())
            {
                return Err(GltfError::InvalidMesh);
            }
            converted.push(PrimitiveMesh {
                positions,
                triangles: indices
                    .chunks_exact(3)
                    .map(|value| [value[0], value[1], value[2]])
                    .collect(),
            });
        }
        output.push(converted);
    }
    Ok(output)
}

fn read_positions(
    root: &Value,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Result<Vec<Vec3>, GltfError> {
    let accessors = array(root, "accessors").ok_or(GltfError::InvalidJson)?;
    let accessor = accessors
        .get(accessor_index)
        .ok_or(GltfError::BufferRange)?;
    if accessor.get("sparse").is_some() {
        return Err(GltfError::SparseAccessor);
    }
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
    {
        return Err(GltfError::UnsupportedPrimitive);
    }
    let count = usize::try_from(
        accessor
            .get("count")
            .and_then(Value::as_u64)
            .ok_or(GltfError::BufferRange)?,
    )
    .map_err(|_| GltfError::BufferRange)?;
    let (buffer, start, stride) = accessor_layout(root, buffers, accessor, 12)?;
    let mut positions = Vec::with_capacity(count);
    for index in 0..count {
        let offset = start
            .checked_add(index.checked_mul(stride).ok_or(GltfError::BufferRange)?)
            .ok_or(GltfError::BufferRange)?;
        let data = buffer
            .get(offset..offset + 12)
            .ok_or(GltfError::BufferRange)?;
        let point = Vec3::new(
            f64::from(read_f32(data, 0)?),
            f64::from(read_f32(data, 4)?),
            f64::from(read_f32(data, 8)?),
        );
        if !point.is_finite() {
            return Err(GltfError::InvalidMesh);
        }
        positions.push(point);
    }
    Ok(positions)
}

fn read_indices(
    root: &Value,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Result<Vec<u32>, GltfError> {
    let accessors = array(root, "accessors").ok_or(GltfError::InvalidJson)?;
    let accessor = accessors
        .get(accessor_index)
        .ok_or(GltfError::BufferRange)?;
    if accessor.get("sparse").is_some() {
        return Err(GltfError::SparseAccessor);
    }
    if accessor.get("type").and_then(Value::as_str) != Some("SCALAR") {
        return Err(GltfError::UnsupportedPrimitive);
    }
    let component = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or(GltfError::UnsupportedPrimitive)?;
    let size = match component {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => return Err(GltfError::UnsupportedPrimitive),
    };
    let count = usize::try_from(
        accessor
            .get("count")
            .and_then(Value::as_u64)
            .ok_or(GltfError::BufferRange)?,
    )
    .map_err(|_| GltfError::BufferRange)?;
    let (buffer, start, stride) = accessor_layout(root, buffers, accessor, size)?;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        let offset = start
            .checked_add(index.checked_mul(stride).ok_or(GltfError::BufferRange)?)
            .ok_or(GltfError::BufferRange)?;
        let value = match component {
            5121 => u32::from(*buffer.get(offset).ok_or(GltfError::BufferRange)?),
            5123 => u32::from(u16::from_le_bytes(
                buffer
                    .get(offset..offset + 2)
                    .ok_or(GltfError::BufferRange)?
                    .try_into()
                    .expect("two-byte slice"),
            )),
            5125 => u32::from_le_bytes(
                buffer
                    .get(offset..offset + 4)
                    .ok_or(GltfError::BufferRange)?
                    .try_into()
                    .expect("four-byte slice"),
            ),
            _ => unreachable!(),
        };
        output.push(value);
    }
    Ok(output)
}

fn accessor_layout<'a>(
    root: &Value,
    buffers: &'a [Vec<u8>],
    accessor: &Value,
    element_size: usize,
) -> Result<(&'a [u8], usize, usize), GltfError> {
    let views = array(root, "bufferViews").ok_or(GltfError::InvalidJson)?;
    let view_index = usize::try_from(
        accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .ok_or(GltfError::BufferRange)?,
    )
    .map_err(|_| GltfError::BufferRange)?;
    let view = views.get(view_index).ok_or(GltfError::BufferRange)?;
    let buffer_index = usize::try_from(
        view.get("buffer")
            .and_then(Value::as_u64)
            .ok_or(GltfError::BufferRange)?,
    )
    .map_err(|_| GltfError::BufferRange)?;
    let buffer = buffers.get(buffer_index).ok_or(GltfError::BufferRange)?;
    let view_offset = usize::try_from(view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0))
        .map_err(|_| GltfError::BufferRange)?;
    let accessor_offset = usize::try_from(
        accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    )
    .map_err(|_| GltfError::BufferRange)?;
    let start = view_offset
        .checked_add(accessor_offset)
        .ok_or(GltfError::BufferRange)?;
    let default_stride = u64::try_from(element_size).map_err(|_| GltfError::BufferRange)?;
    let stride = usize::try_from(
        view.get("byteStride")
            .and_then(Value::as_u64)
            .unwrap_or(default_stride),
    )
    .map_err(|_| GltfError::BufferRange)?;
    if stride < element_size {
        return Err(GltfError::BufferRange);
    }
    Ok((buffer, start, stride))
}

#[allow(clippy::too_many_arguments)]
fn visit_node(
    index: usize,
    nodes: &[Value],
    meshes: &[Vec<PrimitiveMesh>],
    parent: Matrix,
    path: &mut HashSet<usize>,
    depth: usize,
    output: &mut Mesh,
    primitive_count: &mut usize,
    instance_count: &mut usize,
) -> Result<(), GltfError> {
    if depth > MAXIMUM_NODE_DEPTH || !path.insert(index) {
        return Err(GltfError::InvalidNodeGraph);
    }
    let node = nodes.get(index).ok_or(GltfError::InvalidNodeGraph)?;
    let world = parent.multiply(node_matrix(node)?);
    if let Some(mesh_index) = node.get("mesh").and_then(Value::as_u64) {
        let primitives = meshes
            .get(usize::try_from(mesh_index).map_err(|_| GltfError::InvalidMesh)?)
            .ok_or(GltfError::InvalidMesh)?;
        append_primitives(output, primitives, world)?;
        *primitive_count += primitives.len();
        *instance_count += 1;
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            let child = usize::try_from(child.as_u64().ok_or(GltfError::InvalidNodeGraph)?)
                .map_err(|_| GltfError::InvalidNodeGraph)?;
            visit_node(
                child,
                nodes,
                meshes,
                world,
                path,
                depth + 1,
                output,
                primitive_count,
                instance_count,
            )?;
        }
    }
    path.remove(&index);
    Ok(())
}

fn append_primitives(
    output: &mut Mesh,
    primitives: &[PrimitiveMesh],
    transform: Matrix,
) -> Result<(), GltfError> {
    for primitive in primitives {
        let base = u32::try_from(output.positions.len()).map_err(|_| GltfError::TooManyVertices)?;
        for point in &primitive.positions {
            output.positions.push(transform.point(*point)?);
        }
        for triangle in &primitive.triangles {
            output.triangles.push([
                base.checked_add(triangle[0])
                    .ok_or(GltfError::TooManyVertices)?,
                base.checked_add(triangle[1])
                    .ok_or(GltfError::TooManyVertices)?,
                base.checked_add(triangle[2])
                    .ok_or(GltfError::TooManyVertices)?,
            ]);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Matrix([f64; 16]);

impl Matrix {
    const IDENTITY: Self = Self([
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);

    fn multiply(self, other: Self) -> Self {
        let mut result = [0.0; 16];
        for row in 0..4 {
            for column in 0..4 {
                result[row * 4 + column] = (0..4)
                    .map(|inner| self.0[row * 4 + inner] * other.0[inner * 4 + column])
                    .sum();
            }
        }
        Self(result)
    }

    const fn point(self, point: Vec3) -> Result<Vec3, GltfError> {
        let result = Vec3::new(
            self.0[0].mul_add(
                point.x,
                self.0[1].mul_add(point.y, self.0[2].mul_add(point.z, self.0[3])),
            ),
            self.0[4].mul_add(
                point.x,
                self.0[5].mul_add(point.y, self.0[6].mul_add(point.z, self.0[7])),
            ),
            self.0[8].mul_add(
                point.x,
                self.0[9].mul_add(point.y, self.0[10].mul_add(point.z, self.0[11])),
            ),
        );
        if result.is_finite() {
            Ok(result)
        } else {
            Err(GltfError::InvalidTransform)
        }
    }
}

#[allow(clippy::suboptimal_flops)]
fn node_matrix(node: &Value) -> Result<Matrix, GltfError> {
    if let Some(values) = node.get("matrix").and_then(Value::as_array) {
        if values.len() != 16 {
            return Err(GltfError::InvalidTransform);
        }
        let mut row_major = [0.0; 16];
        for row in 0..4 {
            for column in 0..4 {
                row_major[row * 4 + column] = finite(values[column * 4 + row].as_f64())?;
            }
        }
        return Ok(Matrix(row_major));
    }
    let translation = vector(node.get("translation"), [0.0, 0.0, 0.0])?;
    let scale = vector(node.get("scale"), [1.0, 1.0, 1.0])?;
    let [x, y, z, w] = quaternion(node.get("rotation"))?;
    let rotation = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - z * w),
        2.0 * (x * z + y * w),
        2.0 * (x * y + z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - x * w),
        2.0 * (x * z - y * w),
        2.0 * (y * z + x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    Ok(Matrix([
        rotation[0] * scale[0],
        rotation[1] * scale[1],
        rotation[2] * scale[2],
        translation[0],
        rotation[3] * scale[0],
        rotation[4] * scale[1],
        rotation[5] * scale[2],
        translation[1],
        rotation[6] * scale[0],
        rotation[7] * scale[1],
        rotation[8] * scale[2],
        translation[2],
        0.0,
        0.0,
        0.0,
        1.0,
    ]))
}

fn vector(value: Option<&Value>, default: [f64; 3]) -> Result<[f64; 3], GltfError> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Ok(default);
    };
    if values.len() != 3 {
        return Err(GltfError::InvalidTransform);
    }
    Ok([
        finite(values[0].as_f64())?,
        finite(values[1].as_f64())?,
        finite(values[2].as_f64())?,
    ])
}

fn quaternion(value: Option<&Value>) -> Result<[f64; 4], GltfError> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Ok([0.0, 0.0, 0.0, 1.0]);
    };
    if values.len() != 4 {
        return Err(GltfError::InvalidTransform);
    }
    let mut result = [
        finite(values[0].as_f64())?,
        finite(values[1].as_f64())?,
        finite(values[2].as_f64())?,
        finite(values[3].as_f64())?,
    ];
    let length = result.iter().map(|value| value * value).sum::<f64>().sqrt();
    if length <= f64::EPSILON {
        return Err(GltfError::InvalidTransform);
    }
    for value in &mut result {
        *value /= length;
    }
    Ok(result)
}

fn finite(value: Option<f64>) -> Result<f64, GltfError> {
    value
        .filter(|value| value.is_finite())
        .ok_or(GltfError::InvalidTransform)
}

fn array<'a>(root: &'a Value, name: &str) -> Option<&'a [Value]> {
    root.get(name).and_then(Value::as_array).map(Vec::as_slice)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, GltfError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(GltfError::InvalidGlb)?
            .try_into()
            .expect("four-byte slice"),
    ))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, GltfError> {
    Ok(f32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(GltfError::BufferRange)?
            .try_into()
            .expect("four-byte slice"),
    ))
}

// glTF POSITION accessors are deliberately emitted as f32; the import report
// exposes the corresponding conservative precision bound.
#[allow(clippy::cast_possible_truncation)]
fn mesh_binary(mesh: &Mesh) -> Result<(Vec<u8>, usize), GltfError> {
    let mut bytes = Vec::with_capacity(mesh.positions.len() * 12 + mesh.triangles.len() * 12);
    for point in &mesh.positions {
        if !point.is_finite() {
            return Err(GltfError::InvalidMesh);
        }
        for value in [point.x, point.y, point.z] {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        }
    }
    let index_offset = bytes.len();
    for triangle in &mesh.triangles {
        if triangle
            .iter()
            .any(|index| *index as usize >= mesh.positions.len())
        {
            return Err(GltfError::InvalidMesh);
        }
        for index in triangle {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
    }
    Ok((bytes, index_offset))
}

fn document(mesh: &Mesh, binary_length: usize, index_offset: usize, uri: Option<String>) -> Value {
    let mut buffer = json!({ "byteLength": binary_length });
    if let Some(uri) = uri {
        buffer["uri"] = Value::String(uri);
    }
    let (minimum, maximum) = bounds(mesh);
    json!({
        "asset": { "version": "2.0", "generator": "XVARNA" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": "XVARNA canonical mesh" }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1, "mode": 4 }] }],
        "buffers": [buffer],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": index_offset, "target": 34962 },
            { "buffer": 0, "byteOffset": index_offset, "byteLength": binary_length - index_offset, "target": 34963 }
        ],
        "accessors": [
            { "bufferView": 0, "byteOffset": 0, "componentType": 5126, "count": mesh.positions.len(), "type": "VEC3", "min": minimum, "max": maximum },
            { "bufferView": 1, "byteOffset": 0, "componentType": 5125, "count": mesh.triangles.len() * 3, "type": "SCALAR" }
        ]
    })
}

fn bounds(mesh: &Mesh) -> ([f64; 3], [f64; 3]) {
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for point in &mesh.positions {
        for (axis, value) in [point.x, point.y, point.z].into_iter().enumerate() {
            minimum[axis] = minimum[axis].min(value);
            maximum[axis] = maximum[axis].max(value);
        }
    }
    (minimum, maximum)
}

/// Writes a self-contained glTF 2.0 JSON document with one base64 buffer.
pub fn write_gltf_embedded(mesh: &Mesh) -> Result<String, GltfError> {
    if mesh.positions.is_empty() || mesh.triangles.is_empty() {
        return Err(GltfError::InvalidMesh);
    }
    let (binary, index_offset) = mesh_binary(mesh)?;
    let uri = format!(
        "data:application/octet-stream;base64,{}",
        encode_base64(&binary)
    );
    serde_json::to_string_pretty(&document(mesh, binary.len(), index_offset, Some(uri)))
        .map_err(|_| GltfError::InvalidJson)
}

/// Writes binary glTF 2.0 with aligned JSON and BIN chunks.
pub fn write_glb(mesh: &Mesh) -> Result<Vec<u8>, GltfError> {
    if mesh.positions.is_empty() || mesh.triangles.is_empty() {
        return Err(GltfError::InvalidMesh);
    }
    let (mut binary, index_offset) = mesh_binary(mesh)?;
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let mut json = serde_json::to_vec(&document(mesh, binary.len(), index_offset, None))
        .map_err(|_| GltfError::InvalidJson)?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = 12_usize
        .checked_add(8 + json.len())
        .and_then(|value| value.checked_add(8 + binary.len()))
        .ok_or(GltfError::InvalidGlb)?;
    let total = u32::try_from(total).map_err(|_| GltfError::InvalidGlb)?;
    let mut output = Vec::with_capacity(total as usize);
    output.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&total.to_le_bytes());
    output.extend_from_slice(
        &u32::try_from(json.len())
            .map_err(|_| GltfError::InvalidGlb)?
            .to_le_bytes(),
    );
    output.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    output.extend_from_slice(&json);
    output.extend_from_slice(
        &u32::try_from(binary.len())
            .map_err(|_| GltfError::InvalidGlb)?
            .to_le_bytes(),
    );
    output.extend_from_slice(&BIN_CHUNK.to_le_bytes());
    output.extend_from_slice(&binary);
    Ok(output)
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(BASE64[usize::from(first >> 2)]));
        output.push(char::from(
            BASE64[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        output.push(if chunk.len() > 1 {
            char::from(BASE64[usize::from((second & 0x0f) << 2 | third >> 6)])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(BASE64[usize::from(third & 0x3f)])
        } else {
            '='
        });
    }
    output
}

fn decode_data_uri(uri: &str) -> Result<Vec<u8>, GltfError> {
    let (_, encoded) = uri.split_once(";base64,").ok_or_else(|| {
        if uri.contains(':') {
            GltfError::InvalidDataUri
        } else {
            GltfError::ExternalBuffer(uri.to_owned())
        }
    })?;
    decode_base64(encoded)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, GltfError> {
    let cleaned = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if cleaned.len() % 4 != 0 {
        return Err(GltfError::InvalidDataUri);
    }
    let mut output = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks_exact(4) {
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            0
        } else {
            base64_value(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            base64_value(chunk[3])?
        };
        output.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            output.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            output.push((c << 6) | d);
        }
    }
    Ok(output)
}

const fn base64_value(value: u8) -> Result<u8, GltfError> {
    match value {
        b'A'..=b'Z' => Ok(value - b'A'),
        b'a'..=b'z' => Ok(value - b'a' + 26),
        b'0'..=b'9' => Ok(value - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(GltfError::InvalidDataUri),
    }
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
    fn glb_and_embedded_gltf_round_trip() {
        let glb = write_glb(&mesh()).expect("GLB writes");
        let from_glb = parse_glb(&glb).expect("GLB parses");
        assert_eq!(from_glb.mesh, mesh());
        let gltf = write_gltf_embedded(&mesh()).expect("glTF writes");
        let from_gltf = parse_gltf(&gltf).expect("glTF parses");
        assert_eq!(from_gltf.mesh, mesh());
    }

    #[test]
    fn node_translation_is_applied() {
        let gltf = write_gltf_embedded(&mesh()).expect("glTF writes");
        let mut root: Value = serde_json::from_str(&gltf).expect("JSON parses");
        root["nodes"][0]["translation"] = json!([5.0, -2.0, 3.0]);
        let report = parse_gltf(&root.to_string()).expect("translated glTF parses");
        assert_eq!(report.mesh.positions[0], Vec3::new(5.0, -2.0, 3.0));
    }
}
