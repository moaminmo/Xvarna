//! Material-aware solar scenarios and buildable solar/shading envelope screening.

use std::collections::BTreeMap;
use xvarna_geometry::Vec3;
use xvarna_scene::{MaterialLibrary, QueryRay, Scene, TransmissionChannel};
use xvarna_types::{ObjectId, SensorId};
use xvarna_zurvan::SunSet;

use crate::SolarSensor;

const MAXIMUM_RESULT_ENTRIES: usize = 32_000_000;

/// Owned optical design alternative evaluated against one scene and sun set.
#[derive(Clone, Debug, PartialEq)]
pub struct SolarScenario {
    /// Stable caller-defined scenario identifier.
    pub id: u64,
    /// Human-readable non-empty scenario name.
    pub name: String,
    /// Validated object-to-material catalog. Unassigned objects are opaque.
    pub materials: MaterialLibrary,
}

impl SolarScenario {
    /// Creates a named scenario.
    pub fn try_new(
        id: u64,
        name: impl Into<String>,
        materials: MaterialLibrary,
    ) -> Result<Self, SolarIntelligenceError> {
        let name = name.into();
        if name.trim().is_empty() || name.len() > 256 {
            return Err(SolarIntelligenceError::InvalidScenario);
        }
        Ok(Self {
            id,
            name,
            materials,
        })
    }
}

/// Shared policy for material-aware solar scenario comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarScenarioOptions {
    /// Offset along the sensor normal in canonical metres.
    pub sensor_offset_meters: f64,
    /// Maximum scene-query distance; positive infinity is supported.
    pub maximum_distance_meters: f64,
    /// Minimum normal-to-sun cosine for an eligible interval.
    pub minimum_incidence_cosine: f64,
    /// Included scene categories.
    pub category_mask: u64,
    /// Maximum transparent geometric layers per solar ray.
    pub maximum_material_layers: usize,
    /// Throughput below this threshold is treated as zero.
    pub minimum_transmission: f64,
    /// Maximum ranked blocking objects retained per scenario/sensor.
    pub top_k: usize,
}

impl Default for SolarScenarioOptions {
    fn default() -> Self {
        Self {
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            minimum_incidence_cosine: 0.0,
            category_mask: u64::MAX,
            maximum_material_layers: 8,
            minimum_transmission: 1.0e-4,
            top_k: 10,
        }
    }
}

impl SolarScenarioOptions {
    fn validate(self) -> Result<Self, SolarIntelligenceError> {
        if !self.sensor_offset_meters.is_finite() || self.sensor_offset_meters < 0.0 {
            return Err(SolarIntelligenceError::InvalidPolicy);
        }
        if self.maximum_distance_meters.is_nan() || self.maximum_distance_meters <= 0.0 {
            return Err(SolarIntelligenceError::InvalidPolicy);
        }
        if !self.minimum_incidence_cosine.is_finite()
            || !(-1.0..=1.0).contains(&self.minimum_incidence_cosine)
            || !(1..=64).contains(&self.maximum_material_layers)
            || !self.minimum_transmission.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_transmission)
            || !(1..=1_024).contains(&self.top_k)
        {
            return Err(SolarIntelligenceError::InvalidPolicy);
        }
        Ok(self)
    }
}

/// Classification of one material-aware scenario/sensor/time entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SolarScenarioState {
    /// Sun interval is inactive.
    Inactive = 0,
    /// Sun is behind the oriented sensor.
    BackFacing = 1,
    /// Interval is eligible and transmission was evaluated.
    Evaluated = 2,
}

/// Full optical state for one scenario/sensor/time cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarScenarioTimelineEntry {
    /// Eligibility state.
    pub state: SolarScenarioState,
    /// Remaining direct solar transmission, in zero through one.
    pub transmission: f64,
    /// First interacting object, or zero for open sky.
    pub first_object_id: ObjectId,
    /// Number of material/geometric interactions traced.
    pub layer_count: usize,
    /// True when the material-layer safety limit was reached.
    pub layer_limit_reached: bool,
}

/// One aggregate for a scenario/sensor pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarScenarioSummary {
    /// Source scenario identifier.
    pub scenario_id: u64,
    /// Source sensor identifier.
    pub sensor_id: SensorId,
    /// Transmission-weighted direct-sun hours.
    pub received_sun_hours: f64,
    /// Eligible hours removed by material and geometry.
    pub lost_sun_hours: f64,
    /// Total eligible scheduled hours.
    pub eligible_sun_hours: f64,
    /// Received divided by eligible hours.
    pub solar_access_ratio: f64,
    /// Object receiving the largest attributed loss.
    pub dominant_occluder_object_id: ObjectId,
    /// Hours attributed to the dominant object.
    pub dominant_occluder_hours: f64,
}

