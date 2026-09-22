//! Advanced DAENA and XVARNA Study command-line workflows.

use std::process::ExitCode;
use xvarna_daena::{
    ObserverPath, ObserverPathOptions, TargetViewOptions, ViewCorridor, ViewCorridorOptions,
    ViewObserver, ViewTargetPatch, analyze_observer_paths, analyze_observer_paths_with_executor,
    analyze_target_view, analyze_target_view_with_executor, analyze_view_corridors,
    analyze_view_corridors_with_executor,
};
use xvarna_geometry::Vec3;
use xvarna_study::{
    Candidate, Evaluation, MultiObjectiveOptimizer, ObjectiveDirection, OptimizerConfig,
    ProblemSpec, VariableKind, VariableSpec, hypervolume_2d, rank_study, spearman_sensitivity,
};
use xvarna_types::{SensorId, TargetId};

use super::{
    build_cli_scene, format_hash, parse_category_mask, parse_comma_values, parse_nonnegative,
    parse_positive,
};
use crate::compute::{
    DomainBackendOptions, print_domain_execution_json, print_domain_execution_text,
};

#[derive(Clone, Debug)]
struct TargetCliOptions {
    observers: Vec<ViewObserver>,
    patches: Vec<ViewTargetPatch>,
    view: TargetViewOptions,
    thread_count: usize,
    backend: DomainBackendOptions,
    verify_cpu: bool,
    parity_tolerance: f64,
    json: bool,
}

#[derive(Clone, Copy, Debug)]
struct DomainParityReport {
    passed: bool,
    maximum_absolute_delta: f64,
    identity_mismatches: usize,
    content_hash_match: bool,
}

