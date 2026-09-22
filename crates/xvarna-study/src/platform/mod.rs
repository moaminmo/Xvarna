//! Versioned Study/Optimization platform contracts and analysis orchestration.

mod export;
mod sensitivity;
mod workspace;

pub use export::{ReportArtifacts, write_report_package};
pub use sensitivity::{SensitivityOptions, StatisticalSensitivity, statistical_sensitivity};
pub use workspace::{
    BatchEnvelope, BatchEvaluation, BatchStatus, ResumeState, StudyWorkspace, WorkspaceCheckpoint,
};

use crate::{
    Candidate, Evaluation, ObjectiveDirection, OptimizerConfig, ProblemSpec, RankedSolution,
    StudyError, VariableKind, VariableSpec, rank_study,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Current version of the portable study manifest, ledger, and report data model.
pub const STUDY_PLATFORM_SCHEMA_VERSION: &str = "0.16.0";

/// Human-readable definition of one registered design parameter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterDefinition {
    /// Stable numeric ID used by the native optimizer.
    pub parameter_id: u64,
    /// Stable machine name used in tables and Parquet columns.
    pub name: String,
    /// Continuous, integer, or categorical representation.
    pub kind: VariableKind,
    /// Inclusive numeric lower bound.
    pub lower_bound: f64,
    /// Inclusive numeric upper bound.
    pub upper_bound: f64,
    /// Display/engineering unit; empty means dimensionless.
    #[serde(default)]
    pub unit: String,
    /// Labels for categorical levels zero through `upper_bound`.
    #[serde(default)]
    pub categories: Vec<String>,
}

/// Scalar or componentwise-vector objective registered in the manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectiveDefinition {
    /// Stable objective ID.
    pub objective_id: u64,
    /// Stable machine/display name.
    pub name: String,
    /// Minimize or maximize every scalar component.
    pub direction: ObjectiveDirection,
    /// Display/engineering unit.
    #[serde(default)]
    pub unit: String,
    /// Empty means scalar; one or more names define a vector evaluated componentwise.
    #[serde(default)]
    pub components: Vec<String>,
}

impl ObjectiveDefinition {
    /// Number of scalar columns contributed to non-dominated sorting.
    #[must_use]
    pub fn scalar_count(&self) -> usize {
        self.components.len().max(1)
    }

    /// Stable flattened column names in objective order.
    #[must_use]
    pub fn column_names(&self) -> Vec<String> {
        if self.components.is_empty() {
            vec![self.name.clone()]
        } else {
            self.components
                .iter()
                .map(|component| format!("{}.{}", self.name, component))
                .collect()
        }
    }
}

/// Named residual constraint; values are feasible when non-positive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintDefinition {
    /// Stable constraint ID.
    pub constraint_id: u64,
    /// Stable machine/display name.
    pub name: String,
    /// Display/engineering unit.
    #[serde(default)]
    pub unit: String,
}

