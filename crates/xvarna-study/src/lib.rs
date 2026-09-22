//! VAHMAN, XVARNA's deterministic constrained multi-objective Study engine.
//!
//! The optimizer follows an external-evaluation `ask`/`tell` contract suitable
//! for expensive Grasshopper and CLI simulations. It combines stratified
//! initialization, constraint-domination, NSGA-II non-dominated sorting,
//! diversity-preserving crowding, simulated-binary crossover, polynomial
//! mutation, mixed variables, and a bounded historical Pareto archive.

#![forbid(unsafe_code)]

use core::fmt;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod evidence;
pub mod platform;

const MAXIMUM_SOLUTIONS: usize = 4_096;
const MAXIMUM_VARIABLES: usize = 256;
const MAXIMUM_OBJECTIVES: usize = 32;
const MAXIMUM_CONSTRAINTS: usize = 256;
const DOMINANCE_EPSILON: f64 = 1.0e-12;

/// Optimization variable representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum VariableKind {
    /// Real-valued parameter.
    Continuous = 0,
    /// Integer parameter stored as an exactly rounded `f64` value.
    Integer = 1,
    /// Zero-based categorical level stored as an exactly rounded `f64` value.
    Categorical = 2,
}

/// One bounded design variable.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableSpec {
    /// Stable variable identifier.
    pub variable_id: u64,
    /// Continuous, integer, or categorical representation.
    pub kind: VariableKind,
    /// Inclusive lower bound. Categorical variables require zero.
    pub lower_bound: f64,
    /// Inclusive upper bound. For categorical variables this is `levels - 1`.
    pub upper_bound: f64,
}

impl VariableSpec {
    /// Creates a finite bounded variable contract.
    pub fn try_new(
        variable_id: u64,
        kind: VariableKind,
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<Self, StudyError> {
        if !lower_bound.is_finite()
            || !upper_bound.is_finite()
            || lower_bound >= upper_bound
            || matches!(kind, VariableKind::Categorical)
                && (lower_bound != 0.0
                    || upper_bound.fract() != 0.0
                    || upper_bound > f64::from(u32::MAX))
            || matches!(kind, VariableKind::Integer)
                && (lower_bound.fract() != 0.0 || upper_bound.fract() != 0.0)
        {
            return Err(StudyError::InvalidVariable);
        }
        Ok(Self {
            variable_id,
            kind,
            lower_bound,
            upper_bound,
        })
    }

    const fn canonicalize(self, value: f64) -> f64 {
        let clamped = value.clamp(self.lower_bound, self.upper_bound);
        match self.kind {
            VariableKind::Continuous => clamped,
            VariableKind::Integer | VariableKind::Categorical => clamped.round(),
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn validated_u32(value: f64) -> u32 {
    debug_assert!(value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX));
    debug_assert!(value.fract().abs() <= f64::EPSILON);
    value as u32
}

/// Optimization direction for one objective column.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum ObjectiveDirection {
    /// Smaller objective values are preferred.
    Minimize = 0,
    /// Larger objective values are preferred.
    Maximize = 1,
}

/// Immutable dimension contract shared by ranking and optimization.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemSpec {
    /// Ordered design variables.
    pub variables: Vec<VariableSpec>,
    /// Ordered objective directions.
    pub objective_directions: Vec<ObjectiveDirection>,
    /// Number of residual constraints; a residual is feasible when `<= 0`.
    pub constraint_count: usize,
}

impl ProblemSpec {
    /// Validates problem dimensions and unique variable IDs.
    pub fn try_new(
        variables: Vec<VariableSpec>,
        objective_directions: Vec<ObjectiveDirection>,
        constraint_count: usize,
    ) -> Result<Self, StudyError> {
        if variables.is_empty()
            || variables.len() > MAXIMUM_VARIABLES
            || objective_directions.is_empty()
            || objective_directions.len() > MAXIMUM_OBJECTIVES
            || constraint_count > MAXIMUM_CONSTRAINTS
        {
            return Err(StudyError::InvalidDimensions);
        }
        let ids = variables
            .iter()
            .map(|variable| variable.variable_id)
            .collect::<BTreeSet<_>>();
        if ids.len() != variables.len() {
            return Err(StudyError::DuplicateIdentifier);
        }
        Ok(Self {
            variables,
            objective_directions,
            constraint_count,
        })
    }

    fn validate_candidate(&self, candidate: &Candidate) -> Result<(), StudyError> {
        if candidate.values.len() != self.variables.len()
            || candidate.values.iter().any(|value| !value.is_finite())
            || candidate
                .values
                .iter()
                .zip(&self.variables)
                .any(|(value, variable)| {
                    *value < variable.lower_bound
                        || *value > variable.upper_bound
                        || !matches!(variable.kind, VariableKind::Continuous)
                            && value.fract() != 0.0
                })
        {
            return Err(StudyError::InvalidCandidate);
        }
        Ok(())
    }

    fn validate_evaluation(&self, evaluation: &Evaluation) -> Result<(), StudyError> {
        if evaluation.objectives.len() != self.objective_directions.len()
            || evaluation.constraint_residuals.len() != self.constraint_count
            || evaluation.objectives.iter().any(|value| !value.is_finite())
            || evaluation
                .constraint_residuals
                .iter()
                .any(|value| !value.is_finite())
        {
            return Err(StudyError::InvalidEvaluation);
        }
        Ok(())
    }
}

/// One generated or imported design variant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Stable candidate/variant identifier.
    pub candidate_id: u64,
    /// Generation that produced the candidate; imported studies may use zero.
    pub generation: u64,
    /// Ordered parameter values matching [`ProblemSpec::variables`].
    pub values: Vec<f64>,
}

/// Objective and residual-constraint values returned by an external evaluator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    /// Candidate identifier returned by `ask`.
    pub candidate_id: u64,
    /// Ordered objective vector.
    pub objectives: Vec<f64>,
    /// Ordered residual constraints; every value must be `<= 0` for feasibility.
    pub constraint_residuals: Vec<f64>,
}

