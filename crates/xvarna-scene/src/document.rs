//! Versioned, checksummed serialization of canonical scenes.

use super::{Scene, SceneBuildOptions, SceneBuilder, SceneError, SceneLayer, Transform};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::HashMap, fmt};
use xvarna_geometry::{Mesh, Vec3};
use xvarna_types::{InstanceId, ObjectId};

/// Stable schema identifier embedded in every scene document.
pub const SCENE_DOCUMENT_SCHEMA: &str = "https://xvarna.dev/schemas/scene";
/// Current scene-document schema version.
pub const SCENE_DOCUMENT_VERSION: u32 = 1;

const BINARY_MAGIC: [u8; 8] = *b"XVSCN\0\x01\0";
const BINARY_HEADER_BYTES: usize = 8 + 4 + 8 + 32;
const MAXIMUM_DOCUMENT_BYTES: usize = 4 * 1024 * 1024 * 1024;

/// Serializable build policy. Geometry and transforms are always canonical metres.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDocumentBuildOptions {
    /// Absolute geometric tolerance in metres.
    pub absolute_tolerance_meters: f64,
    /// Maximum primitives per BVH leaf.
    pub maximum_leaf_size: usize,
    /// Worker threads; zero selects the runtime default.
    pub thread_count: usize,
}

/// One canonical shared mesh resource.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDocumentMesh {
    /// Stable resource identifier in this document.
    pub mesh_id: u64,
    /// Canonical f64 positions in metres.
    pub positions: Vec<[f64; 3]>,
    /// Triangle vertex indices.
    pub triangles: Vec<[u32; 3]>,
}

/// Static or dynamic acceleration membership.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SceneDocumentLayer {
    /// Geometry expected to remain stable.
    Static,
    /// Geometry intended for interactive updates.
    Dynamic,
}

impl From<SceneLayer> for SceneDocumentLayer {
    fn from(value: SceneLayer) -> Self {
        match value {
            SceneLayer::Static => Self::Static,
            SceneLayer::Dynamic => Self::Dynamic,
        }
    }
}

impl From<SceneDocumentLayer> for SceneLayer {
    fn from(value: SceneDocumentLayer) -> Self {
        match value {
            SceneDocumentLayer::Static => Self::Static,
            SceneDocumentLayer::Dynamic => Self::Dynamic,
        }
    }
}

/// One occurrence of a shared mesh.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDocumentInstance {
    /// Referenced resource identifier.
    pub mesh_id: u64,
    /// Stable source object identifier.
    pub object_id: u64,
    /// Stable occurrence identifier.
    pub instance_id: u64,
    /// Row-major affine transform in canonical metres.
    pub transform: [f64; 16],
    /// Category bit mask.
    pub category_mask: u64,
    /// Static or dynamic acceleration layer.
    pub layer: SceneDocumentLayer,
}

/// Complete portable scene source independent of BVH/backend implementation details.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDocument {
    /// Stable schema identifier.
    pub schema: String,
    /// Schema version after migration.
    pub schema_version: u32,
    /// Coordinate unit. Version one requires `m`.
    pub units: String,
    /// Rebuild policy independent of a machine-specific thread pool.
    pub build: SceneDocumentBuildOptions,
    /// Unique canonical mesh resources.
    pub meshes: Vec<SceneDocumentMesh>,
    /// Stable occurrences and metadata.
    pub instances: Vec<SceneDocumentInstance>,
}

/// Scene-document syntax, integrity, migration, or compilation failure.
#[derive(Debug)]
pub enum SceneDocumentError {
    /// JSON syntax or structure is invalid.
    Json(serde_json::Error),
    /// Schema identifier is not the XVARNA scene contract.
    InvalidSchema,
    /// Document version is newer than this engine or cannot be migrated.
    UnsupportedVersion(u32),
    /// Units are absent or not canonical metres.
    InvalidUnits,
    /// A resource identifier is zero or duplicated.
    InvalidMeshId(u64),
    /// An instance references an absent resource.
    UnknownMeshId(u64),
    /// Binary framing is truncated, oversized, or inconsistent.
    InvalidBinaryEnvelope,
    /// Binary payload checksum does not match its header.
    ChecksumMismatch,
    /// Canonical scene compilation rejected document content.
    Scene(SceneError),
}