/// Versioned root contract for a reproducible Study workspace.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyManifest {
    /// Must equal [`STUDY_PLATFORM_SCHEMA_VERSION`].
    pub schema_version: String,
    /// Stable user/project study identity.
    pub study_id: String,
    /// Human-readable title.
    pub title: String,
    /// Scientific/design intent and scope.
    #[serde(default)]
    pub description: String,
    /// UTC creation timestamp in RFC 3339 form.
    pub created_utc: String,
    /// XVARNA engine version that created the study.
    pub engine_version: String,
    /// Ordered parameter registry.
    pub parameters: Vec<ParameterDefinition>,
    /// Ordered scalar/vector objective registry.
    pub objectives: Vec<ObjectiveDefinition>,
    /// Ordered residual constraints.
    #[serde(default)]
    pub constraints: Vec<ConstraintDefinition>,
    /// Optional baseline variant for scenario deltas.
    #[serde(default)]
    pub baseline_variant_id: Option<u64>,
    /// Deterministic optimizer policy.
    pub optimizer: OptimizerConfig,
    /// Project-specific string metadata preserved in reports.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl StudyManifest {
    /// Validates registries and converts them into the native flattened problem contract.
    pub fn problem(&self) -> Result<ProblemSpec, PlatformError> {
        self.validate()?;
        let variables = self
            .parameters
            .iter()
            .map(|parameter| {
                VariableSpec::try_new(
                    parameter.parameter_id,
                    parameter.kind,
                    parameter.lower_bound,
                    parameter.upper_bound,
                )
                .map_err(PlatformError::Study)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let directions = self
            .objectives
            .iter()
            .flat_map(|objective| {
                std::iter::repeat_n(objective.direction, objective.scalar_count())
            })
            .collect();
        ProblemSpec::try_new(variables, directions, self.constraints.len())
            .map_err(PlatformError::Study)
    }

    /// Returns ordered flattened objective column names.
    #[must_use]
    pub fn objective_column_names(&self) -> Vec<String> {
        self.objectives
            .iter()
            .flat_map(ObjectiveDefinition::column_names)
            .collect()
    }

    /// Enforces schema version, identifiers, names, bounds, and vector/category dimensions.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if self.schema_version != STUDY_PLATFORM_SCHEMA_VERSION
            || self.study_id.trim().is_empty()
            || self.title.trim().is_empty()
            || self.created_utc.trim().is_empty()
            || self.engine_version.trim().is_empty()
            || self.parameters.is_empty()
            || self.objectives.is_empty()
        {
            return Err(PlatformError::InvalidManifest);
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for parameter in &self.parameters {
            if parameter.parameter_id == 0
                || !ids.insert((0_u8, parameter.parameter_id))
                || parameter.name.trim().is_empty()
                || !names.insert((0_u8, parameter.name.to_ascii_lowercase()))
            {
                return Err(PlatformError::InvalidManifest);
            }
            VariableSpec::try_new(
                parameter.parameter_id,
                parameter.kind,
                parameter.lower_bound,
                parameter.upper_bound,
            )?;
            if matches!(parameter.kind, VariableKind::Categorical) {
                let expected =
                    usize::try_from(u64::from(crate::validated_u32(parameter.upper_bound)))
                        .unwrap_or(usize::MAX)
                        .saturating_add(1);
                if parameter.categories.len() != expected
                    || parameter
                        .categories
                        .iter()
                        .any(|value| value.trim().is_empty())
                {
                    return Err(PlatformError::InvalidManifest);
                }
            } else if !parameter.categories.is_empty() {
                return Err(PlatformError::InvalidManifest);
            }
        }
        for objective in &self.objectives {
            if objective.objective_id == 0
                || !ids.insert((1, objective.objective_id))
                || objective.name.trim().is_empty()
                || !names.insert((1, objective.name.to_ascii_lowercase()))
                || objective
                    .components
                    .iter()
                    .any(|value| value.trim().is_empty())
                || objective.components.len() > 32
            {
                return Err(PlatformError::InvalidManifest);
            }
            let component_names = objective
                .components
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect::<BTreeSet<_>>();
            if component_names.len() != objective.components.len() {
                return Err(PlatformError::InvalidManifest);
            }
        }
        for constraint in &self.constraints {
            if constraint.constraint_id == 0
                || !ids.insert((2, constraint.constraint_id))
                || constraint.name.trim().is_empty()
                || !names.insert((2, constraint.name.to_ascii_lowercase()))
            {
                return Err(PlatformError::InvalidManifest);
            }
        }
        if self.objective_column_names().len() > 32 {
            return Err(PlatformError::InvalidManifest);
        }
        Ok(())
    }
}

/// One registered, reproducible design variant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariantRecord {
    /// Stable variant/candidate ID.
    pub variant_id: u64,
    /// Optimizer generation, or zero for imported designs.
    pub generation: u64,
    /// Human-readable label.
    #[serde(default)]
    pub label: String,
    /// Ordered values matching the manifest parameter registry.
    pub parameters: Vec<f64>,
    /// Optional parent/source variant.
    #[serde(default)]
    pub parent_variant_id: Option<u64>,
    /// Optional path/URI to associated model geometry.
    #[serde(default)]
    pub asset_uri: Option<String>,
    /// Searchable project tags.
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

/// A spatial or indexed metric field suitable for scenario delta maps.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricField {
    /// Stable field identity, for example `daylight.totalLux`.
    pub field_id: String,
    /// Display unit.
    #[serde(default)]
    pub unit: String,
    /// Optional world-space positions aligned with values.
    #[serde(default)]
    pub positions: Vec<[f64; 3]>,
    /// Scalar values aligned by sensor/cell index.
    pub values: Vec<f64>,
}