/// Ranked object loss for one scenario/sensor scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarAttributionEntry {
    /// Source scenario identifier.
    pub scenario_id: u64,
    /// Source sensor identifier.
    pub sensor_id: SensorId,
    /// One-based rank.
    pub rank: usize,
    /// Source object.
    pub object_id: ObjectId,
    /// Union of occurrence categories associated with the object.
    pub category_mask: u64,
    /// Attributed lost hours.
    pub lost_sun_hours: f64,
    /// Share of all attributed loss for the scope.
    pub fraction: f64,
}

/// Exact, non-overlapping category-combination loss partition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarCategoryBreakdown {
    /// Source scenario identifier.
    pub scenario_id: u64,
    /// Source sensor identifier.
    pub sensor_id: SensorId,
    /// Exact category mask combination.
    pub category_mask: u64,
    /// Attributed lost hours.
    pub lost_sun_hours: f64,
    /// Share of all attributed loss for the scope.
    pub fraction: f64,
}

/// Delta from the first (baseline) scenario for one sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarScenarioDelta {
    /// Compared scenario identifier.
    pub scenario_id: u64,
    /// Source sensor identifier.
    pub sensor_id: SensorId,
    /// Compared minus baseline received hours.
    pub received_sun_hours_delta: f64,
    /// Compared minus baseline lost hours.
    pub lost_sun_hours_delta: f64,
    /// Compared minus baseline access ratio.
    pub solar_access_ratio_delta: f64,
}

/// Scenario-major material-aware solar result.
#[derive(Clone, Debug, PartialEq)]
pub struct SolarScenarioComparison {
    /// One row per scenario/sensor pair.
    pub summaries: Vec<SolarScenarioSummary>,
    /// Scenario-major, then sensor-major, then time-major optical cells.
    pub timeline: Vec<SolarScenarioTimelineEntry>,
    /// Ranked object loss attribution.
    pub attribution: Vec<SolarAttributionEntry>,
    /// Exact category partitions.
    pub categories: Vec<SolarCategoryBreakdown>,
    /// Deltas for every non-baseline scenario/sensor pair.
    pub deltas: Vec<SolarScenarioDelta>,
    /// Number of sensors in each scenario row block.
    pub sensor_count: usize,
    /// Number of sun samples in each sensor row.
    pub sun_count: usize,
    /// Stable scene, sun, material, input, policy, and result identity.
    pub content_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Default)]
struct SolarLossAccumulator {
    hours: f64,
    category_mask: u64,
}

