//! RASHNU evidence, global sensitivity, robust-scenario, and multi-fidelity contracts.
//!
//! The module deliberately distinguishes implemented estimators from research names:
//! Saltelli/Jansen indices require the generated cross-sampled design, Morris statistics
//! require generated one-at-a-time trajectories, and multi-fidelity selection is a
//! deterministic cost-aware Monte-Carlo EHVI policy rather than a claim of MF-HVKG.

use crate::{Candidate, ObjectiveDirection, SplitMix64, VariableSpec, usize_as_f64};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const PASSPORT_SCHEMA: &str = "xvarna.evidence-passport/1";
const DESIGN_SCHEMA: &str = "xvarna.global-sensitivity-design/1";
const MAXIMUM_DESIGN_ROWS: usize = 2_000_000;
const EPSILON: f64 = 1.0e-12;

/// Fidelity used for one externally evaluated result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FidelityLevel {
    /// Interactive XVARNA approximation.
    Fast,
    /// Expensive reference solver such as version-pinned Radiance.
    Reference,
}

/// Validation attached to one result passport.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceValidation {
    /// Whether CPU/GPU parity was tested for this result.
    pub parity_tested: bool,
    /// Largest absolute parity delta when tested.
    #[serde(default)]
    pub maximum_parity_delta: Option<f64>,
    /// Whether an external reference was executed.
    pub reference_tested: bool,
    /// Reference method/tool version.
    #[serde(default)]
    pub reference_method: String,
    /// Largest absolute reference error when tested.
    #[serde(default)]
    pub maximum_reference_error: Option<f64>,
    /// Fraction accepted by the declared validation envelope.
    #[serde(default)]
    pub accepted_fraction: Option<f64>,
}

/// Portable, hash-protected scientific provenance for one result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidencePassport {
    /// Passport contract identity.
    pub schema_version: String,
    /// Stable result hash being qualified.
    pub result_hash: String,
    /// Stable scene hash.
    pub scene_hash: String,
    /// Optional study identity.
    #[serde(default)]
    pub study_id: String,
    /// Optional variant identity.
    #[serde(default)]
    pub variant_id: Option<u64>,
    /// Human-readable scientific method.
    pub method: String,
    /// Fast or external-reference fidelity.
    pub fidelity: FidelityLevel,
    /// XVARNA engine version.
    pub engine_version: String,
    /// CPU/VAYU/Radiance execution backend.
    pub backend: String,
    /// Device/tool name.
    #[serde(default)]
    pub device: String,
    /// Driver/tool version.
    #[serde(default)]
    pub driver: String,
    /// UTC timestamp in RFC 3339 form.
    pub generated_utc: String,
    /// Deterministic seed when sampling is involved.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Effective sample/ray/time-step count.
    #[serde(default)]
    pub sample_count: u64,
    /// Declared convergence/error diagnostic.
    #[serde(default)]
    pub convergence_delta: Option<f64>,
    /// Wall-clock evaluation time.
    #[serde(default)]
    pub elapsed_microseconds: u64,
    /// Named immutable upstream input hashes.
    #[serde(default)]
    pub input_hashes: BTreeMap<String, String>,
    /// Explicit modelling assumptions.
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// Explicit non-claims and known limitations.
    #[serde(default)]
    pub limitations: Vec<String>,
    /// Method citations or stable identifiers.
    #[serde(default)]
    pub citations: Vec<String>,
    /// Parity/reference validation evidence.
    #[serde(default)]
    pub validation: EvidenceValidation,
    /// BLAKE3 identity of all preceding fields.
    #[serde(default)]
    pub content_hash: String,
}

impl EvidencePassport {
    /// Creates or refreshes the passport hash after validating all fields.
    pub fn seal(mut self) -> Result<Self, EvidenceError> {
        self.content_hash.clear();
        self.validate_fields()?;
        self.content_hash = hash_value(b"XVARNA_RASHNU_PASSPORT_V1\0", &self);
        Ok(self)
    }

    /// Verifies field validity and the stored BLAKE3 identity.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        self.validate_fields()?;
        let mut clone = self.clone();
        clone.content_hash.clear();
        if self.content_hash.is_empty()
            || self.content_hash != hash_value(b"XVARNA_RASHNU_PASSPORT_V1\0", &clone)
        {
            return Err(EvidenceError::HashMismatch);
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), EvidenceError> {
        if self.schema_version != PASSPORT_SCHEMA
            || self.result_hash.trim().is_empty()
            || self.scene_hash.trim().is_empty()
            || self.method.trim().is_empty()
            || self.engine_version.trim().is_empty()
            || self.backend.trim().is_empty()
            || chrono::DateTime::parse_from_rfc3339(&self.generated_utc).is_err()
            || self.variant_id == Some(0)
            || self.assumptions.iter().any(|value| value.trim().is_empty())
            || self.limitations.iter().any(|value| value.trim().is_empty())
            || self.citations.iter().any(|value| value.trim().is_empty())
            || self
                .convergence_delta
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || self
                .validation
                .maximum_parity_delta
                .is_some_and(invalid_error)
            || self
                .validation
                .maximum_reference_error
                .is_some_and(invalid_error)
            || self
                .validation
                .accepted_fraction
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || self
                .input_hashes
                .iter()
                .any(|(name, hash)| name.trim().is_empty() || hash.trim().is_empty())
        {
            return Err(EvidenceError::InvalidPassport);
        }
        if self.validation.parity_tested != self.validation.maximum_parity_delta.is_some()
            || self.validation.reference_tested != self.validation.maximum_reference_error.is_some()
            || self.validation.reference_tested
                && self.validation.reference_method.trim().is_empty()
            || !self.validation.reference_tested
                && (!self.validation.reference_method.is_empty()
                    || self.validation.accepted_fraction.is_some())
        {
            return Err(EvidenceError::InvalidPassport);
        }
        Ok(())
    }
}

const fn invalid_error(value: f64) -> bool {
    !value.is_finite() || value < 0.0
}

/// Supported generated global-sensitivity designs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSensitivityMethod {
    /// Saltelli cross-sampling with first-order Saltelli and total-order Jansen estimators.
    SaltelliJansen,
    /// Morris one-factor-at-a-time elementary-effect trajectories.
    Morris,
}

/// One design row with enough identity to align external evaluations safely.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityDesignRow {
    /// Zero-based row index.
    pub row_index: usize,
    /// External-evaluation parameter vector.
    pub values: Vec<f64>,
    /// Morris trajectory when applicable.
    #[serde(default)]
    pub trajectory: Option<usize>,
    /// Parameter changed from the preceding row in a Morris trajectory.
    #[serde(default)]
    pub changed_parameter: Option<usize>,
}

/// Versioned sampling plan; output analysis is rejected unless the hash matches.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GlobalSensitivityDesign {
    /// Design contract identity.
    pub schema_version: String,
    /// Saltelli/Jansen or Morris.
    pub method: GlobalSensitivityMethod,
    /// Exact variable contract.
    pub variables: Vec<VariableSpec>,
    /// Saltelli base sample count or Morris trajectory count.
    pub base_samples: usize,
    /// Grid levels for Morris; zero for Saltelli/Jansen.
    pub levels: usize,
    /// Deterministic sampling seed.
    pub seed: u64,
    /// Ordered evaluation rows.
    pub rows: Vec<SensitivityDesignRow>,
    /// BLAKE3 identity excluding this field.
    pub content_hash: String,
}

