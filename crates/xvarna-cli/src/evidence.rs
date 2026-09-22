//! RASHNU scientific-evidence and VAHMAN multi-fidelity command surface.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::{fs, path::Path, process::ExitCode};
use xvarna_study::evidence::{
    EvidencePassport, FidelityObservation, GlobalSensitivityDesign, MultiFidelityPolicy,
    ScenarioOutcome, UncertainEvaluation, aggregate_robust_scenarios, analyze_global_sensitivity,
    morris_design, recommend_reference_evaluations, saltelli_jansen_design,
    uncertainty_aware_ranking,
};
use xvarna_study::{Candidate, ObjectiveDirection, VariableSpec};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RobustInput {
    objective_directions: Vec<ObjectiveDirection>,
    constraint_count: usize,
    cvar_alpha: f64,
    outcomes: Vec<ScenarioOutcome>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UncertaintyInput {
    objective_directions: Vec<ObjectiveDirection>,
    constraint_count: usize,
    interval_multiplier: f64,
    evaluations: Vec<UncertainEvaluation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MultiFidelityInput {
    objective_directions: Vec<ObjectiveDirection>,
    candidates: Vec<Candidate>,
    observations: Vec<FidelityObservation>,
    policy: MultiFidelityPolicy,
}

/// Runs one scientific-evidence subcommand.
pub fn run_evidence(arguments: &[String]) -> ExitCode {
    let result = match arguments {
        [command, input, output] if command == "passport" => seal_passport(input, output),
        [command, variables, output, remaining @ ..] if command == "sensitivity-design" => {
            sensitivity_design(variables, output, remaining)
        }
        [command, design, outputs, output] if command == "sensitivity-analyze" => {
            sensitivity_analyze(design, outputs, output)
        }
        [command, input, output] if command == "robust" => robust(input, output),
        [command, input, output] if command == "rank" => rank(input, output),
        [command, input, output] if command == "fidelity" => fidelity(input, output),
        _ => Err("invalid evidence command; run `xvarna help` for usage".to_owned()),
    };
    match result {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn seal_passport(input: &str, output: &str) -> Result<String, String> {
    let passport = read_json::<EvidencePassport>(input, "evidence passport")?
        .seal()
        .map_err(|error| error.to_string())?;
    passport.validate().map_err(|error| error.to_string())?;
    write_json(output, &passport)?;
    Ok(format!(
        "RASHNU evidence passport sealed: {} ({})",
        absolute(output),
        passport.content_hash
    ))
}

fn sensitivity_design(input: &str, output: &str, arguments: &[String]) -> Result<String, String> {
    let variables = read_json::<Vec<VariableSpec>>(input, "variable registry")?;
    let mut method = None;
    let mut samples = None;
    let mut levels = 6_usize;
    let mut seed = 42_u64;
    let mut index = 0;
    while index < arguments.len() {
        if index + 1 >= arguments.len() {
            return Err(format!(
                "incomplete sensitivity-design option {}",
                arguments[index]
            ));
        }
        let value = &arguments[index + 1];
        match arguments[index].as_str() {
            "--method" => method = Some(value.as_str()),
            "--samples" => samples = Some(parse_usize(value, "samples")?),
            "--levels" => levels = parse_usize(value, "levels")?,
            "--seed" => {
                seed = value
                    .parse()
                    .map_err(|_| format!("invalid seed: {value}"))?;
            }
            option => return Err(format!("unknown sensitivity-design option {option}")),
        }
        index += 2;
    }
    let samples = samples.ok_or_else(|| "--samples is required".to_owned())?;
    let design = match method {
        Some("saltelli" | "sobol" | "saltelli-jansen") => {
            saltelli_jansen_design(&variables, samples, seed)
        }
        Some("morris") => morris_design(&variables, samples, levels, seed),
        Some(value) => return Err(format!("unknown sensitivity method: {value}")),
        None => return Err("--method is required".to_owned()),
    }
    .map_err(|error| error.to_string())?;
    write_json(output, &design)?;
    Ok(format!(
        "RASHNU sensitivity design: {} rows, hash {}, {}",
        design.rows.len(),
        design.content_hash,
        absolute(output)
    ))
}

fn sensitivity_analyze(design: &str, outputs: &str, output: &str) -> Result<String, String> {
    let design = read_json::<GlobalSensitivityDesign>(design, "sensitivity design")?;
    if let Err(error) = design.validate() {
        return Err(format!(
            "{error}; stored {}, recomputed {}",
            design.content_hash,
            design.recomputed_content_hash()
        ));
    }
    let outputs = read_json::<Vec<Vec<f64>>>(outputs, "row-aligned outputs")?;
    let result =
        analyze_global_sensitivity(&design, &outputs).map_err(|error| error.to_string())?;
    write_json(output, &result)?;
    Ok(format!(
        "RASHNU global sensitivity: {} outputs, hash {}, {}",
        result.output_count,
        result.content_hash,
        absolute(output)
    ))
}

fn robust(input: &str, output: &str) -> Result<String, String> {
    let input = read_json::<RobustInput>(input, "robust-scenario envelope")?;
    let result = aggregate_robust_scenarios(
        &input.objective_directions,
        input.constraint_count,
        &input.outcomes,
        input.cvar_alpha,
    )
    .map_err(|error| error.to_string())?;
    write_json(output, &result)?;
    Ok(format!(
        "VAHMAN robust aggregation: {} candidates, {}",
        result.len(),
        absolute(output)
    ))
}

fn rank(input: &str, output: &str) -> Result<String, String> {
    let input = read_json::<UncertaintyInput>(input, "uncertainty-ranking envelope")?;
    let result = uncertainty_aware_ranking(
        &input.objective_directions,
        input.constraint_count,
        &input.evaluations,
        input.interval_multiplier,
    )
    .map_err(|error| error.to_string())?;
    write_json(output, &result)?;
    Ok(format!(
        "VAHMAN uncertainty ranking: {} candidates, {}",
        result.len(),
        absolute(output)
    ))
}

fn fidelity(input: &str, output: &str) -> Result<String, String> {
    let input = read_json::<MultiFidelityInput>(input, "multi-fidelity envelope")?;
    let result = recommend_reference_evaluations(
        &input.objective_directions,
        &input.candidates,
        &input.observations,
        &input.policy,
    )
    .map_err(|error| error.to_string())?;
    write_json(output, &result)?;
    Ok(format!(
        "VAHMAN multi-fidelity selection: {} reference jobs, hash {}, {}",
        result.recommendations.len(),
        result.content_hash,
        absolute(output)
    ))
}

fn read_json<T: DeserializeOwned>(path: &str, label: &str) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("cannot read {label}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("invalid {label} JSON: {error}"))
}

fn write_json<T: serde::Serialize>(path: &str, value: &T) -> Result<(), String> {
    let encoded = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize result: {error}"))?;
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create output directory: {error}"))?;
    }
    fs::write(path, encoded).map_err(|error| format!("cannot write result: {error}"))
}

fn parse_usize(value: &str, label: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid {label}: {value}"))
}

fn absolute(path: &str) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| Path::new(path).to_path_buf())
        .display()
        .to_string()
}