/// Compares multiple material assignments over the same geometry and sun schedule.
#[allow(clippy::too_many_lines)]
pub fn compare_solar_scenarios(
    scene: &Scene,
    sensors: &[SolarSensor],
    sun_set: &SunSet,
    scenarios: &[SolarScenario],
    options: SolarScenarioOptions,
) -> Result<SolarScenarioComparison, SolarIntelligenceError> {
    if sensors.is_empty() {
        return Err(SolarIntelligenceError::EmptySensors);
    }
    if sun_set.samples.is_empty() {
        return Err(SolarIntelligenceError::EmptySunSet);
    }
    if scenarios.is_empty() {
        return Err(SolarIntelligenceError::EmptyScenarios);
    }
    let options = options.validate()?;
    let cell_count = scenarios
        .len()
        .checked_mul(sensors.len())
        .and_then(|value| value.checked_mul(sun_set.samples.len()))
        .filter(|value| *value <= MAXIMUM_RESULT_ENTRIES)
        .ok_or(SolarIntelligenceError::ResultTooLarge)?;
    let mut timeline = Vec::with_capacity(cell_count);
    let mut summaries = Vec::with_capacity(scenarios.len() * sensors.len());
    let mut attribution = Vec::new();
    let mut categories = Vec::new();

    for scenario in scenarios {
        for sensor in sensors {
            let mut received = 0.0;
            let mut eligible = 0.0;
            let mut object_loss = BTreeMap::<ObjectId, SolarLossAccumulator>::new();
            let mut category_loss = BTreeMap::<u64, f64>::new();
            for sun in &sun_set.samples {
                if !sun.is_active {
                    timeline.push(SolarScenarioTimelineEntry {
                        state: SolarScenarioState::Inactive,
                        transmission: 0.0,
                        first_object_id: ObjectId::new(0),
                        layer_count: 0,
                        layer_limit_reached: false,
                    });
                    continue;
                }
                if sensor.normal.dot(sun.direction) <= options.minimum_incidence_cosine {
                    timeline.push(SolarScenarioTimelineEntry {
                        state: SolarScenarioState::BackFacing,
                        transmission: 0.0,
                        first_object_id: ObjectId::new(0),
                        layer_count: 0,
                        layer_limit_reached: false,
                    });
                    continue;
                }
                let contribution = sun.duration_hours * sun.weight;
                eligible += contribution;
                let query = QueryRay::try_new(
                    sensor.position + sensor.normal * options.sensor_offset_meters,
                    sun.direction,
                    0.0,
                    options.maximum_distance_meters,
                    options.category_mask,
                )
                .map_err(|_| SolarIntelligenceError::InvalidQuery)?;
                let trace = scene.trace_transmission(
                    query,
                    &scenario.materials,
                    TransmissionChannel::Solar,
                    options.maximum_material_layers,
                    options.minimum_transmission,
                );
                received = contribution.mul_add(trace.transmission, received);
                for layer in &trace.layers {
                    let lost = contribution * layer.attributed_loss;
                    let category = scene
                        .instance_category_mask(layer.hit.instance_id)
                        .unwrap_or(0);
                    let accumulator = object_loss.entry(layer.hit.object_id).or_default();
                    accumulator.hours += lost;
                    accumulator.category_mask |= category;
                    *category_loss.entry(category).or_default() += lost;
                }
                timeline.push(SolarScenarioTimelineEntry {
                    state: SolarScenarioState::Evaluated,
                    transmission: trace.transmission,
                    first_object_id: trace
                        .layers
                        .first()
                        .map_or(ObjectId::new(0), |layer| layer.hit.object_id),
                    layer_count: trace.layers.len(),
                    layer_limit_reached: trace.layer_limit_reached,
                });
            }
            let lost = (eligible - received).max(0.0);
            let dominant = object_loss
                .iter()
                .max_by(|left, right| {
                    left.1
                        .hours
                        .total_cmp(&right.1.hours)
                        .then_with(|| right.0.cmp(left.0))
                })
                .map_or((ObjectId::new(0), 0.0), |(object, value)| {
                    (*object, value.hours)
                });
            summaries.push(SolarScenarioSummary {
                scenario_id: scenario.id,
                sensor_id: sensor.id,
                received_sun_hours: received,
                lost_sun_hours: lost,
                eligible_sun_hours: eligible,
                solar_access_ratio: safe_ratio(received, eligible),
                dominant_occluder_object_id: dominant.0,
                dominant_occluder_hours: dominant.1,
            });

            let mut ranked = object_loss.into_iter().collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                right
                    .1
                    .hours
                    .total_cmp(&left.1.hours)
                    .then_with(|| left.0.cmp(&right.0))
            });
            attribution.extend(ranked.into_iter().take(options.top_k).enumerate().map(
                |(index, (object_id, value))| SolarAttributionEntry {
                    scenario_id: scenario.id,
                    sensor_id: sensor.id,
                    rank: index + 1,
                    object_id,
                    category_mask: value.category_mask,
                    lost_sun_hours: value.hours,
                    fraction: safe_ratio(value.hours, lost),
                },
            ));
            categories.extend(category_loss.into_iter().map(|(category_mask, hours)| {
                SolarCategoryBreakdown {
                    scenario_id: scenario.id,
                    sensor_id: sensor.id,
                    category_mask,
                    lost_sun_hours: hours,
                    fraction: safe_ratio(hours, lost),
                }
            }));
        }
    }

    let baseline = &summaries[..sensors.len()];
    let mut deltas = Vec::with_capacity(sensors.len() * scenarios.len().saturating_sub(1));
    for scenario_index in 1..scenarios.len() {
        for sensor_index in 0..sensors.len() {
            let compared = summaries[scenario_index * sensors.len() + sensor_index];
            let base = baseline[sensor_index];
            deltas.push(SolarScenarioDelta {
                scenario_id: scenarios[scenario_index].id,
                sensor_id: compared.sensor_id,
                received_sun_hours_delta: compared.received_sun_hours - base.received_sun_hours,
                lost_sun_hours_delta: compared.lost_sun_hours - base.lost_sun_hours,
                solar_access_ratio_delta: compared.solar_access_ratio - base.solar_access_ratio,
            });
        }
    }
    let content_hash = solar_scenario_hash(
        scene, sensors, sun_set, scenarios, options, &summaries, &timeline,
    );
    Ok(SolarScenarioComparison {
        summaries,
        timeline,
        attribution,
        categories,
        deltas,
        sensor_count: sensors.len(),
        sun_count: sun_set.samples.len(),
        content_hash,
    })
}

/// Candidate oriented column for freeform solar-envelope screening.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeCandidate {
    /// Stable candidate identifier.
    pub id: u64,
    /// Candidate base point in canonical metres.
    pub position: Vec3,
    /// Unit vector defining the positive build direction.
    pub axis: Vec3,
}