impl GlobalSensitivityDesign {
    /// Recomputes the canonical content identity without trusting the stored hash.
    #[must_use]
    pub fn recomputed_content_hash(&self) -> String {
        let mut clone = self.clone();
        clone.content_hash.clear();
        hash_value(b"XVARNA_SENSITIVITY_DESIGN_V1\0", &clone)
    }

    /// Verifies dimensions, row identity, variable bounds, method layout, and hash.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        validate_variables(&self.variables)?;
        if self.schema_version != DESIGN_SCHEMA
            || self.variables.is_empty()
            || self.base_samples == 0
            || self.rows.is_empty()
            || self.rows.len() > MAXIMUM_DESIGN_ROWS
            || self.rows.iter().enumerate().any(|(index, row)| {
                row.row_index != index || row.values.len() != self.variables.len()
            })
        {
            return Err(EvidenceError::InvalidDesign);
        }
        let expected = match self.method {
            GlobalSensitivityMethod::SaltelliJansen => {
                if self.levels != 0 {
                    return Err(EvidenceError::InvalidDesign);
                }
                self.base_samples
                    .checked_mul(self.variables.len().saturating_add(2))
                    .ok_or(EvidenceError::InvalidDesign)?
            }
            GlobalSensitivityMethod::Morris => {
                if self.levels < 4 || !self.levels.is_multiple_of(2) {
                    return Err(EvidenceError::InvalidDesign);
                }
                self.base_samples
                    .checked_mul(self.variables.len().saturating_add(1))
                    .ok_or(EvidenceError::InvalidDesign)?
            }
        };
        if self.rows.len() != expected {
            return Err(EvidenceError::InvalidDesign);
        }
        match self.method {
            GlobalSensitivityMethod::SaltelliJansen => {
                if self
                    .rows
                    .iter()
                    .any(|row| row.trajectory.is_some() || row.changed_parameter.is_some())
                {
                    return Err(EvidenceError::InvalidDesign);
                }
                validate_saltelli_layout(self)?;
            }
            GlobalSensitivityMethod::Morris => {
                let width = self.variables.len() + 1;
                for (index, row) in self.rows.iter().enumerate() {
                    let trajectory = index / width;
                    let step = index % width;
                    if row.trajectory != Some(trajectory)
                        || step == 0 && row.changed_parameter.is_some()
                        || step > 0
                            && row
                                .changed_parameter
                                .is_none_or(|parameter| parameter >= self.variables.len())
                    {
                        return Err(EvidenceError::InvalidDesign);
                    }
                }
                validate_morris_layout(self)?;
            }
        }
        for row in &self.rows {
            for (value, variable) in row.values.iter().zip(&self.variables) {
                if !value.is_finite()
                    || *value < variable.lower_bound - EPSILON
                    || *value > variable.upper_bound + EPSILON
                {
                    return Err(EvidenceError::InvalidDesign);
                }
            }
        }
        if self.content_hash != self.recomputed_content_hash() {
            return Err(EvidenceError::HashMismatch);
        }
        Ok(())
    }
}

fn validate_saltelli_layout(design: &GlobalSensitivityDesign) -> Result<(), EvidenceError> {
    let count = design.base_samples;
    let variables = design.variables.len();
    for parameter in 0..variables {
        let mixed_start = count * (2 + parameter);
        for row in 0..count {
            let first = &design.rows[row].values;
            let second = &design.rows[count + row].values;
            let mixed = &design.rows[mixed_start + row].values;
            if mixed.iter().enumerate().any(|(column, value)| {
                (*value
                    - if column == parameter {
                        second[column]
                    } else {
                        first[column]
                    })
                .abs()
                    > EPSILON
            }) {
                return Err(EvidenceError::InvalidDesign);
            }
        }
    }
    Ok(())
}

fn validate_morris_layout(design: &GlobalSensitivityDesign) -> Result<(), EvidenceError> {
    let width = design.variables.len() + 1;
    for trajectory in 0..design.base_samples {
        let start = trajectory * width;
        let mut changed = BTreeSet::new();
        for step in 1..width {
            let previous = &design.rows[start + step - 1];
            let current = &design.rows[start + step];
            let parameter = current
                .changed_parameter
                .ok_or(EvidenceError::InvalidDesign)?;
            let actual = current
                .values
                .iter()
                .zip(&previous.values)
                .enumerate()
                .filter_map(|(index, (first, second))| {
                    ((first - second).abs() > EPSILON).then_some(index)
                })
                .collect::<Vec<_>>();
            if actual.as_slice() != [parameter] || !changed.insert(parameter) {
                return Err(EvidenceError::InvalidDesign);
            }
        }
        if changed.len() != design.variables.len() {
            return Err(EvidenceError::InvalidDesign);
        }
    }
    Ok(())
}

/// Generates independent Saltelli A/B matrices and every A-with-B-column matrix.
pub fn saltelli_jansen_design(
    variables: &[VariableSpec],
    base_samples: usize,
    seed: u64,
) -> Result<GlobalSensitivityDesign, EvidenceError> {
    validate_variables(variables)?;
    let row_count = base_samples
        .checked_mul(variables.len().saturating_add(2))
        .filter(|count| *count > 0 && *count <= MAXIMUM_DESIGN_ROWS)
        .ok_or(EvidenceError::InvalidDesign)?;
    // Domain-separated streams avoid structural correlation between the two
    // Monte-Carlo matrices while remaining exactly reproducible.
    let mut first_random = SplitMix64::new(seed ^ 0x5341_4c54_454c_4c49);
    let mut second_random = SplitMix64::new(seed ^ 0x4a41_4e53_454e_5f42);
    let mut first = vec![vec![0.0; variables.len()]; base_samples];
    let mut second = first.clone();
    for row in 0..base_samples {
        for (column, variable) in variables.iter().enumerate() {
            first[row][column] = sample_variable(*variable, first_random.unit());
            second[row][column] = sample_variable(*variable, second_random.unit());
        }
    }
    let mut values = Vec::with_capacity(row_count);
    values.extend(first.iter().cloned());
    values.extend(second.iter().cloned());
    for column in 0..variables.len() {
        for row in 0..base_samples {
            let mut mixed = first[row].clone();
            mixed[column] = second[row][column];
            values.push(mixed);
        }
    }
    seal_design(
        GlobalSensitivityMethod::SaltelliJansen,
        variables,
        base_samples,
        0,
        seed,
        values,
    )
}