pub fn run_target_view(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_target_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "target-view") else {
        return ExitCode::FAILURE;
    };
    let cpu_reference = if options.verify_cpu {
        match analyze_target_view(&scene, &options.observers, &options.patches, options.view) {
            Ok(value) => Some(value),
            Err(error) => {
                eprintln!("error: CPU target-view reference failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let session = match options.backend.create_session(scene) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: VAYU session creation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match analyze_target_view_with_executor(
        &session,
        &options.observers,
        &options.patches,
        options.view,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: target-view failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let parity = cpu_reference
        .as_ref()
        .map(|reference| compare_target_view(reference, &result, options.parity_tolerance));
    if options.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA target-view\",\"hash\":\"{}\",",
            format_hash(&result.content_hash)
        );
        print_domain_execution_json(&result.execution);
        print_parity_json(parity);
        print!(",\"observers\":[");
        for (index, summary) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"id\":{},\"targetView\":{},\"weightedView\":{},\"greenView\":{},\"greenShare\":{},\"dominantTarget\":{},\"convergenceSteradians\":{}}}",
                summary.observer_id.get(),
                summary.target_view_fraction,
                summary.weighted_view_score,
                summary.green_view_index,
                summary.green_share_of_visible_targets,
                summary.dominant_target_id.get(),
                summary.convergence_delta_steradians
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA solid-angle Target / Weighted / Green View");
        println!(
            "Study: {} camera(s), {} target patch(es), {} logical target(s), {} samples/patch",
            options.observers.len(),
            options.patches.len(),
            result.target_ids.len(),
            options.view.samples_per_patch
        );
        for summary in &result.summaries {
            println!(
                "observer {}  target={:.10}  weighted={:.10}  green={:.10}  green-share={:.10}  dominant={}  delta={:.6e} sr",
                summary.observer_id.get(),
                summary.target_view_fraction,
                summary.weighted_view_score,
                summary.green_view_index,
                summary.green_share_of_visible_targets,
                summary.dominant_target_id.get(),
                summary.convergence_delta_steradians
            );
        }
        println!("Result hash: {}", format_hash(&result.content_hash));
        print_domain_execution_text(&result.execution);
        print_parity_text(parity, options.parity_tolerance);
    }
    parity.map_or(ExitCode::SUCCESS, |report| {
        if report.passed {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    })
}

fn parse_target_options(arguments: &[String]) -> Result<TargetCliOptions, ()> {
    let mut output = TargetCliOptions {
        observers: Vec::new(),
        patches: Vec::new(),
        view: TargetViewOptions::default(),
        thread_count: 0,
        backend: DomainBackendOptions::default(),
        verify_cpu: false,
        parity_tolerance: 1.0e-6,
        json: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if argument == "--json" {
            output.json = true;
            index += 1;
            continue;
        }
        if argument == "--two-sided" {
            output.view.two_sided_targets = true;
            index += 1;
            continue;
        }
        if argument == "--verify-cpu" {
            output.verify_cpu = true;
            index += 1;
            continue;
        }
        if argument == "--low-power" {
            output.backend.set_low_power();
            index += 1;
            continue;
        }
        if argument == "--no-cache" {
            output.backend.disable_cache();
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--observer" => output.observers.push(parse_view_observer(value)?),
            "--target" => output.patches.push(parse_target_patch(value)?),
            "--samples" => output.view.samples_per_patch = parse_usize(value, "samples")?,
            "--horizontal-fov" => {
                output.view.horizontal_fov_radians =
                    parse_positive(value, "horizontal-fov")?.to_radians();
            }
            "--vertical-fov" => {
                output.view.vertical_fov_radians =
                    parse_positive(value, "vertical-fov")?.to_radians();
            }
            "--max-distance" => {
                output.view.maximum_distance_meters = parse_positive(value, "max-distance")?;
            }
            "--clearance" => {
                output.view.endpoint_clearance_meters = parse_nonnegative(value, "clearance")?;
            }
            "--distance-reference" => {
                output.view.distance_reference_meters =
                    parse_positive(value, "distance-reference")?;
            }
            "--distance-exponent" => {
                output.view.distance_exponent = parse_nonnegative(value, "distance-exponent")?;
            }
            "--direction-exponent" => {
                output.view.direction_exponent = parse_nonnegative(value, "direction-exponent")?;
            }
            "--green-mask" => output.view.green_category_mask = parse_category_mask(value)?,
            "--target-mask" => output.view.target_category_mask = parse_category_mask(value)?,
            "--occluder-mask" => output.view.occluder_category_mask = parse_category_mask(value)?,
            "--threads" => output.thread_count = parse_usize(value, "threads")?,
            "--parity-tolerance" => {
                output.parity_tolerance = parse_nonnegative(value, "parity-tolerance")?;
            }
            unknown if output.backend.parse_value(unknown, value)? => {}
            unknown => {
                eprintln!("error: unknown target-view option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if output.observers.is_empty() || output.patches.is_empty() {
        eprintln!("error: target-view requires at least one --observer and --target");
        return Err(());
    }
    Ok(output)
}

fn parse_view_observer(value: &str) -> Result<ViewObserver, ()> {
    let values = parse_comma_values(value, "observer")?;
    if values.len() != 11 {
        eprintln!("error: observer needs id,x,y,z,fx,fy,fz,ux,uy,uz,weight");
        return Err(());
    }
    ViewObserver::try_new(
        SensorId::new(exact_u64(values[0], "observer id")?),
        Vec3::new(values[1], values[2], values[3]),
        Vec3::new(values[4], values[5], values[6]),
        Vec3::new(values[7], values[8], values[9]),
        values[10],
    )
    .map_err(|error| eprintln!("error: invalid observer: {error}"))
}

fn parse_target_patch(value: &str) -> Result<ViewTargetPatch, ()> {
    let values = parse_comma_values(value, "target")?;
    if values.len() != 12 {
        eprintln!("error: target needs id,ax,ay,az,bx,by,bz,cx,cy,cz,category,weight");
        return Err(());
    }
    ViewTargetPatch::try_new(
        TargetId::new(exact_u64(values[0], "target id")?),
        [
            Vec3::new(values[1], values[2], values[3]),
            Vec3::new(values[4], values[5], values[6]),
            Vec3::new(values[7], values[8], values[9]),
        ],
        exact_u64(values[10], "target category")?,
        values[11],
    )
    .map_err(|error| eprintln!("error: invalid target: {error}"))
}

fn compare_target_view(
    reference: &xvarna_daena::TargetViewResult,
    candidate: &xvarna_daena::TargetViewResult,
    tolerance: f64,
) -> DomainParityReport {
    let mut maximum = 0.0_f64;
    let mut mismatches = reference
        .summaries
        .len()
        .abs_diff(candidate.summaries.len())
        + reference.entries.len().abs_diff(candidate.entries.len());
    for (left, right) in reference.summaries.iter().zip(&candidate.summaries) {
        mismatches += usize::from(
            left.observer_id != right.observer_id
                || left.dominant_target_id != right.dominant_target_id,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.fov_solid_angle_steradians,
                left.potential_target_solid_angle_steradians,
                left.visible_target_solid_angle_steradians,
                left.target_view_fraction,
                left.target_universe_visibility_fraction,
                left.weighted_view_score,
                left.green_view_index,
                left.green_share_of_visible_targets,
                left.convergence_delta_steradians,
            ],
            &[
                right.fov_solid_angle_steradians,
                right.potential_target_solid_angle_steradians,
                right.visible_target_solid_angle_steradians,
                right.target_view_fraction,
                right.target_universe_visibility_fraction,
                right.weighted_view_score,
                right.green_view_index,
                right.green_share_of_visible_targets,
                right.convergence_delta_steradians,
            ],
        );
    }
    for (left, right) in reference.entries.iter().zip(&candidate.entries) {
        mismatches += usize::from(
            left.observer_id != right.observer_id
                || left.target_id != right.target_id
                || left.category_mask != right.category_mask
                || left.eligible_sample_count != right.eligible_sample_count
                || left.visible_sample_count != right.visible_sample_count
                || left.dominant_blocker_object_id != right.dominant_blocker_object_id,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.potential_solid_angle_steradians,
                left.visible_solid_angle_steradians,
                left.visibility_fraction,
                left.fov_fraction,
                left.weighted_fov_score,
                left.convergence_delta_steradians,
                left.dominant_blocked_solid_angle_steradians,
            ],
            &[
                right.potential_solid_angle_steradians,
                right.visible_solid_angle_steradians,
                right.visibility_fraction,
                right.fov_fraction,
                right.weighted_fov_score,
                right.convergence_delta_steradians,
                right.dominant_blocked_solid_angle_steradians,
            ],
        );
    }
    DomainParityReport {
        passed: mismatches == 0 && maximum <= tolerance,
        maximum_absolute_delta: maximum,
        identity_mismatches: mismatches,
        content_hash_match: reference.content_hash == candidate.content_hash,
    }
}

fn absorb_delta(maximum: &mut f64, reference: &[f64], candidate: &[f64]) {
    for (left, right) in reference.iter().zip(candidate) {
        let delta = if left.is_finite() && right.is_finite() {
            (left - right).abs()
        } else if left.to_bits() == right.to_bits() {
            0.0
        } else {
            f64::INFINITY
        };
        *maximum = maximum.max(delta);
    }
}

fn print_parity_json(report: Option<DomainParityReport>) {
    if let Some(value) = report {
        print!(
            ",\"cpuParity\":{{\"passed\":{},\"maximumAbsoluteDelta\":{},\"identityMismatches\":{},\"contentHashMatch\":{}}}",
            value.passed,
            json_number(value.maximum_absolute_delta),
            value.identity_mismatches,
            value.content_hash_match
        );
    }
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_owned()
    }
}

fn print_parity_text(report: Option<DomainParityReport>, tolerance: f64) {
    if let Some(value) = report {
        println!(
            "CPU parity: {}  max-delta={:.6e}  tolerance={:.6e}  identity-mismatches={}  hash-match={}",
            if value.passed { "PASS" } else { "FAIL" },
            value.maximum_absolute_delta,
            tolerance,
            value.identity_mismatches,
            value.content_hash_match
        );
    }
}

#[derive(Clone, Debug)]
struct CorridorCliOptions {
    corridors: Vec<ViewCorridor>,
    policy: ViewCorridorOptions,
    thread_count: usize,
    backend: DomainBackendOptions,
    verify_cpu: bool,
    parity_tolerance: f64,
    json: bool,
}

pub fn run_view_corridor(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_corridor_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "view-corridor") else {
        return ExitCode::FAILURE;
    };
    let cpu_reference = if options.verify_cpu {
        match analyze_view_corridors(&scene, &options.corridors, options.policy) {
            Ok(value) => Some(value),
            Err(error) => {
                eprintln!("error: CPU view-corridor reference failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let session = match options.backend.create_session(scene) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: VAYU session creation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result =
        match analyze_view_corridors_with_executor(&session, &options.corridors, options.policy) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: view-corridor failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    let parity = cpu_reference
        .as_ref()
        .map(|reference| compare_view_corridors(reference, &result, options.parity_tolerance));
    if options.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA view-corridor\",\"hash\":\"{}\",",
            format_hash(&result.content_hash)
        );
        print_domain_execution_json(&result.execution);
        print_parity_json(parity);
        print!(",\"corridors\":[");
        for (index, summary) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"id\":{},\"openFraction\":{},\"openSteradians\":{},\"dominantBlocker\":{},\"blockerShare\":{},\"nearestBlocker\":{},\"convergence\":{}}}",
                summary.corridor_id,
                summary.open_fraction,
                summary.open_solid_angle_steradians,
                summary.dominant_blocker_object_id.get(),
                summary.dominant_blocker_fraction,
                json_number(summary.nearest_blocker_distance_meters),
                summary.convergence_delta
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA protected View Corridor");
        println!(
            "Study: {} corridor(s) × {} aperture samples",
            options.corridors.len(),
            options.policy.sample_count
        );
        for summary in &result.summaries {
            println!(
                "corridor {}  open={:.10}  open-omega={:.10} sr  blocker={}  share={:.10}  nearest={:.10} m  delta={:.6e}",
                summary.corridor_id,
                summary.open_fraction,
                summary.open_solid_angle_steradians,
                summary.dominant_blocker_object_id.get(),
                summary.dominant_blocker_fraction,
                summary.nearest_blocker_distance_meters,
                summary.convergence_delta
            );
        }
        println!("Result hash: {}", format_hash(&result.content_hash));
        print_domain_execution_text(&result.execution);
        print_parity_text(parity, options.parity_tolerance);
    }
    parity.map_or(ExitCode::SUCCESS, |report| {
        if report.passed {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    })
}

fn parse_corridor_options(arguments: &[String]) -> Result<CorridorCliOptions, ()> {
    let mut output = CorridorCliOptions {
        corridors: Vec::new(),
        policy: ViewCorridorOptions::default(),
        thread_count: 0,
        backend: DomainBackendOptions::default(),
        verify_cpu: false,
        parity_tolerance: 1.0e-6,
        json: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if argument == "--json" {
            output.json = true;
            index += 1;
            continue;
        }
        if argument == "--low-power" {
            output.backend.set_low_power();
            index += 1;
            continue;
        }
        if argument == "--no-cache" {
            output.backend.disable_cache();
            index += 1;
            continue;
        }
        if argument == "--verify-cpu" {
            output.verify_cpu = true;
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--corridor" => output.corridors.push(parse_corridor(value)?),
            "--samples" => output.policy.sample_count = parse_usize(value, "samples")?,
            "--clearance" => {
                output.policy.endpoint_clearance_meters = parse_nonnegative(value, "clearance")?;
            }
            "--category-mask" => output.policy.category_mask = parse_category_mask(value)?,
            "--threads" => output.thread_count = parse_usize(value, "threads")?,
            "--parity-tolerance" => {
                output.parity_tolerance = parse_nonnegative(value, "parity-tolerance")?;
            }
            unknown if output.backend.parse_value(unknown, value)? => {}
            unknown => {
                eprintln!("error: unknown view-corridor option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if output.corridors.is_empty() {
        eprintln!("error: repeat --corridor at least once");
        return Err(());
    }
    Ok(output)
}

fn parse_corridor(value: &str) -> Result<ViewCorridor, ()> {
    let values = parse_comma_values(value, "corridor")?;
    if values.len() != 11 {
        eprintln!("error: corridor needs id,ox,oy,oz,tx,ty,tz,ux,uy,uz,radius");
        return Err(());
    }
    ViewCorridor::try_new(
        exact_u64(values[0], "corridor id")?,
        Vec3::new(values[1], values[2], values[3]),
        Vec3::new(values[4], values[5], values[6]),
        Vec3::new(values[7], values[8], values[9]),
        values[10],
    )
    .map_err(|error| eprintln!("error: invalid corridor: {error}"))
}

fn compare_view_corridors(
    reference: &xvarna_daena::ViewCorridorResult,
    candidate: &xvarna_daena::ViewCorridorResult,
    tolerance: f64,
) -> DomainParityReport {
    let mut maximum = 0.0_f64;
    let mut mismatches = reference
        .summaries
        .len()
        .abs_diff(candidate.summaries.len())
        + reference.samples.len().abs_diff(candidate.samples.len());
    for (left, right) in reference.summaries.iter().zip(&candidate.summaries) {
        mismatches += usize::from(
            left.corridor_id != right.corridor_id
                || left.open_sample_count != right.open_sample_count
                || left.blocked_sample_count != right.blocked_sample_count
                || left.dominant_blocker_object_id != right.dominant_blocker_object_id,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.aperture_solid_angle_steradians,
                left.open_fraction,
                left.open_solid_angle_steradians,
                left.convergence_delta,
                left.dominant_blocker_fraction,
                left.nearest_blocker_distance_meters,
            ],
            &[
                right.aperture_solid_angle_steradians,
                right.open_fraction,
                right.open_solid_angle_steradians,
                right.convergence_delta,
                right.dominant_blocker_fraction,
                right.nearest_blocker_distance_meters,
            ],
        );
    }
    for (left, right) in reference.samples.iter().zip(&candidate.samples) {
        mismatches += usize::from(
            left.corridor_id != right.corridor_id
                || left.state != right.state
                || left.blocker_object_id != right.blocker_object_id
                || left.blocker_instance_id != right.blocker_instance_id
                || left.blocker_mesh_id != right.blocker_mesh_id
                || left.blocker_triangle_id != right.blocker_triangle_id,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.aperture_distance_meters,
                left.first_hit_distance_meters,
            ],
            &[
                right.aperture_distance_meters,
                right.first_hit_distance_meters,
            ],
        );
    }
    DomainParityReport {
        passed: mismatches == 0 && maximum <= tolerance,
        maximum_absolute_delta: maximum,
        identity_mismatches: mismatches,
        content_hash_match: reference.content_hash == candidate.content_hash,
    }
}

#[derive(Clone, Debug)]
struct PathCliOptions {
    paths: Vec<ObserverPath>,
    patches: Vec<ViewTargetPatch>,
    policy: ObserverPathOptions,
    thread_count: usize,
    backend: DomainBackendOptions,
    verify_cpu: bool,
    parity_tolerance: f64,
    json: bool,
}

pub fn run_observer_path(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_path_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "observer-path") else {
        return ExitCode::FAILURE;
    };
    let cpu_reference = if options.verify_cpu {
        match analyze_observer_paths(&scene, &options.paths, &options.patches, options.policy) {
            Ok(value) => Some(value),
            Err(error) => {
                eprintln!("error: CPU observer-path reference failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let session = match options.backend.create_session(scene) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: VAYU session creation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match analyze_observer_paths_with_executor(
        &session,
        &options.paths,
        &options.patches,
        options.policy,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: observer-path failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let parity = cpu_reference
        .as_ref()
        .map(|reference| compare_observer_paths(reference, &result, options.parity_tolerance));
    if options.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA observer-path\",\"hash\":\"{}\",",
            format_hash(&result.content_hash)
        );
        print_domain_execution_json(&result.execution);
        print_parity_json(parity);
        print!(",\"paths\":[");
        for (index, summary) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"id\":{},\"length\":{},\"samples\":{},\"meanTargetView\":{},\"meanWeightedView\":{},\"meanGreenView\":{},\"worstIndex\":{},\"bestIndex\":{}}}",
                summary.path_id,
                summary.path_length_meters,
                summary.sample_count,
                summary.mean_target_view_fraction,
                summary.mean_weighted_view_score,
                summary.mean_green_view_index,
                summary.worst_sample_index,
                summary.best_sample_index
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA Dynamic Observer Path");
        println!(
            "Study: {} path(s), {} ordered sample(s)",
            options.paths.len(),
            result.samples.len()
        );
        for summary in &result.summaries {
            println!(
                "path {}  length={:.8} m  samples={}  target={:.10}  weighted={:.10}  green={:.10}  worst={}  best={}",
                summary.path_id,
                summary.path_length_meters,
                summary.sample_count,
                summary.mean_target_view_fraction,
                summary.mean_weighted_view_score,
                summary.mean_green_view_index,
                summary.worst_sample_index,
                summary.best_sample_index
            );
        }
        println!("Result hash: {}", format_hash(&result.content_hash));
        print_domain_execution_text(&result.execution);
        print_parity_text(parity, options.parity_tolerance);
    }
    parity.map_or(ExitCode::SUCCESS, |report| {
        if report.passed {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    })
}

fn parse_path_options(arguments: &[String]) -> Result<PathCliOptions, ()> {
    let mut output = PathCliOptions {
        paths: Vec::new(),
        patches: Vec::new(),
        policy: ObserverPathOptions::default(),
        thread_count: 0,
        backend: DomainBackendOptions::default(),
        verify_cpu: false,
        parity_tolerance: 1.0e-6,
        json: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if argument == "--json" {
            output.json = true;
            index += 1;
            continue;
        }
        if argument == "--two-sided" {
            output.policy.view.two_sided_targets = true;
            index += 1;
            continue;
        }
        if argument == "--low-power" {
            output.backend.set_low_power();
            index += 1;
            continue;
        }
        if argument == "--no-cache" {
            output.backend.disable_cache();
            index += 1;
            continue;
        }
        if argument == "--verify-cpu" {
            output.verify_cpu = true;
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--path" => output.paths.push(parse_path(value)?),
            "--target" => output.patches.push(parse_target_patch(value)?),
            "--spacing" => output.policy.spacing_meters = parse_positive(value, "spacing")?,
            "--samples" => output.policy.view.samples_per_patch = parse_usize(value, "samples")?,
            "--horizontal-fov" => {
                output.policy.view.horizontal_fov_radians =
                    parse_positive(value, "horizontal-fov")?.to_radians();
            }
            "--vertical-fov" => {
                output.policy.view.vertical_fov_radians =
                    parse_positive(value, "vertical-fov")?.to_radians();
            }
            "--max-distance" => {
                output.policy.view.maximum_distance_meters = parse_positive(value, "max-distance")?;
            }
            "--distance-reference" => {
                output.policy.view.distance_reference_meters =
                    parse_positive(value, "distance-reference")?;
            }
            "--green-mask" => output.policy.view.green_category_mask = parse_category_mask(value)?,
            "--threads" => output.thread_count = parse_usize(value, "threads")?,
            "--parity-tolerance" => {
                output.parity_tolerance = parse_nonnegative(value, "parity-tolerance")?;
            }
            unknown if output.backend.parse_value(unknown, value)? => {}
            unknown => {
                eprintln!("error: unknown observer-path option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if output.paths.is_empty() || output.patches.is_empty() {
        eprintln!("error: observer-path requires --path and --target");
        return Err(());
    }
    Ok(output)
}

fn parse_path(value: &str) -> Result<ObserverPath, ()> {
    let mut groups = value.split(';');
    let Some(header) = groups.next() else {
        return Err(());
    };
    let header = parse_comma_values(header, "path header")?;
    if header.len() != 5 {
        eprintln!("error: path header needs id,upx,upy,upz,weight");
        return Err(());
    }
    let vertices = groups
        .map(|group| {
            let values = parse_comma_values(group, "path vertex")?;
            if values.len() != 3 {
                eprintln!("error: every path vertex needs x,y,z");
                return Err(());
            }
            Ok(Vec3::new(values[0], values[1], values[2]))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    ObserverPath::try_new(
        exact_u64(header[0], "path id")?,
        vertices,
        Vec3::new(header[1], header[2], header[3]),
        header[4],
    )
    .map_err(|error| eprintln!("error: invalid path: {error}"))
}

fn compare_observer_paths(
    reference: &xvarna_daena::ObserverPathResult,
    candidate: &xvarna_daena::ObserverPathResult,
    tolerance: f64,
) -> DomainParityReport {
    let mut maximum = 0.0_f64;
    let mut mismatches = reference
        .summaries
        .len()
        .abs_diff(candidate.summaries.len())
        + reference.samples.len().abs_diff(candidate.samples.len());
    for (left, right) in reference.summaries.iter().zip(&candidate.summaries) {
        mismatches += usize::from(
            left.path_id != right.path_id
                || left.sample_count != right.sample_count
                || left.worst_sample_index != right.worst_sample_index
                || left.best_sample_index != right.best_sample_index,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.path_length_meters,
                left.mean_target_view_fraction,
                left.mean_weighted_view_score,
                left.mean_green_view_index,
                left.minimum_weighted_view_score,
                left.maximum_weighted_view_score,
            ],
            &[
                right.path_length_meters,
                right.mean_target_view_fraction,
                right.mean_weighted_view_score,
                right.mean_green_view_index,
                right.minimum_weighted_view_score,
                right.maximum_weighted_view_score,
            ],
        );
    }
    for (left, right) in reference.samples.iter().zip(&candidate.samples) {
        mismatches += usize::from(
            left.path_id != right.path_id
                || left.sample_index != right.sample_index
                || left.dominant_target_id != right.dominant_target_id,
        );
        absorb_delta(
            &mut maximum,
            &[
                left.distance_along_path_meters,
                left.target_view_fraction,
                left.weighted_view_score,
                left.green_view_index,
                left.convergence_delta_steradians,
            ],
            &[
                right.distance_along_path_meters,
                right.target_view_fraction,
                right.weighted_view_score,
                right.green_view_index,
                right.convergence_delta_steradians,
            ],
        );
    }
    DomainParityReport {
        passed: mismatches == 0 && maximum <= tolerance,
        maximum_absolute_delta: maximum,
        identity_mismatches: mismatches,
        content_hash_match: reference.content_hash == candidate.content_hash,
    }
}

#[derive(Debug)]
struct StudyCliOptions {
    variables: Vec<VariableSpec>,
    directions: Vec<ObjectiveDirection>,
    candidates: Vec<Candidate>,
    evaluations: Vec<Evaluation>,
    reference: Option<[f64; 2]>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
pub fn run_study_rank(arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_study_options(arguments) else {
        return ExitCode::from(2);
    };
    let constraint_count = options
        .evaluations
        .first()
        .map_or(0, |value| value.constraint_residuals.len());
    let problem =
        match ProblemSpec::try_new(options.variables, options.directions, constraint_count) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: invalid study: {error}");
                return ExitCode::from(2);
            }
        };
    let result = match rank_study(&problem, &options.candidates, &options.evaluations) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: study ranking failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let sensitivity =
        match spearman_sensitivity(&problem, &options.candidates, &options.evaluations) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: sensitivity failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    let hypervolume = options
        .reference
        .map(|reference| {
            hypervolume_2d(
                &problem,
                &options.candidates,
                &options.evaluations,
                reference,
            )
        })
        .transpose()
        .unwrap_or_else(|error| {
            eprintln!("warning: hypervolume unavailable: {error}");
            None
        });
    if options.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"VAHMAN Study\",\"hash\":\"{}\",\"fronts\":{},\"paretoIds\":[",
            format_hash(&result.content_hash),
            result.front_count
        );
        for (index, id) in result.pareto_candidate_ids.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!("{id}");
        }
        print!(
            "],\"hypervolume2d\":{},\"solutions\":[",
            hypervolume.map_or_else(|| "null".to_owned(), |value| value.to_string())
        );
        for (index, solution) in result.solutions.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"id\":{},\"rank\":{},\"feasible\":{},\"violation\":{},\"crowding\":{}}}",
                solution.candidate_id,
                solution.pareto_rank,
                solution.feasible,
                solution.total_constraint_violation,
                if solution.crowding_distance.is_finite() {
                    solution.crowding_distance.to_string()
                } else {
                    "null".to_owned()
                }
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA VAHMAN constraint-aware Pareto analysis");
        println!(
            "Study: {} variants, {} parameter(s), {} objective(s), {} constraint(s)",
            options.candidates.len(),
            problem.variables.len(),
            problem.objective_directions.len(),
            problem.constraint_count
        );
        println!(
            "Fronts: {}; feasible Pareto set: {:?}",
            result.front_count, result.pareto_candidate_ids
        );
        if let Some(value) = hypervolume {
            println!("Exact 2D hypervolume: {value:.12}");
        }
        for solution in &result.solutions {
            println!(
                "candidate {}  rank={}  feasible={}  violation={:.8}  crowding={}",
                solution.candidate_id,
                solution.pareto_rank,
                solution.feasible,
                solution.total_constraint_violation,
                solution.crowding_distance
            );
        }
        println!("Spearman coefficients (descriptive, not causal):");
        for coefficient in sensitivity {
            println!(
                "  variable {} → objective {}: rho={:.8}",
                coefficient.variable_index, coefficient.objective_index, coefficient.spearman_rho
            );
        }
        println!("Result hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

fn parse_study_options(arguments: &[String]) -> Result<StudyCliOptions, ()> {
    let mut variables = Vec::new();
    let mut directions = Vec::new();
    let mut candidates = Vec::new();
    let mut evaluations = Vec::new();
    let mut reference = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if argument == "--json" {
            json = true;
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--variable" => variables.push(parse_variable(value)?),
            "--direction" => directions.push(match value.as_str() {
                "min" | "minimize" => ObjectiveDirection::Minimize,
                "max" | "maximize" => ObjectiveDirection::Maximize,
                _ => {
                    eprintln!("error: direction must be min or max");
                    return Err(());
                }
            }),
            "--candidate" => {
                let (candidate, evaluation) = parse_study_row(value)?;
                candidates.push(candidate);
                evaluations.push(evaluation);
            }
            "--reference" => {
                let values = parse_comma_values(value, "reference")?;
                if values.len() != 2 {
                    eprintln!("error: reference needs two values");
                    return Err(());
                }
                reference = Some([values[0], values[1]]);
            }
            unknown => {
                eprintln!("error: unknown study-rank option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if variables.is_empty() || directions.is_empty() || candidates.is_empty() {
        eprintln!("error: study-rank requires variables, directions, and candidates");
        return Err(());
    }
    let constraint_count = evaluations[0].constraint_residuals.len();
    if evaluations
        .iter()
        .any(|value| value.constraint_residuals.len() != constraint_count)
    {
        eprintln!("error: constraint vectors must have equal lengths");
        return Err(());
    }
    Ok(StudyCliOptions {
        variables,
        directions,
        candidates,
        evaluations,
        reference,
        json,
    })
}

fn parse_variable(value: &str) -> Result<VariableSpec, ()> {
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 {
        eprintln!("error: variable needs id,kind,lower,upper");
        return Err(());
    }
    let id = parts[0]
        .parse::<u64>()
        .map_err(|_| eprintln!("error: invalid variable id"))?;
    let kind = match parts[1] {
        "continuous" => VariableKind::Continuous,
        "integer" => VariableKind::Integer,
        "categorical" => VariableKind::Categorical,
        _ => {
            eprintln!("error: variable kind must be continuous, integer, or categorical");
            return Err(());
        }
    };
    let bounds = parse_comma_values(&format!("{},{}", parts[2], parts[3]), "bounds")?;
    VariableSpec::try_new(id, kind, bounds[0], bounds[1])
        .map_err(|error| eprintln!("error: invalid variable: {error}"))
}

fn parse_study_row(value: &str) -> Result<(Candidate, Evaluation), ()> {
    let groups = value.split(';').collect::<Vec<_>>();
    if groups.len() != 4 {
        eprintln!(
            "error: candidate needs id,generation;parameters;objectives;constraints (final group may be empty)"
        );
        return Err(());
    }
    let header = parse_comma_values(groups[0], "candidate header")?;
    if header.len() != 2 {
        eprintln!("error: candidate header needs id,generation");
        return Err(());
    }
    let candidate_id = exact_u64(header[0], "candidate id")?;
    let candidate = Candidate {
        candidate_id,
        generation: exact_u64(header[1], "generation")?,
        values: parse_comma_values(groups[1], "parameters")?,
    };
    let evaluation = Evaluation {
        candidate_id,
        objectives: parse_comma_values(groups[2], "objectives")?,
        constraint_residuals: if groups[3].trim().is_empty() {
            Vec::new()
        } else {
            parse_comma_values(groups[3], "constraints")?
        },
    };
    Ok((candidate, evaluation))
}

#[allow(clippy::too_many_lines)]
pub fn run_optimizer_benchmark(arguments: &[String]) -> ExitCode {
    let mut population = 64_usize;
    let mut generations = 20_usize;
    let mut seed = 42_u64;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--json" {
            json = true;
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {} requires a value", arguments[index]);
            return ExitCode::from(2);
        };
        match arguments[index].as_str() {
            "--population" => {
                population = match parse_usize(value, "population") {
                    Ok(value) => value,
                    Err(()) => return ExitCode::from(2),
                }
            }
            "--generations" => {
                generations = match parse_usize(value, "generations") {
                    Ok(value) => value,
                    Err(()) => return ExitCode::from(2),
                }
            }
            "--seed" => {
                seed = if let Ok(value) = value.parse() {
                    value
                } else {
                    eprintln!("error: seed must be an integer");
                    return ExitCode::from(2);
                };
            }
            unknown => {
                eprintln!("error: unknown optimizer-benchmark option '{unknown}'");
                return ExitCode::from(2);
            }
        }
        index += 2;
    }
    let problem = ProblemSpec::try_new(
        vec![
            VariableSpec::try_new(1, VariableKind::Continuous, 0.0, 1.0).expect("fixed variable"),
            VariableSpec::try_new(2, VariableKind::Integer, 0.0, 20.0).expect("fixed variable"),
            VariableSpec::try_new(3, VariableKind::Categorical, 0.0, 3.0).expect("fixed variable"),
        ],
        vec![ObjectiveDirection::Minimize, ObjectiveDirection::Maximize],
        1,
    )
    .expect("fixed problem");
    let config = OptimizerConfig {
        population_size: population,
        offspring_size: population,
        archive_capacity: population.saturating_mul(4),
        seed,
        ..OptimizerConfig::default()
    };
    let mut optimizer = match MultiObjectiveOptimizer::new(problem, config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: optimizer config failed: {error}");
            return ExitCode::from(2);
        }
    };
    let mut snapshot = None;
    for _ in 0..generations {
        let batch = match optimizer.ask() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: ask failed: {error}");
                return ExitCode::FAILURE;
            }
        };
        let evaluations = batch
            .iter()
            .map(|candidate| {
                let x = candidate.values[0];
                let integer = candidate.values[1];
                let category = candidate.values[2];
                Evaluation {
                    candidate_id: candidate.candidate_id,
                    objectives: vec![
                        category.mul_add(0.01, integer.mul_add(0.002, (x - 0.2).powi(2))),
                        category.mul_add(-0.02, 1.0 - (x - 0.8).abs()),
                    ],
                    constraint_residuals: vec![x + integer / 25.0 - 1.25],
                }
            })
            .collect::<Vec<_>>();
        snapshot = match optimizer.tell(&evaluations) {
            Ok(value) => Some(value),
            Err(error) => {
                eprintln!("error: tell failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    }
    let Some(snapshot) = snapshot else {
        eprintln!("error: generations must be positive");
        return ExitCode::from(2);
    };
    if json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"VAHMAN optimizer\",\"generation\":{},\"evaluations\":{},\"archiveCount\":{},\"meanCrowding\":{},\"hash\":\"{}\",\"paretoIds\":[",
            snapshot.generation,
            snapshot.evaluation_count,
            snapshot.archive_candidate_ids.len(),
            snapshot.mean_finite_crowding_distance,
            format_hash(&snapshot.content_hash)
        );
        for (index, id) in snapshot.population.pareto_candidate_ids.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!("{id}");
        }
        println!("]}}");
    } else {
        println!("XVARNA VAHMAN deterministic mixed-variable constrained optimizer");
        println!(
            "Generation: {}; evaluations: {}; population: {}; historical Pareto archive: {}",
            snapshot.generation,
            snapshot.evaluation_count,
            snapshot.population.solutions.len(),
            snapshot.archive_candidate_ids.len()
        );
        println!(
            "Current feasible Pareto IDs: {:?}",
            snapshot.population.pareto_candidate_ids
        );
        println!(
            "Mean finite crowding distance: {:.10}",
            snapshot.mean_finite_crowding_distance
        );
        println!("State hash: {}", format_hash(&snapshot.content_hash));
    }
    ExitCode::SUCCESS
}

fn parse_usize(value: &str, name: &str) -> Result<usize, ()> {
    value
        .parse()
        .map_err(|_| eprintln!("error: {name} must be a non-negative integer"))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn exact_u64(value: f64, name: &str) -> Result<u64, ()> {
    const MAXIMUM_EXACT_F64_INTEGER: f64 = 9_007_199_254_740_991.0;
    if value < 0.0 || value.fract() != 0.0 || value > MAXIMUM_EXACT_F64_INTEGER {
        eprintln!("error: {name} must be an exact unsigned integer");
        Err(())
    } else {
        Ok(value as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_target_cli_parses_camera_patch_and_policy() {
        let arguments = [
            "--observer",
            "1,0,0,0,1,0,0,0,0,1,1",
            "--target",
            "7,5,-1,-1,5,0,1,5,1,-1,2,0.8",
            "--green-mask",
            "2",
            "--samples",
            "64",
            "--backend",
            "auto",
            "--verify-cpu",
            "--parity-tolerance",
            "0.00001",
            "--two-sided",
            "--json",
        ]
        .map(str::to_owned);
        let options = parse_target_options(&arguments).expect("valid options");
        assert_eq!(options.observers.len(), 1);
        assert_eq!(options.patches.len(), 1);
        assert_eq!(options.view.samples_per_patch, 64);
        assert_eq!(options.view.green_category_mask, 2);
        assert!(options.verify_cpu);
        assert!((options.parity_tolerance - 0.000_01).abs() < f64::EPSILON);
        assert!(options.view.two_sided_targets && options.json);
    }

    #[test]
    fn study_cli_parses_mixed_variables_and_empty_constraints() {
        let arguments = [
            "--variable",
            "1,continuous,0,1",
            "--variable",
            "2,integer,0,10",
            "--direction",
            "min",
            "--direction",
            "max",
            "--candidate",
            "1,0;0.2,3;0.5,0.8;",
            "--candidate",
            "2,0;0.7,8;0.6,0.9;",
        ]
        .map(str::to_owned);
        let options = parse_study_options(&arguments).expect("valid study");
        assert_eq!(options.variables.len(), 2);
        assert_eq!(options.candidates.len(), 2);
        assert!(options.evaluations[0].constraint_residuals.is_empty());
    }

    #[test]
    fn observer_path_cli_parses_semicolon_polyline() {
        let path = parse_path("5,0,0,1,1;0,0,0;2,0,0;2,3,0").expect("valid path");
        assert_eq!(path.vertices.len(), 3);
        assert_eq!(path.path_id, 5);
    }

    #[test]
    fn every_advanced_domain_accepts_cpu_parity_policy() {
        let corridor = [
            "--corridor",
            "9,0,0,0,2,0,0,0,0,1,0.5",
            "--backend",
            "gpu",
            "--verify-cpu",
            "--parity-tolerance",
            "0.000001",
        ]
        .map(str::to_owned);
        let corridor_options = parse_corridor_options(&corridor).expect("valid corridor");
        assert!(corridor_options.verify_cpu);
        assert!((corridor_options.parity_tolerance - 1.0e-6).abs() < f64::EPSILON);

        let path = [
            "--path",
            "5,0,0,1,1;0,0,0;1,0,0",
            "--target",
            "7,2,-1,-1,2,0,1,2,1,-1,2,1",
            "--backend",
            "auto",
            "--verify-cpu",
            "--parity-tolerance",
            "0.000001",
        ]
        .map(str::to_owned);
        let path_options = parse_path_options(&path).expect("valid path");
        assert!(path_options.verify_cpu);
        assert!((path_options.parity_tolerance - 1.0e-6).abs() < f64::EPSILON);
    }
}
