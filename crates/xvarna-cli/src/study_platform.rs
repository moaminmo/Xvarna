//! XVARNA 0.16 persistent Study/Optimization/Report platform commands.

use serde::Deserialize;
use std::{fs, path::Path, process::ExitCode};
use xvarna_study::platform::{
    BatchEvaluation, SensitivityOptions, StudyManifest, StudyWorkspace, study_manifest_schema,
    write_report_package,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommitInput {
    batch_id: u64,
    evaluations: Vec<BatchEvaluation>,
}

pub fn run_study_schema(arguments: &[String]) -> ExitCode {
    if arguments.len() > 1 {
        eprintln!("error: study-schema accepts at most one output path");
        return ExitCode::from(2);
    }
    let encoded = serde_json::to_string_pretty(&study_manifest_schema()).expect("schema JSON");
    if let Some(path) = arguments.first() {
        if let Err(error) = fs::write(path, encoded) {
            eprintln!("error: cannot write schema: {error}");
            return ExitCode::FAILURE;
        }
        println!("Study manifest schema: {}", absolute(path));
    } else {
        println!("{encoded}");
    }
    ExitCode::SUCCESS
}

pub fn run_study_init(workspace: &str, manifest_path: &str, arguments: &[String]) -> ExitCode {
    let json = arguments.iter().any(|value| value == "--json");
    if arguments.iter().any(|value| value != "--json") {
        eprintln!("error: unknown study-init option");
        return ExitCode::from(2);
    }
    let Some(manifest) = read_json::<StudyManifest>(manifest_path, "manifest") else {
        return ExitCode::from(2);
    };
    let workspace = match StudyWorkspace::create(workspace, manifest) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: study workspace initialization failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!(
            "{}",
            serde_json::to_string(&workspace.resume_state()).expect("state JSON")
        );
    } else {
        println!("XVARNA VAHMAN Study workspace 0.16.0");
        println!("root: {}", workspace.root().display());
        println!("study: {}", workspace.manifest().study_id);
        println!("checkpoint: {}", workspace.resume_state().checkpoint_hash);
    }
    ExitCode::SUCCESS
}

pub fn run_study_status(workspace: &str, arguments: &[String]) -> ExitCode {
    let json = arguments.iter().any(|value| value == "--json");
    if arguments.iter().any(|value| value != "--json") {
        eprintln!("error: unknown study-status option");
        return ExitCode::from(2);
    }
    let Some(workspace) = open_workspace(workspace) else {
        return ExitCode::FAILURE;
    };
    let state = workspace.resume_state();
    if json {
        println!("{}", serde_json::to_string(&state).expect("state JSON"));
    } else {
        println!("XVARNA VAHMAN Study status 0.16.0");
        println!("study: {}", workspace.manifest().study_id);
        println!(
            "revision: {}; generation: {}; evaluations: {}",
            state.revision, state.generation, state.evaluation_count
        );
        println!(
            "active batch: {}",
            state
                .active_batch_id
                .map_or_else(|| "none".to_owned(), |value| value.to_string())
        );
        println!("pending: {}", state.pending_candidates.len());
        println!("ledger: {}", state.ledger_hash);
    }
    ExitCode::SUCCESS
}

pub fn run_study_batch(workspace: &str, arguments: &[String]) -> ExitCode {
    let mut output = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--output" if index + 1 < arguments.len() => {
                index += 1;
                output = Some(arguments[index].clone());
            }
            option => {
                eprintln!("error: unknown/incomplete study-batch option {option}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let Some(mut workspace) = open_workspace(workspace) else {
        return ExitCode::FAILURE;
    };
    let batch = match workspace.begin_batch() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: cannot begin/resume study batch: {error}");
            return ExitCode::FAILURE;
        }
    };
    let encoded = serde_json::to_string_pretty(&batch).expect("batch JSON");
    if let Some(ref path) = output
        && let Err(error) = fs::write(path, &encoded)
    {
        eprintln!("error: cannot write batch: {error}");
        return ExitCode::FAILURE;
    }
    if json {
        println!("{encoded}");
    } else {
        println!("XVARNA VAHMAN external evaluation batch 0.16.0");
        println!(
            "batch: {}; generation: {}",
            batch.batch_id, batch.generation
        );
        println!("candidates: {}", batch.candidates.len());
        if let Some(ref path) = output {
            println!("output: {}", absolute(path));
        }
        println!("hash: {}", batch.content_hash);
    }
    ExitCode::SUCCESS
}