impl EnvelopeCandidate {
    /// Creates a finite vertical candidate for backwards-compatible workflows.
    pub const fn try_new(id: u64, position: Vec3) -> Result<Self, SolarIntelligenceError> {
        if !position.is_finite() {
            return Err(SolarIntelligenceError::InvalidCandidate);
        }
        Ok(Self {
            id,
            position,
            axis: Vec3::new(0.0, 0.0, 1.0),
        })
    }

    /// Creates a finite candidate along an arbitrary non-zero axis.
    pub fn try_new_oriented(
        id: u64,
        position: Vec3,
        axis: Vec3,
    ) -> Result<Self, SolarIntelligenceError> {
        if !position.is_finite() {
            return Err(SolarIntelligenceError::InvalidCandidate);
        }
        let axis = axis
            .normalized()
            .ok_or(SolarIntelligenceError::InvalidCandidate)?;
        Ok(Self { id, position, axis })
    }
}

/// Solar-access and shading-envelope policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarEnvelopeOptions {
    /// Radius of the candidate column around its oriented axis in metres.
    pub candidate_radius_meters: f64,
    /// Fraction of baseline transmitted solar hours that must remain available.
    pub required_preserved_fraction: f64,
    /// Fraction of baseline transmitted solar hours a shading device should intercept.
    pub target_shaded_fraction: f64,
    /// Safety clearance along the candidate axis, subtracted/added to limiting distances.
    pub vertical_clearance_meters: f64,
    /// Offset along protected sensor normals.
    pub sensor_offset_meters: f64,
    /// Maximum context query distance.
    pub maximum_distance_meters: f64,
    /// Included context categories.
    pub category_mask: u64,
    /// Maximum transparent context layers.
    pub maximum_material_layers: usize,
    /// Minimum retained context throughput.
    pub minimum_transmission: f64,
}

impl Default for SolarEnvelopeOptions {
    fn default() -> Self {
        Self {
            candidate_radius_meters: 0.5,
            required_preserved_fraction: 0.8,
            target_shaded_fraction: 0.5,
            vertical_clearance_meters: 0.05,
            sensor_offset_meters: 1.0e-4,
            maximum_distance_meters: f64::INFINITY,
            category_mask: u64::MAX,
            maximum_material_layers: 8,
            minimum_transmission: 1.0e-4,
        }
    }
}

impl SolarEnvelopeOptions {
    fn validate(self) -> Result<Self, SolarIntelligenceError> {
        if !self.candidate_radius_meters.is_finite()
            || self.candidate_radius_meters <= 0.0
            || !is_fraction(self.required_preserved_fraction)
            || !is_fraction(self.target_shaded_fraction)
            || !self.vertical_clearance_meters.is_finite()
            || self.vertical_clearance_meters < 0.0
            || !self.sensor_offset_meters.is_finite()
            || self.sensor_offset_meters < 0.0
            || self.maximum_distance_meters.is_nan()
            || self.maximum_distance_meters <= 0.0
            || !(1..=64).contains(&self.maximum_material_layers)
            || !is_fraction(self.minimum_transmission)
        {
            return Err(SolarIntelligenceError::InvalidPolicy);
        }
        Ok(self)
    }
}

/// Limiting solar ray controlling one envelope elevation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeControl {
    /// Protected sensor controlling the threshold.
    pub sensor_id: SensorId,
    /// UTC timestamp of the controlling sun interval.
    pub unix_seconds_utc: i64,
    /// Solar ray elevation at the candidate XY position.
    pub ray_elevation_meters: f64,
    /// Baseline transmission-weighted interval hours.
    pub weighted_hours: f64,
}

impl EnvelopeControl {
    const fn none() -> Self {
        Self {
            sensor_id: SensorId::new(0),
            unix_seconds_utc: 0,
            ray_elevation_meters: f64::NAN,
            weighted_hours: 0.0,
        }
    }
}

/// Solar access and shading thresholds at one oriented-column candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarEnvelopeCell {
    /// Candidate identifier.
    pub candidate_id: u64,
    /// Candidate base point.
    pub position: Vec3,
    /// Highest absolute elevation preserving the required access fraction.
    pub maximum_solar_access_elevation_meters: f64,
    /// Distance along the positive candidate axis for the access envelope.
    pub maximum_solar_access_height_meters: f64,
    /// Lowest absolute elevation intercepting the target shaded fraction.
    pub minimum_shading_elevation_meters: f64,
    /// Distance along the positive candidate axis for the shading envelope.
    pub minimum_shading_height_meters: f64,
    /// Baseline transmitted hours whose projected rays cross the candidate column.
    pub considered_baseline_sun_hours: f64,
    /// Number of projected sensor/time constraints.
    pub constraint_count: usize,
    /// Access-envelope limiting ray.
    pub access_control: EnvelopeControl,
    /// Shading-envelope limiting ray.
    pub shading_control: EnvelopeControl,
}