/// Objective, constraint, and field results for one completed variant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationRecord {
    /// Evaluated variant ID.
    pub variant_id: u64,
    /// One vector per manifest objective; scalar objectives contain one value.
    pub objective_values: Vec<Vec<f64>>,
    /// Residual constraints aligned with the manifest; feasible when all are non-positive.
    #[serde(default)]
    pub constraint_residuals: Vec<f64>,
    /// Optional spatial/indexed result fields for delta-map comparison.
    #[serde(default)]
    pub metric_fields: Vec<MetricField>,
    /// Evaluation runtime in microseconds.
    #[serde(default)]
    pub elapsed_microseconds: u64,
    /// Stable upstream geometry/analysis identity.
    #[serde(default)]
    pub provenance_hash: String,
}

impl EvaluationRecord {
    fn flatten_objectives(&self, manifest: &StudyManifest) -> Result<Vec<f64>, PlatformError> {
        if self.objective_values.len() != manifest.objectives.len() {
            return Err(PlatformError::InvalidLedger);
        }
        let mut flattened = Vec::with_capacity(manifest.objective_column_names().len());
        for (values, objective) in self.objective_values.iter().zip(&manifest.objectives) {
            if values.len() != objective.scalar_count()
                || values.iter().any(|value| !value.is_finite())
            {
                return Err(PlatformError::InvalidLedger);
            }
            flattened.extend(values);
        }
        Ok(flattened)
    }
}

/// Append-only logical table of variants and their completed evaluations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyLedger {
    /// Ledger schema version.
    pub schema_version: String,
    /// Monotonic transaction revision.
    pub revision: u64,
    /// Registered variants in creation order.
    pub variants: Vec<VariantRecord>,
    /// Completed evaluations; pending variants have no row here.
    pub evaluations: Vec<EvaluationRecord>,
    /// Stable hash excluding this field.
    pub content_hash: String,
}

impl StudyLedger {
    /// Creates an empty, hash-protected ledger.
    #[must_use]
    pub fn empty() -> Self {
        let mut value = Self {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            revision: 0,
            variants: Vec::new(),
            evaluations: Vec::new(),
            content_hash: String::new(),
        };
        value.refresh_hash();
        value
    }