/// Generates deterministic Morris OAT trajectories on an even-level grid.
pub fn morris_design(
    variables: &[VariableSpec],
    trajectories: usize,
    levels: usize,
    seed: u64,
) -> Result<GlobalSensitivityDesign, EvidenceError> {
    validate_variables(variables)?;
    if levels < 4 || !levels.is_multiple_of(2) || trajectories == 0 {
        return Err(EvidenceError::InvalidDesign);
    }
    let row_count = trajectories
        .checked_mul(variables.len().saturating_add(1))
        .filter(|count| *count <= MAXIMUM_DESIGN_ROWS)
        .ok_or(EvidenceError::InvalidDesign)?;
    let delta = usize_as_f64(levels) / (2.0 * usize_as_f64(levels.saturating_sub(1)));
    let grid_max = 1.0 - delta;
    let grid_steps = levels / 2;
    let mut random = SplitMix64::new(seed);
    let mut rows = Vec::with_capacity(row_count);
    for trajectory in 0..trajectories {
        let mut order = (0..variables.len()).collect::<Vec<_>>();
        random.shuffle(&mut order);
        let mut directions = Vec::<f64>::with_capacity(variables.len());
        let mut unit = Vec::with_capacity(variables.len());
        for _ in variables {
            let positive = random.unit() >= 0.5;
            directions.push(if positive { 1.0 } else { -1.0 });
            let grid_index = (random.unit() * usize_as_f64(grid_steps))
                .floor()
                .clamp(0.0, usize_as_f64(grid_steps.saturating_sub(1)));
            let base = if grid_steps <= 1 {
                0.0
            } else {
                grid_index / usize_as_f64(grid_steps - 1) * grid_max
            };
            unit.push(if positive { base } else { base + delta });
        }
        rows.push(SensitivityDesignRow {
            row_index: rows.len(),
            values: map_unit_row(variables, &unit),
            trajectory: Some(trajectory),
            changed_parameter: None,
        });
        for parameter in order {
            unit[parameter] = directions[parameter]
                .mul_add(delta, unit[parameter])
                .clamp(0.0, 1.0);
            let mut mapped = map_unit_row(variables, &unit);
            let previous = &rows.last().expect("trajectory has initial row").values;
            if (mapped[parameter] - previous[parameter]).abs() <= EPSILON {
                mapped[parameter] = if directions[parameter] > 0.0 {
                    variables[parameter].upper_bound
                } else {
                    variables[parameter].lower_bound
                };
            }
            rows.push(SensitivityDesignRow {
                row_index: rows.len(),
                values: mapped,
                trajectory: Some(trajectory),
                changed_parameter: Some(parameter),
            });
        }
    }
    let mut design = GlobalSensitivityDesign {
        schema_version: DESIGN_SCHEMA.to_owned(),
        method: GlobalSensitivityMethod::Morris,
        variables: variables.to_vec(),
        base_samples: trajectories,
        levels,
        seed,
        rows,
        content_hash: String::new(),
    };
    design.content_hash = hash_value(b"XVARNA_SENSITIVITY_DESIGN_V1\0", &design);
    design.validate()?;
    Ok(design)
}

/// Saltelli first-order and Jansen total-order result for one parameter/output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SobolIndex {
    /// Zero-based input parameter column.
    pub parameter_index: usize,
    /// Zero-based evaluated output column.
    pub output_index: usize,
    /// Saltelli first-order contribution estimate.
    pub first_order: f64,
    /// Jansen total-order contribution estimate.
    pub total_order: f64,
    /// Output variance used to normalize both indices.
    pub variance: f64,
    /// Number of rows in each base matrix.
    pub base_samples: usize,
}

/// Morris elementary-effect result for one parameter/output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorrisEffect {
    /// Zero-based input parameter column.
    pub parameter_index: usize,
    /// Zero-based evaluated output column.
    pub output_index: usize,
    /// Signed elementary-effect mean, also called mu.
    pub mean: f64,
    /// Absolute elementary-effect mean, also called mu-star.
    pub mean_absolute: f64,
    /// Sample standard deviation of elementary effects.
    pub standard_deviation: f64,
    /// Number of valid OAT trajectories.
    pub trajectories: usize,
}

/// Result of analyzing the exact generated sensitivity design.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSensitivityResult {
    /// Hash of the exact design that produced the aligned outputs.
    pub design_hash: String,
    /// Estimator family used.
    pub method: GlobalSensitivityMethod,
    /// Number of output columns analyzed.
    pub output_count: usize,
    /// Saltelli/Jansen indices, empty for Morris designs.
    pub sobol: Vec<SobolIndex>,
    /// Morris effects, empty for Saltelli/Jansen designs.
    pub morris: Vec<MorrisEffect>,
    /// BLAKE3 identity of this result.
    pub content_hash: String,
}

/// Analyzes row-aligned output vectors using only the estimator valid for the design.
pub fn analyze_global_sensitivity(
    design: &GlobalSensitivityDesign,
    outputs: &[Vec<f64>],
) -> Result<GlobalSensitivityResult, EvidenceError> {
    design.validate()?;
    let output_count = outputs.first().map_or(0, Vec::len);
    if outputs.len() != design.rows.len()
        || output_count == 0
        || outputs
            .iter()
            .any(|row| row.len() != output_count || row.iter().any(|value| !value.is_finite()))
    {
        return Err(EvidenceError::InvalidOutputs);
    }
    let (sobol, morris) = match design.method {
        GlobalSensitivityMethod::SaltelliJansen => {
            (analyze_sobol(design, outputs, output_count)?, Vec::new())
        }
        GlobalSensitivityMethod::Morris => {
            (Vec::new(), analyze_morris(design, outputs, output_count)?)
        }
    };
    let mut result = GlobalSensitivityResult {
        design_hash: design.content_hash.clone(),
        method: design.method,
        output_count,
        sobol,
        morris,
        content_hash: String::new(),
    };
    result.content_hash = hash_value(b"XVARNA_GLOBAL_SENSITIVITY_V1\0", &result);
    Ok(result)
}

/// One scenario-specific uncertain outcome.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioOutcome {
    /// Stable candidate identifier.
    pub candidate_id: u64,
    /// Stable scenario identifier within the candidate.
    pub scenario_id: u64,
    /// Positive scenario weight; normalized per candidate.
    pub probability: f64,
    /// Ordered objective vector.
    pub objectives: Vec<f64>,
    /// Residual constraints, feasible at or below zero.
    #[serde(default)]
    pub constraint_residuals: Vec<f64>,
}

/// Robust climate/material summary suitable for conservative objectives and chance constraints.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobustScenarioSummary {
    /// Stable candidate identifier.
    pub candidate_id: u64,
    /// Number of aggregated scenarios.
    pub scenario_count: usize,
    /// Probability-weighted objective means.
    pub mean_objectives: Vec<f64>,
    /// Probability-weighted population standard deviations.
    pub standard_deviations: Vec<f64>,
    /// Direction-aware worst observed objectives.
    pub worst_objectives: Vec<f64>,
    /// Direction-aware upper-loss-tail conditional values at risk.
    pub cvar_objectives: Vec<f64>,
    /// Probability of at least one residual constraint violation.
    pub violation_probability: f64,
}

