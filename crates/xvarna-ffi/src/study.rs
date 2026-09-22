//! Fixed-layout XVARNA Study ranking and stateful ask/tell optimization ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Instant,
};
use xvarna_study::{
    Candidate, Evaluation, MultiObjectiveOptimizer, ObjectiveDirection, OptimizerConfig,
    ProblemSpec, StudyError, VariableKind, VariableSpec, hypervolume_2d, rank_study,
    spearman_sensitivity,
};

const SOLUTION_FEASIBLE_FLAG: u32 = 1 << 0;

/// Fixed-layout mixed-variable specification.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvVariableSpec {
    /// Byte size.
    pub structure_size: u32,
    /// Numeric [`VariableKind`].
    pub kind: u32,
    /// Stable variable identifier.
    pub variable_id: u64,
    /// Inclusive lower bound.
    pub lower_bound: f64,
    /// Inclusive upper bound.
    pub upper_bound: f64,
}

/// Fixed-layout Pareto rank, feasibility, and crowding record.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvRankedSolution {
    /// Byte size.
    pub structure_size: u32,
    /// Stable flags; bit zero means feasible.
    pub flags: u32,
    /// Candidate identifier.
    pub candidate_id: u64,
    /// Zero-based Pareto rank.
    pub pareto_rank: u64,
    /// Total positive residual violation.
    pub total_constraint_violation: f64,
    /// NSGA-II crowding distance.
    pub crowding_distance: f64,
    /// Number of solutions dominated by this candidate.
    pub dominated_solution_count: u64,
}

/// Fixed-layout ranked-study dimensions, runtime, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvStudyMetadata {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Candidate count.
    pub candidate_count: u64,
    /// Objective count.
    pub objective_count: u64,
    /// Constraint count.
    pub constraint_count: u64,
    /// Non-dominated front count.
    pub front_count: u64,
    /// Feasible rank-zero count.
    pub pareto_count: u64,
    /// Native runtime.
    pub analysis_time_microseconds: u64,
    /// Content hash.
    pub content_hash_0: u64,
    /// Content hash.
    pub content_hash_1: u64,
    /// Content hash.
    pub content_hash_2: u64,
    /// Content hash.
    pub content_hash_3: u64,
}

/// Fixed-layout descriptive Spearman coefficient.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSensitivityCoefficient {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Variable column.
    pub variable_index: u64,
    /// Objective column.
    pub objective_index: u64,
    /// Spearman rank correlation.
    pub spearman_rho: f64,
}

