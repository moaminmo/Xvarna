//! Deterministic daylight-coefficient matrices and climate-based daylight metrics.

use core::{f64::consts::TAU, fmt};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use xvarna_geometry::Vec3;
use xvarna_scene::{AnalysisMaterial, MaterialLibrary, QueryRay, Scene, TransmissionChannel};
use xvarna_types::{ObjectId, SensorId};
use xvarna_zurvan::{EpwRecord, EpwWeather, SolarOptions, calculate_sun_set};

const PHOTOPIC_RED: f64 = 0.265;
const PHOTOPIC_GREEN: f64 = 0.670;
const PHOTOPIC_BLUE: f64 = 0.065;
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
const MINIMUM_SKY_PATCHES: usize = 64;
const MAXIMUM_SKY_PATCHES: usize = 65_536;
const MAXIMUM_RESULT_CELLS: usize = 32_000_000;
const MATRIX_CACHE_CAPACITY: usize = 8;

static MATRIX_CACHE: OnceLock<Mutex<VecDeque<Arc<DaylightCoefficientMatrix>>>> = OnceLock::new();
static MATRIX_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static MATRIX_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// Radiance-compatible optical primitive used by daylight analysis and export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OpticalMaterialKind {
    /// Opaque diffuse/specular dielectric.
    Plastic = 0,
    /// Thin or solid transparent glazing.
    Glass = 1,
    /// Opaque conductive surface.
    Metal = 2,
    /// Diffuse and specular transmissive surface.
    Trans = 3,
    /// Idealized mirror primitive.
    Mirror = 4,
}

/// Physically bounded RGB optical material with a stable identifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpticalMaterial {
    /// Stable non-zero identifier.
    pub id: u64,
    /// Radiance primitive family.
    pub kind: OpticalMaterialKind,
    /// Linear RGB diffuse/specular reflectance.
    pub reflectance: [f64; 3],
    /// Linear RGB visible transmittance.
    pub transmittance: [f64; 3],
    /// Specular fraction in zero through one.
    pub specularity: f64,
    /// Surface roughness in zero through one.
    pub roughness: f64,
    /// Refractive index; normally approximately 1.52 for glass.
    pub refractive_index: f64,
}

impl OpticalMaterial {
    /// Validates energy bounds and parameters.
    pub fn try_new(
        id: u64,
        kind: OpticalMaterialKind,
        reflectance: [f64; 3],
        transmittance: [f64; 3],
        specularity: f64,
        roughness: f64,
        refractive_index: f64,
    ) -> Result<Self, DaylightError> {
        if id == 0
            || reflectance.into_iter().any(|value| !is_fraction(value))
            || transmittance.into_iter().any(|value| !is_fraction(value))
            || reflectance
                .into_iter()
                .zip(transmittance)
                .any(|(reflected, transmitted)| reflected + transmitted > 1.0 + 1.0e-12)
            || !is_fraction(specularity)
            || !is_fraction(roughness)
            || !refractive_index.is_finite()
            || !(1.0..=3.0).contains(&refractive_index)
        {
            return Err(DaylightError::InvalidMaterial);
        }
        if !matches!(
            kind,
            OpticalMaterialKind::Glass | OpticalMaterialKind::Trans
        ) && transmittance.iter().any(|value| *value > 0.0)
        {
            return Err(DaylightError::InvalidMaterial);
        }
        Ok(Self {
            id,
            kind,
            reflectance,
            transmittance,
            specularity,
            roughness,
            refractive_index,
        })
    }

    /// Photopic RGB-weighted reflectance used by the fast path.
    #[must_use]
    pub const fn photopic_reflectance(self) -> f64 {
        photopic(self.reflectance)
    }

    /// Photopic RGB-weighted transmittance used by the fast path.
    #[must_use]
    pub const fn photopic_transmittance(self) -> f64 {
        photopic(self.transmittance)
    }
}

/// Immutable optical catalog assigned by exact scene Object ID.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpticalMaterialLibrary {
    materials: BTreeMap<u64, OpticalMaterial>,
    assignments: BTreeMap<ObjectId, u64>,
}

impl OpticalMaterialLibrary {
    /// Builds a catalog. Unassigned scene objects remain opaque with zero transmission.
    pub fn try_new(
        materials: impl IntoIterator<Item = OpticalMaterial>,
        assignments: impl IntoIterator<Item = (ObjectId, u64)>,
    ) -> Result<Self, DaylightError> {
        let mut material_map = BTreeMap::new();
        for material in materials {
            let material = OpticalMaterial::try_new(
                material.id,
                material.kind,
                material.reflectance,
                material.transmittance,
                material.specularity,
                material.roughness,
                material.refractive_index,
            )?;
            if material_map.insert(material.id, material).is_some() {
                return Err(DaylightError::DuplicateMaterial);
            }
        }
        let mut assignment_map = BTreeMap::new();
        for (object, material) in assignments {
            if !material_map.contains_key(&material) {
                return Err(DaylightError::UnknownMaterial);
            }
            if assignment_map.insert(object, material).is_some() {
                return Err(DaylightError::DuplicateAssignment);
            }
        }
        Ok(Self {
            materials: material_map,
            assignments: assignment_map,
        })
    }

    /// Explicit material count.
    #[must_use]
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Explicit object assignment count.
    #[must_use]
    pub fn assignment_count(&self) -> usize {
        self.assignments.len()
    }

    /// Looks up one assigned material.
    #[must_use]
    pub fn material_for_object(&self, object: ObjectId) -> Option<OpticalMaterial> {
        self.assignments
            .get(&object)
            .and_then(|id| self.materials.get(id))
            .copied()
    }

    /// Iterates deterministic material definitions.
    pub fn materials(&self) -> impl Iterator<Item = OpticalMaterial> + '_ {
        self.materials.values().copied()
    }

    /// Iterates deterministic object assignments.
    pub fn assignments(&self) -> impl Iterator<Item = (ObjectId, u64)> + '_ {
        self.assignments
            .iter()
            .map(|(object, material)| (*object, *material))
    }

    /// Stable semantic content identity.
    #[must_use]
    pub fn content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"XVARNA_OPTICAL_MATERIALS_V1\0");
        for material in self.materials.values() {
            hasher.update(&material.id.to_le_bytes());
            hasher.update(&(material.kind as u32).to_le_bytes());
            for value in material
                .reflectance
                .into_iter()
                .chain(material.transmittance)
                .chain([
                    material.specularity,
                    material.roughness,
                    material.refractive_index,
                ])
            {
                hasher.update(&value.to_bits().to_le_bytes());
            }
        }
        for (object, material) in &self.assignments {
            hasher.update(&object.get().to_le_bytes());
            hasher.update(&material.to_le_bytes());
        }
        *hasher.finalize().as_bytes()
    }

    fn analysis_library(&self) -> MaterialLibrary {
        let materials = self.materials.values().map(|material| {
            AnalysisMaterial::try_new(
                material.id,
                material.photopic_transmittance(),
                material.photopic_transmittance(),
                material.photopic_reflectance(),
            )
            .expect("optical validation implies scalar energy conservation")
        });
        MaterialLibrary::try_new(materials, self.assignments()).expect("catalog is validated")
    }
}