impl fmt::Display for SceneDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid scene JSON: {error}"),
            Self::InvalidSchema => formatter.write_str("invalid XVARNA scene schema identifier"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported scene schema version {version}")
            }
            Self::InvalidUnits => {
                formatter.write_str("scene document units must be canonical metres")
            }
            Self::InvalidMeshId(id) => write!(
                formatter,
                "scene mesh identifier {id} is invalid or duplicated"
            ),
            Self::UnknownMeshId(id) => {
                write!(formatter, "scene instance references unknown mesh {id}")
            }
            Self::InvalidBinaryEnvelope => {
                formatter.write_str("invalid XVARNA binary scene envelope")
            }
            Self::ChecksumMismatch => formatter.write_str("XVARNA binary scene checksum mismatch"),
            Self::Scene(error) => write!(formatter, "scene compilation failed: {error}"),
        }
    }
}

impl std::error::Error for SceneDocumentError {}

impl From<serde_json::Error> for SceneDocumentError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<SceneError> for SceneDocumentError {
    fn from(value: SceneError) -> Self {
        Self::Scene(value)
    }
}

impl Scene {
    /// Captures the full canonical scene source without serializing acceleration internals.
    #[must_use]
    pub fn to_document(&self) -> SceneDocument {
        let meshes = self
            .resources
            .iter()
            .map(|resource| SceneDocumentMesh {
                mesh_id: resource.id.get(),
                positions: resource
                    .mesh
                    .positions
                    .iter()
                    .map(|point| [point.x, point.y, point.z])
                    .collect(),
                triangles: resource.mesh.triangles.clone(),
            })
            .collect();
        let instances = self
            .source_instances
            .iter()
            .map(|instance| SceneDocumentInstance {
                mesh_id: self.resources[instance.mesh_index].id.get(),
                object_id: instance.object_id.get(),
                instance_id: instance.instance_id.get(),
                transform: instance.transform.to_row_major(),
                category_mask: instance.category_mask,
                layer: instance.layer.into(),
            })
            .collect();
        SceneDocument {
            schema: SCENE_DOCUMENT_SCHEMA.to_owned(),
            schema_version: SCENE_DOCUMENT_VERSION,
            units: "m".to_owned(),
            build: SceneDocumentBuildOptions {
                absolute_tolerance_meters: self.options.absolute_tolerance_meters,
                maximum_leaf_size: self.options.maximum_leaf_size,
                thread_count: self.options.thread_count,
            },
            meshes,
            instances,
        }
    }
}