/// Rank, feasibility, and diversity metadata for one evaluated variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankedSolution {
    /// Candidate identifier.
    pub candidate_id: u64,
    /// Zero-based non-dominated front.
    pub pareto_rank: usize,
    /// Whether every constraint residual is non-positive.
    pub feasible: bool,
    /// Sum of positive normalized residuals (raw residuals in this contract).
    pub total_constraint_violation: f64,
    /// NSGA-II crowding distance; boundary points use positive infinity.
    pub crowding_distance: f64,
    /// Number of solutions directly dominated by this solution.
    pub dominated_solution_count: usize,
}

/// Immutable ranked study table and Pareto archive.
#[derive(Clone, Debug, PartialEq)]
pub struct StudyResult {
    /// Ranked rows in the same order as input candidates.
    pub solutions: Vec<RankedSolution>,
    /// Candidate IDs in the feasible rank-zero Pareto set.
    pub pareto_candidate_ids: Vec<u64>,
    /// Number of fronts produced by constraint-domination.
    pub front_count: usize,
    /// Stable identity of candidates, evaluations, directions, and ranks.
    pub content_hash: [u8; 32],
}

#[derive(Clone, Debug)]
struct Individual {
    candidate: Candidate,
    evaluation: Evaluation,
    rank: usize,
    crowding: f64,
    feasible: bool,
    violation: f64,
    dominated_count: usize,
}

/// Ranks an imported or generated multi-objective study with constraint-domination.
pub fn rank_study(
    problem: &ProblemSpec,
    candidates: &[Candidate],
    evaluations: &[Evaluation],
) -> Result<StudyResult, StudyError> {
    let individuals = bind_individuals(problem, candidates, evaluations)?;
    let ranked = rank_individuals(problem, individuals);
    Ok(study_result(problem, &ranked))
}

fn bind_individuals(
    problem: &ProblemSpec,
    candidates: &[Candidate],
    evaluations: &[Evaluation],
) -> Result<Vec<Individual>, StudyError> {
    if candidates.is_empty()
        || candidates.len() > MAXIMUM_SOLUTIONS
        || candidates.len() != evaluations.len()
    {
        return Err(StudyError::InvalidDimensions);
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        problem.validate_candidate(candidate)?;
        if !candidate_ids.insert(candidate.candidate_id) {
            return Err(StudyError::DuplicateIdentifier);
        }
    }
    let mut evaluation_map = BTreeMap::new();
    for evaluation in evaluations {
        problem.validate_evaluation(evaluation)?;
        if evaluation_map
            .insert(evaluation.candidate_id, evaluation)
            .is_some()
        {
            return Err(StudyError::DuplicateIdentifier);
        }
    }
    candidates
        .iter()
        .map(|candidate| {
            let evaluation = evaluation_map
                .get(&candidate.candidate_id)
                .ok_or(StudyError::UnknownCandidate)?;
            let violation = constraint_violation(&evaluation.constraint_residuals);
            Ok(Individual {
                candidate: candidate.clone(),
                evaluation: (*evaluation).clone(),
                rank: usize::MAX,
                crowding: 0.0,
                feasible: violation <= DOMINANCE_EPSILON,
                violation,
                dominated_count: 0,
            })
        })
        .collect()
}

fn rank_individuals(problem: &ProblemSpec, mut individuals: Vec<Individual>) -> Vec<Individual> {
    let relation = (0..individuals.len())
        .into_par_iter()
        .map(|first| {
            let mut dominates = Vec::new();
            let mut dominated_by = 0_usize;
            for second in 0..individuals.len() {
                if first == second {
                    continue;
                }
                match dominance(problem, &individuals[first], &individuals[second]) {
                    Dominance::First => dominates.push(second),
                    Dominance::Second => dominated_by += 1,
                    Dominance::Neither => {}
                }
            }
            (dominates, dominated_by)
        })
        .collect::<Vec<_>>();
    let mut remaining_domination = relation.iter().map(|(_, count)| *count).collect::<Vec<_>>();
    let mut front = remaining_domination
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<Vec<_>>();
    let mut rank = 0;
    while !front.is_empty() {
        assign_crowding(problem, &mut individuals, &front);
        let mut next = Vec::new();
        for &index in &front {
            individuals[index].rank = rank;
            individuals[index].dominated_count = relation[index].0.len();
            for &dominated in &relation[index].0 {
                remaining_domination[dominated] = remaining_domination[dominated].saturating_sub(1);
                if remaining_domination[dominated] == 0 {
                    next.push(dominated);
                }
            }
        }
        next.sort_unstable();
        next.dedup();
        front = next;
        rank += 1;
    }
    individuals
}

#[derive(Clone, Copy)]
enum Dominance {
    First,
    Second,
    Neither,
}

fn dominance(problem: &ProblemSpec, first: &Individual, second: &Individual) -> Dominance {
    if first.feasible != second.feasible {
        return if first.feasible {
            Dominance::First
        } else {
            Dominance::Second
        };
    }
    if !first.feasible {
        return if first.violation + DOMINANCE_EPSILON < second.violation {
            Dominance::First
        } else if second.violation + DOMINANCE_EPSILON < first.violation {
            Dominance::Second
        } else {
            Dominance::Neither
        };
    }
    let mut first_better = false;
    let mut second_better = false;
    for ((first_value, second_value), direction) in first
        .evaluation
        .objectives
        .iter()
        .zip(&second.evaluation.objectives)
        .zip(&problem.objective_directions)
    {
        let (first_value, second_value) = match direction {
            ObjectiveDirection::Minimize => (*first_value, *second_value),
            ObjectiveDirection::Maximize => (-*first_value, -*second_value),
        };
        if first_value + DOMINANCE_EPSILON < second_value {
            first_better = true;
        } else if second_value + DOMINANCE_EPSILON < first_value {
            second_better = true;
        }
    }
    match (first_better, second_better) {
        (true, false) => Dominance::First,
        (false, true) => Dominance::Second,
        _ => Dominance::Neither,
    }
}