/// Oriented daylight workplane point with an area weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightSensor {
    /// Stable identifier.
    pub id: SensorId,
    /// Position in canonical metres.
    pub position: Vec3,
    /// Unit workplane normal.
    pub normal: Vec3,
    /// Positive represented area in square metres.
    pub area_square_meters: f64,
}

impl DaylightSensor {
    /// Validates and normalizes a daylight sensor.
    pub fn try_new(
        id: SensorId,
        position: Vec3,
        normal: Vec3,
        area_square_meters: f64,
    ) -> Result<Self, DaylightError> {
        if !position.is_finite() || !area_square_meters.is_finite() || area_square_meters <= 0.0 {
            return Err(DaylightError::InvalidSensor);
        }
        let normal = normal.normalized().ok_or(DaylightError::InvalidSensor)?;
        Ok(Self {
            id,
            position,
            normal,
            area_square_meters,
        })
    }
}

/// Sky luminance distribution used by the deterministic fast path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DaylightSkyModel {
    /// Uniform diffuse sky normalized by horizontal illuminance.
    Isotropic = 0,
    /// CIE standard overcast relative distribution `1 + 2 sin(altitude)`.
    CieOvercast = 1,
}

/// Static ray and optical policy for a reusable daylight-coefficient matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightMatrixOptions {
    /// Equal-solid-angle upper-hemisphere patch count.
    pub sky_patch_count: usize,
    /// Sensor normal offset in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum context distance; infinity is supported.
    pub maximum_distance_meters: f64,
    /// Included scene categories.
    pub category_mask: u64,
    /// Maximum transparent geometric interactions.
    pub maximum_material_layers: usize,
    /// Throughput below this fraction is zero.
    pub minimum_transmission: f64,
}

impl Default for DaylightMatrixOptions {
    fn default() -> Self {
        Self {
            sky_patch_count: 576,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
            maximum_material_layers: 8,
            minimum_transmission: 1.0e-4,
        }
    }
}

impl DaylightMatrixOptions {
    fn validate(self) -> Result<Self, DaylightError> {
        if !(MINIMUM_SKY_PATCHES..=MAXIMUM_SKY_PATCHES).contains(&self.sky_patch_count)
            || !self.sensor_offset_meters.is_finite()
            || self.sensor_offset_meters < 0.0
            || self.maximum_distance_meters.is_nan()
            || self.maximum_distance_meters <= 0.0
            || !(1..=64).contains(&self.maximum_material_layers)
            || !is_fraction(self.minimum_transmission)
        {
            return Err(DaylightError::InvalidOptions);
        }
        Ok(self)
    }
}

/// Reusable sensor-by-sky daylight coefficients.
#[derive(Clone, Debug, PartialEq)]
pub struct DaylightCoefficientMatrix {
    /// Ordered upper-hemisphere patch directions.
    pub directions: Vec<Vec3>,
    /// Sensor-major projected solid-angle transmission coefficients.
    pub coefficients: Vec<f64>,
    /// Source sensor count.
    pub sensor_count: usize,
    /// Patches per sensor.
    pub patch_count: usize,
    /// Isotropic horizontal normalization integral.
    pub isotropic_horizontal_normalizer: f64,
    /// CIE-overcast horizontal normalization integral.
    pub overcast_horizontal_normalizer: f64,
    /// Stable scene/sensor/material/policy identity.
    pub content_hash: [u8; 32],
}

/// Matrix-cache counters for observable annual reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DaylightMatrixCacheStats {
    /// Process-lifetime memory hits.
    pub hits: u64,
    /// Process-lifetime matrix builds.
    pub misses: u64,
    /// Current retained matrices.
    pub entries: usize,
}

/// Reads daylight matrix cache telemetry.
#[must_use]
pub fn daylight_matrix_cache_stats() -> DaylightMatrixCacheStats {
    DaylightMatrixCacheStats {
        hits: MATRIX_CACHE_HITS.load(Ordering::Relaxed),
        misses: MATRIX_CACHE_MISSES.load(Ordering::Relaxed),
        entries: matrix_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
    }
}