    /// Validates referential integrity, rectangular dimensions, finiteness, and hash identity.
    pub fn validate(&self, manifest: &StudyManifest) -> Result<(), PlatformError> {
        if self.schema_version != STUDY_PLATFORM_SCHEMA_VERSION
            || self.expected_hash() != self.content_hash
        {
            return Err(PlatformError::CorruptWorkspace);
        }
        let problem = manifest.problem()?;
        let mut ids = BTreeSet::new();
        for variant in &self.variants {
            let candidate = Candidate {
                candidate_id: variant.variant_id,
                generation: variant.generation,
                values: variant.parameters.clone(),
            };
            problem.validate_candidate(&candidate)?;
            if variant.variant_id == 0 || !ids.insert(variant.variant_id) {
                return Err(PlatformError::InvalidLedger);
            }
        }
        let mut evaluated = BTreeSet::new();
        for evaluation in &self.evaluations {
            if !ids.contains(&evaluation.variant_id) || !evaluated.insert(evaluation.variant_id) {
                return Err(PlatformError::InvalidLedger);
            }
            let flattened = evaluation.flatten_objectives(manifest)?;
            problem.validate_evaluation(&Evaluation {
                candidate_id: evaluation.variant_id,
                objectives: flattened,
                constraint_residuals: evaluation.constraint_residuals.clone(),
            })?;
            for field in &evaluation.metric_fields {
                if field.field_id.trim().is_empty()
                    || field.values.is_empty()
                    || field.values.iter().any(|value| !value.is_finite())
                    || !field.positions.is_empty() && field.positions.len() != field.values.len()
                    || field
                        .positions
                        .iter()
                        .flatten()
                        .any(|value| !value.is_finite())
                {
                    return Err(PlatformError::InvalidLedger);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn refresh_hash(&mut self) {
        self.content_hash = self.expected_hash();
    }

    fn expected_hash(&self) -> String {
        let mut clone = self.clone();
        clone.content_hash.clear();
        hash_serializable(b"XVARNA_STUDY_LEDGER_V1\0", &clone)
    }
}

/// Report-friendly ranking row aligned with the manifest and ledger.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedVariant {
    /// Variant ID.
    pub variant_id: u64,
    /// Constraint-aware Pareto front.
    pub pareto_rank: usize,
    /// True when every registered constraint is satisfied.
    pub feasible: bool,
    /// Sum of positive residuals.
    pub total_constraint_violation: f64,
    /// NSGA-II crowding distance; JSON uses `null` for infinity.
    pub crowding_distance: Option<f64>,
    /// Number of directly dominated variants.
    pub dominated_variant_count: usize,
}

impl From<RankedSolution> for RankedVariant {
    fn from(value: RankedSolution) -> Self {
        Self {
            variant_id: value.candidate_id,
            pareto_rank: value.pareto_rank,
            feasible: value.feasible,
            total_constraint_violation: value.total_constraint_violation,
            crowding_distance: value
                .crowding_distance
                .is_finite()
                .then_some(value.crowding_distance),
            dominated_variant_count: value.dominated_solution_count,
        }
    }
}

/// One baseline-to-variant scalar field delta map.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaMap {
    /// Metric field identity.
    pub field_id: String,
    /// Display unit.
    pub unit: String,
    /// Current minus baseline values.
    pub values: Vec<f64>,
    /// Optional aligned positions.
    pub positions: Vec<[f64; 3]>,
    /// Minimum delta.
    pub minimum: f64,
    /// Arithmetic mean delta.
    pub mean: f64,
    /// Maximum delta.
    pub maximum: f64,
    /// Root-mean-square delta.
    pub root_mean_square: f64,
}

/// Baseline-relative comparison for one variant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioComparison {
    /// Baseline variant ID.
    pub baseline_variant_id: u64,
    /// Compared variant ID.
    pub variant_id: u64,
    /// Parameter vector delta, current minus baseline.
    pub parameter_delta: Vec<f64>,
    /// Grouped scalar/vector objective delta, current minus baseline.
    pub objective_delta: Vec<Vec<f64>>,
    /// Compatible metric field delta maps.
    pub delta_maps: Vec<DeltaMap>,
}

/// Complete derived analysis used by JSON, HTML, Parquet, glTF, and clients.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyAnalysis {
    /// Ranked evaluated variants.
    pub ranked_variants: Vec<RankedVariant>,
    /// Feasible non-dominated variant IDs.
    pub pareto_variant_ids: Vec<u64>,
    /// Number of non-dominated fronts.
    pub front_count: usize,
    /// Bootstrap/permutation rank sensitivity.
    pub sensitivity: Vec<StatisticalSensitivity>,
    /// Baseline-relative objective/parameter/field changes.
    pub comparisons: Vec<ScenarioComparison>,
    /// Stable derived-result identity.
    pub content_hash: String,
}

/// Runs ranking, statistically qualified sensitivity, and scenario/delta-map comparison.
pub fn analyze_platform_study(
    manifest: &StudyManifest,
    ledger: &StudyLedger,
    sensitivity_options: SensitivityOptions,
) -> Result<StudyAnalysis, PlatformError> {
    ledger.validate(manifest)?;
    let problem = manifest.problem()?;
    let (candidates, evaluations) = completed_core_tables(manifest, ledger)?;
    let ranked = rank_study(&problem, &candidates, &evaluations)?;
    let sensitivity = statistical_sensitivity(
        &problem,
        &candidates,
        &evaluations,
        &manifest.objective_column_names(),
        sensitivity_options,
    )?;
    let comparisons = compare_scenarios(manifest, ledger)?;
    let mut result = StudyAnalysis {
        ranked_variants: ranked.solutions.into_iter().map(Into::into).collect(),
        pareto_variant_ids: ranked.pareto_candidate_ids,
        front_count: ranked.front_count,
        sensitivity,
        comparisons,
        content_hash: String::new(),
    };
    result.content_hash = hash_serializable(b"XVARNA_STUDY_ANALYSIS_V1\0", &result);
    Ok(result)
}