/// Aggregates weighted scenarios using a direction-aware upper-tail `CVaR` of loss.
#[allow(clippy::too_many_lines)]
pub fn aggregate_robust_scenarios(
    directions: &[ObjectiveDirection],
    constraint_count: usize,
    outcomes: &[ScenarioOutcome],
    cvar_alpha: f64,
) -> Result<Vec<RobustScenarioSummary>, EvidenceError> {
    if directions.is_empty()
        || outcomes.is_empty()
        || !cvar_alpha.is_finite()
        || !(0.0..1.0).contains(&cvar_alpha)
    {
        return Err(EvidenceError::InvalidScenario);
    }
    let mut groups = BTreeMap::<u64, Vec<&ScenarioOutcome>>::new();
    let mut scenario_ids = BTreeSet::new();
    for outcome in outcomes {
        if outcome.candidate_id == 0
            || outcome.scenario_id == 0
            || !outcome.probability.is_finite()
            || outcome.probability <= 0.0
            || outcome.objectives.len() != directions.len()
            || outcome.objectives.iter().any(|value| !value.is_finite())
            || outcome.constraint_residuals.len() != constraint_count
            || outcome
                .constraint_residuals
                .iter()
                .any(|value| !value.is_finite())
            || !scenario_ids.insert((outcome.candidate_id, outcome.scenario_id))
        {
            return Err(EvidenceError::InvalidScenario);
        }
        groups
            .entry(outcome.candidate_id)
            .or_default()
            .push(outcome);
    }
    let mut result = Vec::with_capacity(groups.len());
    for (candidate_id, rows) in groups {
        let total_weight = rows.iter().map(|row| row.probability).sum::<f64>();
        if !total_weight.is_finite() || total_weight <= 0.0 {
            return Err(EvidenceError::InvalidScenario);
        }
        let weights = rows
            .iter()
            .map(|row| row.probability / total_weight)
            .collect::<Vec<_>>();
        let mut means = vec![0.0; directions.len()];
        for (row, weight) in rows.iter().zip(&weights) {
            for (index, value) in row.objectives.iter().enumerate() {
                means[index] += value * weight;
            }
        }
        let mut deviations = vec![0.0; directions.len()];
        for (row, weight) in rows.iter().zip(&weights) {
            for (index, value) in row.objectives.iter().enumerate() {
                deviations[index] += weight * (value - means[index]).powi(2);
            }
        }
        for value in &mut deviations {
            *value = value.sqrt();
        }
        let mut worst = vec![0.0; directions.len()];
        let mut cvar = vec![0.0; directions.len()];
        for (objective, direction) in directions.iter().enumerate() {
            let mut losses = rows
                .iter()
                .zip(&weights)
                .map(|(row, weight)| {
                    let loss = match direction {
                        ObjectiveDirection::Minimize => row.objectives[objective],
                        ObjectiveDirection::Maximize => -row.objectives[objective],
                    };
                    (loss, *weight)
                })
                .collect::<Vec<_>>();
            losses.sort_by(|a, b| b.0.total_cmp(&a.0));
            let tail_weight = 1.0 - cvar_alpha;
            let mut remaining = tail_weight;
            let mut tail_sum = 0.0;
            for (loss, weight) in &losses {
                let used = remaining.min(*weight);
                tail_sum += loss * used;
                remaining -= used;
                if remaining <= EPSILON {
                    break;
                }
            }
            let worst_loss = losses[0].0;
            worst[objective] = if matches!(direction, ObjectiveDirection::Minimize) {
                worst_loss
            } else {
                -worst_loss
            };
            let tail_loss = tail_sum / tail_weight;
            cvar[objective] = if matches!(direction, ObjectiveDirection::Minimize) {
                tail_loss
            } else {
                -tail_loss
            };
        }
        let violation_probability = rows
            .iter()
            .zip(&weights)
            .filter(|(row, _)| row.constraint_residuals.iter().any(|value| *value > 0.0))
            .map(|(_, weight)| *weight)
            .sum();
        result.push(RobustScenarioSummary {
            candidate_id,
            scenario_count: rows.len(),
            mean_objectives: means,
            standard_deviations: deviations,
            worst_objectives: worst,
            cvar_objectives: cvar,
            violation_probability,
        });
    }
    Ok(result)
}

/// Mean and standard deviation for a noisy candidate evaluation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncertainEvaluation {
    /// Stable candidate identifier.
    pub candidate_id: u64,
    /// Ordered objective posterior/sample means.
    pub objective_means: Vec<f64>,
    /// Non-negative objective standard deviations.
    pub objective_standard_deviations: Vec<f64>,
    /// Ordered residual-constraint means.
    #[serde(default)]
    pub constraint_means: Vec<f64>,
    /// Non-negative residual-constraint standard deviations.
    #[serde(default)]
    pub constraint_standard_deviations: Vec<f64>,
}

/// Conservative feasibility classification at the requested normal interval multiplier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FeasibilityConfidence {
    /// Every constraint interval lies at or below zero.
    Proven,
    /// At least one constraint interval crosses zero.
    Possible,
    /// At least one constraint interval lies entirely above zero.
    Infeasible,
}

/// Interval-dominance rank for one uncertain candidate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UncertaintyRank {
    /// Stable candidate identifier.
    pub candidate_id: u64,
    /// Zero-based conservative non-dominated front.
    pub robust_pareto_rank: usize,
    /// Confidence-interval constraint classification.
    pub feasibility: FeasibilityConfidence,
    /// Number of candidates this interval safely dominates.
    pub dominated_count: usize,
    /// Lower confidence bounds for all objectives.
    pub lower_objective_bounds: Vec<f64>,
    /// Upper confidence bounds for all objectives.
    pub upper_objective_bounds: Vec<f64>,
}

/// Performs interval Pareto sorting; dominance requires non-overlapping confidence bounds.
#[allow(clippy::too_many_lines)]
pub fn uncertainty_aware_ranking(
    directions: &[ObjectiveDirection],
    constraint_count: usize,
    evaluations: &[UncertainEvaluation],
    interval_multiplier: f64,
) -> Result<Vec<UncertaintyRank>, EvidenceError> {
    if directions.is_empty()
        || evaluations.is_empty()
        || !interval_multiplier.is_finite()
        || interval_multiplier < 0.0
    {
        return Err(EvidenceError::InvalidUncertainty);
    }
    let mut ids = BTreeSet::new();
    let mut rows = Vec::with_capacity(evaluations.len());
    for evaluation in evaluations {
        if evaluation.candidate_id == 0
            || !ids.insert(evaluation.candidate_id)
            || evaluation.objective_means.len() != directions.len()
            || evaluation.objective_standard_deviations.len() != directions.len()
            || evaluation.constraint_means.len() != constraint_count
            || evaluation.constraint_standard_deviations.len() != constraint_count
            || evaluation
                .objective_means
                .iter()
                .chain(&evaluation.objective_standard_deviations)
                .chain(&evaluation.constraint_means)
                .chain(&evaluation.constraint_standard_deviations)
                .any(|value| !value.is_finite())
            || evaluation
                .objective_standard_deviations
                .iter()
                .chain(&evaluation.constraint_standard_deviations)
                .any(|value| *value < 0.0)
        {
            return Err(EvidenceError::InvalidUncertainty);
        }
        let lower = evaluation
            .objective_means
            .iter()
            .zip(&evaluation.objective_standard_deviations)
            .map(|(mean, deviation)| mean - interval_multiplier * deviation)
            .collect::<Vec<_>>();
        let upper = evaluation
            .objective_means
            .iter()
            .zip(&evaluation.objective_standard_deviations)
            .map(|(mean, deviation)| mean + interval_multiplier * deviation)
            .collect::<Vec<_>>();
        let feasibility = if constraint_count == 0
            || evaluation
                .constraint_means
                .iter()
                .zip(&evaluation.constraint_standard_deviations)
                .all(|(mean, deviation)| mean + interval_multiplier * deviation <= 0.0)
        {
            FeasibilityConfidence::Proven
        } else if evaluation
            .constraint_means
            .iter()
            .zip(&evaluation.constraint_standard_deviations)
            .any(|(mean, deviation)| mean - interval_multiplier * deviation > 0.0)
        {
            FeasibilityConfidence::Infeasible
        } else {
            FeasibilityConfidence::Possible
        };
        rows.push((evaluation.candidate_id, lower, upper, feasibility));
    }
    let mut dominates = vec![Vec::new(); rows.len()];
    let mut dominated_by = vec![0usize; rows.len()];
    for first in 0..rows.len() {
        for second in 0..rows.len() {
            if first == second {
                continue;
            }
            if robustly_dominates(directions, &rows[first], &rows[second]) {
                dominates[first].push(second);
                dominated_by[second] += 1;
            }
        }
    }
    let mut ranks = vec![usize::MAX; rows.len()];
    let mut front = dominated_by
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<Vec<_>>();
    let mut rank = 0;
    while !front.is_empty() {
        let mut next = Vec::new();
        for index in front {
            ranks[index] = rank;
            for &dominated in &dominates[index] {
                dominated_by[dominated] = dominated_by[dominated].saturating_sub(1);
                if dominated_by[dominated] == 0 {
                    next.push(dominated);
                }
            }
        }
        next.sort_unstable();
        next.dedup();
        front = next;
        rank += 1;
    }
    Ok(rows
        .into_iter()
        .enumerate()
        .map(
            |(index, (candidate_id, lower, upper, feasibility))| UncertaintyRank {
                candidate_id,
                robust_pareto_rank: ranks[index],
                feasibility,
                dominated_count: dominates[index].len(),
                lower_objective_bounds: lower,
                upper_objective_bounds: upper,
            },
        )
        .collect())
}