/// Clears only the process-memory daylight coefficient cache.
pub fn clear_daylight_matrix_cache() {
    matrix_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// Builds or reuses one exact daylight coefficient matrix.
pub fn daylight_coefficient_matrix(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    options: DaylightMatrixOptions,
) -> Result<(Arc<DaylightCoefficientMatrix>, bool), DaylightError> {
    daylight_coefficient_matrix_controlled(scene, sensors, materials, options, None, None)
}

fn daylight_coefficient_matrix_controlled(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    options: DaylightMatrixOptions,
    cancelled: Option<&AtomicBool>,
    completed: Option<&AtomicU64>,
) -> Result<(Arc<DaylightCoefficientMatrix>, bool), DaylightError> {
    if sensors.is_empty() {
        return Err(DaylightError::EmptySensors);
    }
    let options = options.validate()?;
    let cell_count = sensors
        .len()
        .checked_mul(options.sky_patch_count)
        .filter(|count| *count <= MAXIMUM_RESULT_CELLS)
        .ok_or(DaylightError::ResultTooLarge)?;
    let key = matrix_key(scene, sensors, materials, options);
    {
        let mut cache = matrix_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = cache.iter().position(|entry| entry.content_hash == key) {
            let entry = cache.remove(index).expect("located matrix exists");
            cache.push_front(Arc::clone(&entry));
            drop(cache);
            MATRIX_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            if let Some(progress) = completed {
                progress.fetch_add(
                    u64::try_from(cell_count).unwrap_or(u64::MAX),
                    Ordering::Relaxed,
                );
            }
            return Ok((entry, true));
        }
    }
    MATRIX_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let directions = fibonacci_upper_hemisphere(options.sky_patch_count);
    let solid_angle = TAU / usize_to_f64(options.sky_patch_count);
    let analysis_materials = materials.analysis_library();
    let mut coefficients = Vec::with_capacity(cell_count);
    for sensor in sensors {
        for direction in &directions {
            check_cancelled(cancelled)?;
            let incidence = sensor.normal.dot(*direction).max(0.0);
            if incidence <= 0.0 {
                coefficients.push(0.0);
            } else {
                let ray = QueryRay::try_new(
                    sensor.position + sensor.normal * options.sensor_offset_meters,
                    *direction,
                    0.0,
                    options.maximum_distance_meters,
                    options.category_mask,
                )
                .map_err(|_| DaylightError::InvalidQuery)?;
                let trace = scene.trace_transmission(
                    ray,
                    &analysis_materials,
                    TransmissionChannel::Visible,
                    options.maximum_material_layers,
                    options.minimum_transmission,
                );
                coefficients.push(incidence * solid_angle * trace.transmission);
            }
            if let Some(progress) = completed {
                progress.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    let isotropic_horizontal_normalizer = directions
        .iter()
        .map(|direction| direction.z * solid_angle)
        .sum();
    let overcast_horizontal_normalizer = directions
        .iter()
        .map(|direction| 2.0_f64.mul_add(direction.z, 1.0) * direction.z * solid_angle)
        .sum();
    let matrix = Arc::new(DaylightCoefficientMatrix {
        directions,
        coefficients,
        sensor_count: sensors.len(),
        patch_count: options.sky_patch_count,
        isotropic_horizontal_normalizer,
        overcast_horizontal_normalizer,
        content_hash: key,
    });
    let mut cache = matrix_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.push_front(Arc::clone(&matrix));
    while cache.len() > MATRIX_CACHE_CAPACITY {
        cache.pop_back();
    }
    drop(cache);
    Ok((matrix, false))
}

/// One photometric sky and sun condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightMoment {
    /// UTC Unix timestamp for provenance.
    pub unix_seconds_utc: i64,
    /// Unit direction from the sensor toward the sun.
    pub sun_direction: Vec3,
    /// Direct-normal illuminance in lux.
    pub direct_normal_illuminance_lux: f64,
    /// Diffuse-horizontal illuminance in lux.
    pub diffuse_horizontal_illuminance_lux: f64,
}

impl DaylightMoment {
    /// Creates a bounded photometric condition.
    pub fn try_new(
        unix_seconds_utc: i64,
        sun_direction: Vec3,
        direct_normal_illuminance_lux: f64,
        diffuse_horizontal_illuminance_lux: f64,
    ) -> Result<Self, DaylightError> {
        let sun_direction = sun_direction
            .normalized()
            .ok_or(DaylightError::InvalidMoment)?;
        if !direct_normal_illuminance_lux.is_finite()
            || direct_normal_illuminance_lux < 0.0
            || !diffuse_horizontal_illuminance_lux.is_finite()
            || diffuse_horizontal_illuminance_lux < 0.0
        {
            return Err(DaylightError::InvalidMoment);
        }
        Ok(Self {
            unix_seconds_utc,
            sun_direction,
            direct_normal_illuminance_lux,
            diffuse_horizontal_illuminance_lux,
        })
    }
}

/// Direct, diffuse, and total point-in-time illuminance for one sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointIlluminanceEntry {
    /// Source sensor.
    pub sensor_id: SensorId,
    /// Received direct illuminance in lux.
    pub direct_lux: f64,
    /// Received sky-diffuse illuminance in lux.
    pub diffuse_lux: f64,
    /// Direct plus diffuse illuminance in lux.
    pub total_lux: f64,
    /// Remaining direct-path visible transmission.
    pub direct_transmission: f64,
}

/// Complete fast-path point-in-time result.
#[derive(Clone, Debug, PartialEq)]
pub struct PointIlluminanceResult {
    /// One entry per sensor.
    pub entries: Vec<PointIlluminanceEntry>,
    /// True when an exact coefficient matrix was reused.
    pub matrix_reused: bool,
    /// Daylight coefficient identity.
    pub matrix_hash: [u8; 32],
    /// Scene, material, condition, and output identity.
    pub content_hash: [u8; 32],
}

/// Computes material-aware point-in-time illuminance.
pub fn analyze_point_illuminance(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    moment: DaylightMoment,
    sky_model: DaylightSkyModel,
    options: DaylightMatrixOptions,
) -> Result<PointIlluminanceResult, DaylightError> {
    analyze_point_illuminance_controlled(
        scene, sensors, materials, moment, sky_model, options, None, None,
    )
}

/// Cancellable point-in-time analysis with matrix-build and direct-ray progress.
#[allow(clippy::too_many_arguments)]
pub fn analyze_point_illuminance_controlled(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    moment: DaylightMoment,
    sky_model: DaylightSkyModel,
    options: DaylightMatrixOptions,
    cancelled: Option<&AtomicBool>,
    completed: Option<&AtomicU64>,
) -> Result<PointIlluminanceResult, DaylightError> {
    let moment = DaylightMoment::try_new(
        moment.unix_seconds_utc,
        moment.sun_direction,
        moment.direct_normal_illuminance_lux,
        moment.diffuse_horizontal_illuminance_lux,
    )?;
    let options = options.validate()?;
    let (matrix, matrix_reused) = daylight_coefficient_matrix_controlled(
        scene, sensors, materials, options, cancelled, completed,
    )?;
    let analysis_materials = materials.analysis_library();
    let mut entries = Vec::with_capacity(sensors.len());
    for (sensor_index, sensor) in sensors.iter().enumerate() {
        check_cancelled(cancelled)?;
        entries.push(point_entry(
            scene,
            *sensor,
            sensor_index,
            &matrix,
            &analysis_materials,
            moment,
            sky_model,
            options,
        )?);
        if let Some(progress) = completed {
            progress.fetch_add(1, Ordering::Relaxed);
        }
    }
    let content_hash = point_hash(
        scene, sensors, materials, moment, sky_model, options, &entries,
    );
    Ok(PointIlluminanceResult {
        entries,
        matrix_reused,
        matrix_hash: matrix.content_hash,
        content_hash,
    })
}

/// CIE-overcast daylight factor for one sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightFactorEntry {
    /// Source sensor.
    pub sensor_id: SensorId,
    /// Interior workplane illuminance in lux.
    pub interior_illuminance_lux: f64,
    /// Interior divided by declared exterior horizontal illuminance, percent.
    pub daylight_factor_percent: f64,
}

/// Complete daylight-factor result.
#[derive(Clone, Debug, PartialEq)]
pub struct DaylightFactorResult {
    /// One value per sensor.
    pub entries: Vec<DaylightFactorEntry>,
    /// Exterior CIE-overcast horizontal illuminance used for scaling.
    pub exterior_horizontal_illuminance_lux: f64,
    /// True when the coefficient matrix was reused.
    pub matrix_reused: bool,
    /// Matrix identity.
    pub matrix_hash: [u8; 32],
    /// Deterministic result identity.
    pub content_hash: [u8; 32],
}

/// Calculates CIE-overcast daylight factor without a direct-sun component.
pub fn analyze_daylight_factor(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    exterior_horizontal_illuminance_lux: f64,
    options: DaylightMatrixOptions,
) -> Result<DaylightFactorResult, DaylightError> {
    if !exterior_horizontal_illuminance_lux.is_finite()
        || exterior_horizontal_illuminance_lux <= 0.0
    {
        return Err(DaylightError::InvalidMoment);
    }
    let (matrix, matrix_reused) = daylight_coefficient_matrix(scene, sensors, materials, options)?;
    let entries = sensors
        .iter()
        .enumerate()
        .map(|(index, sensor)| {
            let interior = diffuse_illuminance(
                index,
                &matrix,
                exterior_horizontal_illuminance_lux,
                DaylightSkyModel::CieOvercast,
            );
            DaylightFactorEntry {
                sensor_id: sensor.id,
                interior_illuminance_lux: interior,
                daylight_factor_percent: 100.0 * interior / exterior_horizontal_illuminance_lux,
            }
        })
        .collect::<Vec<_>>();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAYLIGHT_FACTOR_V1\0");
    hasher.update(&matrix.content_hash);
    hasher.update(&exterior_horizontal_illuminance_lux.to_bits().to_le_bytes());
    for entry in &entries {
        hasher.update(&entry.sensor_id.get().to_le_bytes());
        hasher.update(&entry.interior_illuminance_lux.to_bits().to_le_bytes());
        hasher.update(&entry.daylight_factor_percent.to_bits().to_le_bytes());
    }
    Ok(DaylightFactorResult {
        entries,
        exterior_horizontal_illuminance_lux,
        matrix_reused,
        matrix_hash: matrix.content_hash,
        content_hash: *hasher.finalize().as_bytes(),
    })
}

/// Annual climate, occupancy, luminous-efficacy, and LM-83/UDI thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualDaylightOptions {
    /// Static daylight coefficient policy.
    pub matrix: DaylightMatrixOptions,
    /// Solar-position and north-rotation policy.
    pub solar: SolarOptions,
    /// Explicit direct luminous efficacy in lumens per watt.
    pub direct_luminous_efficacy_lm_per_w: f64,
    /// Explicit diffuse luminous efficacy in lumens per watt.
    pub diffuse_luminous_efficacy_lm_per_w: f64,
    /// Local-standard occupied start hour, inclusive.
    pub occupied_start_hour: f64,
    /// Local-standard occupied end hour, exclusive.
    pub occupied_end_hour: f64,
    /// sDA illuminance threshold, normally 300 lux.
    pub sda_threshold_lux: f64,
    /// Required occupied fraction, normally 0.5.
    pub sda_required_fraction: f64,
    /// ASE direct-sun threshold, normally 1000 lux.
    pub ase_threshold_lux: f64,
    /// ASE permitted exceedance hours, normally 250.
    pub ase_maximum_hours: f64,
    /// UDI lower useful threshold, normally 100 lux.
    pub udi_lower_lux: f64,
    /// UDI preferred threshold, normally 300 lux.
    pub udi_preferred_lux: f64,
    /// UDI upper useful threshold, normally 3000 lux.
    pub udi_upper_lux: f64,
    /// Diffuse sky model used for every climate interval.
    pub sky_model: DaylightSkyModel,
}