pub(crate) fn completed_core_tables(
    manifest: &StudyManifest,
    ledger: &StudyLedger,
) -> Result<(Vec<Candidate>, Vec<Evaluation>), PlatformError> {
    let variant_map = ledger
        .variants
        .iter()
        .map(|variant| (variant.variant_id, variant))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = Vec::with_capacity(ledger.evaluations.len());
    let mut evaluations = Vec::with_capacity(ledger.evaluations.len());
    for evaluation in &ledger.evaluations {
        let variant = variant_map
            .get(&evaluation.variant_id)
            .ok_or(PlatformError::InvalidLedger)?;
        candidates.push(Candidate {
            candidate_id: variant.variant_id,
            generation: variant.generation,
            values: variant.parameters.clone(),
        });
        evaluations.push(Evaluation {
            candidate_id: evaluation.variant_id,
            objectives: evaluation.flatten_objectives(manifest)?,
            constraint_residuals: evaluation.constraint_residuals.clone(),
        });
    }
    Ok((candidates, evaluations))
}

fn compare_scenarios(
    manifest: &StudyManifest,
    ledger: &StudyLedger,
) -> Result<Vec<ScenarioComparison>, PlatformError> {
    let Some(baseline_id) = manifest.baseline_variant_id else {
        return Ok(Vec::new());
    };
    let baseline_variant = ledger
        .variants
        .iter()
        .find(|value| value.variant_id == baseline_id)
        .ok_or(PlatformError::InvalidBaseline)?;
    let baseline_evaluation = ledger
        .evaluations
        .iter()
        .find(|value| value.variant_id == baseline_id)
        .ok_or(PlatformError::InvalidBaseline)?;
    let variants = ledger
        .variants
        .iter()
        .map(|value| (value.variant_id, value))
        .collect::<BTreeMap<_, _>>();
    let baseline_fields = baseline_evaluation
        .metric_fields
        .iter()
        .map(|value| (value.field_id.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut output = Vec::new();
    for evaluation in &ledger.evaluations {
        if evaluation.variant_id == baseline_id {
            continue;
        }
        let variant = variants
            .get(&evaluation.variant_id)
            .ok_or(PlatformError::InvalidLedger)?;
        let objective_delta = evaluation
            .objective_values
            .iter()
            .zip(&baseline_evaluation.objective_values)
            .map(|(current, baseline)| {
                current
                    .iter()
                    .zip(baseline)
                    .map(|(current, baseline)| current - baseline)
                    .collect()
            })
            .collect();
        let mut delta_maps = Vec::new();
        for field in &evaluation.metric_fields {
            let Some(baseline) = baseline_fields.get(field.field_id.as_str()) else {
                continue;
            };
            if baseline.values.len() != field.values.len()
                || baseline.unit != field.unit
                || !baseline.positions.is_empty() && baseline.positions != field.positions
            {
                return Err(PlatformError::IncompatibleDeltaMap);
            }
            let values = field
                .values
                .iter()
                .zip(&baseline.values)
                .map(|(current, baseline)| current - baseline)
                .collect::<Vec<_>>();
            let minimum = values.iter().copied().reduce(f64::min).unwrap_or(0.0);
            let maximum = values.iter().copied().reduce(f64::max).unwrap_or(0.0);
            let count = u32::try_from(values.len()).map_or_else(|_| f64::from(u32::MAX), f64::from);
            let mean = values.iter().sum::<f64>() / count;
            let root_mean_square =
                (values.iter().map(|value| value * value).sum::<f64>() / count).sqrt();
            delta_maps.push(DeltaMap {
                field_id: field.field_id.clone(),
                unit: field.unit.clone(),
                values,
                positions: field.positions.clone(),
                minimum,
                mean,
                maximum,
                root_mean_square,
            });
        }
        output.push(ScenarioComparison {
            baseline_variant_id: baseline_id,
            variant_id: evaluation.variant_id,
            parameter_delta: variant
                .parameters
                .iter()
                .zip(&baseline_variant.parameters)
                .map(|(current, baseline)| current - baseline)
                .collect(),
            objective_delta,
            delta_maps,
        });
    }
    Ok(output)
}

/// Draft 2020-12 JSON Schema for [`StudyManifest`].
#[must_use]
pub fn study_manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://xvarna.dev/schema/study-manifest-0.16.0.json",
        "title": "XVARNA Study Manifest 0.16.0",
        "type": "object",
        "additionalProperties": false,
        "required": ["schemaVersion", "studyId", "title", "createdUtc", "engineVersion", "parameters", "objectives", "optimizer"],
        "properties": {
            "schemaVersion": { "const": STUDY_PLATFORM_SCHEMA_VERSION },
            "studyId": { "type": "string", "minLength": 1 },
            "title": { "type": "string", "minLength": 1 },
            "description": { "type": "string" },
            "createdUtc": { "type": "string", "format": "date-time" },
            "engineVersion": { "type": "string", "minLength": 1 },
            "parameters": { "type": "array", "minItems": 1, "maxItems": 256, "items": { "$ref": "#/$defs/parameter" } },
            "objectives": { "type": "array", "minItems": 1, "maxItems": 32, "items": { "$ref": "#/$defs/objective" } },
            "constraints": { "type": "array", "maxItems": 256, "items": { "$ref": "#/$defs/constraint" } },
            "baselineVariantId": { "type": ["integer", "null"], "minimum": 1 },
            "optimizer": { "$ref": "#/$defs/optimizer" },
            "metadata": { "type": "object", "additionalProperties": { "type": "string" } }
        },
        "$defs": {
            "parameter": {
                "type": "object", "additionalProperties": false,
                "required": ["parameterId", "name", "kind", "lowerBound", "upperBound"],
                "properties": {
                    "parameterId": { "type": "integer", "minimum": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "kind": { "enum": ["continuous", "integer", "categorical"] },
                    "lowerBound": { "type": "number" }, "upperBound": { "type": "number" },
                    "unit": { "type": "string" },
                    "categories": { "type": "array", "items": { "type": "string", "minLength": 1 } }
                }
            },
            "objective": {
                "type": "object", "additionalProperties": false,
                "required": ["objectiveId", "name", "direction"],
                "properties": {
                    "objectiveId": { "type": "integer", "minimum": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "direction": { "enum": ["minimize", "maximize"] },
                    "unit": { "type": "string" },
                    "components": { "type": "array", "maxItems": 32, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } }
                }
            },
            "constraint": {
                "type": "object", "additionalProperties": false,
                "required": ["constraintId", "name"],
                "properties": { "constraintId": { "type": "integer", "minimum": 1 }, "name": { "type": "string", "minLength": 1 }, "unit": { "type": "string" } }
            },
            "optimizer": {
                "type": "object", "additionalProperties": false,
                "required": ["populationSize", "offspringSize", "archiveCapacity", "seed", "crossoverProbability", "mutationProbability", "crossoverDistributionIndex", "mutationDistributionIndex"],
                "properties": {
                    "populationSize": { "type": "integer", "minimum": 4, "maximum": 4096 },
                    "offspringSize": { "type": "integer", "minimum": 1, "maximum": 4096 },
                    "archiveCapacity": { "type": "integer", "minimum": 1, "maximum": 4096 },
                    "seed": { "type": "integer", "minimum": 0 },
                    "crossoverProbability": { "type": "number", "minimum": 0, "maximum": 1 },
                    "mutationProbability": { "type": "number", "minimum": 0, "maximum": 1 },
                    "crossoverDistributionIndex": { "type": "number", "exclusiveMinimum": 0 },
                    "mutationDistributionIndex": { "type": "number", "exclusiveMinimum": 0 }
                }
            }
        }
    })
}

