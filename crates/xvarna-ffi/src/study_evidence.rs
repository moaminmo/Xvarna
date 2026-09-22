//! JSON C ABI for RASHNU evidence and VAHMAN multi-fidelity analysis.

use super::XvStatus;
use super::study_platform::{ffi_status, read_utf8, write_utf8};
use serde::{Deserialize, Serialize};
use xvarna_study::evidence::{
    EvidencePassport, FidelityObservation, GlobalSensitivityDesign, GlobalSensitivityMethod,
    MultiFidelityPolicy, ScenarioOutcome, UncertainEvaluation, aggregate_robust_scenarios,
    analyze_global_sensitivity, morris_design, recommend_reference_evaluations,
    saltelli_jansen_design, uncertainty_aware_ranking,
};
use xvarna_study::{Candidate, ObjectiveDirection, VariableSpec};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SensitivityDesignRequest {
    variables: Vec<VariableSpec>,
    method: GlobalSensitivityMethod,
    base_samples: usize,
    #[serde(default = "default_morris_levels")]
    levels: usize,
    seed: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SensitivityAnalysisRequest {
    design: GlobalSensitivityDesign,
    outputs: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RobustScenarioRequest {
    objective_directions: Vec<ObjectiveDirection>,
    constraint_count: usize,
    cvar_alpha: f64,
    outcomes: Vec<ScenarioOutcome>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UncertaintyRankingRequest {
    objective_directions: Vec<ObjectiveDirection>,
    constraint_count: usize,
    interval_multiplier: f64,
    evaluations: Vec<UncertainEvaluation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MultiFidelityRequest {
    objective_directions: Vec<ObjectiveDirection>,
    candidates: Vec<Candidate>,
    observations: Vec<FidelityObservation>,
    policy: MultiFidelityPolicy,
}

const fn default_morris_levels() -> usize {
    6
}

type JsonOperation = fn(&str) -> Result<String, XvStatus>;

unsafe fn run_json(
    input_utf8: *const u8,
    input_length: usize,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
    operation: JsonOperation,
) -> i32 {
    ffi_status(|| {
        // SAFETY: Pointer/count and output contract are declared by every public wrapper.
        let input = unsafe { read_utf8(input_utf8, input_length)? };
        let output = operation(input)?;
        // SAFETY: Pointer/capacity and sizing pointer follow the public wrapper contract.
        unsafe { write_utf8(&output, output_utf8, output_capacity, required_bytes) }
    })
}

fn serialize<T: Serialize>(value: &T) -> Result<String, XvStatus> {
    serde_json::to_string(value).map_err(|_| XvStatus::InvalidState)
}

fn parse<T: for<'de> Deserialize<'de>>(input: &str) -> Result<T, XvStatus> {
    serde_json::from_str(input).map_err(|_| XvStatus::InvalidArgument)
}

fn seal_passport(input: &str) -> Result<String, XvStatus> {
    let passport = parse::<EvidencePassport>(input)?
        .seal()
        .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&passport)
}

fn create_design(input: &str) -> Result<String, XvStatus> {
    let request = parse::<SensitivityDesignRequest>(input)?;
    let design = match request.method {
        GlobalSensitivityMethod::SaltelliJansen => {
            saltelli_jansen_design(&request.variables, request.base_samples, request.seed)
        }
        GlobalSensitivityMethod::Morris => morris_design(
            &request.variables,
            request.base_samples,
            request.levels,
            request.seed,
        ),
    }
    .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&design)
}

fn analyze_sensitivity(input: &str) -> Result<String, XvStatus> {
    let request = parse::<SensitivityAnalysisRequest>(input)?;
    let result = analyze_global_sensitivity(&request.design, &request.outputs)
        .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&result)
}

fn aggregate_scenarios(input: &str) -> Result<String, XvStatus> {
    let request = parse::<RobustScenarioRequest>(input)?;
    let result = aggregate_robust_scenarios(
        &request.objective_directions,
        request.constraint_count,
        &request.outcomes,
        request.cvar_alpha,
    )
    .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&result)
}