impl Default for AnnualDaylightOptions {
    fn default() -> Self {
        Self {
            matrix: DaylightMatrixOptions::default(),
            solar: SolarOptions::default(),
            direct_luminous_efficacy_lm_per_w: 100.0,
            diffuse_luminous_efficacy_lm_per_w: 120.0,
            occupied_start_hour: 8.0,
            occupied_end_hour: 18.0,
            sda_threshold_lux: 300.0,
            sda_required_fraction: 0.5,
            ase_threshold_lux: 1_000.0,
            ase_maximum_hours: 250.0,
            udi_lower_lux: 100.0,
            udi_preferred_lux: 300.0,
            udi_upper_lux: 3_000.0,
            sky_model: DaylightSkyModel::Isotropic,
        }
    }
}

impl AnnualDaylightOptions {
    fn validate(self) -> Result<Self, DaylightError> {
        self.matrix.validate()?;
        if !positive(self.direct_luminous_efficacy_lm_per_w)
            || !positive(self.diffuse_luminous_efficacy_lm_per_w)
            || !hour(self.occupied_start_hour)
            || !hour(self.occupied_end_hour)
            || (self.occupied_start_hour - self.occupied_end_hour).abs() <= f64::EPSILON
            || !positive(self.sda_threshold_lux)
            || !is_fraction(self.sda_required_fraction)
            || !positive(self.ase_threshold_lux)
            || !self.ase_maximum_hours.is_finite()
            || self.ase_maximum_hours < 0.0
            || !positive(self.udi_lower_lux)
            || !positive(self.udi_preferred_lux)
            || !positive(self.udi_upper_lux)
            || !(self.udi_lower_lux < self.udi_preferred_lux
                && self.udi_preferred_lux < self.udi_upper_lux)
        {
            return Err(DaylightError::InvalidOptions);
        }
        Ok(self)
    }
}

/// One annual sensor/time photometric cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualDaylightTimelineEntry {
    /// UTC midpoint inherited from EPW.
    pub unix_seconds_utc: i64,
    /// True when the local-standard midpoint is inside the declared occupied window.
    pub occupied: bool,
    /// Received direct illuminance in lux.
    pub direct_lux: f64,
    /// Received diffuse illuminance in lux.
    pub diffuse_lux: f64,
    /// Total workplane illuminance in lux.
    pub total_lux: f64,
}

/// Annual sensor-level sDA, ASE, UDI, and illuminance distribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualDaylightSensorSummary {
    /// Source sensor.
    pub sensor_id: SensorId,
    /// Occupied duration represented by the EPW timeline.
    pub occupied_hours: f64,
    /// Occupied hours meeting the sDA threshold.
    pub sda_qualified_hours: f64,
    /// Fraction of occupied hours meeting the threshold.
    pub sda_occupied_fraction: f64,
    /// Direct-sun hours exceeding the ASE threshold.
    pub ase_exceedance_hours: f64,
    /// True when exceedance hours are above the declared ASE limit.
    pub ase_fails: bool,
    /// Occupied fraction below UDI lower bound.
    pub udi_below_fraction: f64,
    /// Occupied fraction in lower-to-preferred range.
    pub udi_supplemental_fraction: f64,
    /// Occupied fraction in preferred-to-upper range.
    pub udi_useful_fraction: f64,
    /// Occupied fraction above UDI upper bound.
    pub udi_exceeded_fraction: f64,
    /// Occupied duration-weighted mean illuminance.
    pub mean_occupied_lux: f64,
    /// Minimum occupied illuminance.
    pub minimum_occupied_lux: f64,
    /// Maximum occupied illuminance.
    pub maximum_occupied_lux: f64,
}

/// Area-weighted annual daylight metrics for the complete sensor grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualDaylightProjectSummary {
    /// Sensor area meeting sDA threshold for the required occupied fraction, percent.
    pub sda_area_percent: f64,
    /// Sensor area failing the ASE hour limit, percent.
    pub ase_area_percent: f64,
    /// Area-weighted UDI below fraction.
    pub udi_below_percent: f64,
    /// Area-weighted UDI supplemental fraction.
    pub udi_supplemental_percent: f64,
    /// Area-weighted UDI preferred/useful fraction.
    pub udi_useful_percent: f64,
    /// Area-weighted UDI exceeded fraction.
    pub udi_exceeded_percent: f64,
    /// Total represented sensor area in square metres.
    pub total_sensor_area_square_meters: f64,
}