pub(crate) fn hash_serializable<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&serde_json::to_vec(value).expect("serializable platform value"));
    hasher.finalize().to_hex().to_string()
}

/// Persistence, schema, analysis, or export failure in the 0.16 platform.
#[derive(Debug)]
pub enum PlatformError {
    /// Manifest registry or schema-version contract is invalid.
    InvalidManifest,
    /// Variant/evaluation ledger is inconsistent.
    InvalidLedger,
    /// Selected baseline is missing or unevaluated.
    InvalidBaseline,
    /// Baseline/current metric fields cannot be aligned exactly.
    IncompatibleDeltaMap,
    /// Workspace hash, transaction, or checkpoint is corrupt.
    CorruptWorkspace,
    /// A workspace already exists at the requested path.
    WorkspaceExists,
    /// No initialized workspace exists at the requested path.
    WorkspaceMissing,
    /// Batch/evaluation state transition is not valid or not idempotent.
    InvalidBatchState,
    /// Underlying deterministic study engine rejected the data.
    Study(StudyError),
    /// Filesystem operation failed.
    Io(std::io::Error),
    /// JSON serialization or parsing failed.
    Json(serde_json::Error),
    /// Parquet/Arrow export failed.
    Parquet(String),
}

impl From<StudyError> for PlatformError {
    fn from(value: StudyError) -> Self {
        Self::Study(value)
    }
}