/// One fast/reference observation used to calibrate fidelity discrepancy.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FidelityObservation {
    /// Stable candidate identifier.
    pub candidate_id: u64,
    /// Fast approximation or expensive reference.
    pub fidelity: FidelityLevel,
    /// Ordered objective vector.
    pub objectives: Vec<f64>,
    /// Positive measured or estimated evaluation cost.
    pub cost: f64,
    /// Non-negative per-objective observation noise.
    #[serde(default)]
    pub noise_standard_deviations: Vec<f64>,
    /// Hash of the supporting Evidence Passport or result.
    pub evidence_hash: String,
}

/// Deterministic cost-aware reference-selection policy.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiFidelityPolicy {
    /// Maximum number of reference jobs to recommend.
    pub batch_size: usize,
    /// Monte-Carlo draws per candidate for two-objective EHVI.
    pub monte_carlo_samples: usize,
    /// Deterministic Monte-Carlo seed.
    pub seed: u64,
    /// Direction-aware hypervolume reference point in source units.
    pub reference_point: Vec<f64>,
    /// Positive floor for calibrated discrepancy uncertainty.
    pub minimum_standard_deviation: f64,
}

/// Per-objective autoregressive linear correction `reference = intercept + slope * fast`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityCalibration {
    /// Zero-based objective column.
    pub objective_index: usize,
    /// Number of aligned Fast/reference candidates.
    pub paired_samples: usize,
    /// Linear correction intercept.
    pub intercept: f64,
    /// Linear correction slope.
    pub slope: f64,
    /// Residual discrepancy standard deviation.
    pub residual_standard_deviation: f64,
    /// Coefficient of determination of the linear correction.
    pub r_squared: f64,
}

/// One selected reference evaluation with transparent acquisition terms.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityRecommendation {
    /// Candidate selected for an expensive reference evaluation.
    pub candidate_id: u64,
    /// Calibrated reference objective means.
    pub predicted_reference_mean: Vec<f64>,
    /// Propagated discrepancy and observation uncertainty.
    pub predicted_reference_standard_deviation: Vec<f64>,
    /// Monte-Carlo expected hypervolume improvement for two objectives.
    pub expected_hypervolume_improvement: f64,
    /// Mean observed reference-evaluation cost.
    pub expected_cost: f64,
    /// Acquisition value divided by expected cost.
    pub acquisition_per_cost: f64,
}

/// Complete selection result for Fast-to-reference validation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiFidelitySelection {
    /// Per-objective Fast-to-reference calibration diagnostics.
    pub calibrations: Vec<FidelityCalibration>,
    /// Ordered reference-evaluation recommendations.
    pub recommendations: Vec<FidelityRecommendation>,
    /// Candidate IDs used to fit the calibration.
    pub paired_candidate_ids: Vec<u64>,
    /// BLAKE3 identity of the selection and calibration.
    pub content_hash: String,
}

/// Calibrates Fast/reference pairs and greedily selects costly reference evaluations.
///
/// For exactly two objectives, acquisition is Monte-Carlo expected hypervolume improvement
/// divided by observed mean reference cost. This is noise- and cost-aware, but it does not
/// claim the joint-batch fantasies or Gaussian-process kernel of qNEHVI/MF-HVKG.
#[allow(clippy::too_many_lines)]
pub fn recommend_reference_evaluations(
    directions: &[ObjectiveDirection],
    candidates: &[Candidate],
    observations: &[FidelityObservation],
    policy: &MultiFidelityPolicy,
) -> Result<MultiFidelitySelection, EvidenceError> {
    validate_fidelity_inputs(directions, candidates, observations, policy)?;
    let candidate_map = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id, candidate))
        .collect::<BTreeMap<_, _>>();
    let mut fast = BTreeMap::new();
    let mut reference = BTreeMap::new();
    for observation in observations {
        if !candidate_map.contains_key(&observation.candidate_id) {
            return Err(EvidenceError::UnknownCandidate);
        }
        let target = match observation.fidelity {
            FidelityLevel::Fast => &mut fast,
            FidelityLevel::Reference => &mut reference,
        };
        if target
            .insert(observation.candidate_id, observation)
            .is_some()
        {
            return Err(EvidenceError::DuplicateObservation);
        }
    }
    let paired = fast
        .keys()
        .filter(|id| reference.contains_key(id))
        .copied()
        .collect::<Vec<_>>();
    if paired.len() < 2 {
        return Err(EvidenceError::InsufficientPairs);
    }
    let calibrations = (0..directions.len())
        .map(|objective| {
            calibrate_objective(
                objective,
                &paired,
                &fast,
                &reference,
                policy.minimum_standard_deviation,
            )
        })
        .collect::<Vec<_>>();
    let reference_cost =
        reference.values().map(|value| value.cost).sum::<f64>() / usize_as_f64(reference.len());
    let current_points = reference
        .values()
        .map(|value| transform_objectives(&value.objectives, directions))
        .collect::<Vec<_>>();
    let transformed_reference = transform_objectives(&policy.reference_point, directions);
    let current_hv = if directions.len() == 2 {
        hypervolume_min_2d(&current_points, &transformed_reference)
    } else {
        0.0
    };
    let mut random = SplitMix64::new(policy.seed);
    let mut scored = Vec::new();
    for (candidate_id, observation) in &fast {
        if reference.contains_key(candidate_id) {
            continue;
        }
        let means = calibrations
            .iter()
            .enumerate()
            .map(|(index, calibration)| {
                calibration
                    .slope
                    .mul_add(observation.objectives[index], calibration.intercept)
            })
            .collect::<Vec<_>>();
        let deviations = calibrations
            .iter()
            .enumerate()
            .map(|(index, calibration)| {
                calibration
                    .residual_standard_deviation
                    .hypot(calibration.slope.abs() * observation.noise_standard_deviations[index])
            })
            .collect::<Vec<_>>();
        let improvement = if directions.len() == 2 {
            let sum = (0..policy.monte_carlo_samples)
                .map(|_| {
                    let sampled = means
                        .iter()
                        .zip(&deviations)
                        .map(|(mean, deviation)| mean + deviation * standard_normal(&mut random))
                        .collect::<Vec<_>>();
                    let mut points = current_points.clone();
                    points.push(transform_objectives(&sampled, directions));
                    (hypervolume_min_2d(&points, &transformed_reference) - current_hv).max(0.0)
                })
                .sum::<f64>();
            sum / usize_as_f64(policy.monte_carlo_samples)
        } else {
            deviations.iter().sum::<f64>()
        };
        scored.push(FidelityRecommendation {
            candidate_id: *candidate_id,
            predicted_reference_mean: means,
            predicted_reference_standard_deviation: deviations,
            expected_hypervolume_improvement: improvement,
            expected_cost: reference_cost,
            acquisition_per_cost: improvement / reference_cost,
        });
    }
    scored.sort_by(|a, b| {
        b.acquisition_per_cost
            .total_cmp(&a.acquisition_per_cost)
            .then_with(|| a.candidate_id.cmp(&b.candidate_id))
    });
    scored.truncate(policy.batch_size.min(scored.len()));
    let mut selection = MultiFidelitySelection {
        calibrations,
        recommendations: scored,
        paired_candidate_ids: paired,
        content_hash: String::new(),
    };
    selection.content_hash = hash_value(b"XVARNA_MULTI_FIDELITY_SELECTION_V1\0", &selection);
    Ok(selection)
}