/// Complete climate-based fast-path daylight result.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnualDaylightResult {
    /// One summary per sensor.
    pub summaries: Vec<AnnualDaylightSensorSummary>,
    /// Sensor-major EPW timeline.
    pub timeline: Vec<AnnualDaylightTimelineEntry>,
    /// Area-weighted project metrics.
    pub project: AnnualDaylightProjectSummary,
    /// EPW records per sensor row.
    pub weather_count: usize,
    /// True when the exact coefficient matrix came from process memory.
    pub matrix_reused: bool,
    /// Matrix identity.
    pub matrix_hash: [u8; 32],
    /// Stable scene/weather/material/policy/result identity.
    pub content_hash: [u8; 32],
}

/// Runs annual daylight metrics using reusable static coefficients and direct climate rays.
pub fn analyze_annual_daylight(
    scene: &Scene,
    sensors: &[DaylightSensor],
    weather: &EpwWeather,
    materials: &OpticalMaterialLibrary,
    options: AnnualDaylightOptions,
) -> Result<AnnualDaylightResult, DaylightError> {
    analyze_annual_daylight_controlled(scene, sensors, weather, materials, options, None, None)
}

/// Cancellable annual daylight analysis with safe-boundary progress.
#[allow(clippy::too_many_lines)]
pub fn analyze_annual_daylight_controlled(
    scene: &Scene,
    sensors: &[DaylightSensor],
    weather: &EpwWeather,
    materials: &OpticalMaterialLibrary,
    options: AnnualDaylightOptions,
    cancelled: Option<&AtomicBool>,
    completed: Option<&AtomicU64>,
) -> Result<AnnualDaylightResult, DaylightError> {
    if sensors.is_empty() {
        return Err(DaylightError::EmptySensors);
    }
    if weather.records.is_empty() {
        return Err(DaylightError::EmptyWeather);
    }
    let options = options.validate()?;
    sensors
        .len()
        .checked_mul(weather.records.len())
        .filter(|count| *count <= MAXIMUM_RESULT_CELLS)
        .ok_or(DaylightError::ResultTooLarge)?;
    let (matrix, matrix_reused) = daylight_coefficient_matrix_controlled(
        scene,
        sensors,
        materials,
        options.matrix,
        cancelled,
        completed,
    )?;
    let sun_set = calculate_sun_set(
        weather.location.solar,
        &weather.time_samples(),
        options.solar,
    )
    .map_err(|_| DaylightError::Solar)?;
    let analysis_materials = materials.analysis_library();
    let mut timeline = Vec::with_capacity(sensors.len() * weather.records.len());
    let mut summaries = Vec::with_capacity(sensors.len());
    for (sensor_index, sensor) in sensors.iter().copied().enumerate() {
        let row_start = timeline.len();
        for (record, sun) in weather.records.iter().zip(&sun_set.samples) {
            check_cancelled(cancelled)?;
            let duration = record.duration_hours;
            let direct_normal_w_m2 = record.direct_normal_wh_m2 / duration;
            let diffuse_horizontal_w_m2 = record.diffuse_horizontal_wh_m2 / duration;
            let moment = DaylightMoment {
                unix_seconds_utc: record.midpoint_unix_seconds_utc,
                sun_direction: sun.direction,
                direct_normal_illuminance_lux: direct_normal_w_m2
                    * options.direct_luminous_efficacy_lm_per_w,
                diffuse_horizontal_illuminance_lux: diffuse_horizontal_w_m2
                    * options.diffuse_luminous_efficacy_lm_per_w,
            };
            let entry = point_entry(
                scene,
                sensor,
                sensor_index,
                &matrix,
                &analysis_materials,
                moment,
                options.sky_model,
                options.matrix,
            )?;
            timeline.push(AnnualDaylightTimelineEntry {
                unix_seconds_utc: record.midpoint_unix_seconds_utc,
                occupied: is_occupied(record, options),
                direct_lux: entry.direct_lux,
                diffuse_lux: entry.diffuse_lux,
                total_lux: entry.total_lux,
            });
            if let Some(progress) = completed {
                progress.fetch_add(1, Ordering::Relaxed);
            }
        }
        summaries.push(summarize_annual_sensor(
            sensor,
            &weather.records,
            &timeline[row_start..],
            options,
        ));
    }
    let project = summarize_project(sensors, &summaries, options);
    let content_hash = annual_hash(
        scene,
        sensors,
        weather,
        materials,
        options,
        &summaries,
        project,
        matrix.content_hash,
    );
    Ok(AnnualDaylightResult {
        summaries,
        timeline,
        project,
        weather_count: weather.records.len(),
        matrix_reused,
        matrix_hash: matrix.content_hash,
        content_hash,
    })
}

/// Numerical comparison between fast-path and Radiance illuminance arrays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightValidationReport {
    /// Compared pair count.
    pub count: usize,
    /// Mean signed fast-minus-reference bias in lux.
    pub mean_bias_lux: f64,
    /// Mean absolute error in lux.
    pub mean_absolute_error_lux: f64,
    /// Root mean square error in lux.
    pub root_mean_square_error_lux: f64,
    /// Mean absolute percentage error over non-negligible references.
    pub mean_absolute_percentage_error: f64,
    /// Largest absolute error in lux.
    pub maximum_absolute_error_lux: f64,
    /// Coefficient of determination against the reference.
    pub r_squared: f64,
    /// Fraction within the caller's absolute and relative acceptance envelope.
    pub accepted_fraction: f64,
    /// True when every finite pair meets the declared envelope.
    pub accepted: bool,
    /// Stable input and metric identity.
    pub content_hash: [u8; 32],
}