/// Complete material-aware, freeform oriented-column solar/shading envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct SolarEnvelopeResult {
    /// One cell per source candidate.
    pub cells: Vec<SolarEnvelopeCell>,
    /// Number of protected sensors.
    pub sensor_count: usize,
    /// Number of temporal sun samples.
    pub sun_count: usize,
    /// Stable provenance of all scene, material, solar, and policy inputs and cells.
    pub content_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
struct ProjectedConstraint {
    axis_distance: f64,
    elevation: f64,
    weight: f64,
    sensor_id: SensorId,
    unix_seconds_utc: i64,
}

/// Screens a freeform solar-access envelope and a shading envelope.
///
/// Each candidate is modeled as a circular column along its own normalized axis. The closest
/// point between a protected solar ray and that axis produces a signed axis constraint; only
/// constraints in the positive build direction are retained. Vertical candidates retain the
/// original absolute-elevation semantics. The method is deterministic and suited to early
/// massing constraints; it is not a replacement for a final code-compliance construction
/// envelope or full radiosity simulation.
#[allow(clippy::too_many_lines)]
pub fn analyze_solar_envelope(
    scene: &Scene,
    protected_sensors: &[SolarSensor],
    candidates: &[EnvelopeCandidate],
    sun_set: &SunSet,
    materials: &MaterialLibrary,
    options: SolarEnvelopeOptions,
) -> Result<SolarEnvelopeResult, SolarIntelligenceError> {
    if protected_sensors.is_empty() {
        return Err(SolarIntelligenceError::EmptySensors);
    }
    if candidates.is_empty() {
        return Err(SolarIntelligenceError::EmptyCandidates);
    }
    if sun_set.samples.is_empty() {
        return Err(SolarIntelligenceError::EmptySunSet);
    }
    let options = options.validate()?;
    protected_sensors
        .len()
        .checked_mul(candidates.len())
        .and_then(|value| value.checked_mul(sun_set.samples.len()))
        .filter(|value| *value <= MAXIMUM_RESULT_ENTRIES)
        .ok_or(SolarIntelligenceError::ResultTooLarge)?;

    let mut cells = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut constraints = Vec::<ProjectedConstraint>::new();
        for sensor in protected_sensors {
            for sun in &sun_set.samples {
                if !sun.is_active || sensor.normal.dot(sun.direction) <= 0.0 {
                    continue;
                }
                let direction_axis_dot = sun.direction.dot(candidate.axis);
                let denominator = direction_axis_dot.mul_add(-direction_axis_dot, 1.0);
                if denominator <= 1.0e-14 {
                    continue;
                }
                let sensor_from_candidate = sensor.position - candidate.position;
                let direction_offset_dot = sun.direction.dot(sensor_from_candidate);
                let axis_offset_dot = candidate.axis.dot(sensor_from_candidate);
                let parameter = direction_axis_dot.mul_add(axis_offset_dot, -direction_offset_dot)
                    / denominator;
                if parameter <= 0.0 {
                    continue;
                }
                let axis_distance = direction_axis_dot.mul_add(parameter, axis_offset_dot);
                if axis_distance < 0.0 {
                    continue;
                }
                let lateral = sensor_from_candidate + sun.direction * parameter
                    - candidate.axis * axis_distance;
                let lateral_squared = lateral.dot(lateral);
                if lateral_squared > options.candidate_radius_meters.powi(2) {
                    continue;
                }
                if parameter > options.maximum_distance_meters {
                    continue;
                }
                let query = QueryRay::try_new(
                    sensor.position + sensor.normal * options.sensor_offset_meters,
                    sun.direction,
                    0.0,
                    parameter.max(options.sensor_offset_meters),
                    options.category_mask,
                )
                .map_err(|_| SolarIntelligenceError::InvalidQuery)?;
                let trace = scene.trace_transmission(
                    query,
                    materials,
                    TransmissionChannel::Solar,
                    options.maximum_material_layers,
                    options.minimum_transmission,
                );
                let weight = sun.duration_hours * sun.weight * trace.transmission;
                if weight <= 0.0 {
                    continue;
                }
                constraints.push(ProjectedConstraint {
                    axis_distance,
                    elevation: endpoint_elevation(candidate, axis_distance),
                    weight,
                    sensor_id: sensor.id,
                    unix_seconds_utc: sun.unix_seconds_utc,
                });
            }
        }
        constraints.sort_by(|left, right| {
            left.axis_distance
                .total_cmp(&right.axis_distance)
                .then_with(|| left.sensor_id.cmp(&right.sensor_id))
                .then_with(|| left.unix_seconds_utc.cmp(&right.unix_seconds_utc))
        });
        let total_weight = constraints.iter().map(|value| value.weight).sum::<f64>();
        let access_index = weighted_quantile_index(
            &constraints,
            total_weight * (1.0 - options.required_preserved_fraction),
        );
        let shading_index =
            weighted_quantile_index(&constraints, total_weight * options.target_shaded_fraction);
        let (access_distance, access_control) =
            access_index.map_or((f64::INFINITY, EnvelopeControl::none()), |index| {
                let value = constraints[index];
                (
                    (value.axis_distance - options.vertical_clearance_meters).max(0.0),
                    control(value),
                )
            });
        let (shading_distance, shading_control) =
            shading_index.map_or((f64::INFINITY, EnvelopeControl::none()), |index| {
                let value = constraints[index];
                (
                    (value.axis_distance + options.vertical_clearance_meters).max(0.0),
                    control(value),
                )
            });
        cells.push(SolarEnvelopeCell {
            candidate_id: candidate.id,
            position: candidate.position,
            maximum_solar_access_elevation_meters: endpoint_elevation(candidate, access_distance),
            maximum_solar_access_height_meters: access_distance,
            minimum_shading_elevation_meters: endpoint_elevation(candidate, shading_distance),
            minimum_shading_height_meters: shading_distance,
            considered_baseline_sun_hours: total_weight,
            constraint_count: constraints.len(),
            access_control,
            shading_control,
        });
    }
    let content_hash = envelope_hash(
        scene,
        protected_sensors,
        candidates,
        sun_set,
        materials,
        options,
        &cells,
    );
    Ok(SolarEnvelopeResult {
        cells,
        sensor_count: protected_sensors.len(),
        sun_count: sun_set.samples.len(),
        content_hash,
    })
}

