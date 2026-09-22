//! Deterministic bootstrap confidence intervals and permutation significance for sensitivity.

use super::PlatformError;
use crate::{Candidate, Evaluation, ProblemSpec, SplitMix64, average_ranks, pearson};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Reproducible inferential sensitivity policy.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityOptions {
    /// Bootstrap resamples used by the percentile confidence interval.
    pub bootstrap_samples: usize,
    /// Random permutations used for the two-sided null test.
    pub permutation_samples: usize,
    /// Confidence level in `(0.5, 1.0)`.
    pub confidence_level: f64,
    /// Family-wise significance level before Holm correction.
    pub alpha: f64,
    /// Deterministic random seed.
    pub seed: u64,
}

impl Default for SensitivityOptions {
    fn default() -> Self {
        Self {
            bootstrap_samples: 1_000,
            permutation_samples: 1_000,
            confidence_level: 0.95,
            alpha: 0.05,
            seed: 0x58_56_41_52_4E_41_16,
        }
    }
}

impl SensitivityOptions {
    fn validate(self) -> Result<Self, PlatformError> {
        if !(32..=10_000).contains(&self.bootstrap_samples)
            || !(32..=10_000).contains(&self.permutation_samples)
            || !self.confidence_level.is_finite()
            || !(0.5..1.0).contains(&self.confidence_level)
            || !self.alpha.is_finite()
            || !(0.0..0.5).contains(&self.alpha)
        {
            return Err(PlatformError::InvalidLedger);
        }
        Ok(self)
    }
}

/// Statistically qualified monotonic parameter-to-objective association.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticalSensitivity {
    /// Parameter column index.
    pub parameter_index: usize,
    /// Flattened scalar objective column index.
    pub objective_index: usize,
    /// Stable flattened objective name.
    pub objective_name: String,
    /// Number of evaluated variants.
    pub sample_count: usize,
    /// Spearman rank correlation.
    pub spearman_rho: f64,
    /// Bootstrap percentile confidence lower bound.
    pub confidence_lower: f64,
    /// Bootstrap percentile confidence upper bound.
    pub confidence_upper: f64,
    /// Two-sided permutation p-value.
    pub permutation_p_value: f64,
    /// Holm-adjusted family-wise p-value across all parameter/objective pairs.
    pub holm_adjusted_p_value: f64,
    /// Whether adjusted p-value is at most the requested alpha.
    pub statistically_significant: bool,
    /// True when either input column is constant and sensitivity is undefined-as-zero.
    pub constant_column: bool,
}