/// Compares aligned Fast Path and Radiance values without hiding zero-reference cases.
pub fn compare_daylight_results(
    fast_lux: &[f64],
    radiance_lux: &[f64],
    absolute_tolerance_lux: f64,
    relative_tolerance: f64,
) -> Result<DaylightValidationReport, DaylightError> {
    if fast_lux.is_empty() || fast_lux.len() != radiance_lux.len() {
        return Err(DaylightError::MismatchedInputs);
    }
    if fast_lux
        .iter()
        .chain(radiance_lux)
        .any(|value| !value.is_finite())
        || !absolute_tolerance_lux.is_finite()
        || absolute_tolerance_lux < 0.0
        || !is_fraction(relative_tolerance)
    {
        return Err(DaylightError::InvalidOptions);
    }
    let count = fast_lux.len();
    let mean_reference = radiance_lux.iter().sum::<f64>() / usize_to_f64(count);
    let mut signed = 0.0;
    let mut absolute = 0.0;
    let mut squared = 0.0;
    let mut maximum = 0.0_f64;
    let mut percentage = 0.0;
    let mut percentage_count = 0_usize;
    let mut residual_squared = 0.0;
    let mut reference_squared = 0.0;
    let mut accepted_count = 0_usize;
    for (&fast, &reference) in fast_lux.iter().zip(radiance_lux) {
        let error = fast - reference;
        let abs = error.abs();
        signed += error;
        absolute += abs;
        squared = error.mul_add(error, squared);
        maximum = maximum.max(abs);
        residual_squared = error.mul_add(error, residual_squared);
        reference_squared =
            (reference - mean_reference).mul_add(reference - mean_reference, reference_squared);
        if reference.abs() > 1.0e-9 {
            percentage += abs / reference.abs();
            percentage_count += 1;
        }
        if abs <= absolute_tolerance_lux.max(relative_tolerance * reference.abs()) {
            accepted_count += 1;
        }
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAYLIGHT_VALIDATION_V1\0");
    for value in fast_lux.iter().chain(radiance_lux) {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&absolute_tolerance_lux.to_bits().to_le_bytes());
    hasher.update(&relative_tolerance.to_bits().to_le_bytes());
    Ok(DaylightValidationReport {
        count,
        mean_bias_lux: signed / usize_to_f64(count),
        mean_absolute_error_lux: absolute / usize_to_f64(count),
        root_mean_square_error_lux: (squared / usize_to_f64(count)).sqrt(),
        mean_absolute_percentage_error: if percentage_count == 0 {
            0.0
        } else {
            percentage / usize_to_f64(percentage_count)
        },
        maximum_absolute_error_lux: maximum,
        r_squared: if reference_squared <= f64::EPSILON {
            if residual_squared <= f64::EPSILON {
                1.0
            } else {
                0.0
            }
        } else {
            1.0 - residual_squared / reference_squared
        },
        accepted_fraction: usize_to_f64(accepted_count) / usize_to_f64(count),
        accepted: accepted_count == count,
        content_hash: *hasher.finalize().as_bytes(),
    })
}

/// Daylight input, resource, cancellation, or numerical failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DaylightError {
    /// At least one sensor is required.
    EmptySensors,
    /// At least one weather interval is required.
    EmptyWeather,
    /// An optical material is invalid.
    InvalidMaterial,
    /// A material ID was repeated.
    DuplicateMaterial,
    /// An object assignment was repeated.
    DuplicateAssignment,
    /// An assignment references no material.
    UnknownMaterial,
    /// Sensor geometry or area is invalid.
    InvalidSensor,
    /// Matrix, thresholds, or tolerance policy is invalid.
    InvalidOptions,
    /// Photometric moment is invalid.
    InvalidMoment,
    /// A ray query could not be constructed.
    InvalidQuery,
    /// Solar-position computation failed.
    Solar,
    /// Requested matrix or timeline exceeds the production cap.
    ResultTooLarge,
    /// Aligned comparison arrays differ in length.
    MismatchedInputs,
    /// Cooperative cancellation was observed.
    Cancelled,
}

impl fmt::Display for DaylightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptySensors => "at least one daylight sensor is required",
            Self::EmptyWeather => "at least one weather interval is required",
            Self::InvalidMaterial => "optical material values are invalid or non-conserving",
            Self::DuplicateMaterial => "optical material identifier is duplicated",
            Self::DuplicateAssignment => "optical object assignment is duplicated",
            Self::UnknownMaterial => "optical assignment references an unknown material",
            Self::InvalidSensor => "daylight sensor position, normal, or area is invalid",
            Self::InvalidOptions => "daylight matrix, metric, or tolerance policy is invalid",
            Self::InvalidMoment => "point-in-time photometric condition is invalid",
            Self::InvalidQuery => "daylight ray query is invalid",
            Self::Solar => "solar position calculation failed",
            Self::ResultTooLarge => "daylight result exceeds the production resource cap",
            Self::MismatchedInputs => "daylight comparison arrays must be non-empty and aligned",
            Self::Cancelled => "daylight analysis was cancelled",
        })
    }
}

impl std::error::Error for DaylightError {}

#[allow(clippy::too_many_arguments)]
fn point_entry(
    scene: &Scene,
    sensor: DaylightSensor,
    sensor_index: usize,
    matrix: &DaylightCoefficientMatrix,
    materials: &MaterialLibrary,
    moment: DaylightMoment,
    sky_model: DaylightSkyModel,
    options: DaylightMatrixOptions,
) -> Result<PointIlluminanceEntry, DaylightError> {
    let diffuse_lux = diffuse_illuminance(
        sensor_index,
        matrix,
        moment.diffuse_horizontal_illuminance_lux,
        sky_model,
    );
    let incidence = sensor.normal.dot(moment.sun_direction).max(0.0);
    let direct_transmission = if incidence <= 0.0 || moment.sun_direction.z <= 0.0 {
        0.0
    } else {
        let ray = QueryRay::try_new(
            sensor.position + sensor.normal * options.sensor_offset_meters,
            moment.sun_direction,
            0.0,
            options.maximum_distance_meters,
            options.category_mask,
        )
        .map_err(|_| DaylightError::InvalidQuery)?;
        scene
            .trace_transmission(
                ray,
                materials,
                TransmissionChannel::Visible,
                options.maximum_material_layers,
                options.minimum_transmission,
            )
            .transmission
    };
    let direct_lux = moment.direct_normal_illuminance_lux * incidence * direct_transmission;
    Ok(PointIlluminanceEntry {
        sensor_id: sensor.id,
        direct_lux,
        diffuse_lux,
        total_lux: direct_lux + diffuse_lux,
        direct_transmission,
    })
}

fn diffuse_illuminance(
    sensor_index: usize,
    matrix: &DaylightCoefficientMatrix,
    horizontal_lux: f64,
    model: DaylightSkyModel,
) -> f64 {
    let row = &matrix.coefficients
        [sensor_index * matrix.patch_count..(sensor_index + 1) * matrix.patch_count];
    let weighted = row
        .iter()
        .zip(&matrix.directions)
        .map(|(coefficient, direction)| coefficient * relative_sky_luminance(model, *direction))
        .sum::<f64>();
    let normalizer = match model {
        DaylightSkyModel::Isotropic => matrix.isotropic_horizontal_normalizer,
        DaylightSkyModel::CieOvercast => matrix.overcast_horizontal_normalizer,
    };
    horizontal_lux * weighted / normalizer
}