fn assign_crowding(problem: &ProblemSpec, individuals: &mut [Individual], front: &[usize]) {
    if front.is_empty() {
        return;
    }
    for &index in front {
        individuals[index].crowding = 0.0;
    }
    if front.len() <= 2 {
        for &index in front {
            individuals[index].crowding = f64::INFINITY;
        }
        return;
    }
    for objective in 0..problem.objective_directions.len() {
        let mut ordered = front.to_vec();
        ordered.sort_by(|left, right| {
            transformed_objective(problem, &individuals[*left], objective)
                .total_cmp(&transformed_objective(
                    problem,
                    &individuals[*right],
                    objective,
                ))
                .then_with(|| {
                    individuals[*left]
                        .candidate
                        .candidate_id
                        .cmp(&individuals[*right].candidate.candidate_id)
                })
        });
        let minimum = transformed_objective(problem, &individuals[ordered[0]], objective);
        let maximum = transformed_objective(
            problem,
            &individuals[*ordered.last().expect("front non-empty")],
            objective,
        );
        individuals[ordered[0]].crowding = f64::INFINITY;
        individuals[*ordered.last().expect("front non-empty")].crowding = f64::INFINITY;
        let range = maximum - minimum;
        if range <= DOMINANCE_EPSILON {
            continue;
        }
        for window in ordered.windows(3) {
            let current = window[1];
            if individuals[current].crowding.is_infinite() {
                continue;
            }
            let previous = transformed_objective(problem, &individuals[window[0]], objective);
            let next = transformed_objective(problem, &individuals[window[2]], objective);
            individuals[current].crowding += (next - previous) / range;
        }
    }
}

fn transformed_objective(problem: &ProblemSpec, individual: &Individual, index: usize) -> f64 {
    match problem.objective_directions[index] {
        ObjectiveDirection::Minimize => individual.evaluation.objectives[index],
        ObjectiveDirection::Maximize => -individual.evaluation.objectives[index],
    }
}

fn constraint_violation(residuals: &[f64]) -> f64 {
    residuals.iter().map(|value| value.max(0.0)).sum()
}

fn study_result(problem: &ProblemSpec, individuals: &[Individual]) -> StudyResult {
    let solutions = individuals
        .iter()
        .map(|individual| RankedSolution {
            candidate_id: individual.candidate.candidate_id,
            pareto_rank: individual.rank,
            feasible: individual.feasible,
            total_constraint_violation: individual.violation,
            crowding_distance: individual.crowding,
            dominated_solution_count: individual.dominated_count,
        })
        .collect::<Vec<_>>();
    let mut pareto_candidate_ids = individuals
        .iter()
        .filter(|individual| individual.feasible && individual.rank == 0)
        .map(|individual| individual.candidate.candidate_id)
        .collect::<Vec<_>>();
    pareto_candidate_ids.sort_unstable();
    let front_count = individuals
        .iter()
        .map(|individual| individual.rank)
        .max()
        .map_or(0, |rank| rank + 1);
    let content_hash = study_hash(problem, individuals);
    StudyResult {
        solutions,
        pareto_candidate_ids,
        front_count,
        content_hash,
    }
}

/// Returns exact dominated hypervolume for a feasible two-objective Pareto set.
///
/// `reference_point` is expressed in original objective directions and must be
/// worse than retained points after direction transformation.
pub fn hypervolume_2d(
    problem: &ProblemSpec,
    candidates: &[Candidate],
    evaluations: &[Evaluation],
    reference_point: [f64; 2],
) -> Result<f64, StudyError> {
    if problem.objective_directions.len() != 2
        || reference_point.iter().any(|value| !value.is_finite())
    {
        return Err(StudyError::InvalidReferencePoint);
    }
    let individuals =
        rank_individuals(problem, bind_individuals(problem, candidates, evaluations)?);
    let reference = [
        transform_value(reference_point[0], problem.objective_directions[0]),
        transform_value(reference_point[1], problem.objective_directions[1]),
    ];
    let mut points = individuals
        .iter()
        .filter(|individual| individual.feasible && individual.rank == 0)
        .map(|individual| {
            [
                transformed_objective(problem, individual, 0),
                transformed_objective(problem, individual, 1),
            ]
        })
        .filter(|point| point[0] <= reference[0] && point[1] <= reference[1])
        .collect::<Vec<_>>();
    points.sort_by(|left, right| {
        left[0]
            .total_cmp(&right[0])
            .then_with(|| left[1].total_cmp(&right[1]))
    });
    let mut hypervolume = 0.0;
    let mut height = reference[1];
    for point in points {
        if point[1] < height {
            hypervolume = (reference[0] - point[0])
                .max(0.0)
                .mul_add((height - point[1]).max(0.0), hypervolume);
            height = point[1];
        }
    }
    Ok(hypervolume)
}

/// One rank-based global sensitivity coefficient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensitivityCoefficient {
    /// Variable column.
    pub variable_index: usize,
    /// Objective column.
    pub objective_index: usize,
    /// Spearman rank correlation in `[-1, 1]`.
    pub spearman_rho: f64,
}

/// Computes descriptive Spearman sensitivity for a completed study table.
///
/// It deliberately does not claim confidence intervals or causal sensitivity.
pub fn spearman_sensitivity(
    problem: &ProblemSpec,
    candidates: &[Candidate],
    evaluations: &[Evaluation],
) -> Result<Vec<SensitivityCoefficient>, StudyError> {
    let bound = bind_individuals(problem, candidates, evaluations)?;
    let mut coefficients =
        Vec::with_capacity(problem.variables.len() * problem.objective_directions.len());
    for variable in 0..problem.variables.len() {
        let parameter_ranks = average_ranks(
            &bound
                .iter()
                .map(|individual| individual.candidate.values[variable])
                .collect::<Vec<_>>(),
        );
        for objective in 0..problem.objective_directions.len() {
            let objective_ranks = average_ranks(
                &bound
                    .iter()
                    .map(|individual| individual.evaluation.objectives[objective])
                    .collect::<Vec<_>>(),
            );
            coefficients.push(SensitivityCoefficient {
                variable_index: variable,
                objective_index: objective,
                spearman_rho: pearson(&parameter_ranks, &objective_ranks),
            });
        }
    }
    Ok(coefficients)
}