fn validate_variables(variables: &[VariableSpec]) -> Result<(), EvidenceError> {
    if variables.is_empty() || variables.len() > 256 {
        return Err(EvidenceError::InvalidDesign);
    }
    let mut ids = BTreeSet::new();
    for variable in variables {
        if variable.variable_id == 0 || !ids.insert(variable.variable_id) {
            return Err(EvidenceError::InvalidDesign);
        }
        VariableSpec::try_new(
            variable.variable_id,
            variable.kind,
            variable.lower_bound,
            variable.upper_bound,
        )
        .map_err(|_| EvidenceError::InvalidDesign)?;
    }
    Ok(())
}

fn sample_variable(variable: VariableSpec, unit: f64) -> f64 {
    variable.canonicalize(
        variable
            .lower_bound
            .mul_add(1.0 - unit, variable.upper_bound * unit),
    )
}

fn map_unit_row(variables: &[VariableSpec], unit: &[f64]) -> Vec<f64> {
    variables
        .iter()
        .zip(unit)
        .map(|(variable, value)| sample_variable(*variable, *value))
        .collect()
}

fn seal_design(
    method: GlobalSensitivityMethod,
    variables: &[VariableSpec],
    base_samples: usize,
    levels: usize,
    seed: u64,
    values: Vec<Vec<f64>>,
) -> Result<GlobalSensitivityDesign, EvidenceError> {
    let rows = values
        .into_iter()
        .enumerate()
        .map(|(row_index, values)| SensitivityDesignRow {
            row_index,
            values,
            trajectory: None,
            changed_parameter: None,
        })
        .collect();
    let mut design = GlobalSensitivityDesign {
        schema_version: DESIGN_SCHEMA.to_owned(),
        method,
        variables: variables.to_vec(),
        base_samples,
        levels,
        seed,
        rows,
        content_hash: String::new(),
    };
    design.content_hash = hash_value(b"XVARNA_SENSITIVITY_DESIGN_V1\0", &design);
    design.validate()?;
    Ok(design)
}

fn analyze_sobol(
    design: &GlobalSensitivityDesign,
    outputs: &[Vec<f64>],
    output_count: usize,
) -> Result<Vec<SobolIndex>, EvidenceError> {
    let count = design.base_samples;
    let variables = design.variables.len();
    let mut result = Vec::with_capacity(variables * output_count);
    #[allow(clippy::needless_range_loop)]
    for output in 0..output_count {
        let combined = (0..count)
            .flat_map(|row| [outputs[row][output], outputs[count + row][output]])
            .collect::<Vec<_>>();
        let mean = combined.iter().sum::<f64>() / usize_as_f64(combined.len());
        let variance = combined
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / usize_as_f64(combined.len().saturating_sub(1));
        if !variance.is_finite() || variance <= EPSILON {
            return Err(EvidenceError::ZeroVariance);
        }
        for parameter in 0..variables {
            let mixed_start = count * (2 + parameter);
            let first_order = (0..count)
                .map(|row| {
                    outputs[count + row][output]
                        * (outputs[mixed_start + row][output] - outputs[row][output])
                })
                .sum::<f64>()
                / usize_as_f64(count)
                / variance;
            let total_order = (0..count)
                .map(|row| (outputs[row][output] - outputs[mixed_start + row][output]).powi(2))
                .sum::<f64>()
                / (2.0 * usize_as_f64(count) * variance);
            result.push(SobolIndex {
                parameter_index: parameter,
                output_index: output,
                first_order,
                total_order,
                variance,
                base_samples: count,
            });
        }
    }
    Ok(result)
}

fn analyze_morris(
    design: &GlobalSensitivityDesign,
    outputs: &[Vec<f64>],
    output_count: usize,
) -> Result<Vec<MorrisEffect>, EvidenceError> {
    let mut effects = vec![vec![Vec::<f64>::new(); output_count]; design.variables.len()];
    let width = design.variables.len() + 1;
    for trajectory in 0..design.base_samples {
        let start = trajectory * width;
        for step in 1..width {
            let current = &design.rows[start + step];
            let previous = &design.rows[start + step - 1];
            let parameter = current
                .changed_parameter
                .ok_or(EvidenceError::InvalidDesign)?;
            let delta = current.values[parameter] - previous.values[parameter];
            if delta.abs() <= EPSILON {
                return Err(EvidenceError::InvalidDesign);
            }
            for output in 0..output_count {
                effects[parameter][output].push(
                    (outputs[start + step][output] - outputs[start + step - 1][output]) / delta,
                );
            }
        }
    }
    let mut result = Vec::with_capacity(design.variables.len() * output_count);
    for (parameter, outputs_by_parameter) in effects.iter().enumerate() {
        for (output, values) in outputs_by_parameter.iter().enumerate() {
            if values.len() != design.base_samples {
                return Err(EvidenceError::InvalidDesign);
            }
            let mean = values.iter().sum::<f64>() / usize_as_f64(values.len());
            let mean_absolute =
                values.iter().map(|value| value.abs()).sum::<f64>() / usize_as_f64(values.len());
            let standard_deviation = if values.len() < 2 {
                0.0
            } else {
                (values
                    .iter()
                    .map(|value| (value - mean).powi(2))
                    .sum::<f64>()
                    / usize_as_f64(values.len().saturating_sub(1)))
                .sqrt()
            };
            result.push(MorrisEffect {
                parameter_index: parameter,
                output_index: output,
                mean,
                mean_absolute,
                standard_deviation,
                trajectories: values.len(),
            });
        }
    }
    Ok(result)
}

fn robustly_dominates(
    directions: &[ObjectiveDirection],
    first: &(u64, Vec<f64>, Vec<f64>, FeasibilityConfidence),
    second: &(u64, Vec<f64>, Vec<f64>, FeasibilityConfidence),
) -> bool {
    let confidence_tier = |confidence| match confidence {
        FeasibilityConfidence::Proven => 2_u8,
        FeasibilityConfidence::Possible => 1_u8,
        FeasibilityConfidence::Infeasible => 0_u8,
    };
    match confidence_tier(first.3).cmp(&confidence_tier(second.3)) {
        std::cmp::Ordering::Greater => return true,
        std::cmp::Ordering::Less => return false,
        std::cmp::Ordering::Equal => {}
    }
    let mut strict = false;
    directions
        .iter()
        .enumerate()
        .all(|(index, direction)| match direction {
            ObjectiveDirection::Minimize => {
                strict |= first.2[index] < second.1[index] - EPSILON;
                first.2[index] <= second.1[index] + EPSILON
            }
            ObjectiveDirection::Maximize => {
                strict |= first.1[index] > second.2[index] + EPSILON;
                first.1[index] >= second.2[index] - EPSILON
            }
        })
        && strict
}