fn summarize_annual_sensor(
    sensor: DaylightSensor,
    records: &[EpwRecord],
    row: &[AnnualDaylightTimelineEntry],
    options: AnnualDaylightOptions,
) -> AnnualDaylightSensorSummary {
    let mut occupied_hours = 0.0;
    let mut sda_hours = 0.0;
    let mut ase_hours = 0.0;
    let mut below = 0.0;
    let mut supplemental = 0.0;
    let mut useful = 0.0;
    let mut exceeded = 0.0;
    let mut weighted_lux = 0.0;
    let mut minimum = f64::INFINITY;
    let mut maximum = 0.0_f64;
    for (record, entry) in records.iter().zip(row) {
        if !entry.occupied {
            continue;
        }
        let hours = record.duration_hours;
        occupied_hours += hours;
        weighted_lux = entry.total_lux.mul_add(hours, weighted_lux);
        minimum = minimum.min(entry.total_lux);
        maximum = maximum.max(entry.total_lux);
        if entry.total_lux >= options.sda_threshold_lux {
            sda_hours += hours;
        }
        if entry.direct_lux > options.ase_threshold_lux {
            ase_hours += hours;
        }
        if entry.total_lux < options.udi_lower_lux {
            below += hours;
        } else if entry.total_lux < options.udi_preferred_lux {
            supplemental += hours;
        } else if entry.total_lux <= options.udi_upper_lux {
            useful += hours;
        } else {
            exceeded += hours;
        }
    }
    let ratio = |value| safe_ratio(value, occupied_hours);
    AnnualDaylightSensorSummary {
        sensor_id: sensor.id,
        occupied_hours,
        sda_qualified_hours: sda_hours,
        sda_occupied_fraction: ratio(sda_hours),
        ase_exceedance_hours: ase_hours,
        ase_fails: ase_hours > options.ase_maximum_hours,
        udi_below_fraction: ratio(below),
        udi_supplemental_fraction: ratio(supplemental),
        udi_useful_fraction: ratio(useful),
        udi_exceeded_fraction: ratio(exceeded),
        mean_occupied_lux: safe_ratio(weighted_lux, occupied_hours),
        minimum_occupied_lux: if occupied_hours > 0.0 { minimum } else { 0.0 },
        maximum_occupied_lux: maximum,
    }
}

fn summarize_project(
    sensors: &[DaylightSensor],
    summaries: &[AnnualDaylightSensorSummary],
    options: AnnualDaylightOptions,
) -> AnnualDaylightProjectSummary {
    let total_area = sensors
        .iter()
        .map(|sensor| sensor.area_square_meters)
        .sum::<f64>();
    let mut sda_area = 0.0;
    let mut ase_area = 0.0;
    let mut below = 0.0;
    let mut supplemental = 0.0;
    let mut useful = 0.0;
    let mut exceeded = 0.0;
    for (sensor, summary) in sensors.iter().zip(summaries) {
        let area = sensor.area_square_meters;
        if summary.sda_occupied_fraction >= options.sda_required_fraction {
            sda_area += area;
        }
        if summary.ase_fails {
            ase_area += area;
        }
        below = (summary.udi_below_fraction * area).mul_add(1.0, below);
        supplemental = (summary.udi_supplemental_fraction * area).mul_add(1.0, supplemental);
        useful = (summary.udi_useful_fraction * area).mul_add(1.0, useful);
        exceeded = (summary.udi_exceeded_fraction * area).mul_add(1.0, exceeded);
    }
    AnnualDaylightProjectSummary {
        sda_area_percent: 100.0 * safe_ratio(sda_area, total_area),
        ase_area_percent: 100.0 * safe_ratio(ase_area, total_area),
        udi_below_percent: 100.0 * safe_ratio(below, total_area),
        udi_supplemental_percent: 100.0 * safe_ratio(supplemental, total_area),
        udi_useful_percent: 100.0 * safe_ratio(useful, total_area),
        udi_exceeded_percent: 100.0 * safe_ratio(exceeded, total_area),
        total_sensor_area_square_meters: total_area,
    }
}

fn fibonacci_upper_hemisphere(count: usize) -> Vec<Vec3> {
    (0..count)
        .map(|index| {
            let z = (usize_to_f64(index) + 0.5) / usize_to_f64(count);
            let radius = (1.0 - z * z).sqrt();
            let angle = usize_to_f64(index) * GOLDEN_ANGLE;
            Vec3::new(radius * angle.cos(), radius * angle.sin(), z)
        })
        .collect()
}

const fn relative_sky_luminance(model: DaylightSkyModel, direction: Vec3) -> f64 {
    match model {
        DaylightSkyModel::Isotropic => 1.0,
        DaylightSkyModel::CieOvercast => 1.0 + 2.0 * direction.z,
    }
}

fn is_occupied(record: &EpwRecord, options: AnnualDaylightOptions) -> bool {
    let end_hour = f64::from(record.hour - 1) + f64::from(record.minute) / 60.0;
    let midpoint = (end_hour - record.duration_hours / 2.0).rem_euclid(24.0);
    if options.occupied_start_hour < options.occupied_end_hour {
        midpoint >= options.occupied_start_hour && midpoint < options.occupied_end_hour
    } else {
        midpoint >= options.occupied_start_hour || midpoint < options.occupied_end_hour
    }
}