impl From<std::io::Error> for PlatformError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for PlatformError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl core::fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidManifest => {
                formatter.write_str("study manifest is invalid or incompatible")
            }
            Self::InvalidLedger => {
                formatter.write_str("study variant/evaluation ledger is invalid")
            }
            Self::InvalidBaseline => {
                formatter.write_str("study baseline is missing or unevaluated")
            }
            Self::IncompatibleDeltaMap => {
                formatter.write_str("scenario metric fields cannot be aligned")
            }
            Self::CorruptWorkspace => {
                formatter.write_str("study workspace is corrupt or partially written")
            }
            Self::WorkspaceExists => formatter.write_str("study workspace already exists"),
            Self::WorkspaceMissing => formatter.write_str("study workspace is not initialized"),
            Self::InvalidBatchState => formatter.write_str("study batch transition is invalid"),
            Self::Study(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "study I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "study JSON failed: {error}"),
            Self::Parquet(error) => write!(formatter, "study Parquet failed: {error}"),
        }
    }
}

impl std::error::Error for PlatformError {}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::file::reader::{FileReader, SerializedFileReader};
    use std::{fs, fs::File, time::SystemTime};

    fn manifest() -> StudyManifest {
        StudyManifest {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            study_id: "courtyard-search".to_owned(),
            title: "Courtyard daylight and view".to_owned(),
            description: "Analytic platform fixture".to_owned(),
            created_utc: "2026-09-02T12:00:00Z".to_owned(),
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            parameters: vec![
                ParameterDefinition {
                    parameter_id: 1,
                    name: "depth".to_owned(),
                    kind: VariableKind::Continuous,
                    lower_bound: 0.0,
                    upper_bound: 1.0,
                    unit: "m".to_owned(),
                    categories: Vec::new(),
                },
                ParameterDefinition {
                    parameter_id: 2,
                    name: "facade".to_owned(),
                    kind: VariableKind::Categorical,
                    lower_bound: 0.0,
                    upper_bound: 1.0,
                    unit: String::new(),
                    categories: vec!["clear".to_owned(), "frit".to_owned()],
                },
            ],
            objectives: vec![
                ObjectiveDefinition {
                    objective_id: 1,
                    name: "daylight".to_owned(),
                    direction: ObjectiveDirection::Maximize,
                    unit: "%".to_owned(),
                    components: vec!["sDA".to_owned(), "UDI".to_owned()],
                },
                ObjectiveDefinition {
                    objective_id: 2,
                    name: "energy".to_owned(),
                    direction: ObjectiveDirection::Minimize,
                    unit: "kWh".to_owned(),
                    components: Vec::new(),
                },
            ],
            constraints: vec![ConstraintDefinition {
                constraint_id: 1,
                name: "ase-limit".to_owned(),
                unit: "%".to_owned(),
            }],
            baseline_variant_id: Some(1),
            optimizer: OptimizerConfig {
                population_size: 4,
                offspring_size: 4,
                archive_capacity: 16,
                seed: 17,
                ..OptimizerConfig::default()
            },
            metadata: BTreeMap::from([("author".to_owned(), "XVARNA".to_owned())]),
        }
    }

    fn result(candidate: &Candidate) -> BatchEvaluation {
        let depth = candidate.values[0];
        let facade = candidate.values[1];
        BatchEvaluation {
            variant_id: candidate.candidate_id,
            objective_values: vec![
                vec![depth.mul_add(40.0, 50.0), facade.mul_add(10.0, 60.0)],
                vec![facade.mul_add(5.0, depth.mul_add(-30.0, 100.0))],
            ],
            constraint_residuals: vec![depth - 0.85],
            metric_fields: vec![MetricField {
                field_id: "daylight.totalLux".to_owned(),
                unit: "lux".to_owned(),
                positions: vec![[0.0, 0.0, 0.8], [1.0, 0.0, 0.8]],
                values: vec![depth * 1000.0, facade.mul_add(100.0, 500.0)],
            }],
            elapsed_microseconds: 10,
            provenance_hash: format!("hash-{}", candidate.candidate_id),
        }
    }

    fn temporary_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "xvarna-study-platform-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn manifest_schema_is_valid_and_accepts_the_typed_contract() {
        let schema = study_manifest_schema();
        assert!(jsonschema::meta::validate(&schema).is_ok());
        let instance = serde_json::to_value(manifest()).expect("manifest json");
        assert!(jsonschema::validate(&schema, &instance).is_ok());
        assert_eq!(
            manifest()
                .problem()
                .expect("problem")
                .objective_directions
                .len(),
            3
        );
    }

    #[test]
    fn workspace_batch_resume_report_parquet_and_gltf_are_real() {
        let root = temporary_directory();
        let mut workspace = StudyWorkspace::create(&root, manifest()).expect("create");
        let first = workspace.begin_batch().expect("first batch");
        assert_eq!(first.candidates.len(), 4);
        let transaction = json!({
            "ledger": serde_json::from_slice::<Value>(
                &fs::read(root.join("ledger.json")).expect("ledger bytes")
            ).expect("ledger json"),
            "checkpoint": serde_json::from_slice::<Value>(
                &fs::read(root.join("checkpoint.json")).expect("checkpoint bytes")
            ).expect("checkpoint json"),
            "batch": serde_json::from_slice::<Value>(
                &fs::read(root.join("batches").join(format!("batch-{:08}.json", first.batch_id)))
                    .expect("batch bytes")
            ).expect("batch json"),
        });
        fs::write(
            root.join("transaction.json"),
            serde_json::to_vec_pretty(&transaction).expect("transaction json"),
        )
        .expect("simulate transaction journal");
        fs::write(root.join("checkpoint.json"), b"partial write")
            .expect("simulate interrupted checkpoint write");
        let reopened = StudyWorkspace::open(&root).expect("resume pending");
        assert_eq!(reopened.resume_state().pending_candidates, first.candidates);
        assert!(!root.join("transaction.json").exists());
        workspace
            .commit_batch(
                first.batch_id,
                first.candidates.iter().map(result).collect(),
            )
            .expect("commit");
        let workspace = StudyWorkspace::open(&root).expect("reopen complete");
        let analysis = workspace
            .analyze(SensitivityOptions {
                bootstrap_samples: 64,
                permutation_samples: 64,
                ..SensitivityOptions::default()
            })
            .expect("analysis");
        assert_eq!(analysis.ranked_variants.len(), 4);
        assert_eq!(analysis.sensitivity.len(), 6);
        assert_eq!(analysis.comparisons.len(), 3);
        assert!(
            analysis
                .comparisons
                .iter()
                .all(|value| value.delta_maps.len() == 1)
        );
        let artifacts = write_report_package(
            root.join("report"),
            workspace.manifest(),
            workspace.ledger(),
            &analysis,
        )
        .expect("report");
        let reader =
            SerializedFileReader::new(File::open(&artifacts.variants_parquet).expect("parquet"))
                .expect("valid parquet");
        assert_eq!(reader.metadata().file_metadata().num_rows(), 4);
        let gltf: Value =
            serde_json::from_slice(&fs::read(&artifacts.objective_space_gltf).expect("gltf"))
                .expect("gltf json");
        assert_eq!(gltf["asset"]["version"], "2.0");
        assert_eq!(gltf["accessors"][0]["count"], 4);
        let html = fs::read_to_string(&artifacts.standalone_viewer).expect("viewer");
        assert!(html.contains("VAHMAN · XVARNA 0.16"));
        assert!(html.contains("courtyard-search"));
        fs::remove_dir_all(&root).expect("remove isolated test directory");
    }
}