fn validate_fidelity_inputs(
    directions: &[ObjectiveDirection],
    candidates: &[Candidate],
    observations: &[FidelityObservation],
    policy: &MultiFidelityPolicy,
) -> Result<(), EvidenceError> {
    if directions.len() < 2
        || candidates.is_empty()
        || observations.is_empty()
        || policy.batch_size == 0
        || policy.monte_carlo_samples == 0
        || policy.monte_carlo_samples > 100_000
        || policy.reference_point.len() != directions.len()
        || policy
            .reference_point
            .iter()
            .any(|value| !value.is_finite())
        || !policy.minimum_standard_deviation.is_finite()
        || policy.minimum_standard_deviation <= 0.0
    {
        return Err(EvidenceError::InvalidFidelity);
    }
    let mut ids = BTreeSet::new();
    if candidates
        .iter()
        .any(|candidate| candidate.candidate_id == 0 || !ids.insert(candidate.candidate_id))
    {
        return Err(EvidenceError::InvalidFidelity);
    }
    for observation in observations {
        if observation.candidate_id == 0
            || observation.objectives.len() != directions.len()
            || observation.noise_standard_deviations.len() != directions.len()
            || observation
                .objectives
                .iter()
                .any(|value| !value.is_finite())
            || observation
                .noise_standard_deviations
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || !observation.cost.is_finite()
            || observation.cost <= 0.0
            || observation.evidence_hash.trim().is_empty()
        {
            return Err(EvidenceError::InvalidFidelity);
        }
    }
    Ok(())
}

fn calibrate_objective(
    objective: usize,
    paired: &[u64],
    fast: &BTreeMap<u64, &FidelityObservation>,
    reference: &BTreeMap<u64, &FidelityObservation>,
    minimum: f64,
) -> FidelityCalibration {
    let x = paired
        .iter()
        .map(|id| fast[id].objectives[objective])
        .collect::<Vec<_>>();
    let y = paired
        .iter()
        .map(|id| reference[id].objectives[objective])
        .collect::<Vec<_>>();
    let mean_x = x.iter().sum::<f64>() / usize_as_f64(x.len());
    let mean_y = y.iter().sum::<f64>() / usize_as_f64(y.len());
    let variance_x = x.iter().map(|value| (value - mean_x).powi(2)).sum::<f64>();
    let slope = if variance_x <= EPSILON {
        1.0
    } else {
        x.iter()
            .zip(&y)
            .map(|(first, second)| (first - mean_x) * (second - mean_y))
            .sum::<f64>()
            / variance_x
    };
    let intercept = mean_y - slope * mean_x;
    let residuals = x
        .iter()
        .zip(&y)
        .map(|(first, second)| second - (intercept + slope * first))
        .collect::<Vec<_>>();
    let residual_sum = residuals.iter().map(|value| value * value).sum::<f64>();
    let total_sum = y.iter().map(|value| (value - mean_y).powi(2)).sum::<f64>();
    FidelityCalibration {
        objective_index: objective,
        paired_samples: paired.len(),
        intercept,
        slope,
        residual_standard_deviation: (residual_sum
            / usize_as_f64(paired.len().saturating_sub(1).max(1)))
        .sqrt()
        .max(minimum),
        r_squared: if total_sum <= EPSILON {
            1.0
        } else {
            (1.0 - residual_sum / total_sum).clamp(-1.0, 1.0)
        },
    }
}

fn transform_objectives(values: &[f64], directions: &[ObjectiveDirection]) -> Vec<f64> {
    values
        .iter()
        .zip(directions)
        .map(|(value, direction)| {
            if matches!(direction, ObjectiveDirection::Minimize) {
                *value
            } else {
                -*value
            }
        })
        .collect()
}

fn hypervolume_min_2d(points: &[Vec<f64>], reference: &[f64]) -> f64 {
    if reference.len() != 2 {
        return 0.0;
    }
    let mut valid = points
        .iter()
        .filter(|point| point.len() == 2 && point[0] < reference[0] && point[1] < reference[1])
        .map(|point| [point[0], point[1]])
        .collect::<Vec<_>>();
    valid.sort_by(|a, b| a[0].total_cmp(&b[0]).then_with(|| a[1].total_cmp(&b[1])));
    let mut best_y = reference[1];
    let mut area = 0.0;
    for point in valid {
        if point[1] < best_y {
            area = (reference[0] - point[0])
                .max(0.0)
                .mul_add(best_y - point[1], area);
            best_y = point[1];
        }
    }
    area.max(0.0)
}

fn standard_normal(random: &mut SplitMix64) -> f64 {
    let first = random.unit().max(f64::MIN_POSITIVE);
    let second = random.unit();
    (-2.0 * first.ln()).sqrt() * (std::f64::consts::TAU * second).cos()
}

fn hash_value<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&serde_json::to_vec(value).expect("serializable evidence contract"));
    hasher.finalize().to_hex().to_string()
}

/// Evidence/sensitivity/multi-fidelity contract failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceError {
    /// Passport fields are missing or mutually inconsistent.
    InvalidPassport,
    /// Stored content hash does not match the payload.
    HashMismatch,
    /// Sensitivity variables, layout, or sampling policy is invalid.
    InvalidDesign,
    /// Sensitivity outputs do not align with the design.
    InvalidOutputs,
    /// A normalized sensitivity estimator received a constant output.
    ZeroVariance,
    /// Scenario dimensions, weights, or values are invalid.
    InvalidScenario,
    /// Uncertainty dimensions, deviations, or values are invalid.
    InvalidUncertainty,
    /// Multi-fidelity observations or policy are invalid.
    InvalidFidelity,
    /// An observation references a candidate outside the registry.
    UnknownCandidate,
    /// A candidate has multiple observations at one fidelity.
    DuplicateObservation,
    /// Fewer than two aligned Fast/reference pairs are available.
    InsufficientPairs,
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPassport => "evidence passport fields are incomplete or inconsistent",
            Self::HashMismatch => "evidence content hash does not match its payload",
            Self::InvalidDesign => "global sensitivity design is invalid or inconsistent",
            Self::InvalidOutputs => {
                "sensitivity outputs are non-finite or not aligned to the design"
            }
            Self::ZeroVariance => "sensitivity output variance is zero",
            Self::InvalidScenario => "robust scenario outcomes are invalid",
            Self::InvalidUncertainty => "uncertainty vectors are invalid",
            Self::InvalidFidelity => "multi-fidelity policy or observations are invalid",
            Self::UnknownCandidate => "fidelity observation references an unknown candidate",
            Self::DuplicateObservation => "candidate has duplicate observations at one fidelity",
            Self::InsufficientPairs => "at least two aligned Fast/reference pairs are required",
        })
    }
}