fn matrix_key(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    options: DaylightMatrixOptions,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_DAYLIGHT_MATRIX_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&materials.content_hash());
    hash_matrix_options(&mut hasher, options);
    for sensor in sensors {
        hasher.update(&sensor.id.get().to_le_bytes());
        for value in [
            sensor.position.x,
            sensor.position.y,
            sensor.position.z,
            sensor.normal.x,
            sensor.normal.y,
            sensor.normal.z,
            sensor.area_square_meters,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

#[allow(clippy::too_many_arguments)]
fn point_hash(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
    moment: DaylightMoment,
    sky_model: DaylightSkyModel,
    options: DaylightMatrixOptions,
    entries: &[PointIlluminanceEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_POINT_ILLUMINANCE_V1\0");
    hasher.update(&matrix_key(scene, sensors, materials, options));
    hasher.update(&(sky_model as u32).to_le_bytes());
    hasher.update(&moment.unix_seconds_utc.to_le_bytes());
    for value in [
        moment.sun_direction.x,
        moment.sun_direction.y,
        moment.sun_direction.z,
        moment.direct_normal_illuminance_lux,
        moment.diffuse_horizontal_illuminance_lux,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for entry in entries {
        hasher.update(&entry.sensor_id.get().to_le_bytes());
        for value in [
            entry.direct_lux,
            entry.diffuse_lux,
            entry.total_lux,
            entry.direct_transmission,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

#[allow(clippy::too_many_arguments)]
fn annual_hash(
    scene: &Scene,
    sensors: &[DaylightSensor],
    weather: &EpwWeather,
    materials: &OpticalMaterialLibrary,
    options: AnnualDaylightOptions,
    summaries: &[AnnualDaylightSensorSummary],
    project: AnnualDaylightProjectSummary,
    matrix_hash: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_ANNUAL_DAYLIGHT_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&weather.content_hash);
    hasher.update(&materials.content_hash());
    hasher.update(&matrix_hash);
    hasher.update(&(options.sky_model as u32).to_le_bytes());
    for value in [
        options.direct_luminous_efficacy_lm_per_w,
        options.diffuse_luminous_efficacy_lm_per_w,
        options.occupied_start_hour,
        options.occupied_end_hour,
        options.sda_threshold_lux,
        options.sda_required_fraction,
        options.ase_threshold_lux,
        options.ase_maximum_hours,
        options.udi_lower_lux,
        options.udi_preferred_lux,
        options.udi_upper_lux,
        project.sda_area_percent,
        project.ase_area_percent,
        project.udi_below_percent,
        project.udi_supplemental_percent,
        project.udi_useful_percent,
        project.udi_exceeded_percent,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for sensor in sensors {
        hasher.update(&sensor.id.get().to_le_bytes());
    }
    for summary in summaries {
        hasher.update(&summary.sensor_id.get().to_le_bytes());
        hasher.update(&summary.sda_occupied_fraction.to_bits().to_le_bytes());
        hasher.update(&summary.ase_exceedance_hours.to_bits().to_le_bytes());
        hasher.update(&summary.mean_occupied_lux.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_matrix_options(hasher: &mut blake3::Hasher, options: DaylightMatrixOptions) {
    hasher.update(
        &u64::try_from(options.sky_patch_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&options.sensor_offset_meters.to_bits().to_le_bytes());
    hasher.update(&options.maximum_distance_meters.to_bits().to_le_bytes());
    hasher.update(&options.category_mask.to_le_bytes());
    hasher.update(
        &u64::try_from(options.maximum_material_layers)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&options.minimum_transmission.to_bits().to_le_bytes());
}

fn matrix_cache() -> &'static Mutex<VecDeque<Arc<DaylightCoefficientMatrix>>> {
    MATRIX_CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn check_cancelled(cancelled: Option<&AtomicBool>) -> Result<(), DaylightError> {
    if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        Err(DaylightError::Cancelled)
    } else {
        Ok(())
    }
}

const fn photopic(rgb: [f64; 3]) -> f64 {
    PHOTOPIC_RED * rgb[0] + PHOTOPIC_GREEN * rgb[1] + PHOTOPIC_BLUE * rgb[2]
}

const fn is_fraction(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

const fn hour(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value < 24.0
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

fn usize_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("bounded daylight count fits u32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::MeshId;
    use xvarna_zurvan::parse_epw;

    fn roof_scene() -> Scene {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(-1_000.0, -1_000.0, 1.0),
                Vec3::new(1_000.0, -1_000.0, 1.0),
                Vec3::new(1_000.0, 1_000.0, 1.0),
                Vec3::new(-1_000.0, 1_000.0, 1.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("builder");
        let mesh_id = builder.add_mesh(mesh).expect("mesh");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(9),
                xvarna_types::InstanceId::new(1),
                1,
            )
            .expect("instance");
        builder.build().expect("scene")
    }

    fn open_scene() -> Scene {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(100.0, 100.0, -1.0),
                Vec3::new(101.0, 100.0, -1.0),
                Vec3::new(100.0, 101.0, -1.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("builder");
        let mesh_id = builder.add_mesh(mesh).expect("mesh");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(1),
                xvarna_types::InstanceId::new(1),
                1,
            )
            .expect("instance");
        builder.build().expect("scene")
    }

    #[test]
    fn open_sky_recovers_horizontal_illuminance_and_reuses_matrix() {
        clear_daylight_matrix_cache();
        let scene = open_scene();
        let sensor =
            DaylightSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 1.0)
                .expect("sensor");
        let moment = DaylightMoment::try_new(0, Vec3::new(0.0, 0.0, 1.0), 50_000.0, 10_000.0)
            .expect("moment");
        let first = analyze_point_illuminance(
            &scene,
            &[sensor],
            &OpticalMaterialLibrary::default(),
            moment,
            DaylightSkyModel::Isotropic,
            DaylightMatrixOptions {
                sky_patch_count: 256,
                ..DaylightMatrixOptions::default()
            },
        )
        .expect("analysis");
        assert!((first.entries[0].diffuse_lux - 10_000.0).abs() < 1.0e-8);
        assert!((first.entries[0].direct_lux - 50_000.0).abs() < 1.0e-8);
        assert!(!first.matrix_reused);
        let second = analyze_point_illuminance(
            &scene,
            &[sensor],
            &OpticalMaterialLibrary::default(),
            moment,
            DaylightSkyModel::Isotropic,
            DaylightMatrixOptions {
                sky_patch_count: 256,
                ..DaylightMatrixOptions::default()
            },
        )
        .expect("analysis");
        assert!(second.matrix_reused);
    }

    #[test]
    fn glazing_transmission_and_overcast_daylight_factor_are_explicit() {
        clear_daylight_matrix_cache();
        let scene = roof_scene();
        let glass = OpticalMaterial::try_new(
            1,
            OpticalMaterialKind::Glass,
            [0.1; 3],
            [0.5; 3],
            0.0,
            0.0,
            1.52,
        )
        .expect("glass");
        let materials = OpticalMaterialLibrary::try_new([glass], [(ObjectId::new(9), glass.id)])
            .expect("materials");
        let sensor =
            DaylightSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 1.0)
                .expect("sensor");
        let factor = analyze_daylight_factor(
            &scene,
            &[sensor],
            &materials,
            10_000.0,
            DaylightMatrixOptions {
                sky_patch_count: 256,
                ..DaylightMatrixOptions::default()
            },
        )
        .expect("factor");
        assert!((factor.entries[0].daylight_factor_percent - 50.0).abs() < 1.0e-8);
    }

    #[test]
    fn annual_metrics_and_validation_statistics_are_deterministic() {
        clear_daylight_matrix_cache();
        let weather = parse_epw(concat!(
            "LOCATION,Test,State,Country,Source,1,0,0,0,0\n",
            "DESIGN CONDITIONS,0\n",
            "TYPICAL/EXTREME PERIODS,0\n",
            "GROUND TEMPERATURES,0\n",
            "HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0\n",
            "COMMENTS 1,test\n",
            "COMMENTS 2,test\n",
            "DATA PERIODS,1,1,Data,Monday,1/1,12/31\n",
            "2024,3,20,12,60,A,20,0,0,101325,0,0,333,500,0,100,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.2,0,0\n",
            "2024,3,20,13,60,A,20,0,0,101325,0,0,333,500,0,100,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.2,0,0\n",
        ))
        .expect("weather");
        let scene = open_scene();
        let sensor =
            DaylightSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 2.0)
                .expect("sensor");
        let result = analyze_annual_daylight(
            &scene,
            &[sensor],
            &weather,
            &OpticalMaterialLibrary::default(),
            AnnualDaylightOptions {
                matrix: DaylightMatrixOptions {
                    sky_patch_count: 128,
                    ..DaylightMatrixOptions::default()
                },
                ..AnnualDaylightOptions::default()
            },
        )
        .expect("annual");
        assert_eq!(result.timeline.len(), 2);
        assert!((result.project.sda_area_percent - 100.0).abs() < 1.0e-12);
        let report = compare_daylight_results(&[100.0, 200.0], &[110.0, 190.0], 10.0, 0.1)
            .expect("comparison");
        assert!(report.accepted);
        assert!((report.mean_absolute_error_lux - 10.0).abs() < 1.0e-12);
    }

    #[test]
    fn optical_material_rejects_energy_creation() {
        let result = OpticalMaterial::try_new(
            1,
            OpticalMaterialKind::Glass,
            [0.7; 3],
            [0.5; 3],
            0.0,
            0.0,
            1.52,
        );
        assert_eq!(result, Err(DaylightError::InvalidMaterial));
        let _ = MeshId::new(1);
    }
}