/// Validation or capacity failure in advanced HVARE solar intelligence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolarIntelligenceError {
    /// At least one sensor is required.
    EmptySensors,
    /// At least one sun sample is required.
    EmptySunSet,
    /// At least one scenario is required.
    EmptyScenarios,
    /// At least one envelope candidate is required.
    EmptyCandidates,
    /// Scenario name or identity is invalid.
    InvalidScenario,
    /// Candidate coordinates are non-finite.
    InvalidCandidate,
    /// A numeric or resource policy is invalid.
    InvalidPolicy,
    /// A production query could not be constructed.
    InvalidQuery,
    /// Requested result exceeds production safety limits.
    ResultTooLarge,
}

impl core::fmt::Display for SolarIntelligenceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::EmptySensors => "solar intelligence requires at least one sensor",
            Self::EmptySunSet => "solar intelligence requires at least one sun sample",
            Self::EmptyScenarios => "solar comparison requires at least one scenario",
            Self::EmptyCandidates => "solar envelope requires at least one candidate",
            Self::InvalidScenario => "solar scenario name is empty or too long",
            Self::InvalidCandidate => "solar envelope candidate must be finite",
            Self::InvalidPolicy => "solar intelligence policy is invalid",
            Self::InvalidQuery => "solar intelligence ray construction failed",
            Self::ResultTooLarge => "solar intelligence result exceeds the production safety cap",
        })
    }
}

impl std::error::Error for SolarIntelligenceError {}

const fn control(value: ProjectedConstraint) -> EnvelopeControl {
    EnvelopeControl {
        sensor_id: value.sensor_id,
        unix_seconds_utc: value.unix_seconds_utc,
        ray_elevation_meters: value.elevation,
        weighted_hours: value.weight,
    }
}

fn weighted_quantile_index(
    constraints: &[ProjectedConstraint],
    target_weight: f64,
) -> Option<usize> {
    if constraints.is_empty() {
        return None;
    }
    if target_weight <= 0.0 {
        return Some(0);
    }
    let mut cumulative = 0.0;
    for (index, value) in constraints.iter().enumerate() {
        cumulative += value.weight;
        if cumulative >= target_weight {
            return Some(index);
        }
    }
    Some(constraints.len() - 1)
}