impl std::error::Error for EvidenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableKind;

    fn variables() -> Vec<VariableSpec> {
        vec![
            VariableSpec::try_new(1, VariableKind::Continuous, 0.0, 1.0).unwrap(),
            VariableSpec::try_new(2, VariableKind::Continuous, 0.0, 1.0).unwrap(),
        ]
    }

    #[test]
    fn passport_detects_mutation() {
        let passport = EvidencePassport {
            schema_version: PASSPORT_SCHEMA.to_owned(),
            result_hash: "result".to_owned(),
            scene_hash: "scene".to_owned(),
            study_id: "study".to_owned(),
            variant_id: Some(1),
            method: "DAENA target view".to_owned(),
            fidelity: FidelityLevel::Fast,
            engine_version: "0.18.0".to_owned(),
            backend: "VAYU GPU".to_owned(),
            device: "test".to_owned(),
            driver: "1".to_owned(),
            generated_utc: "2026-09-03T00:00:00Z".to_owned(),
            seed: Some(42),
            sample_count: 128,
            convergence_delta: Some(0.01),
            elapsed_microseconds: 100,
            input_hashes: BTreeMap::from([("weather".to_owned(), "hash".to_owned())]),
            assumptions: vec!["single bounce".to_owned()],
            limitations: vec!["not compliance".to_owned()],
            citations: vec!["doi:test".to_owned()],
            validation: EvidenceValidation {
                parity_tested: true,
                maximum_parity_delta: Some(0.0),
                reference_tested: false,
                reference_method: String::new(),
                maximum_reference_error: None,
                accepted_fraction: None,
            },
            content_hash: String::new(),
        }
        .seal()
        .unwrap();
        passport.validate().unwrap();
        let mut changed = passport;
        changed.sample_count += 1;
        assert_eq!(changed.validate(), Err(EvidenceError::HashMismatch));
    }

    #[test]
    fn generated_sensitivity_designs_recover_known_effects() {
        let sobol = saltelli_jansen_design(&variables(), 4096, 42).unwrap();
        let output = sobol
            .rows
            .iter()
            .map(|row| vec![2.0_f64.mul_add(row.values[1], row.values[0])])
            .collect::<Vec<_>>();
        let result = analyze_global_sensitivity(&sobol, &output).unwrap();
        let first = result
            .sobol
            .iter()
            .find(|value| value.parameter_index == 0)
            .unwrap();
        let second = result
            .sobol
            .iter()
            .find(|value| value.parameter_index == 1)
            .unwrap();
        assert!(
            (first.first_order - 0.2).abs() < 0.06,
            "{}",
            first.first_order
        );
        assert!(
            (second.first_order - 0.8).abs() < 0.06,
            "{}",
            second.first_order
        );
        assert!((first.total_order - 0.2).abs() < 0.04);
        assert!((second.total_order - 0.8).abs() < 0.04);

        let morris = morris_design(&variables(), 32, 6, 7).unwrap();
        let output = morris
            .rows
            .iter()
            .map(|row| vec![2.0_f64.mul_add(-row.values[1], 3.0 * row.values[0])])
            .collect::<Vec<_>>();
        let result = analyze_global_sensitivity(&morris, &output).unwrap();
        assert!((result.morris[0].mean - 3.0).abs() < 1.0e-12);
        assert!((result.morris[1].mean + 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn robust_and_interval_ranking_are_direction_aware() {
        let summaries = aggregate_robust_scenarios(
            &[ObjectiveDirection::Minimize],
            1,
            &[
                ScenarioOutcome {
                    candidate_id: 1,
                    scenario_id: 1,
                    probability: 0.8,
                    objectives: vec![1.0],
                    constraint_residuals: vec![-1.0],
                },
                ScenarioOutcome {
                    candidate_id: 1,
                    scenario_id: 2,
                    probability: 0.2,
                    objectives: vec![5.0],
                    constraint_residuals: vec![1.0],
                },
            ],
            0.8,
        )
        .unwrap();
        assert!((summaries[0].mean_objectives[0] - 1.8).abs() < 1.0e-12);
        assert!((summaries[0].cvar_objectives[0] - 5.0).abs() < 1.0e-12);
        assert!((summaries[0].violation_probability - 0.2).abs() < 1.0e-12);

        let ranks = uncertainty_aware_ranking(
            &[ObjectiveDirection::Minimize],
            0,
            &[
                UncertainEvaluation {
                    candidate_id: 1,
                    objective_means: vec![1.0],
                    objective_standard_deviations: vec![0.1],
                    constraint_means: vec![],
                    constraint_standard_deviations: vec![],
                },
                UncertainEvaluation {
                    candidate_id: 2,
                    objective_means: vec![2.0],
                    objective_standard_deviations: vec![0.1],
                    constraint_means: vec![],
                    constraint_standard_deviations: vec![],
                },
            ],
            1.96,
        )
        .unwrap();
        assert_eq!(ranks[0].robust_pareto_rank, 0);
        assert_eq!(ranks[1].robust_pareto_rank, 1);
    }

    #[test]
    fn multi_fidelity_selection_calibrates_and_skips_existing_reference() {
        let candidates = (1_u32..=5)
            .map(|id| Candidate {
                candidate_id: u64::from(id),
                generation: 0,
                values: vec![f64::from(id)],
            })
            .collect::<Vec<_>>();
        let mut observations = Vec::new();
        for id in 1_u32..=5 {
            observations.push(FidelityObservation {
                candidate_id: u64::from(id),
                fidelity: FidelityLevel::Fast,
                objectives: vec![f64::from(id), 6.0 - f64::from(id)],
                cost: 1.0,
                noise_standard_deviations: vec![0.01, 0.01],
                evidence_hash: format!("fast-{id}"),
            });
        }
        for id in 1_u32..=3 {
            observations.push(FidelityObservation {
                candidate_id: u64::from(id),
                fidelity: FidelityLevel::Reference,
                objectives: vec![
                    2.0_f64.mul_add(f64::from(id), 1.0),
                    2.0_f64.mul_add(6.0 - f64::from(id), 1.0),
                ],
                cost: 50.0,
                noise_standard_deviations: vec![0.0, 0.0],
                evidence_hash: format!("reference-{id}"),
            });
        }
        let result = recommend_reference_evaluations(
            &[ObjectiveDirection::Minimize, ObjectiveDirection::Minimize],
            &candidates,
            &observations,
            &MultiFidelityPolicy {
                batch_size: 2,
                monte_carlo_samples: 128,
                seed: 9,
                reference_point: vec![20.0, 20.0],
                minimum_standard_deviation: 0.01,
            },
        )
        .unwrap();
        assert_eq!(result.recommendations.len(), 2);
        assert!(
            result
                .recommendations
                .iter()
                .all(|row| row.candidate_id > 3)
        );
        assert!((result.calibrations[0].slope - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn tracked_json_schemas_accept_native_documents() {
        let passport_schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/evidence-passport.schema.json"
        ))
        .unwrap();
        let passport_validator = jsonschema::validator_for(&passport_schema).unwrap();
        let passport: EvidencePassport =
            serde_json::from_str(include_str!("../../../validation/evidence/passport.json"))
                .unwrap();
        let sealed = passport.seal().unwrap();
        let value = serde_json::to_value(sealed).unwrap();
        assert!(passport_validator.is_valid(&value));

        let design_schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/global-sensitivity-design.schema.json"
        ))
        .unwrap();
        let design_validator = jsonschema::validator_for(&design_schema).unwrap();
        let design = morris_design(&variables(), 4, 6, 9).unwrap();
        let value = serde_json::to_value(design).unwrap();
        assert!(design_validator.is_valid(&value));
    }

    #[test]
    fn sensitivity_design_hash_survives_pretty_json_round_trip() {
        let design = saltelli_jansen_design(&variables(), 4096, 42).unwrap();
        let encoded = serde_json::to_string_pretty(&design).unwrap();
        let decoded: GlobalSensitivityDesign = serde_json::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, design);
    }
}