fn average_ranks(values: &[f64]) -> Vec<f64> {
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        values[*left]
            .total_cmp(&values[*right])
            .then_with(|| left.cmp(right))
    });
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]].to_bits() == values[order[start]].to_bits() {
            end += 1;
        }
        let average = (usize_as_f64(start) + usize_as_f64(end - 1)) * 0.5;
        for &index in &order[start..end] {
            ranks[index] = average;
        }
        start = end;
    }
    ranks
}

fn pearson(first: &[f64], second: &[f64]) -> f64 {
    let count = usize_as_f64(first.len());
    let first_mean = first.iter().sum::<f64>() / count;
    let second_mean = second.iter().sum::<f64>() / count;
    let mut covariance = 0.0;
    let mut first_variance = 0.0;
    let mut second_variance = 0.0;
    for (first, second) in first.iter().zip(second) {
        let first_delta = *first - first_mean;
        let second_delta = *second - second_mean;
        covariance = first_delta.mul_add(second_delta, covariance);
        first_variance = first_delta.mul_add(first_delta, first_variance);
        second_variance = second_delta.mul_add(second_delta, second_variance);
    }
    if first_variance <= f64::EPSILON || second_variance <= f64::EPSILON {
        0.0
    } else {
        covariance / (first_variance * second_variance).sqrt()
    }
}

/// Evolutionary search configuration.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerConfig {
    /// Parent population size.
    pub population_size: usize,
    /// Candidates generated after every completed generation.
    pub offspring_size: usize,
    /// Maximum retained historical Pareto solutions.
    pub archive_capacity: usize,
    /// Deterministic random seed.
    pub seed: u64,
    /// Simulated-binary crossover probability.
    pub crossover_probability: f64,
    /// Per-variable mutation probability; zero selects `1 / variable_count`.
    pub mutation_probability: f64,
    /// Simulated-binary crossover distribution index.
    pub crossover_distribution_index: f64,
    /// Polynomial mutation distribution index.
    pub mutation_distribution_index: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            population_size: 64,
            offspring_size: 64,
            archive_capacity: 256,
            seed: 42,
            crossover_probability: 0.9,
            mutation_probability: 0.0,
            crossover_distribution_index: 15.0,
            mutation_distribution_index: 20.0,
        }
    }
}

impl OptimizerConfig {
    fn validate(self, variable_count: usize) -> Result<Self, StudyError> {
        if !(4..=MAXIMUM_SOLUTIONS).contains(&self.population_size)
            || !(1..=MAXIMUM_SOLUTIONS).contains(&self.offspring_size)
            || !(1..=MAXIMUM_SOLUTIONS).contains(&self.archive_capacity)
            || !self.crossover_probability.is_finite()
            || !(0.0..=1.0).contains(&self.crossover_probability)
            || !self.mutation_probability.is_finite()
            || !(0.0..=1.0).contains(&self.mutation_probability)
            || !self.crossover_distribution_index.is_finite()
            || self.crossover_distribution_index <= 0.0
            || !self.mutation_distribution_index.is_finite()
            || self.mutation_distribution_index <= 0.0
            || variable_count == 0
        {
            return Err(StudyError::InvalidOptimizerConfig);
        }
        Ok(self)
    }
}

/// Observable state after a completed optimizer generation.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerSnapshot {
    /// Number of completed external-evaluation generations.
    pub generation: u64,
    /// Total evaluations accepted by `tell`.
    pub evaluation_count: u64,
    /// Current parent population ranking.
    pub population: StudyResult,
    /// Candidate IDs retained in the bounded historical Pareto archive.
    pub archive_candidate_ids: Vec<u64>,
    /// Finite mean crowding distance across non-boundary population points.
    pub mean_finite_crowding_distance: f64,
    /// Stable optimizer-state identity.
    pub content_hash: [u8; 32],
}

/// One evaluated design retained by an optimizer checkpoint.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatedCandidate {
    /// Generated/imported parameter vector.
    pub candidate: Candidate,
    /// Aligned objective and constraint values.
    pub evaluation: Evaluation,
}

/// Complete, versioned optimizer state required for bitwise deterministic resume.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerCheckpoint {
    /// Checkpoint contract identity.
    pub schema_version: String,
    /// Optimization problem including variable bounds and objective directions.
    pub problem: ProblemSpec,
    /// Evolutionary policy and seed.
    pub config: OptimizerConfig,
    /// Number of completed generations.
    pub generation: u64,
    /// Number of accepted evaluations.
    pub evaluation_count: u64,
    /// ID to allocate to the next generated candidate.
    pub next_candidate_id: u64,
    /// Exact deterministic pseudo-random-generator state.
    pub random_state: u64,
    /// Ranked parent population; ranks are reconstructed during restore.
    pub population: Vec<EvaluatedCandidate>,
    /// Bounded historical feasible Pareto archive.
    pub archive: Vec<EvaluatedCandidate>,
    /// Current unevaluated batch, when checkpointed between `ask` and `tell`.
    pub pending: Vec<Candidate>,
}

/// Stateful deterministic constrained multi-objective ask/tell optimizer.
#[derive(Clone, Debug)]
pub struct MultiObjectiveOptimizer {
    problem: ProblemSpec,
    config: OptimizerConfig,
    generation: u64,
    evaluation_count: u64,
    next_candidate_id: u64,
    random: SplitMix64,
    population: Vec<Individual>,
    archive: Vec<Individual>,
    pending: Vec<Candidate>,
}

