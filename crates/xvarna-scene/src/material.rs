//! Analysis material catalog and deterministic multi-layer transmission.

use super::Hit;
use std::collections::BTreeMap;
use xvarna_types::ObjectId;

/// Stable material identifier. Zero is reserved for the implicit opaque material.
pub type MaterialId = u64;

/// Optical properties used by visibility and solar screening analyses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalysisMaterial {
    /// Stable non-zero material identifier.
    pub id: MaterialId,
    /// Fraction of visible energy transmitted through one geometric hit.
    pub visible_transmittance: f64,
    /// Fraction of direct solar energy transmitted through one geometric hit.
    pub solar_transmittance: f64,
    /// Diffuse reflectance retained for daylight/radiance export and reporting.
    pub reflectance: f64,
}

impl AnalysisMaterial {
    /// Validates a physically bounded analysis material.
    pub fn try_new(
        id: MaterialId,
        visible_transmittance: f64,
        solar_transmittance: f64,
        reflectance: f64,
    ) -> Result<Self, MaterialError> {
        if id == 0 {
            return Err(MaterialError::ReservedIdentifier);
        }
        if !is_fraction(visible_transmittance)
            || !is_fraction(solar_transmittance)
            || !is_fraction(reflectance)
            || visible_transmittance + reflectance > 1.0 + 1.0e-12
            || solar_transmittance + reflectance > 1.0 + 1.0e-12
        {
            return Err(MaterialError::InvalidOpticalProperty);
        }
        Ok(Self {
            id,
            visible_transmittance,
            solar_transmittance,
            reflectance,
        })
    }

    /// Canonical fully opaque, non-reflective analysis material.
    #[must_use]
    pub const fn opaque(id: MaterialId) -> Self {
        Self {
            id,
            visible_transmittance: 0.0,
            solar_transmittance: 0.0,
            reflectance: 0.0,
        }
    }
}

/// Failure while constructing an analysis material catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialError {
    /// Material identifier zero is reserved for implicit opaque context.
    ReservedIdentifier,
    /// A transmittance/reflectance was non-finite, outside zero through one, or non-conserving.
    InvalidOpticalProperty,
    /// A material identifier was repeated.
    DuplicateMaterial,
    /// An object was assigned more than once.
    DuplicateAssignment,
    /// An assignment referenced a material absent from the catalog.
    UnknownMaterial,
}

impl core::fmt::Display for MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReservedIdentifier => formatter.write_str("material identifier zero is reserved"),
            Self::InvalidOpticalProperty => formatter.write_str(
                "material transmittance and reflectance must be finite, bounded, and energy-conserving",
            ),
            Self::DuplicateMaterial => formatter.write_str("material identifier is duplicated"),
            Self::DuplicateAssignment => formatter.write_str("object material assignment is duplicated"),
            Self::UnknownMaterial => formatter.write_str("object assignment references an unknown material"),
        }
    }
}

impl std::error::Error for MaterialError {}

/// Immutable mapping from source objects to validated optical materials.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialLibrary {
    materials: BTreeMap<MaterialId, AnalysisMaterial>,
    assignments: BTreeMap<ObjectId, MaterialId>,
}

impl MaterialLibrary {
    /// Creates a catalog. Unassigned objects remain fully opaque.
    pub fn try_new(
        materials: impl IntoIterator<Item = AnalysisMaterial>,
        assignments: impl IntoIterator<Item = (ObjectId, MaterialId)>,
    ) -> Result<Self, MaterialError> {
        let mut material_map = BTreeMap::new();
        for material in materials {
            AnalysisMaterial::try_new(
                material.id,
                material.visible_transmittance,
                material.solar_transmittance,
                material.reflectance,
            )?;
            if material_map.insert(material.id, material).is_some() {
                return Err(MaterialError::DuplicateMaterial);
            }
        }
        let mut assignment_map = BTreeMap::new();
        for (object_id, material_id) in assignments {
            if !material_map.contains_key(&material_id) {
                return Err(MaterialError::UnknownMaterial);
            }
            if assignment_map.insert(object_id, material_id).is_some() {
                return Err(MaterialError::DuplicateAssignment);
            }
        }
        Ok(Self {
            materials: material_map,
            assignments: assignment_map,
        })
    }

    /// Number of explicit material definitions.
    #[must_use]
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Number of object-to-material assignments.
    #[must_use]
    pub fn assignment_count(&self) -> usize {
        self.assignments.len()
    }

    /// Material assigned to an object; `None` means implicit opaque context.
    #[must_use]
    pub fn material_for_object(&self, object_id: ObjectId) -> Option<AnalysisMaterial> {
        self.assignments
            .get(&object_id)
            .and_then(|id| self.materials.get(id))
            .copied()
    }

    /// Deterministic content identity for scenario provenance.
    #[must_use]
    pub fn content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"XVARNA_ANALYSIS_MATERIALS_V1\0");
        for material in self.materials.values() {
            hasher.update(&material.id.to_le_bytes());
            hasher.update(&material.visible_transmittance.to_bits().to_le_bytes());
            hasher.update(&material.solar_transmittance.to_bits().to_le_bytes());
            hasher.update(&material.reflectance.to_bits().to_le_bytes());
        }
        for (object, material) in &self.assignments {
            hasher.update(&object.get().to_le_bytes());
            hasher.update(&material.to_le_bytes());
        }
        *hasher.finalize().as_bytes()
    }
}

/// Optical channel selected for a multi-layer ray.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransmissionChannel {
    /// Human-visible transmission.
    Visible,
    /// Direct solar transmission.
    Solar,
}

/// One ordered interaction in a material-aware ray trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransmissionLayer {
    /// Geometric hit with cumulative distance from the original ray origin.
    pub hit: Hit,
    /// Throughput arriving at this hit.
    pub incident_transmission: f64,
    /// Fraction removed at this hit from the original unit signal.
    pub attributed_loss: f64,
    /// Material identifier; zero denotes implicit opaque context.
    pub material_id: MaterialId,
}

/// Ordered multi-hit material result.
#[derive(Clone, Debug, PartialEq)]
pub struct TransmissionTrace {
    /// Remaining fraction after all traced layers.
    pub transmission: f64,
    /// Whether tracing stopped at the configured layer cap while throughput remained.
    pub layer_limit_reached: bool,
    /// Ordered front-to-back interactions.
    pub layers: Vec<TransmissionLayer>,
}

const fn is_fraction(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}