/// Ranks a completed constrained multi-objective study.
///
/// Objective directions are numeric [`ObjectiveDirection`] values. Parameters,
/// objectives, and constraints are row-major matrices.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_study_rank(
    variables: *const XvVariableSpec,
    variable_count: usize,
    objective_directions: *const u32,
    objective_count: usize,
    constraint_count: usize,
    candidate_ids: *const u64,
    generations: *const u64,
    parameters: *const f64,
    objectives: *const f64,
    constraints: *const f64,
    candidate_count: usize,
    output_metadata: *mut XvStudyMetadata,
    output_solutions: *mut XvRankedSolution,
    solution_capacity: usize,
    output_pareto_ids: *mut u64,
    pareto_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if solution_capacity < candidate_count || pareto_capacity < candidate_count {
            return Err(XvStatus::InvalidLength);
        }
        let parameter_count = candidate_count
            .checked_mul(variable_count)
            .ok_or(XvStatus::InvalidLength)?;
        let objective_value_count = candidate_count
            .checked_mul(objective_count)
            .ok_or(XvStatus::InvalidLength)?;
        let constraint_value_count = candidate_count
            .checked_mul(constraint_count)
            .ok_or(XvStatus::InvalidLength)?;
        // SAFETY: Shared helpers validate pointers and lengths.
        let variable_values = unsafe { input_slice(variables, variable_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let direction_values = unsafe { input_slice(objective_directions, objective_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let candidate_ids = unsafe { input_slice(candidate_ids, candidate_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let generations = unsafe { input_slice(generations, candidate_count)? };
        // SAFETY: Matrix length multiplication was checked.
        let parameters = unsafe { input_slice(parameters, parameter_count)? };
        // SAFETY: Matrix length multiplication was checked.
        let objectives = unsafe { input_slice(objectives, objective_value_count)? };
        // SAFETY: Matrix length multiplication was checked. Null is permitted for zero constraints.
        let constraints = unsafe { input_slice(constraints, constraint_value_count)? };
        // SAFETY: Capacity was checked.
        let solution_output = unsafe { output_slice(output_solutions, candidate_count)? };
        // SAFETY: Caller allocated worst-case candidate count.
        let pareto_output = unsafe { output_slice(output_pareto_ids, candidate_count)? };
        let problem = problem_from_ffi(variable_values, direction_values, constraint_count)?;
        let candidates = candidate_ids
            .iter()
            .zip(generations)
            .zip(parameters.chunks_exact(variable_count))
            .map(|((&candidate_id, &generation), values)| Candidate {
                candidate_id,
                generation,
                values: values.to_vec(),
            })
            .collect::<Vec<_>>();
        let evaluations = candidate_ids
            .iter()
            .zip(objectives.chunks_exact(objective_count))
            .enumerate()
            .map(|(index, (&candidate_id, objective_values))| Evaluation {
                candidate_id,
                objectives: objective_values.to_vec(),
                constraint_residuals: if constraint_count == 0 {
                    Vec::new()
                } else {
                    constraints[index * constraint_count..(index + 1) * constraint_count].to_vec()
                },
            })
            .collect::<Vec<_>>();
        let started = Instant::now();
        let result = rank_study(&problem, &candidates, &evaluations).map_err(study_error_status)?;
        let elapsed = micros(started);
        write_ranked(solution_output, &result.solutions);
        for (destination, source) in pareto_output.iter_mut().zip(&result.pareto_candidate_ids) {
            *destination = *source;
        }
        // SAFETY: Caller provides writable metadata.
        unsafe {
            output_metadata.write(XvStudyMetadata {
                structure_size: structure_size::<XvStudyMetadata>(),
                reserved: 0,
                candidate_count: usize_u64(candidate_count),
                objective_count: usize_u64(objective_count),
                constraint_count: usize_u64(constraint_count),
                front_count: usize_u64(result.front_count),
                pareto_count: usize_u64(result.pareto_candidate_ids.len()),
                analysis_time_microseconds: elapsed,
                content_hash_0: hash_word(&result.content_hash, 0),
                content_hash_1: hash_word(&result.content_hash, 1),
                content_hash_2: hash_word(&result.content_hash, 2),
                content_hash_3: hash_word(&result.content_hash, 3),
            });
        }
        Ok(())
    })
}

/// Computes descriptive Spearman parameter/objective sensitivity.
///
/// # Safety
/// Input and output pointers reference the declared live, non-overlapping arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_study_spearman(
    variables: *const XvVariableSpec,
    variable_count: usize,
    objective_directions: *const u32,
    objective_count: usize,
    constraint_count: usize,
    candidate_ids: *const u64,
    parameters: *const f64,
    objectives: *const f64,
    constraints: *const f64,
    candidate_count: usize,
    output_coefficients: *mut XvSensitivityCoefficient,
    coefficient_capacity: usize,
) -> i32 {
    ffi_status(|| {
        let coefficient_count = variable_count
            .checked_mul(objective_count)
            .ok_or(XvStatus::InvalidLength)?;
        if coefficient_capacity < coefficient_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Shared helpers validate every pointer and derived matrix length.
        let variable_values = unsafe { input_slice(variables, variable_count)? };
        // SAFETY: Shared helpers validate every pointer and derived matrix length.
        let direction_values = unsafe { input_slice(objective_directions, objective_count)? };
        // SAFETY: Shared helpers validate every pointer and derived matrix length.
        let ids = unsafe { input_slice(candidate_ids, candidate_count)? };
        // SAFETY: Multiplication is checked below by helper.
        let parameters = unsafe {
            input_slice(
                parameters,
                candidate_count
                    .checked_mul(variable_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Multiplication is checked below by helper.
        let objectives = unsafe {
            input_slice(
                objectives,
                candidate_count
                    .checked_mul(objective_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Null is accepted for a zero-length constraint matrix.
        let constraints = unsafe {
            input_slice(
                constraints,
                candidate_count
                    .checked_mul(constraint_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Capacity was checked.
        let output = unsafe { output_slice(output_coefficients, coefficient_count)? };
        let problem = problem_from_ffi(variable_values, direction_values, constraint_count)?;
        let candidates = ids
            .iter()
            .zip(parameters.chunks_exact(variable_count))
            .map(|(&candidate_id, values)| Candidate {
                candidate_id,
                generation: 0,
                values: values.to_vec(),
            })
            .collect::<Vec<_>>();
        let evaluations = ids
            .iter()
            .zip(objectives.chunks_exact(objective_count))
            .enumerate()
            .map(|(index, (&candidate_id, values))| Evaluation {
                candidate_id,
                objectives: values.to_vec(),
                constraint_residuals: if constraint_count == 0 {
                    Vec::new()
                } else {
                    constraints[index * constraint_count..(index + 1) * constraint_count].to_vec()
                },
            })
            .collect::<Vec<_>>();
        let coefficients = spearman_sensitivity(&problem, &candidates, &evaluations)
            .map_err(study_error_status)?;
        for (destination, source) in output.iter_mut().zip(coefficients) {
            *destination = XvSensitivityCoefficient {
                structure_size: structure_size::<XvSensitivityCoefficient>(),
                reserved: 0,
                variable_index: usize_u64(source.variable_index),
                objective_index: usize_u64(source.objective_index),
                spearman_rho: source.spearman_rho,
            };
        }
        Ok(())
    })
}

/// Computes exact feasible Pareto hypervolume for two objectives.
///
/// # Safety
/// Input pointers reference the declared live arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_study_hypervolume_2d(
    variables: *const XvVariableSpec,
    variable_count: usize,
    objective_directions: *const u32,
    constraint_count: usize,
    candidate_ids: *const u64,
    parameters: *const f64,
    objectives: *const f64,
    constraints: *const f64,
    candidate_count: usize,
    reference_first: f64,
    reference_second: f64,
    output_hypervolume: *mut f64,
) -> i32 {
    ffi_status(|| {
        if output_hypervolume.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Shared helpers validate pointers and lengths.
        let variable_values = unsafe { input_slice(variables, variable_count)? };
        // SAFETY: Exactly two directions are required.
        let direction_values = unsafe { input_slice(objective_directions, 2)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let ids = unsafe { input_slice(candidate_ids, candidate_count)? };
        // SAFETY: Matrix dimensions are checked.
        let parameters = unsafe {
            input_slice(
                parameters,
                candidate_count
                    .checked_mul(variable_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Two-objective matrix dimensions are checked.
        let objectives = unsafe {
            input_slice(
                objectives,
                candidate_count
                    .checked_mul(2)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Null is accepted for zero constraint count.
        let constraints = unsafe {
            input_slice(
                constraints,
                candidate_count
                    .checked_mul(constraint_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        let problem = problem_from_ffi(variable_values, direction_values, constraint_count)?;
        let candidates = ids
            .iter()
            .zip(parameters.chunks_exact(variable_count))
            .map(|(&candidate_id, values)| Candidate {
                candidate_id,
                generation: 0,
                values: values.to_vec(),
            })
            .collect::<Vec<_>>();
        let evaluations = ids
            .iter()
            .zip(objectives.chunks_exact(2))
            .enumerate()
            .map(|(index, (&candidate_id, values))| Evaluation {
                candidate_id,
                objectives: values.to_vec(),
                constraint_residuals: if constraint_count == 0 {
                    Vec::new()
                } else {
                    constraints[index * constraint_count..(index + 1) * constraint_count].to_vec()
                },
            })
            .collect::<Vec<_>>();
        let value = hypervolume_2d(
            &problem,
            &candidates,
            &evaluations,
            [reference_first, reference_second],
        )
        .map_err(study_error_status)?;
        // SAFETY: Output pointer is non-null and writable.
        unsafe { output_hypervolume.write(value) };
        Ok(())
    })
}

/// Fixed-layout evolutionary optimizer configuration.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvOptimizerConfig {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Parent population size.
    pub population_size: u64,
    /// Offspring batch size.
    pub offspring_size: u64,
    /// Historical Pareto archive capacity.
    pub archive_capacity: u64,
    /// Deterministic seed.
    pub seed: u64,
    /// SBX probability.
    pub crossover_probability: f64,
    /// Per-variable mutation probability; zero means inverse dimension.
    pub mutation_probability: f64,
    /// SBX distribution index.
    pub crossover_distribution_index: f64,
    /// Polynomial-mutation distribution index.
    pub mutation_distribution_index: f64,
}

/// Fixed-layout ask/tell optimizer state.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvOptimizerMetadata {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved.
    pub reserved: u32,
    /// Completed generation count.
    pub generation: u64,
    /// Accepted evaluation count.
    pub evaluation_count: u64,
    /// Candidate count in this ask batch.
    pub pending_candidate_count: u64,
    /// Current parent population count.
    pub population_count: u64,
    /// Historical Pareto archive count.
    pub archive_count: u64,
    /// Mean finite crowding distance.
    pub mean_finite_crowding_distance: f64,
    /// Content hash, zero before the first completed generation.
    pub content_hash_0: u64,
    /// Content hash.
    pub content_hash_1: u64,
    /// Content hash.
    pub content_hash_2: u64,
    /// Content hash.
    pub content_hash_3: u64,
}

static OPTIMIZERS: OnceLock<RwLock<BTreeMap<u64, Arc<Mutex<MultiObjectiveOptimizer>>>>> =
    OnceLock::new();
static NEXT_OPTIMIZER_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn optimizers() -> &'static RwLock<BTreeMap<u64, Arc<Mutex<MultiObjectiveOptimizer>>>> {
    OPTIMIZERS.get_or_init(|| RwLock::new(BTreeMap::new()))
}

/// Creates a stateful mixed-variable constrained multi-objective optimizer.
///
/// # Safety
/// Input pointers reference the declared arrays and `output_handle` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_optimizer_create(
    variables: *const XvVariableSpec,
    variable_count: usize,
    objective_directions: *const u32,
    objective_count: usize,
    constraint_count: usize,
    config: *const XvOptimizerConfig,
    output_handle: *mut u64,
) -> i32 {
    ffi_status(|| {
        if config.is_null() || output_handle.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required fixed-size input is non-null.
        let config = unsafe { config.read() };
        if config.structure_size != structure_size::<XvOptimizerConfig>() || config.reserved != 0 {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Shared helpers validate pointers and lengths.
        let variables = unsafe { input_slice(variables, variable_count)? };
        // SAFETY: Shared helpers validate pointers and lengths.
        let directions = unsafe { input_slice(objective_directions, objective_count)? };
        let problem = problem_from_ffi(variables, directions, constraint_count)?;
        let optimizer = MultiObjectiveOptimizer::new(
            problem,
            OptimizerConfig {
                population_size: usize::try_from(config.population_size)
                    .map_err(|_| XvStatus::InvalidLength)?,
                offspring_size: usize::try_from(config.offspring_size)
                    .map_err(|_| XvStatus::InvalidLength)?,
                archive_capacity: usize::try_from(config.archive_capacity)
                    .map_err(|_| XvStatus::InvalidLength)?,
                seed: config.seed,
                crossover_probability: config.crossover_probability,
                mutation_probability: config.mutation_probability,
                crossover_distribution_index: config.crossover_distribution_index,
                mutation_distribution_index: config.mutation_distribution_index,
            },
        )
        .map_err(study_error_status)?;
        let handle = NEXT_OPTIMIZER_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if handle == 0 {
            return Err(XvStatus::InvalidState);
        }
        optimizers()
            .write()
            .map_err(|_| XvStatus::InvalidState)?
            .insert(handle, Arc::new(Mutex::new(optimizer)));
        // SAFETY: Caller provides writable handle storage.
        unsafe { output_handle.write(handle) };
        Ok(())
    })
}

/// Produces an initial or offspring candidate batch for external evaluation.
///
/// # Safety
/// Output pointers reference live buffers with the declared capacities.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_optimizer_ask(
    handle: u64,
    variable_count: usize,
    output_metadata: *mut XvOptimizerMetadata,
    output_candidate_ids: *mut u64,
    output_generations: *mut u64,
    output_parameters: *mut f64,
    candidate_capacity: usize,
    parameter_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let optimizer = get_optimizer(handle)?;
        let mut optimizer = optimizer.lock().map_err(|_| XvStatus::InvalidState)?;
        let candidates = optimizer.ask().map_err(study_error_status)?;
        drop(optimizer);
        let parameter_count = candidates
            .len()
            .checked_mul(variable_count)
            .ok_or(XvStatus::InvalidLength)?;
        if candidate_capacity < candidates.len() || parameter_capacity < parameter_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacities were checked.
        let ids = unsafe { output_slice(output_candidate_ids, candidates.len())? };
        // SAFETY: Capacities were checked.
        let generations = unsafe { output_slice(output_generations, candidates.len())? };
        // SAFETY: Matrix capacity multiplication was checked.
        let parameters = unsafe { output_slice(output_parameters, parameter_count)? };
        for (index, candidate) in candidates.iter().enumerate() {
            if candidate.values.len() != variable_count {
                return Err(XvStatus::InvalidLength);
            }
            ids[index] = candidate.candidate_id;
            generations[index] = candidate.generation;
            parameters[index * variable_count..(index + 1) * variable_count]
                .copy_from_slice(&candidate.values);
        }
        // SAFETY: Caller provides writable metadata.
        unsafe {
            output_metadata.write(XvOptimizerMetadata {
                structure_size: structure_size::<XvOptimizerMetadata>(),
                reserved: 0,
                generation: candidates
                    .first()
                    .map_or(0, |candidate| candidate.generation),
                evaluation_count: 0,
                pending_candidate_count: usize_u64(candidates.len()),
                population_count: 0,
                archive_count: 0,
                mean_finite_crowding_distance: 0.0,
                content_hash_0: 0,
                content_hash_1: 0,
                content_hash_2: 0,
                content_hash_3: 0,
            });
        }
        Ok(())
    })
}

/// Completes the current candidate batch and returns population/archive state.
///
/// # Safety
/// Input and output pointers reference the declared live arrays.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub unsafe extern "C" fn xv_optimizer_tell(
    handle: u64,
    objective_count: usize,
    constraint_count: usize,
    candidate_ids: *const u64,
    objectives: *const f64,
    constraints: *const f64,
    candidate_count: usize,
    output_metadata: *mut XvOptimizerMetadata,
    output_population: *mut XvRankedSolution,
    population_capacity: usize,
    output_archive_ids: *mut u64,
    archive_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Shared helpers validate pointers and lengths.
        let ids = unsafe { input_slice(candidate_ids, candidate_count)? };
        // SAFETY: Matrix multiplication is checked.
        let objectives = unsafe {
            input_slice(
                objectives,
                candidate_count
                    .checked_mul(objective_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        // SAFETY: Null is accepted for zero constraint count.
        let constraints = unsafe {
            input_slice(
                constraints,
                candidate_count
                    .checked_mul(constraint_count)
                    .ok_or(XvStatus::InvalidLength)?,
            )?
        };
        let evaluations = ids
            .iter()
            .zip(objectives.chunks_exact(objective_count))
            .enumerate()
            .map(|(index, (&candidate_id, objective_values))| Evaluation {
                candidate_id,
                objectives: objective_values.to_vec(),
                constraint_residuals: if constraint_count == 0 {
                    Vec::new()
                } else {
                    constraints[index * constraint_count..(index + 1) * constraint_count].to_vec()
                },
            })
            .collect::<Vec<_>>();
        let optimizer = get_optimizer(handle)?;
        let mut optimizer = optimizer.lock().map_err(|_| XvStatus::InvalidState)?;
        let snapshot = optimizer.tell(&evaluations).map_err(study_error_status)?;
        drop(optimizer);
        if population_capacity < snapshot.population.solutions.len()
            || archive_capacity < snapshot.archive_candidate_ids.len()
        {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacity was checked.
        let population =
            unsafe { output_slice(output_population, snapshot.population.solutions.len())? };
        // SAFETY: Capacity was checked.
        let archive =
            unsafe { output_slice(output_archive_ids, snapshot.archive_candidate_ids.len())? };
        write_ranked(population, &snapshot.population.solutions);
        archive.copy_from_slice(&snapshot.archive_candidate_ids);
        // SAFETY: Caller provides writable metadata.
        unsafe {
            output_metadata.write(XvOptimizerMetadata {
                structure_size: structure_size::<XvOptimizerMetadata>(),
                reserved: 0,
                generation: snapshot.generation,
                evaluation_count: snapshot.evaluation_count,
                pending_candidate_count: 0,
                population_count: usize_u64(snapshot.population.solutions.len()),
                archive_count: usize_u64(snapshot.archive_candidate_ids.len()),
                mean_finite_crowding_distance: snapshot.mean_finite_crowding_distance,
                content_hash_0: hash_word(&snapshot.content_hash, 0),
                content_hash_1: hash_word(&snapshot.content_hash, 1),
                content_hash_2: hash_word(&snapshot.content_hash, 2),
                content_hash_3: hash_word(&snapshot.content_hash, 3),
            });
        }
        Ok(())
    })
}

/// Releases one optimizer handle.
#[unsafe(no_mangle)]
pub extern "C" fn xv_optimizer_release(handle: u64) -> i32 {
    ffi_status(|| {
        optimizers()
            .write()
            .map_err(|_| XvStatus::InvalidState)?
            .remove(&handle)
            .map_or(Err(XvStatus::InvalidHandle), |_| Ok(()))
    })
}

fn get_optimizer(handle: u64) -> Result<Arc<Mutex<MultiObjectiveOptimizer>>, XvStatus> {
    optimizers()
        .read()
        .map_err(|_| XvStatus::InvalidState)?
        .get(&handle)
        .cloned()
        .ok_or(XvStatus::InvalidHandle)
}

fn problem_from_ffi(
    variables: &[XvVariableSpec],
    directions: &[u32],
    constraint_count: usize,
) -> Result<ProblemSpec, XvStatus> {
    let variables = variables
        .iter()
        .map(|value| {
            if value.structure_size != structure_size::<XvVariableSpec>() {
                return Err(XvStatus::InvalidArgument);
            }
            let kind = match value.kind {
                0 => VariableKind::Continuous,
                1 => VariableKind::Integer,
                2 => VariableKind::Categorical,
                _ => return Err(XvStatus::InvalidArgument),
            };
            VariableSpec::try_new(
                value.variable_id,
                kind,
                value.lower_bound,
                value.upper_bound,
            )
            .map_err(study_error_status)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let directions = directions
        .iter()
        .map(|value| match value {
            0 => Ok(ObjectiveDirection::Minimize),
            1 => Ok(ObjectiveDirection::Maximize),
            _ => Err(XvStatus::InvalidArgument),
        })
        .collect::<Result<Vec<_>, _>>()?;
    ProblemSpec::try_new(variables, directions, constraint_count).map_err(study_error_status)
}

fn write_ranked(output: &mut [XvRankedSolution], values: &[xvarna_study::RankedSolution]) {
    for (destination, source) in output.iter_mut().zip(values) {
        *destination = XvRankedSolution {
            structure_size: structure_size::<XvRankedSolution>(),
            flags: if source.feasible {
                SOLUTION_FEASIBLE_FLAG
            } else {
                0
            },
            candidate_id: source.candidate_id,
            pareto_rank: usize_u64(source.pareto_rank),
            total_constraint_violation: source.total_constraint_violation,
            crowding_distance: source.crowding_distance,
            dominated_solution_count: usize_u64(source.dominated_solution_count),
        };
    }
}

const fn study_error_status(error: StudyError) -> XvStatus {
    match error {
        StudyError::PendingEvaluation
        | StudyError::IncompleteEvaluation
        | StudyError::NoCompletedGeneration => XvStatus::InvalidState,
        StudyError::InvalidDimensions => XvStatus::InvalidLength,
        _ => XvStatus::InvalidArgument,
    }
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

fn usize_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn study_abi_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvVariableSpec>(), 32);
        assert_eq!(mem::size_of::<XvRankedSolution>(), 48);
        assert_eq!(mem::size_of::<XvStudyMetadata>(), 88);
        assert_eq!(mem::size_of::<XvSensitivityCoefficient>(), 32);
        assert_eq!(mem::size_of::<XvOptimizerConfig>(), 72);
        assert_eq!(mem::size_of::<XvOptimizerMetadata>(), 88);
    }
}