fn rank_uncertainty(input: &str) -> Result<String, XvStatus> {
    let request = parse::<UncertaintyRankingRequest>(input)?;
    let result = uncertainty_aware_ranking(
        &request.objective_directions,
        request.constraint_count,
        &request.evaluations,
        request.interval_multiplier,
    )
    .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&result)
}

fn select_fidelity(input: &str) -> Result<String, XvStatus> {
    let request = parse::<MultiFidelityRequest>(input)?;
    let result = recommend_reference_evaluations(
        &request.objective_directions,
        &request.candidates,
        &request.observations,
        &request.policy,
    )
    .map_err(|_| XvStatus::InvalidArgument)?;
    serialize(&result)
}

macro_rules! json_entry_point {
    ($name:ident, $operation:ident, $description:literal) => {
        #[doc = $description]
        ///
        /// Supports the sizing-pass contract: null output with zero capacity returns the
        /// required byte count including the trailing NUL.
        ///
        /// # Safety
        ///
        /// Input references `input_length` readable UTF-8 bytes. Output is null with zero
        /// capacity or writable to `output_capacity`; `required_bytes` is writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            input_utf8: *const u8,
            input_length: usize,
            output_utf8: *mut u8,
            output_capacity: usize,
            required_bytes: *mut usize,
        ) -> i32 {
            // SAFETY: This wrapper forwards its documented pointer/count contract.
            unsafe {
                run_json(
                    input_utf8,
                    input_length,
                    output_utf8,
                    output_capacity,
                    required_bytes,
                    $operation,
                )
            }
        }
    };
}

json_entry_point!(
    xv_evidence_passport_seal_json,
    seal_passport,
    "Validates and seals an Evidence Passport JSON document."
);
json_entry_point!(
    xv_sensitivity_design_json,
    create_design,
    "Generates a hash-protected Saltelli/Jansen or Morris design from JSON."
);
json_entry_point!(
    xv_sensitivity_analyze_json,
    analyze_sensitivity,
    "Analyzes outputs aligned to a hash-protected global-sensitivity design."
);
json_entry_point!(
    xv_robust_scenarios_json,
    aggregate_scenarios,
    "Aggregates weighted scenario outcomes into mean, worst-case, `CVaR`, and violation risk."
);
json_entry_point!(
    xv_uncertainty_rank_json,
    rank_uncertainty,
    "Computes confidence-interval feasibility and conservative Pareto ranks."
);
json_entry_point!(
    xv_multifidelity_recommend_json,
    select_fidelity,
    "Calibrates Fast/reference observations and selects cost-aware reference evaluations."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_design_entry_point_supports_sizing_pass() {
        let input = br#"{"variables":[{"variableId":1,"kind":"continuous","lowerBound":0.0,"upperBound":1.0}],"method":"morris","baseSamples":4,"levels":6,"seed":7}"#;
        let mut required = 0_usize;
        // SAFETY: Valid input and a documented sizing pass.
        let status = unsafe {
            xv_sensitivity_design_json(
                input.as_ptr(),
                input.len(),
                core::ptr::null_mut(),
                0,
                &raw mut required,
            )
        };
        assert_eq!(status, XvStatus::Success as i32);
        assert!(required > 100);
        let mut output = vec![0_u8; required];
        // SAFETY: Output buffer has exactly the required capacity.
        let status = unsafe {
            xv_sensitivity_design_json(
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &raw mut required,
            )
        };
        assert_eq!(status, XvStatus::Success as i32);
        let design: GlobalSensitivityDesign =
            serde_json::from_slice(&output[..output.len() - 1]).expect("design");
        design.validate().expect("valid design");
        assert_eq!(design.rows.len(), 8);
    }
}