const fn endpoint_elevation(candidate: &EnvelopeCandidate, axis_distance: f64) -> f64 {
    if axis_distance.is_finite() {
        axis_distance.mul_add(candidate.axis.z, candidate.position.z)
    } else {
        axis_distance
    }
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

fn is_fraction(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[allow(clippy::too_many_arguments)]
fn solar_scenario_hash(
    scene: &Scene,
    sensors: &[SolarSensor],
    sun_set: &SunSet,
    scenarios: &[SolarScenario],
    options: SolarScenarioOptions,
    summaries: &[SolarScenarioSummary],
    timeline: &[SolarScenarioTimelineEntry],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_HVARE_SOLAR_SCENARIOS_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&sun_set.content_hash);
    for scenario in scenarios {
        hasher.update(&scenario.id.to_le_bytes());
        hasher.update(&(scenario.name.len() as u64).to_le_bytes());
        hasher.update(scenario.name.as_bytes());
        hasher.update(&scenario.materials.content_hash());
    }
    for sensor in sensors {
        hasher.update(&sensor.id.get().to_le_bytes());
        hash_vec3(&mut hasher, sensor.position);
        hash_vec3(&mut hasher, sensor.normal);
    }
    for value in [
        options.sensor_offset_meters,
        options.maximum_distance_meters,
        options.minimum_incidence_cosine,
        options.minimum_transmission,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for value in [
        options.category_mask,
        options.maximum_material_layers as u64,
        options.top_k as u64,
    ] {
        hasher.update(&value.to_le_bytes());
    }
    for summary in summaries {
        hasher.update(&summary.scenario_id.to_le_bytes());
        hasher.update(&summary.sensor_id.get().to_le_bytes());
        for value in [
            summary.received_sun_hours,
            summary.lost_sun_hours,
            summary.eligible_sun_hours,
            summary.solar_access_ratio,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    for entry in timeline {
        hasher.update(&[entry.state as u8]);
        hasher.update(&entry.transmission.to_bits().to_le_bytes());
        hasher.update(&entry.first_object_id.get().to_le_bytes());
        hasher.update(&(entry.layer_count as u64).to_le_bytes());
        hasher.update(&[u8::from(entry.layer_limit_reached)]);
    }
    *hasher.finalize().as_bytes()
}

#[allow(clippy::too_many_arguments)]
fn envelope_hash(
    scene: &Scene,
    sensors: &[SolarSensor],
    candidates: &[EnvelopeCandidate],
    sun_set: &SunSet,
    materials: &MaterialLibrary,
    options: SolarEnvelopeOptions,
    cells: &[SolarEnvelopeCell],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_HVARE_SOLAR_ENVELOPE_V2\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&sun_set.content_hash);
    hasher.update(&materials.content_hash());
    for sensor in sensors {
        hasher.update(&sensor.id.get().to_le_bytes());
        hash_vec3(&mut hasher, sensor.position);
        hash_vec3(&mut hasher, sensor.normal);
    }
    for candidate in candidates {
        hasher.update(&candidate.id.to_le_bytes());
        hash_vec3(&mut hasher, candidate.position);
        hash_vec3(&mut hasher, candidate.axis);
    }
    for value in [
        options.candidate_radius_meters,
        options.required_preserved_fraction,
        options.target_shaded_fraction,
        options.vertical_clearance_meters,
        options.sensor_offset_meters,
        options.maximum_distance_meters,
        options.minimum_transmission,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&options.category_mask.to_le_bytes());
    hasher.update(&(options.maximum_material_layers as u64).to_le_bytes());
    for cell in cells {
        hasher.update(&cell.candidate_id.to_le_bytes());
        for value in [
            cell.maximum_solar_access_elevation_meters,
            cell.minimum_shading_elevation_meters,
            cell.considered_baseline_sun_hours,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&(cell.constraint_count as u64).to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_vec3(hasher: &mut blake3::Hasher, value: Vec3) {
    for coordinate in [value.x, value.y, value.z] {
        hasher.update(&coordinate.to_bits().to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::Mesh;
    use xvarna_scene::{AnalysisMaterial, SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::{InstanceId, MeshId};
    use xvarna_zurvan::{SolarLocation, SolarOptions, SunSample};

    fn vertical_screen() -> Scene {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(1.0, -2.0, 0.0),
                Vec3::new(1.0, 2.0, 0.0),
                Vec3::new(1.0, 2.0, 4.0),
                Vec3::new(1.0, -2.0, 4.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("valid options");
        let mesh_id = builder.add_mesh(mesh).expect("valid mesh");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(9),
                InstanceId::new(4),
                8,
            )
            .expect("valid instance");
        builder.build().expect("valid scene")
    }

    fn sun_set() -> SunSet {
        let direction = Vec3::new(1.0, 0.0, 1.0)
            .normalized()
            .expect("valid direction");
        SunSet {
            location: SolarLocation::try_new(0.0, 0.0, 0.0).expect("valid location"),
            options: SolarOptions::default(),
            samples: vec![SunSample {
                unix_seconds_utc: 100,
                direction,
                altitude_degrees: 45.0,
                azimuth_degrees: 90.0,
                duration_hours: 2.0,
                weight: 0.5,
                is_active: true,
            }],
            content_hash: [7; 32],
        }
    }

    #[test]
    fn solar_scenarios_expose_material_delta_and_attribution() {
        let scene = vertical_screen();
        let opaque = SolarScenario::try_new(1, "opaque", MaterialLibrary::default())
            .expect("valid scenario");
        let glass_material = AnalysisMaterial::try_new(3, 0.5, 0.4, 0.1).expect("valid material");
        let glass_library =
            MaterialLibrary::try_new([glass_material], [(ObjectId::new(9), glass_material.id)])
                .expect("valid library");
        let glass = SolarScenario::try_new(2, "glass", glass_library).expect("valid scenario");
        let sensor = SolarSensor::try_new(SensorId::new(1), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
            .expect("valid sensor");
        let result = compare_solar_scenarios(
            &scene,
            &[sensor],
            &sun_set(),
            &[opaque, glass],
            SolarScenarioOptions::default(),
        )
        .expect("analysis succeeds");
        assert_eq!(result.summaries.len(), 2);
        assert!(result.summaries[1].received_sun_hours > result.summaries[0].received_sun_hours);
        assert_eq!(result.attribution[0].object_id, ObjectId::new(9));
        assert_eq!(result.attribution[0].category_mask, 8);
        assert!(result.deltas[0].received_sun_hours_delta > 0.0);
    }

    #[test]
    fn solar_envelope_returns_access_and_shading_thresholds() {
        let empty_scene = {
            let mut builder =
                SceneBuilder::new(SceneBuildOptions::default()).expect("valid options");
            let mesh = Mesh {
                positions: vec![
                    Vec3::new(100.0, 100.0, 100.0),
                    Vec3::new(101.0, 100.0, 100.0),
                    Vec3::new(100.0, 101.0, 100.0),
                ],
                triangles: vec![[0, 1, 2]],
            };
            let mesh_id: MeshId = builder.add_mesh(mesh).expect("valid mesh");
            builder
                .add_instance(
                    mesh_id,
                    Transform::IDENTITY,
                    ObjectId::new(1),
                    InstanceId::new(1),
                    1,
                )
                .expect("valid instance");
            builder.build().expect("valid scene")
        };
        let sensor = SolarSensor::try_new(SensorId::new(5), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
            .expect("valid sensor");
        let candidate =
            EnvelopeCandidate::try_new(12, Vec3::new(1.0, 0.0, 0.0)).expect("valid candidate");
        let result = analyze_solar_envelope(
            &empty_scene,
            &[sensor],
            &[candidate],
            &sun_set(),
            &MaterialLibrary::default(),
            SolarEnvelopeOptions {
                candidate_radius_meters: 0.1,
                vertical_clearance_meters: 0.0,
                ..SolarEnvelopeOptions::default()
            },
        )
        .expect("analysis succeeds");
        assert_eq!(result.cells[0].constraint_count, 1);
        assert!((result.cells[0].maximum_solar_access_elevation_meters - 1.0).abs() < 1.0e-12);
        assert!((result.cells[0].minimum_shading_elevation_meters - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn solar_envelope_supports_freeform_candidate_axes() {
        let empty_scene = {
            let mut builder =
                SceneBuilder::new(SceneBuildOptions::default()).expect("valid options");
            let mesh = Mesh {
                positions: vec![
                    Vec3::new(100.0, 100.0, 100.0),
                    Vec3::new(101.0, 100.0, 100.0),
                    Vec3::new(100.0, 101.0, 100.0),
                ],
                triangles: vec![[0, 1, 2]],
            };
            let mesh_id: MeshId = builder.add_mesh(mesh).expect("valid mesh");
            builder
                .add_instance(
                    mesh_id,
                    Transform::IDENTITY,
                    ObjectId::new(1),
                    InstanceId::new(1),
                    1,
                )
                .expect("valid instance");
            builder.build().expect("valid scene")
        };
        let sensor = SolarSensor::try_new(SensorId::new(5), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
            .expect("valid sensor");
        let candidate = EnvelopeCandidate::try_new_oriented(
            12,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
        )
        .expect("valid candidate");
        let result = analyze_solar_envelope(
            &empty_scene,
            &[sensor],
            &[candidate],
            &sun_set(),
            &MaterialLibrary::default(),
            SolarEnvelopeOptions {
                candidate_radius_meters: 0.1,
                vertical_clearance_meters: 0.0,
                ..SolarEnvelopeOptions::default()
            },
        )
        .expect("analysis succeeds");
        let cell = result.cells[0];
        assert_eq!(cell.constraint_count, 1);
        assert!((cell.maximum_solar_access_height_meters - 1.0).abs() < 1.0e-12);
        assert!((cell.maximum_solar_access_elevation_meters - 1.0).abs() < 1.0e-12);
        assert_eq!(candidate.axis, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn solar_envelope_rejects_invalid_candidate_axes() {
        assert_eq!(
            EnvelopeCandidate::try_new_oriented(1, Vec3::ZERO, Vec3::ZERO),
            Err(SolarIntelligenceError::InvalidCandidate)
        );
    }
}