/// Computes Spearman rho with bootstrap uncertainty, permutation p-values, and Holm correction.
pub fn statistical_sensitivity(
    problem: &ProblemSpec,
    candidates: &[Candidate],
    evaluations: &[Evaluation],
    objective_names: &[String],
    options: SensitivityOptions,
) -> Result<Vec<StatisticalSensitivity>, PlatformError> {
    let options = options.validate()?;
    if candidates.len() < 4
        || candidates.len() != evaluations.len()
        || objective_names.len() != problem.objective_directions.len()
    {
        return Err(PlatformError::InvalidLedger);
    }
    let evaluation_map = evaluations
        .iter()
        .map(|value| (value.candidate_id, value))
        .collect::<BTreeMap<_, _>>();
    let objective_columns = (0..objective_names.len())
        .map(|objective| {
            candidates
                .iter()
                .map(|candidate| {
                    evaluation_map
                        .get(&candidate.candidate_id)
                        .and_then(|evaluation| evaluation.objectives.get(objective))
                        .copied()
                        .ok_or(PlatformError::InvalidLedger)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let parameter_columns = (0..problem.variables.len())
        .map(|parameter| {
            candidates
                .iter()
                .map(|candidate| candidate.values[parameter])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let pairs = (0..parameter_columns.len())
        .flat_map(|parameter| {
            (0..objective_columns.len()).map(move |objective| (parameter, objective))
        })
        .collect::<Vec<_>>();
    let mut output = pairs
        .into_par_iter()
        .map(|(parameter, objective)| {
            pair_sensitivity(
                parameter,
                objective,
                &objective_names[objective],
                &parameter_columns[parameter],
                &objective_columns[objective],
                options,
            )
        })
        .collect::<Vec<_>>();
    apply_holm_correction(&mut output, options.alpha);
    output.sort_by_key(|value| (value.parameter_index, value.objective_index));
    Ok(output)
}

fn pair_sensitivity(
    parameter_index: usize,
    objective_index: usize,
    objective_name: &str,
    parameters: &[f64],
    objectives: &[f64],
    options: SensitivityOptions,
) -> StatisticalSensitivity {
    let parameter_ranks = average_ranks(parameters);
    let objective_ranks = average_ranks(objectives);
    let constant_column = is_constant(parameters) || is_constant(objectives);
    let observed = if constant_column {
        0.0
    } else {
        pearson(&parameter_ranks, &objective_ranks).clamp(-1.0, 1.0)
    };
    let pair_seed = options.seed
        ^ u64::try_from(parameter_index)
            .unwrap_or(u64::MAX)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::try_from(objective_index)
            .unwrap_or(u64::MAX)
            .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let (confidence_lower, confidence_upper, permutation_p_value) = if constant_column {
        (0.0, 0.0, 1.0)
    } else {
        let interval = bootstrap_interval(parameters, objectives, options, pair_seed);
        let p_value = permutation_p_value(
            &parameter_ranks,
            &objective_ranks,
            observed,
            options.permutation_samples,
            pair_seed ^ 0x94D0_49BB_1331_11EB,
        );
        (interval.0, interval.1, p_value)
    };
    StatisticalSensitivity {
        parameter_index,
        objective_index,
        objective_name: objective_name.to_owned(),
        sample_count: parameters.len(),
        spearman_rho: observed,
        confidence_lower,
        confidence_upper,
        permutation_p_value,
        holm_adjusted_p_value: 1.0,
        statistically_significant: false,
        constant_column,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn bootstrap_interval(
    parameters: &[f64],
    objectives: &[f64],
    options: SensitivityOptions,
    seed: u64,
) -> (f64, f64) {
    let mut random = SplitMix64::new(seed);
    let mut correlations = Vec::with_capacity(options.bootstrap_samples);
    let mut sampled_parameters = vec![0.0; parameters.len()];
    let mut sampled_objectives = vec![0.0; objectives.len()];
    for _ in 0..options.bootstrap_samples {
        for index in 0..parameters.len() {
            let source = random.index(parameters.len());
            sampled_parameters[index] = parameters[source];
            sampled_objectives[index] = objectives[source];
        }
        let rho = pearson(
            &average_ranks(&sampled_parameters),
            &average_ranks(&sampled_objectives),
        );
        correlations.push(if rho.is_finite() {
            rho.clamp(-1.0, 1.0)
        } else {
            0.0
        });
    }
    correlations.sort_by(f64::total_cmp);
    let tail = (1.0 - options.confidence_level) * 0.5;
    let maximum = correlations.len().saturating_sub(1);
    let lower = (tail * maximum as f64).floor() as usize;
    let upper = ((1.0 - tail) * maximum as f64).ceil() as usize;
    (correlations[lower], correlations[upper.min(maximum)])
}

fn permutation_p_value(
    parameter_ranks: &[f64],
    objective_ranks: &[f64],
    observed: f64,
    samples: usize,
    seed: u64,
) -> f64 {
    let mut random = SplitMix64::new(seed);
    let mut permuted = objective_ranks.to_vec();
    let mut extreme = 0_u64;
    for _ in 0..samples {
        random.shuffle(&mut permuted);
        if pearson(parameter_ranks, &permuted).abs() + f64::EPSILON >= observed.abs() {
            extreme = extreme.saturating_add(1);
        }
    }
    let numerator =
        u32::try_from(extreme.saturating_add(1)).map_or_else(|_| f64::from(u32::MAX), f64::from);
    let denominator =
        u32::try_from(samples.saturating_add(1)).map_or_else(|_| f64::from(u32::MAX), f64::from);
    numerator / denominator
}

fn apply_holm_correction(values: &mut [StatisticalSensitivity], alpha: f64) {
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        values[*left]
            .permutation_p_value
            .total_cmp(&values[*right].permutation_p_value)
            .then_with(|| left.cmp(right))
    });
    let mut running = 0.0_f64;
    let total = values.len();
    for (position, index) in order.into_iter().enumerate() {
        let multiplier = u32::try_from(total.saturating_sub(position))
            .map_or_else(|_| f64::from(u32::MAX), f64::from);
        running = running.max((values[index].permutation_p_value * multiplier).min(1.0));
        values[index].holm_adjusted_p_value = running;
        values[index].statistically_significant = running <= alpha;
    }
}

fn is_constant(values: &[f64]) -> bool {
    values.first().is_none_or(|first| {
        values
            .iter()
            .all(|value| value.to_bits() == first.to_bits())
    })
}