impl MultiObjectiveOptimizer {
    /// Creates an empty deterministic optimizer.
    pub fn new(problem: ProblemSpec, config: OptimizerConfig) -> Result<Self, StudyError> {
        let config = config.validate(problem.variables.len())?;
        Ok(Self {
            problem,
            config,
            generation: 0,
            evaluation_count: 0,
            next_candidate_id: 1,
            random: SplitMix64::new(config.seed),
            population: Vec::new(),
            archive: Vec::new(),
            pending: Vec::new(),
        })
    }

    /// Restores an optimizer from a complete checkpoint, validating every dimension and ID.
    pub fn from_checkpoint(checkpoint: OptimizerCheckpoint) -> Result<Self, StudyError> {
        if checkpoint.schema_version != "xvarna.optimizer-checkpoint/1" {
            return Err(StudyError::InvalidCheckpoint);
        }
        let problem = ProblemSpec::try_new(
            checkpoint.problem.variables.clone(),
            checkpoint.problem.objective_directions.clone(),
            checkpoint.problem.constraint_count,
        )?;
        let config = checkpoint.config.validate(problem.variables.len())?;
        if checkpoint.population.len() > config.population_size
            || checkpoint.archive.len() > config.archive_capacity
            || checkpoint.next_candidate_id == 0
        {
            return Err(StudyError::InvalidCheckpoint);
        }
        let population = restore_individuals(&problem, &checkpoint.population)?;
        let archive = restore_individuals(&problem, &checkpoint.archive)?;
        let mut pending_ids = BTreeSet::new();
        let retained_ids = population
            .iter()
            .map(|value| value.candidate.candidate_id)
            .collect::<BTreeSet<_>>();
        for candidate in &checkpoint.pending {
            problem.validate_candidate(candidate)?;
            if !pending_ids.insert(candidate.candidate_id)
                || retained_ids.contains(&candidate.candidate_id)
            {
                return Err(StudyError::InvalidCheckpoint);
            }
        }
        let maximum_id = checkpoint
            .population
            .iter()
            .chain(&checkpoint.archive)
            .map(|value| value.candidate.candidate_id)
            .chain(checkpoint.pending.iter().map(|value| value.candidate_id))
            .max()
            .unwrap_or(0);
        if checkpoint.next_candidate_id <= maximum_id {
            return Err(StudyError::InvalidCheckpoint);
        }
        Ok(Self {
            problem,
            config,
            generation: checkpoint.generation,
            evaluation_count: checkpoint.evaluation_count,
            next_candidate_id: checkpoint.next_candidate_id,
            random: SplitMix64 {
                state: checkpoint.random_state,
            },
            population,
            archive,
            pending: checkpoint.pending,
        })
    }

    /// Captures exact state before or after a batch for atomic disk persistence and resume.
    #[must_use]
    pub fn checkpoint(&self) -> OptimizerCheckpoint {
        OptimizerCheckpoint {
            schema_version: "xvarna.optimizer-checkpoint/1".to_owned(),
            problem: self.problem.clone(),
            config: self.config,
            generation: self.generation,
            evaluation_count: self.evaluation_count,
            next_candidate_id: self.next_candidate_id,
            random_state: self.random.state,
            population: checkpoint_individuals(&self.population),
            archive: checkpoint_individuals(&self.archive),
            pending: self.pending.clone(),
        }
    }

    /// Returns a stratified initial population or diversity-preserving offspring.
    /// A second call before `tell` is rejected to prevent duplicate evaluations.
    pub fn ask(&mut self) -> Result<Vec<Candidate>, StudyError> {
        if !self.pending.is_empty() {
            return Err(StudyError::PendingEvaluation);
        }
        self.pending = if self.population.is_empty() {
            self.stratified_initial_population()
        } else {
            self.generate_offspring()
        };
        Ok(self.pending.clone())
    }

    /// Accepts objective and residual-constraint vectors for every pending candidate.
    pub fn tell(&mut self, evaluations: &[Evaluation]) -> Result<OptimizerSnapshot, StudyError> {
        if self.pending.is_empty() || evaluations.len() != self.pending.len() {
            return Err(StudyError::IncompleteEvaluation);
        }
        let evaluated = bind_individuals(&self.problem, &self.pending, evaluations)?;
        self.evaluation_count = self
            .evaluation_count
            .saturating_add(u64::try_from(evaluated.len()).unwrap_or(u64::MAX));
        let mut combined = self.population.clone();
        combined.extend(evaluated);
        combined = rank_individuals(&self.problem, combined);
        combined.sort_by(selection_order);
        combined.truncate(self.config.population_size);
        self.population = rank_individuals(&self.problem, combined);
        self.update_archive();
        self.pending.clear();
        self.generation = self.generation.saturating_add(1);
        self.snapshot()
    }

    /// Returns a completed-generation state without changing it.
    pub fn snapshot(&self) -> Result<OptimizerSnapshot, StudyError> {
        if self.population.is_empty() {
            return Err(StudyError::NoCompletedGeneration);
        }
        let population = study_result(&self.problem, &self.population);
        let mut archive_candidate_ids = self
            .archive
            .iter()
            .map(|individual| individual.candidate.candidate_id)
            .collect::<Vec<_>>();
        archive_candidate_ids.sort_unstable();
        let finite = self
            .population
            .iter()
            .map(|individual| individual.crowding)
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        let mean_finite_crowding_distance = if finite.is_empty() {
            0.0
        } else {
            finite.iter().sum::<f64>() / usize_as_f64(finite.len())
        };
        let content_hash = optimizer_hash(
            &self.problem,
            self.config,
            self.generation,
            self.evaluation_count,
            self.next_candidate_id,
            self.random.state,
            &self.population,
            &self.archive,
            &self.pending,
        );
        Ok(OptimizerSnapshot {
            generation: self.generation,
            evaluation_count: self.evaluation_count,
            population,
            archive_candidate_ids,
            mean_finite_crowding_distance,
            content_hash,
        })
    }