impl SceneDocument {
    /// Parses JSON and migrates every supported historical schema to the current contract.
    pub fn from_json(source: &str) -> Result<Self, SceneDocumentError> {
        let mut value: Value = serde_json::from_str(source)?;
        migrate_value(&mut value)?;
        let document: Self = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document)
    }

    /// Serializes deterministic, human-auditable pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, SceneDocumentError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Produces a checksummed binary envelope containing canonical compact JSON.
    pub fn to_binary(&self) -> Result<Vec<u8>, SceneDocumentError> {
        self.validate()?;
        let payload = serde_json::to_vec(self)?;
        if payload.len() > MAXIMUM_DOCUMENT_BYTES {
            return Err(SceneDocumentError::InvalidBinaryEnvelope);
        }
        let mut bytes = Vec::with_capacity(BINARY_HEADER_BYTES + payload.len());
        bytes.extend_from_slice(&BINARY_MAGIC);
        bytes.extend_from_slice(&SCENE_DOCUMENT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(blake3::hash(&payload).as_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    /// Reads and verifies a checksummed binary scene document.
    pub fn from_binary(bytes: &[u8]) -> Result<Self, SceneDocumentError> {
        if bytes.len() < BINARY_HEADER_BYTES || bytes[..8] != BINARY_MAGIC {
            return Err(SceneDocumentError::InvalidBinaryEnvelope);
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().expect("fixed header slice"));
        if version > SCENE_DOCUMENT_VERSION {
            return Err(SceneDocumentError::UnsupportedVersion(version));
        }
        let payload_length =
            u64::from_le_bytes(bytes[12..20].try_into().expect("fixed binary length slice"));
        let payload_length = usize::try_from(payload_length)
            .map_err(|_| SceneDocumentError::InvalidBinaryEnvelope)?;
        if payload_length > MAXIMUM_DOCUMENT_BYTES
            || BINARY_HEADER_BYTES.checked_add(payload_length) != Some(bytes.len())
        {
            return Err(SceneDocumentError::InvalidBinaryEnvelope);
        }
        let expected = &bytes[20..52];
        let payload = &bytes[BINARY_HEADER_BYTES..];
        if blake3::hash(payload).as_bytes() != expected {
            return Err(SceneDocumentError::ChecksumMismatch);
        }
        Self::from_json(
            std::str::from_utf8(payload).map_err(|_| SceneDocumentError::InvalidBinaryEnvelope)?,
        )
    }

    /// Rebuilds an immutable CPU scene from canonical resources and occurrences.
    pub fn compile(&self) -> Result<Scene, SceneDocumentError> {
        self.validate()?;
        let mut builder = SceneBuilder::new(SceneBuildOptions {
            unit_scale_to_meters: 1.0,
            absolute_tolerance_meters: self.build.absolute_tolerance_meters,
            maximum_leaf_size: self.build.maximum_leaf_size,
            thread_count: self.build.thread_count,
        })?;
        let mut identifiers = HashMap::with_capacity(self.meshes.len());
        for resource in &self.meshes {
            let mesh = Mesh {
                positions: resource
                    .positions
                    .iter()
                    .map(|point| Vec3::new(point[0], point[1], point[2]))
                    .collect(),
                triangles: resource.triangles.clone(),
            };
            let compiled = builder.add_mesh(mesh)?;
            identifiers.insert(resource.mesh_id, compiled);
        }
        for instance in &self.instances {
            let mesh_id = identifiers
                .get(&instance.mesh_id)
                .copied()
                .ok_or(SceneDocumentError::UnknownMeshId(instance.mesh_id))?;
            let transform = Transform::try_from_row_major(instance.transform)
                .map_err(|_| SceneDocumentError::Scene(SceneError::InvalidTransform))?;
            builder.add_instance_in_layer(
                mesh_id,
                transform,
                ObjectId::new(instance.object_id),
                InstanceId::new(instance.instance_id),
                instance.category_mask,
                instance.layer.into(),
            )?;
        }
        Ok(builder.build()?)
    }

    fn validate(&self) -> Result<(), SceneDocumentError> {
        if self.schema != SCENE_DOCUMENT_SCHEMA {
            return Err(SceneDocumentError::InvalidSchema);
        }
        if self.schema_version != SCENE_DOCUMENT_VERSION {
            return Err(SceneDocumentError::UnsupportedVersion(self.schema_version));
        }
        if self.units != "m" {
            return Err(SceneDocumentError::InvalidUnits);
        }
        let mut ids = std::collections::HashSet::with_capacity(self.meshes.len());
        for mesh in &self.meshes {
            if mesh.mesh_id == 0 || !ids.insert(mesh.mesh_id) {
                return Err(SceneDocumentError::InvalidMeshId(mesh.mesh_id));
            }
        }
        if let Some(instance) = self
            .instances
            .iter()
            .find(|instance| !ids.contains(&instance.mesh_id))
        {
            return Err(SceneDocumentError::UnknownMeshId(instance.mesh_id));
        }
        Ok(())
    }
}

fn migrate_value(value: &mut Value) -> Result<(), SceneDocumentError> {
    let object = value
        .as_object_mut()
        .ok_or(SceneDocumentError::InvalidSchema)?;
    let version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if version > u64::from(SCENE_DOCUMENT_VERSION) {
        return Err(SceneDocumentError::UnsupportedVersion(
            u32::try_from(version).unwrap_or(u32::MAX),
        ));
    }
    if version == 0 {
        migrate_zero_to_one(object);
    }
    Ok(())
}

fn migrate_zero_to_one(object: &mut Map<String, Value>) {
    object.insert(
        "schema".to_owned(),
        Value::String(SCENE_DOCUMENT_SCHEMA.to_owned()),
    );
    object.insert(
        "schemaVersion".to_owned(),
        Value::from(SCENE_DOCUMENT_VERSION),
    );
    object
        .entry("units".to_owned())
        .or_insert_with(|| Value::String("m".to_owned()));
    if let Some(instances) = object.get_mut("instances").and_then(Value::as_array_mut) {
        for instance in instances {
            if let Some(instance) = instance.as_object_mut() {
                instance
                    .entry("categoryMask".to_owned())
                    .or_insert_with(|| Value::from(u64::MAX));
                instance
                    .entry("layer".to_owned())
                    .or_insert_with(|| Value::String("static".to_owned()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("valid options");
        let mesh = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                triangles: vec![[0, 1, 2]],
            })
            .expect("valid mesh");
        builder
            .add_instance_in_layer(
                mesh,
                Transform::IDENTITY,
                ObjectId::new(7),
                InstanceId::new(11),
                4,
                SceneLayer::Dynamic,
            )
            .expect("valid instance");
        builder.build().expect("scene builds")
    }

    #[test]
    fn json_and_binary_round_trip_complete_scene() {
        let document = scene().to_document();
        let json = document.to_json_pretty().expect("JSON serializes");
        let from_json = SceneDocument::from_json(&json).expect("JSON parses");
        let binary = from_json.to_binary().expect("binary serializes");
        let restored = SceneDocument::from_binary(&binary)
            .expect("binary parses")
            .compile()
            .expect("scene recompiles");
        assert_eq!(restored.stats().unique_triangle_count, 1);
        assert_eq!(restored.stats().instance_count, 1);
        let document = restored.to_document();
        assert_eq!(document.instances[0].object_id, 7);
        assert_eq!(document.instances[0].instance_id, 11);
        assert_eq!(document.instances[0].category_mask, 4);
        assert_eq!(document.instances[0].layer, SceneDocumentLayer::Dynamic);
    }

    #[test]
    fn binary_checksum_detects_corruption() {
        let mut binary = scene()
            .to_document()
            .to_binary()
            .expect("binary serializes");
        let last = binary.last_mut().expect("payload exists");
        *last ^= 0x01;
        assert!(matches!(
            SceneDocument::from_binary(&binary),
            Err(SceneDocumentError::ChecksumMismatch)
        ));
    }

    #[test]
    fn version_zero_defaults_are_migrated() {
        let current = scene().to_document();
        let mut value = serde_json::to_value(current).expect("value serializes");
        let object = value.as_object_mut().expect("document is object");
        object.remove("schema");
        object.remove("schemaVersion");
        object.remove("units");
        let instance = object["instances"].as_array_mut().expect("instances array")[0]
            .as_object_mut()
            .expect("instance object");
        instance.remove("categoryMask");
        instance.remove("layer");
        let migrated = SceneDocument::from_json(&value.to_string()).expect("v0 migrates");
        assert_eq!(migrated.schema_version, SCENE_DOCUMENT_VERSION);
        assert_eq!(migrated.instances[0].category_mask, u64::MAX);
        assert_eq!(migrated.instances[0].layer, SceneDocumentLayer::Static);
    }
}