pub fn run_study_commit(workspace: &str, evaluations_path: &str, arguments: &[String]) -> ExitCode {
    let json = arguments.iter().any(|value| value == "--json");
    if arguments.iter().any(|value| value != "--json") {
        eprintln!("error: unknown study-commit option");
        return ExitCode::from(2);
    }
    let Some(input) = read_json::<CommitInput>(evaluations_path, "batch evaluations") else {
        return ExitCode::from(2);
    };
    let Some(mut workspace) = open_workspace(workspace) else {
        return ExitCode::FAILURE;
    };
    let state = match workspace.commit_batch(input.batch_id, input.evaluations) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: cannot commit study batch: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!("{}", serde_json::to_string(&state).expect("state JSON"));
    } else {
        println!("XVARNA VAHMAN batch committed");
        println!(
            "generation: {}; evaluations: {}; variants: {}",
            state.generation, state.evaluation_count, state.variant_count
        );
        println!("checkpoint: {}", state.checkpoint_hash);
    }
    ExitCode::SUCCESS
}

pub fn run_study_report(workspace: &str, arguments: &[String]) -> ExitCode {
    let mut options = SensitivityOptions::default();
    let mut output = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        if option == "--json" {
            json = true;
            index += 1;
            continue;
        }
        if index + 1 >= arguments.len() {
            eprintln!("error: incomplete study-report option {option}");
            return ExitCode::from(2);
        }
        index += 1;
        let value = &arguments[index];
        let parsed = match option {
            "--output" => {
                output = Some(value.clone());
                Ok(())
            }
            "--bootstrap" => parse_usize(value, "bootstrap").map(|v| options.bootstrap_samples = v),
            "--permutations" => {
                parse_usize(value, "permutations").map(|v| options.permutation_samples = v)
            }
            "--confidence" => parse_f64(value, "confidence").map(|v| options.confidence_level = v),
            "--alpha" => parse_f64(value, "alpha").map(|v| options.alpha = v),
            "--seed" => value
                .parse::<u64>()
                .map(|v| options.seed = v)
                .map_err(|_| format!("invalid seed: {value}")),
            _ => Err(format!("unknown study-report option {option}")),
        };
        if let Err(error) = parsed {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
        index += 1;
    }
    let Some(workspace) = open_workspace(workspace) else {
        return ExitCode::FAILURE;
    };
    let analysis = match workspace.analyze(options) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: study analysis failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let output = output.map_or_else(|| workspace.root().join("report"), Into::into);
    let artifacts =
        match write_report_package(&output, workspace.manifest(), workspace.ledger(), &analysis) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: study report export failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    if json {
        println!(
            "{{\"schemaVersion\":\"0.16.0\",\"studyId\":{},\"variants\":{},\"pareto\":{},\"analysisHash\":{},\"report\":{},\"viewer\":{},\"parquet\":{},\"gltf\":{}}}",
            quoted(&workspace.manifest().study_id),
            workspace.ledger().evaluations.len(),
            analysis.pareto_variant_ids.len(),
            quoted(&analysis.content_hash),
            quoted(&artifacts.html_report.display().to_string()),
            quoted(&artifacts.standalone_viewer.display().to_string()),
            quoted(&artifacts.variants_parquet.display().to_string()),
            quoted(&artifacts.objective_space_gltf.display().to_string()),
        );
    } else {
        println!("XVARNA VAHMAN Study Report 0.16.0");
        println!(
            "evaluated: {}; Pareto: {}; fronts: {}",
            workspace.ledger().evaluations.len(),
            analysis.pareto_variant_ids.len(),
            analysis.front_count
        );
        println!("report: {}", artifacts.html_report.display());
        println!("viewer: {}", artifacts.standalone_viewer.display());
        println!("Parquet: {}", artifacts.variants_parquet.display());
        println!("glTF: {}", artifacts.objective_space_gltf.display());
        println!("hash: {}", analysis.content_hash);
    }
    ExitCode::SUCCESS
}

fn open_workspace(path: &str) -> Option<StudyWorkspace> {
    StudyWorkspace::open(path)
        .map_err(|error| eprintln!("error: cannot open study workspace: {error}"))
        .ok()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &str, name: &str) -> Option<T> {
    fs::read(path)
        .map_err(|error| eprintln!("error: cannot read {name}: {error}"))
        .and_then(|bytes| {
            serde_json::from_slice(&bytes)
                .map_err(|error| eprintln!("error: invalid {name} JSON: {error}"))
        })
        .ok()
}

fn parse_usize(value: &str, name: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn parse_f64(value: &str, name: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("invalid {name}: {value}"))
}

fn absolute(path: &str) -> String {
    fs::canonicalize(Path::new(path))
        .unwrap_or_else(|_| Path::new(path).to_path_buf())
        .display()
        .to_string()
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("JSON string")
}