    /// Returns pending candidate IDs without exposing mutable optimizer state.
    #[must_use]
    pub fn pending_candidate_ids(&self) -> Vec<u64> {
        self.pending
            .iter()
            .map(|candidate| candidate.candidate_id)
            .collect()
    }

    fn stratified_initial_population(&mut self) -> Vec<Candidate> {
        let count = self.config.population_size;
        let mut columns = Vec::with_capacity(self.problem.variables.len());
        for variable in &self.problem.variables {
            let mut strata = (0..count).collect::<Vec<_>>();
            self.random.shuffle(&mut strata);
            columns.push(
                strata
                    .into_iter()
                    .map(|stratum| {
                        let unit =
                            (usize_as_f64(stratum) + self.random.unit()) / usize_as_f64(count);
                        variable.canonicalize(
                            variable
                                .lower_bound
                                .mul_add(1.0 - unit, variable.upper_bound * unit),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
        }
        (0..count)
            .map(|row| {
                let values = columns.iter().map(|column| column[row]).collect();
                self.next_candidate(values)
            })
            .collect()
    }

    fn generate_offspring(&mut self) -> Vec<Candidate> {
        let mut output = Vec::with_capacity(self.config.offspring_size);
        while output.len() < self.config.offspring_size {
            let first = self.tournament();
            let second = self.tournament();
            let (mut first_values, mut second_values) = self.crossover(
                &self.population[first].candidate.values.clone(),
                &self.population[second].candidate.values.clone(),
            );
            self.mutate(&mut first_values);
            self.mutate(&mut second_values);
            output.push(self.next_candidate(first_values));
            if output.len() < self.config.offspring_size {
                output.push(self.next_candidate(second_values));
            }
        }
        output
    }

    fn tournament(&mut self) -> usize {
        let first = self.random.index(self.population.len());
        let second = self.random.index(self.population.len());
        if selection_order(&self.population[first], &self.population[second]).is_le() {
            first
        } else {
            second
        }
    }

    fn crossover(&mut self, first: &[f64], second: &[f64]) -> (Vec<f64>, Vec<f64>) {
        if self.random.unit() > self.config.crossover_probability {
            return (first.to_vec(), second.to_vec());
        }
        let mut first_child = Vec::with_capacity(first.len());
        let mut second_child = Vec::with_capacity(second.len());
        for (index, variable) in self.problem.variables.iter().copied().enumerate() {
            let (first_value, second_value) = if matches!(variable.kind, VariableKind::Categorical)
            {
                if self.random.unit() < 0.5 {
                    (first[index], second[index])
                } else {
                    (second[index], first[index])
                }
            } else {
                let unit = self.random.unit();
                let beta = if unit <= 0.5 {
                    (2.0 * unit).powf(1.0 / (self.config.crossover_distribution_index + 1.0))
                } else {
                    (1.0 / (2.0 * (1.0 - unit)))
                        .powf(1.0 / (self.config.crossover_distribution_index + 1.0))
                };
                (
                    0.5 * (1.0 - beta).mul_add(second[index], (1.0 + beta) * first[index]),
                    0.5 * (1.0 + beta).mul_add(second[index], (1.0 - beta) * first[index]),
                )
            };
            first_child.push(variable.canonicalize(first_value));
            second_child.push(variable.canonicalize(second_value));
        }
        (first_child, second_child)
    }

    fn mutate(&mut self, values: &mut [f64]) {
        let probability = if self.config.mutation_probability == 0.0 {
            1.0 / usize_as_f64(self.problem.variables.len())
        } else {
            self.config.mutation_probability
        };
        for (value, variable) in values.iter_mut().zip(&self.problem.variables) {
            if self.random.unit() > probability {
                continue;
            }
            if matches!(variable.kind, VariableKind::Categorical) {
                let levels = usize::try_from(u64::from(validated_u32(variable.upper_bound)) + 1)
                    .unwrap_or(usize::MAX);
                let mut replacement = u32::try_from(self.random.index(levels))
                    .expect("categorical level is bounded by u32");
                let current = validated_u32(*value);
                if levels > 1 && replacement == current {
                    replacement = u32::try_from(
                        (usize::try_from(replacement).expect("u32 fits usize") + 1) % levels,
                    )
                    .expect("categorical level is bounded by u32");
                }
                *value = f64::from(replacement);
                continue;
            }
            let range = variable.upper_bound - variable.lower_bound;
            let unit = self.random.unit();
            let delta = if unit < 0.5 {
                (2.0 * unit).powf(1.0 / (self.config.mutation_distribution_index + 1.0)) - 1.0
            } else {
                1.0 - (2.0 * (1.0 - unit))
                    .powf(1.0 / (self.config.mutation_distribution_index + 1.0))
            };
            *value = variable.canonicalize(*value + delta * range);
        }
    }

    const fn next_candidate(&mut self, values: Vec<f64>) -> Candidate {
        let candidate = Candidate {
            candidate_id: self.next_candidate_id,
            generation: self.generation,
            values,
        };
        self.next_candidate_id = self.next_candidate_id.saturating_add(1);
        candidate
    }

    fn update_archive(&mut self) {
        let mut combined = self.archive.clone();
        combined.extend(self.population.clone());
        let mut seen = BTreeSet::new();
        combined.retain(|individual| seen.insert(individual.candidate.candidate_id));
        combined = rank_individuals(&self.problem, combined);
        combined.retain(|individual| individual.feasible && individual.rank == 0);
        combined.sort_by(selection_order);
        combined.truncate(self.config.archive_capacity);
        self.archive = combined;
    }
}

fn checkpoint_individuals(individuals: &[Individual]) -> Vec<EvaluatedCandidate> {
    individuals
        .iter()
        .map(|individual| EvaluatedCandidate {
            candidate: individual.candidate.clone(),
            evaluation: individual.evaluation.clone(),
        })
        .collect()
}

fn restore_individuals(
    problem: &ProblemSpec,
    values: &[EvaluatedCandidate],
) -> Result<Vec<Individual>, StudyError> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let candidates = values
        .iter()
        .map(|value| value.candidate.clone())
        .collect::<Vec<_>>();
    let evaluations = values
        .iter()
        .map(|value| value.evaluation.clone())
        .collect::<Vec<_>>();
    Ok(rank_individuals(
        problem,
        bind_individuals(problem, &candidates, &evaluations)?,
    ))
}

fn selection_order(first: &Individual, second: &Individual) -> core::cmp::Ordering {
    first
        .rank
        .cmp(&second.rank)
        .then_with(|| second.crowding.total_cmp(&first.crowding))
        .then_with(|| {
            first
                .candidate
                .candidate_id
                .cmp(&second.candidate.candidate_id)
        })
}

fn transform_value(value: f64, direction: ObjectiveDirection) -> f64 {
    match direction {
        ObjectiveDirection::Minimize => value,
        ObjectiveDirection::Maximize => -value,
    }
}

#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    const fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    #[allow(clippy::cast_precision_loss)]
    fn unit(&mut self) -> f64 {
        let mantissa = self.next() >> 11;
        mantissa as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    fn index(&mut self, length: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(length).expect("non-empty bounded slice"))
            .expect("modulo fits usize")
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let replacement = self.index(index + 1);
            values.swap(index, replacement);
        }
    }
}

fn study_hash(problem: &ProblemSpec, individuals: &[Individual]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_STUDY_RANK_V1\0");
    for variable in &problem.variables {
        hasher.update(&variable.variable_id.to_le_bytes());
        hasher.update(&[variable.kind as u8]);
        hasher.update(&variable.lower_bound.to_bits().to_le_bytes());
        hasher.update(&variable.upper_bound.to_bits().to_le_bytes());
    }
    for direction in &problem.objective_directions {
        hasher.update(&[*direction as u8]);
    }
    hasher.update(
        &u64::try_from(problem.constraint_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for individual in individuals {
        hasher.update(&individual.candidate.candidate_id.to_le_bytes());
        hasher.update(&individual.candidate.generation.to_le_bytes());
        for value in &individual.candidate.values {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        for value in &individual.evaluation.objectives {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        for value in &individual.evaluation.constraint_residuals {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(
            &u64::try_from(individual.rank)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hasher.update(&individual.crowding.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[allow(clippy::too_many_arguments)]
fn optimizer_hash(
    problem: &ProblemSpec,
    config: OptimizerConfig,
    generation: u64,
    evaluation_count: u64,
    next_candidate_id: u64,
    random_state: u64,
    population: &[Individual],
    archive: &[Individual],
    pending: &[Candidate],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_STUDY_OPTIMIZER_V2\0");
    hasher.update(&study_hash(problem, population));
    hasher.update(&study_hash(problem, archive));
    hasher.update(&generation.to_le_bytes());
    hasher.update(&evaluation_count.to_le_bytes());
    hasher.update(&next_candidate_id.to_le_bytes());
    hasher.update(&random_state.to_le_bytes());
    hasher.update(&config.seed.to_le_bytes());
    for value in [
        config.crossover_probability,
        config.mutation_probability,
        config.crossover_distribution_index,
        config.mutation_distribution_index,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for candidate in pending {
        hasher.update(&candidate.candidate_id.to_le_bytes());
        hasher.update(&candidate.generation.to_le_bytes());
        for value in &candidate.values {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

fn usize_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("study production caps fit u32"))
}

/// Validation or state-machine failure in Study/Optimization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudyError {
    /// Problem, candidate, or result matrix dimensions are invalid.
    InvalidDimensions,
    /// A variable bound or discrete representation is invalid.
    InvalidVariable,
    /// Variable or candidate identifiers are not unique.
    DuplicateIdentifier,
    /// Candidate parameters do not satisfy the problem contract.
    InvalidCandidate,
    /// Objective/constraint values are non-finite or dimensionally invalid.
    InvalidEvaluation,
    /// An evaluation does not belong to the candidate batch.
    UnknownCandidate,
    /// Hypervolume reference point is invalid for the study.
    InvalidReferencePoint,
    /// Optimizer population, probability, or operator settings are invalid.
    InvalidOptimizerConfig,
    /// `ask` was called again before the pending batch was evaluated.
    PendingEvaluation,
    /// `tell` did not provide exactly one evaluation per pending candidate.
    IncompleteEvaluation,
    /// No completed generation exists yet.
    NoCompletedGeneration,
    /// Checkpoint schema, dimensions, IDs, or retained state are inconsistent.
    InvalidCheckpoint,
}

impl fmt::Display for StudyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDimensions => "study dimensions are empty or exceed production limits",
            Self::InvalidVariable => "study variable bounds or discrete representation are invalid",
            Self::DuplicateIdentifier => "study identifiers must be unique",
            Self::InvalidCandidate => "candidate parameters violate the problem contract",
            Self::InvalidEvaluation => "evaluation objective or constraint vectors are invalid",
            Self::UnknownCandidate => "evaluation references an unknown candidate",
            Self::InvalidReferencePoint => {
                "hypervolume requires a finite two-objective reference point"
            }
            Self::InvalidOptimizerConfig => "optimizer configuration is outside production bounds",
            Self::PendingEvaluation => "pending candidates must be evaluated before asking again",
            Self::IncompleteEvaluation => {
                "tell requires exactly one evaluation for every pending candidate"
            }
            Self::NoCompletedGeneration => "optimizer has no completed generation",
            Self::InvalidCheckpoint => "optimizer checkpoint is incompatible or inconsistent",
        })
    }
}

impl std::error::Error for StudyError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> ProblemSpec {
        ProblemSpec::try_new(
            vec![
                VariableSpec::try_new(1, VariableKind::Continuous, 0.0, 1.0).unwrap(),
                VariableSpec::try_new(2, VariableKind::Integer, 1.0, 10.0).unwrap(),
                VariableSpec::try_new(3, VariableKind::Categorical, 0.0, 2.0).unwrap(),
            ],
            vec![ObjectiveDirection::Minimize, ObjectiveDirection::Maximize],
            1,
        )
        .unwrap()
    }

    fn candidates() -> Vec<Candidate> {
        vec![
            Candidate {
                candidate_id: 1,
                generation: 0,
                values: vec![0.1, 1.0, 0.0],
            },
            Candidate {
                candidate_id: 2,
                generation: 0,
                values: vec![0.5, 5.0, 1.0],
            },
            Candidate {
                candidate_id: 3,
                generation: 0,
                values: vec![0.9, 9.0, 2.0],
            },
            Candidate {
                candidate_id: 4,
                generation: 0,
                values: vec![0.3, 3.0, 1.0],
            },
        ]
    }

    #[test]
    fn constrained_pareto_ranking_prefers_feasible_tradeoffs() {
        let problem = problem();
        let evaluations = vec![
            Evaluation {
                candidate_id: 1,
                objectives: vec![1.0, 1.0],
                constraint_residuals: vec![-1.0],
            },
            Evaluation {
                candidate_id: 2,
                objectives: vec![2.0, 3.0],
                constraint_residuals: vec![-1.0],
            },
            Evaluation {
                candidate_id: 3,
                objectives: vec![0.0, 10.0],
                constraint_residuals: vec![0.5],
            },
            Evaluation {
                candidate_id: 4,
                objectives: vec![4.0, 0.0],
                constraint_residuals: vec![-1.0],
            },
        ];
        let result = rank_study(&problem, &candidates(), &evaluations).unwrap();
        assert_eq!(result.pareto_candidate_ids, vec![1, 2]);
        assert!(!result.solutions[2].feasible);
        assert!(result.solutions[3].pareto_rank > 0);
    }

    #[test]
    fn hypervolume_and_spearman_are_deterministic() {
        let problem = problem();
        let evaluations = vec![
            Evaluation {
                candidate_id: 1,
                objectives: vec![1.0, 1.0],
                constraint_residuals: vec![-1.0],
            },
            Evaluation {
                candidate_id: 2,
                objectives: vec![2.0, 3.0],
                constraint_residuals: vec![-1.0],
            },
            Evaluation {
                candidate_id: 3,
                objectives: vec![3.0, 4.0],
                constraint_residuals: vec![-1.0],
            },
            Evaluation {
                candidate_id: 4,
                objectives: vec![4.0, 0.0],
                constraint_residuals: vec![-1.0],
            },
        ];
        let hypervolume =
            hypervolume_2d(&problem, &candidates(), &evaluations, [5.0, -1.0]).unwrap();
        assert!(hypervolume > 0.0);
        let sensitivity = spearman_sensitivity(&problem, &candidates(), &evaluations).unwrap();
        assert_eq!(sensitivity.len(), 6);
        assert!(
            sensitivity
                .iter()
                .all(|coefficient| coefficient.spearman_rho.abs() <= 1.0)
        );
    }

    #[test]
    fn ask_tell_optimizer_is_reproducible_mixed_and_constrained() {
        fn run(seed: u64) -> OptimizerSnapshot {
            let mut optimizer = MultiObjectiveOptimizer::new(
                problem(),
                OptimizerConfig {
                    population_size: 16,
                    offspring_size: 16,
                    archive_capacity: 32,
                    seed,
                    ..OptimizerConfig::default()
                },
            )
            .unwrap();
            for _ in 0..4 {
                let candidates = optimizer.ask().unwrap();
                assert!(optimizer.ask().is_err());
                let evaluations = candidates
                    .iter()
                    .map(|candidate| {
                        let x = candidate.values[0];
                        let integer = candidate.values[1];
                        Evaluation {
                            candidate_id: candidate.candidate_id,
                            objectives: vec![integer.mul_add(0.01, x * x), 1.0 - (x - 0.8).abs()],
                            constraint_residuals: vec![x + integer / 20.0 - 1.2],
                        }
                    })
                    .collect::<Vec<_>>();
                optimizer.tell(&evaluations).unwrap();
            }
            optimizer.snapshot().unwrap()
        }
        let first = run(77);
        let second = run(77);
        assert_eq!(first.content_hash, second.content_hash);
        assert_eq!(first.generation, 4);
        assert_eq!(first.evaluation_count, 64);
        assert!(!first.archive_candidate_ids.is_empty());
    }

    #[test]
    fn checkpoint_restores_pending_batch_and_exact_future_random_stream() {
        let config = OptimizerConfig {
            population_size: 8,
            offspring_size: 8,
            archive_capacity: 16,
            seed: 991,
            ..OptimizerConfig::default()
        };
        let mut original = MultiObjectiveOptimizer::new(problem(), config).unwrap();
        let initial = original.ask().unwrap();
        let initial_evaluations = initial
            .iter()
            .map(|candidate| Evaluation {
                candidate_id: candidate.candidate_id,
                objectives: vec![candidate.values[0], candidate.values[1]],
                constraint_residuals: vec![-1.0],
            })
            .collect::<Vec<_>>();
        original.tell(&initial_evaluations).unwrap();
        let pending = original.ask().unwrap();
        let encoded = serde_json::to_vec(&original.checkpoint()).unwrap();
        let checkpoint: OptimizerCheckpoint = serde_json::from_slice(&encoded).unwrap();
        let mut restored = MultiObjectiveOptimizer::from_checkpoint(checkpoint).unwrap();
        assert_eq!(
            restored.pending_candidate_ids(),
            original.pending_candidate_ids()
        );
        let pending_evaluations = pending
            .iter()
            .map(|candidate| Evaluation {
                candidate_id: candidate.candidate_id,
                objectives: vec![candidate.values[0], candidate.values[1]],
                constraint_residuals: vec![-1.0],
            })
            .collect::<Vec<_>>();
        assert_eq!(
            original.tell(&pending_evaluations).unwrap().content_hash,
            restored.tell(&pending_evaluations).unwrap().content_hash
        );
        assert_eq!(original.ask().unwrap(), restored.ask().unwrap());
    }
}
