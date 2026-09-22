//! Command-line entry point for XVARNA engineering builds.

mod advanced;
mod compute;
mod daylight;
mod evidence;
mod evidence_table;
mod intelligence;
mod interop;
mod study_platform;

use advanced::{
    run_observer_path, run_optimizer_benchmark, run_study_rank, run_target_view, run_view_corridor,
};
use chrono::{DateTime, Utc};
use compute::{run_backend_benchmark, run_cache, run_devices};
use core::f64::consts::TAU;
use daylight::{
    run_annual_daylight, run_daylight_compare, run_daylight_factor, run_daylight_point,
    run_radiance_annual_reference, run_radiance_export, run_radiance_point_reference,
};
use evidence::run_evidence;
use evidence_table::run_evidence_table;
use intelligence::{
    run_isovist_3d, run_landmark_visibility, run_solar_envelope, run_solar_scenarios,
};
use interop::{run_mesh_convert, run_scene_pack, run_scene_unpack, run_weather_convert};
use std::{
    fmt::Write as _,
    fs,
    io::{BufWriter, Write as _},
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};
use study_platform::{
    run_study_batch, run_study_commit, run_study_init, run_study_report, run_study_schema,
    run_study_status,
};
use xvarna_daena::{
    IntervisibilityOptions, IsovistOptions, IsovistRayState, SparseVisibilityGraphOptions,
    Viewpoint, VisibilityGraphOptions, VisibilityNode, VisibilityObserver, VisibilityState,
    VisibilityTarget, analyze_intervisibility, analyze_isovists, analyze_sparse_visibility_graph,
    analyze_visibility_graph,
};
use xvarna_geometry::{
    Mesh, MeshAuditOptions, MeshAuditReport, Ray, SurfaceGridOptions, Triangle, Vec3, audit_mesh,
    generate_surface_grid, intersect_triangle_reference, repair_mesh_conservative,
};
use xvarna_hvare::{
    AnnualIrradianceOptions, SkyRayState, SkyViewOptions, SolarSensor, analyze_annual_irradiance,
    analyze_sky_view,
};
use xvarna_io::{parse_obj, write_obj};
use xvarna_scene::{QueryRay, Scene, SceneBuildOptions, SceneBuilder, Transform};
use xvarna_types::{ABI_VERSION, InstanceId, ObjectId, SensorId, TargetId, engine_version};
use xvarna_zurvan::{SolarLocation, SolarOptions, TimeSample, calculate_sun_set, parse_epw};

#[allow(clippy::too_many_lines)]
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "help" || arg == "--help" || arg == "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("xvarna {}", engine_version());
            ExitCode::SUCCESS
        }
        [command] if command == "info" => {
            print_info(false);
            ExitCode::SUCCESS
        }
        [command, format] if command == "info" && format == "--json" => {
            print_info(true);
            ExitCode::SUCCESS
        }
        [command] if command == "self-test" => run_self_test(),
        [command, remaining @ ..] if command == "devices" => run_devices(remaining),
        [command, remaining @ ..] if command == "cache" => run_cache(remaining),
        [command, input, remaining @ ..] if command == "backend-benchmark" => {
            run_backend_benchmark(input, remaining)
        }
        [command, remaining @ ..] if command == "sun-position" => run_sun_position(remaining),
        [command, input, remaining @ ..] if command == "sky-view" => run_sky_view(input, remaining),
        [command, input, remaining @ ..] if command == "isovist" => run_isovist(input, remaining),
        [command, input, remaining @ ..] if command == "isovist-3d" => {
            run_isovist_3d(input, remaining)
        }
        [command, input, remaining @ ..] if command == "landmark-visibility" => {
            run_landmark_visibility(input, remaining)
        }
        [command, input, remaining @ ..] if command == "solar-envelope" => {
            run_solar_envelope(input, remaining)
        }
        [command, input, remaining @ ..] if command == "solar-scenarios" => {
            run_solar_scenarios(input, remaining)
        }
        [command, input, remaining @ ..] if command == "intervisibility" => {
            run_intervisibility(input, remaining)
        }
        [command, input, remaining @ ..] if command == "visibility-graph" => {
            run_visibility_graph(input, remaining)
        }
        [command, input, remaining @ ..] if command == "target-view" => {
            run_target_view(input, remaining)
        }
        [command, input, remaining @ ..] if command == "view-corridor" => {
            run_view_corridor(input, remaining)
        }
        [command, input, remaining @ ..] if command == "observer-path" => {
            run_observer_path(input, remaining)
        }
        [command, remaining @ ..] if command == "study-rank" => run_study_rank(remaining),
        [command, remaining @ ..] if command == "evidence" => run_evidence(remaining),
        [command, input, output] if command == "evidence-table" => {
            run_evidence_table(input, output)
        }
        [command, input, output] if command == "daylight-evidence-table" => {
            evidence_table::run_daylight_evidence_table(input, output)
        }
        [command, remaining @ ..] if command == "study-schema" => run_study_schema(remaining),
        [command, workspace, manifest, remaining @ ..] if command == "study-init" => {
            run_study_init(workspace, manifest, remaining)
        }
        [command, workspace, remaining @ ..] if command == "study-status" => {
            run_study_status(workspace, remaining)
        }
        [command, workspace, remaining @ ..] if command == "study-batch" => {
            run_study_batch(workspace, remaining)
        }
        [command, workspace, evaluations, remaining @ ..] if command == "study-commit" => {
            run_study_commit(workspace, evaluations, remaining)
        }
        [command, workspace, remaining @ ..] if command == "study-report" => {
            run_study_report(workspace, remaining)
        }
        [command, remaining @ ..] if command == "optimizer-benchmark" => {
            run_optimizer_benchmark(remaining)
        }
        [command, input, weather, remaining @ ..] if command == "annual-irradiance" => {
            run_annual_irradiance(input, weather, remaining)
        }
        [command, input, remaining @ ..] if command == "daylight-point" => {
            run_daylight_point(input, remaining)
        }
        [command, input, remaining @ ..] if command == "daylight-factor" => {
            run_daylight_factor(input, remaining)
        }
        [command, input, weather, remaining @ ..] if command == "annual-daylight" => {
            run_annual_daylight(input, weather, remaining)
        }
        [command, input, output, remaining @ ..] if command == "radiance-export" => {
            run_radiance_export(input, output, remaining)
        }
        [command, input, remaining @ ..] if command == "radiance-point" => {
            run_radiance_point_reference(input, remaining)
        }
        [command, input, weather, remaining @ ..] if command == "radiance-annual" => {
            run_radiance_annual_reference(input, weather, remaining)
        }
        [command, fast, reference, remaining @ ..] if command == "daylight-compare" => {
            run_daylight_compare(fast, reference, remaining)
        }
        [command, input, remaining @ ..] if command == "mesh-check" => {
            run_mesh_check(input, remaining)
        }
        [command, input, output, remaining @ ..] if command == "surface-grid" => {
            run_surface_grid(input, output, remaining)
        }
        [command, input, output, remaining @ ..] if command == "mesh-repair" => {
            run_mesh_repair(input, output, remaining)
        }
        [command, input, output, remaining @ ..] if command == "mesh-convert" => {
            run_mesh_convert(input, output, remaining)
        }
        [command, input, output, remaining @ ..] if command == "scene-pack" => {
            run_scene_pack(input, output, remaining)
        }
        [command, input, output, remaining @ ..] if command == "scene-unpack" => {
            run_scene_unpack(input, output, remaining)
        }
        [command, input, output, remaining @ ..] if command == "weather-convert" => {
            run_weather_convert(input, output, remaining)
        }
        [command, input, remaining @ ..] if command == "scene-bench" => {
            run_scene_benchmark(input, remaining)
        }
        _ => {
            eprintln!("error: unknown command\n");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "XVARNA engineering CLI\n\nUSAGE:\n    xvarna <COMMAND>\n\nCOMMANDS:\n    info [--json]\n        Print engine and host information\n    devices [--json]\n        Enumerate VAYU portable GPU adapters and drivers\n    cache status [--json] | clear | configure <DIR> <MEMORY_BYTES> <DISK_BYTES>\n        Inspect or configure the checksummed bounded VAYU scene cache\n    backend-benchmark <INPUT.obj> [--backend auto|gpu|cpu] [--adapter <NAME>] [--rays <COUNT>] [--warmup <N>] [--iterations <N>] [--json]\n        Run cold/warm CPU/GPU parity, persistent-arena telemetry, fallback, and throughput validation\n    sun-position --latitude <DEG> --longitude <DEG> --time <RFC3339> [...]\n        Calculate high-accuracy HVARE solar positions with explicit UTC timestamps\n    sky-view <INPUT.obj> --sensor <x,y,z,nx,ny,nz> [...] [--samples <COUNT>] [--json]\n        Compute ASMAN Sky View Factor and Shadow Mask attribution\n    isovist <INPUT.obj> --viewpoint <x,y,z,nx,ny,nz,fx,fy,fz> [...] [--json]\n        Compute DAENA planar isovists and first-blocker attribution\n    isovist-3d <INPUT.obj> --viewpoint <x,y,z> [...] [--material <id,Tv,Ts,R>] [--assign <object,material>] [--json]\n        Compute DAENA volumetric isovists, general attribution, and exact remove-one counterfactuals\n    landmark-visibility <INPUT.obj> --observer <id,x,y,z,fx,fy,fz,ux,uy,uz> --landmark <id,x,y,z,radius,weight> [...] [--json]\n        Compute material-aware partial landmark visibility and solid-angle scores\n    solar-envelope <INPUT.obj> --sensor <id,x,y,z,nx,ny,nz> --candidate <id,x,y,z> --sun <unix,dx,dy,dz,duration,weight> [...] [--json]\n        Compute early-massing solar-access and target-shading thresholds\n    solar-scenarios <INPUT.obj> --sensor <...> --sun <...> --scenario <id> [--material ... --assign ...] [...] [--json]\n        Compare optical alternatives with timeline, attribution, and baseline deltas\n    intervisibility <INPUT.obj> --observer <x,y,z,weight> [...] --target <x,y,z,sensitivity[,fx,fy,fz]> [...] [--json]\n        Compute the DAENA directed visibility and privacy matrix\n    visibility-graph <INPUT.obj> --node <x,y,z> [...] [--json]\n        Build the DAENA all-pairs visibility graph and topology\n    target-view <INPUT.obj> --observer <id,x,y,z,fx,fy,fz,ux,uy,uz,weight> --target <id,ax,ay,az,bx,by,bz,cx,cy,cz,category,weight> [...] [--json]\n        Compute solid-angle Target, Weighted, and Green View\n    view-corridor <INPUT.obj> --corridor <id,ox,oy,oz,tx,ty,tz,ux,uy,uz,radius> [...] [--json]\n        Test protected target-aperture corridors and attribute conflicts\n    observer-path <INPUT.obj> --path <id,upx,upy,upz,weight;x,y,z;x,y,z;...> --target <...> [...] [--json]\n        Compute dynamic Target/Weighted/Green View along paths\n    study-rank --variable <id,continuous|integer|categorical,lower,upper> --direction <min|max> --candidate <id,generation;parameters;objectives;constraints> [...] [--reference <f1,f2>] [--json]\n        Rank a constrained study, sensitivity, and exact 2D hypervolume\n    optimizer-benchmark [--population <N>] [--generations <N>] [--seed <N>] [--json]\n        Benchmark the mixed-variable constrained ask/tell engine on a reproducible reference problem\n    annual-irradiance <INPUT.obj> <WEATHER.epw> --sensor <x,y,z,nx,ny,nz> [...] [--json]\n        Compute MEHR annual irradiance and obstruction attribution\n    surface-grid <INPUT.obj> <OUTPUT.csv> --cell-size <VALUE> [--mesh-obj <OUTPUT.obj>] [--json]\n        Generate deterministic area-aware sensors\n    mesh-check <INPUT.obj> [--tolerance <VALUE>] [--json]\n        Audit OBJ geometry without modifying it\n    mesh-repair <INPUT.obj> <OUTPUT.obj> [--tolerance <VALUE>]\n        Conservatively repair an OBJ mesh\n    scene-bench <INPUT.obj> [--rays <COUNT>] [--threads <COUNT>] [--json]\n        Benchmark and differentially validate ZAMYAD\n    self-test\n        Run the analytic reference-kernel smoke test\n    help\n        Print this help\n"
    );
    println!(
        "INTEROPERABILITY / CORE 0.17\n    mesh-convert <INPUT> <OUTPUT> [--weld <METRES>] [--repair-winding] [--clip <minx,miny,minz,maxx,maxy,maxz>] [--json]\n    scene-pack <INPUT> <OUTPUT.xvscene|json> [--weld <METRES>] [--dynamic] [--json]\n    scene-unpack <INPUT.xvscene|json> <OUTPUT.obj|stl|ply|gltf|glb> [--json]\n    weather-convert <INPUT.epw|wea|csv> <OUTPUT.wea|csv> [--missing reject|zero|interpolate] [--drop-leap] [--preserve-gaps] [--json]\n"
    );
    println!(
        "ADVANCED DAENA BACKEND OPTIONS\n    target-view, view-corridor, and observer-path accept:\n    --backend <auto|gpu|cpu>  --adapter <NAME>  --low-power\n    --max-error <METRES>  --max-triangles <COUNT>  --max-rays-per-dispatch <COUNT>\n    --geometry-chunk-mb <MIB>  --gpu-memory-mb <MIB>  --no-cache\n    --verify-cpu  --parity-tolerance <ABSOLUTE_DELTA>\n"
    );
    println!(
        "SCIENTIFIC EVIDENCE / MULTI-FIDELITY 0.18\n    evidence passport <INPUT.json> <OUTPUT.json>\n    evidence sensitivity-design <VARIABLES.json> <OUTPUT.json> --method <saltelli|morris> --samples <N> [--levels <EVEN>] [--seed <N>]\n    evidence sensitivity-analyze <DESIGN.json> <OUTPUTS.json> <OUTPUT.json>\n    evidence robust <INPUT.json> <OUTPUT.json>\n    evidence rank <INPUT.json> <OUTPUT.json>\n    evidence fidelity <INPUT.json> <OUTPUT.json>\n        Seal RASHNU provenance, run valid global sensitivity/robust analysis, and select costly reference jobs\n"
    );
    println!(
        "REPRODUCIBILITY TABLES\n    evidence-table <INPUT.json> <OUTPUT.parquet>\n        Convert typed Phase-2 benchmark rows into Apache Parquet with Snappy compression\n    daylight-evidence-table <INPUT.json> <OUTPUT.parquet>\n        Convert paired Fast/Reference daylight rows into a typed Apache Parquet table\n"
    );
    println!(
        "STUDY / OPTIMIZATION / REPORT 0.16\n    study-schema [OUTPUT.json]\n    study-init <WORKSPACE> <MANIFEST.json> [--json]\n    study-status <WORKSPACE> [--json]\n    study-batch <WORKSPACE> [--output <BATCH.json>] [--json]\n    study-commit <WORKSPACE> <EVALUATIONS.json> [--json]\n    study-report <WORKSPACE> [--output <DIR>] [--bootstrap <N>] [--permutations <N>] [--json]\n"
    );
    println!(
        "DAYLIGHT / RADIANCE 0.15\n    daylight-point <INPUT.obj> --sensor <id,x,y,z,nx,ny,nz,area> --moment <unix,dx,dy,dz,DNI-lux,DHI-lux> [--material ... --assign ...] [--json]\n    daylight-factor <INPUT.obj> --sensor <...> [--exterior-lux <LUX>] [--json]\n    annual-daylight <INPUT.obj> <WEATHER.epw> --sensor <...> [--timeline-csv <PATH>] [--json]\n    radiance-export <INPUT.obj> <OUTPUT_DIR> --sensor <...> [--material ... --assign ...]\n    radiance-point <INPUT.obj> --sensor <...> --moment <...> [--radiance-bin <DIR>] [--json]\n    radiance-annual <INPUT.obj> <WEATHER.epw> --sensor <...> --matrix-cache <DIR> [--radiance-bin <DIR>]\n    daylight-compare <FAST_VALUES> <RADIANCE_VALUES> [--absolute-lux <LUX>] [--relative <FRACTION>] [--json]\n"
    );
}

#[derive(Clone, Debug)]
struct SunPositionCliOptions {
    latitude: f64,
    longitude: f64,
    elevation_meters: f64,
    times: Vec<i64>,
    duration_hours: f64,
    weight: f64,
    solar: SolarOptions,
    json: bool,
}

fn run_sun_position(arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_sun_position_options(arguments) else {
        return ExitCode::from(2);
    };
    let location = match SolarLocation::try_new(
        options.latitude,
        options.longitude,
        options.elevation_meters,
    ) {
        Ok(location) => location,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let schedule = options
        .times
        .iter()
        .map(|timestamp| TimeSample::try_new(*timestamp, options.duration_hours, options.weight))
        .collect::<Result<Vec<_>, _>>();
    let schedule = match schedule {
        Ok(schedule) => schedule,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let sun_set = match calculate_sun_set(location, &schedule, options.solar) {
        Ok(sun_set) => sun_set,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    if options.json {
        print_sun_position_json(&sun_set);
    } else {
        print_sun_position_report(&sun_set);
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_sun_position_options(arguments: &[String]) -> Result<SunPositionCliOptions, ()> {
    let mut latitude = None;
    let mut longitude = None;
    let mut elevation_meters = 0.0;
    let mut times = Vec::new();
    let mut duration_hours = 1.0;
    let mut weight = 1.0;
    let mut solar = SolarOptions::default();
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
        let parse_number = |name: &str| {
            value.parse::<f64>().map_err(|_| {
                eprintln!("error: {name} must be a number");
            })
        };
        match argument {
            "--latitude" => latitude = Some(parse_number("latitude")?),
            "--longitude" => longitude = Some(parse_number("longitude")?),
            "--elevation" => elevation_meters = parse_number("elevation")?,
            "--duration-hours" => duration_hours = parse_number("duration-hours")?,
            "--weight" => weight = parse_number("weight")?,
            "--delta-t" => solar.delta_t_seconds = parse_number("delta-t")?,
            "--pressure" => solar.pressure_millibars = parse_number("pressure")?,
            "--temperature" => solar.temperature_celsius = parse_number("temperature")?,
            "--north" => solar.north_rotation_degrees = parse_number("north")?,
            "--min-altitude" => {
                solar.minimum_altitude_degrees = parse_number("min-altitude")?;
            }
            "--time" => {
                let timestamp = DateTime::parse_from_rfc3339(value)
                    .map_err(|_| {
                        eprintln!(
                            "error: time must be RFC 3339 with an explicit offset, for example 2026-06-21T12:00:00+03:30"
                        );
                    })?
                    .timestamp();
                times.push(timestamp);
            }
            unknown => {
                eprintln!("error: unknown sun-position option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    let Some(latitude) = latitude else {
        eprintln!("error: --latitude is required");
        return Err(());
    };
    let Some(longitude) = longitude else {
        eprintln!("error: --longitude is required");
        return Err(());
    };
    if times.is_empty() {
        eprintln!("error: repeat --time at least once");
        return Err(());
    }
    Ok(SunPositionCliOptions {
        latitude,
        longitude,
        elevation_meters,
        times,
        duration_hours,
        weight,
        solar,
        json,
    })
}

fn print_sun_position_report(sun_set: &xvarna_zurvan::SunSet) {
    println!("XVARNA HVARE Solar Position {}", engine_version());
    println!(
        "Location: {:.8} latitude, {:.8} longitude, {:.3} m",
        sun_set.location.latitude_degrees,
        sun_set.location.longitude_degrees,
        sun_set.location.elevation_meters
    );
    println!(
        "Method: Reda-Andreas SPA; delta T: {:.3} s; refraction: {}",
        sun_set.options.delta_t_seconds,
        if sun_set.options.pressure_millibars > 0.0 {
            "enabled"
        } else {
            "disabled"
        }
    );
    for sample in &sun_set.samples {
        let timestamp = DateTime::<Utc>::from_timestamp(sample.unix_seconds_utc, 0)
            .expect("validated solar timestamp");
        println!(
            "{}  altitude={:.8} deg  azimuth={:.8} deg  vector=({:.10}, {:.10}, {:.10})  active={}",
            timestamp.to_rfc3339(),
            sample.altitude_degrees,
            sample.azimuth_degrees,
            sample.direction.x,
            sample.direction.y,
            sample.direction.z,
            sample.is_active
        );
    }
    println!("Sun-set hash: {}", format_hash(&sun_set.content_hash));
}

fn print_sun_position_json(sun_set: &xvarna_zurvan::SunSet) {
    print!(
        concat!(
            "{{\"schemaVersion\":\"0.16.0\",\"method\":\"Reda-Andreas SPA\",",
            "\"latitude\":{},\"longitude\":{},\"elevationMeters\":{},",
            "\"sunSetHash\":\"{}\",\"samples\":["
        ),
        sun_set.location.latitude_degrees,
        sun_set.location.longitude_degrees,
        sun_set.location.elevation_meters,
        format_hash(&sun_set.content_hash)
    );
    for (index, sample) in sun_set.samples.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!(
            concat!(
                "{{\"unixSecondsUtc\":{},\"altitudeDegrees\":{},\"azimuthDegrees\":{},",
                "\"direction\":[{},{},{}],\"durationHours\":{},\"weight\":{},\"active\":{}}}"
            ),
            sample.unix_seconds_utc,
            sample.altitude_degrees,
            sample.azimuth_degrees,
            sample.direction.x,
            sample.direction.y,
            sample.direction.z,
            sample.duration_hours,
            sample.weight,
            sample.is_active
        );
    }
    println!("]}}");
}

#[derive(Clone, Debug)]
struct SkyViewCliOptions {
    sensors: Vec<SolarSensor>,
    sky: SkyViewOptions,
    thread_count: usize,
    mask_csv: Option<String>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
fn run_sky_view(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_sky_view_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let audit_options = MeshAuditOptions::try_new(1.0e-6, 0.25).expect("fixed options valid");
    let Ok(audit) = audit_mesh(&imported.mesh, audit_options) else {
        eprintln!("error: mesh audit configuration is invalid");
        return ExitCode::FAILURE;
    };
    if !audit.is_analysis_ready {
        eprintln!("error: input requires mesh repair before sky-view analysis");
        return ExitCode::FAILURE;
    }

    let mut builder = SceneBuilder::new(SceneBuildOptions {
        thread_count: options.thread_count,
        ..SceneBuildOptions::default()
    })
    .expect("CLI sky-view options are valid");
    let mesh_id = match builder.add_mesh(imported.mesh) {
        Ok(mesh_id) => mesh_id,
        Err(error) => {
            eprintln!("error: scene mesh ingest failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = builder.add_instance(
        mesh_id,
        Transform::IDENTITY,
        ObjectId::new(1),
        InstanceId::new(1),
        u64::MAX,
    ) {
        eprintln!("error: scene instance failed: {error}");
        return ExitCode::FAILURE;
    }
    let scene = match builder.build() {
        Ok(scene) => scene,
        Err(error) => {
            eprintln!("error: scene compile failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let started = Instant::now();
    let result = match analyze_sky_view(&scene, &options.sensors, options.sky) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("error: sky-view analysis failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let analysis_microseconds = started.elapsed().as_micros();
    if let Some(path) = &options.mask_csv
        && let Err(error) = write_shadow_mask_csv(path, &options.sensors, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    if options.json {
        print_sky_view_json(&result, analysis_microseconds, options.mask_csv.as_deref());
    } else {
        print_sky_view_report(
            input,
            &result,
            analysis_microseconds,
            options.mask_csv.as_deref(),
        );
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_sky_view_options(arguments: &[String]) -> Result<SkyViewCliOptions, ()> {
    let mut sensors = Vec::new();
    let mut sky = SkyViewOptions::default();
    let mut thread_count = 0;
    let mut mask_csv = None;
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
            "--sensor" => {
                let values = parse_comma_vector(value, "sensor", 6)?;
                let sensor_id = u64::try_from(sensors.len() + 1).map_err(|_| {
                    eprintln!("error: too many sensors");
                })?;
                sensors.push(
                    SolarSensor::try_new(
                        SensorId::new(sensor_id),
                        Vec3::new(values[0], values[1], values[2]),
                        Vec3::new(values[3], values[4], values[5]),
                    )
                    .map_err(|error| eprintln!("error: invalid sensor: {error}"))?,
                );
            }
            "--samples" => {
                sky.sample_count = value.parse().map_err(|_| {
                    eprintln!("error: samples must be an integer");
                })?;
                if !(16..=262_144).contains(&sky.sample_count) {
                    eprintln!("error: samples must be between 16 and 262144");
                    return Err(());
                }
            }
            "--seed" => {
                sky.seed = value.parse().map_err(|_| {
                    eprintln!("error: seed must be an unsigned 64-bit integer");
                })?;
            }
            "--offset" => {
                sky.sensor_offset_meters = parse_finite_number(value, "offset")?;
                if sky.sensor_offset_meters < 0.0 {
                    eprintln!("error: offset must be non-negative");
                    return Err(());
                }
            }
            "--max-distance" => {
                sky.maximum_distance_meters = parse_finite_number(value, "max-distance")?;
                if sky.maximum_distance_meters <= 0.0 {
                    eprintln!("error: max-distance must be positive");
                    return Err(());
                }
            }
            "--category-mask" => {
                sky.category_mask = value.parse().map_err(|_| {
                    eprintln!("error: category-mask must be an unsigned 64-bit integer");
                })?;
            }
            "--threads" => {
                thread_count = value.parse().map_err(|_| {
                    eprintln!("error: threads must be a non-negative integer");
                })?;
                if thread_count > 256 {
                    eprintln!("error: threads must be at most 256");
                    return Err(());
                }
            }
            "--mask-csv" => mask_csv = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown sky-view option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if sensors.is_empty() {
        eprintln!("error: repeat --sensor x,y,z,nx,ny,nz at least once");
        return Err(());
    }
    Ok(SkyViewCliOptions {
        sensors,
        sky,
        thread_count,
        mask_csv,
        json,
    })
}

fn parse_comma_vector(value: &str, name: &str, count: usize) -> Result<Vec<f64>, ()> {
    let values = value
        .split(',')
        .map(|item| parse_finite_number(item, name))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != count {
        eprintln!("error: {name} requires exactly {count} comma-separated numbers");
        return Err(());
    }
    Ok(values)
}

fn parse_finite_number(value: &str, name: &str) -> Result<f64, ()> {
    let parsed = value.parse::<f64>().map_err(|_| {
        eprintln!("error: {name} must contain finite numbers");
    })?;
    if !parsed.is_finite() {
        eprintln!("error: {name} must contain finite numbers");
        return Err(());
    }
    Ok(parsed)
}

fn write_shadow_mask_csv(
    path: &str,
    sensors: &[SolarSensor],
    result: &xvarna_hvare::SkyViewResult,
) -> std::io::Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "sensor_index,sensor_id,sample_index,direction_x,direction_y,direction_z,visible,object_id,instance_id,mesh_id,triangle_id,distance_meters"
    )?;
    for (sensor_index, sensor) in sensors.iter().enumerate() {
        let row_start = sensor_index * result.sample_count;
        for (sample_index, entry) in result.timeline[row_start..row_start + result.sample_count]
            .iter()
            .enumerate()
        {
            let blocked = entry.state == SkyRayState::Blocked;
            writeln!(
                writer,
                "{sensor_index},{},{sample_index},{},{},{},{},{},{},{},{},{}",
                sensor.id.get(),
                entry.direction.x,
                entry.direction.y,
                entry.direction.z,
                !blocked,
                entry.object_id.get(),
                entry.instance_id.get(),
                entry.mesh_id.get(),
                if blocked { entry.triangle_id } else { u32::MAX },
                if blocked {
                    entry.distance_meters.to_string()
                } else {
                    String::new()
                }
            )?;
        }
    }
    writer.flush()
}

fn print_sky_view_report(
    input: &str,
    result: &xvarna_hvare::SkyViewResult,
    analysis_microseconds: u128,
    mask_csv: Option<&str>,
) {
    println!("XVARNA ASMAN Sky Intelligence {}", engine_version());
    println!("Context: {}", Path::new(input).display());
    println!(
        "Estimator: {} equal-solid-angle Fibonacci samples per sensor; Lambert cosine weighting",
        result.sample_count
    );
    println!(
        "Analysis: {} sensor(s), {} rays, {} ms",
        result.summaries.len(),
        result.timeline.len(),
        format_milliseconds(analysis_microseconds)
    );
    for (index, summary) in result.summaries.iter().enumerate() {
        println!(
            "Sensor {index} / ID {}: SVF={:.8}, visible hemisphere={:.8}, convergence delta={:.6e}, visible={}, blocked={}, dominant object={} ({:.8})",
            summary.sensor_id,
            summary.cosine_weighted_svf,
            summary.visible_hemisphere_fraction,
            summary.cosine_convergence_delta,
            summary.visible_count,
            summary.blocked_count,
            summary.dominant_occluder_object_id,
            summary.dominant_occluder_projected_fraction
        );
    }
    println!("Result hash: {}", format_hash(&result.content_hash));
    if let Some(path) = mask_csv {
        println!("Shadow Mask CSV: {}", Path::new(path).display());
    }
}

fn print_sky_view_json(
    result: &xvarna_hvare::SkyViewResult,
    analysis_microseconds: u128,
    mask_csv: Option<&str>,
) {
    print!(
        concat!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"ASMAN\",",
            "\"samplesPerSensor\":{},\"sensorCount\":{},\"rayCount\":{},",
            "\"analysisMicroseconds\":{},\"resultHash\":\"{}\",\"summaries\":["
        ),
        result.sample_count,
        result.summaries.len(),
        result.timeline.len(),
        analysis_microseconds,
        format_hash(&result.content_hash)
    );
    for (index, summary) in result.summaries.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!(
            concat!(
                "{{\"sensorId\":{},\"skyViewFactor\":{},\"visibleHemisphere\":{},",
                "\"visibleSolidAngleSteradians\":{},\"convergenceDelta\":{},",
                "\"visibleCount\":{},\"blockedCount\":{},\"dominantObjectId\":{},",
                "\"dominantBlockedFraction\":{}}}"
            ),
            summary.sensor_id.get(),
            summary.cosine_weighted_svf,
            summary.visible_hemisphere_fraction,
            summary.visible_solid_angle_steradians,
            summary.cosine_convergence_delta,
            summary.visible_count,
            summary.blocked_count,
            summary.dominant_occluder_object_id.get(),
            summary.dominant_occluder_projected_fraction
        );
    }
    print!("]");
    if let Some(path) = mask_csv {
        print!(",\"shadowMaskCsv\":\"{}\"", json_escape(path));
    }
    println!("}}");
}

#[derive(Clone, Debug)]
struct AnnualIrradianceCliOptions {
    sensors: Vec<SolarSensor>,
    irradiance: AnnualIrradianceOptions,
    thread_count: usize,
    timeline_csv: Option<String>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
fn run_annual_irradiance(input: &str, weather_path: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_annual_irradiance_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let weather_source = match fs::read_to_string(weather_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!(
                "error: cannot read {}: {error}",
                Path::new(weather_path).display()
            );
            return ExitCode::FAILURE;
        }
    };
    let weather = match parse_epw(&weather_source) {
        Ok(weather) => weather,
        Err(error) => {
            eprintln!("error: invalid EPW: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut builder = match SceneBuilder::new(SceneBuildOptions {
        thread_count: options.thread_count,
        ..SceneBuildOptions::default()
    }) {
        Ok(builder) => builder,
        Err(error) => {
            eprintln!("error: invalid scene options: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mesh_id = match builder.add_mesh(imported.mesh) {
        Ok(mesh_id) => mesh_id,
        Err(error) => {
            eprintln!("error: scene mesh ingest failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = builder.add_instance(
        mesh_id,
        Transform::IDENTITY,
        ObjectId::new(1),
        InstanceId::new(1),
        u64::MAX,
    ) {
        eprintln!("error: scene instance failed: {error}");
        return ExitCode::FAILURE;
    }
    let scene = match builder.build() {
        Ok(scene) => scene,
        Err(error) => {
            eprintln!("error: scene compile failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let started = Instant::now();
    let result =
        match analyze_annual_irradiance(&scene, &options.sensors, &weather, options.irradiance) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("error: annual irradiance failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    let elapsed = started.elapsed().as_micros();
    if let Some(path) = &options.timeline_csv
        && let Err(error) = write_irradiance_csv(path, &options.sensors, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    if options.json {
        print_annual_irradiance_json(
            input,
            weather_path,
            &weather,
            &result,
            elapsed,
            options.timeline_csv.as_deref(),
        );
    } else {
        print_annual_irradiance_report(
            input,
            weather_path,
            &weather,
            &result,
            elapsed,
            options.timeline_csv.as_deref(),
        );
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_annual_irradiance_options(arguments: &[String]) -> Result<AnnualIrradianceCliOptions, ()> {
    let mut sensors = Vec::new();
    let mut irradiance = AnnualIrradianceOptions::default();
    let mut thread_count = 0;
    let mut timeline_csv = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
                continue;
            }
            "--no-weather-albedo" => {
                irradiance.use_weather_albedo = false;
                index += 1;
                continue;
            }
            _ => {}
        }
        let argument = arguments[index].as_str();
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--sensor" => {
                let values = parse_comma_vector(value, "sensor", 6)?;
                let id = u64::try_from(sensors.len() + 1).map_err(|_| {
                    eprintln!("error: too many sensors");
                })?;
                sensors.push(
                    SolarSensor::try_new(
                        SensorId::new(id),
                        Vec3::new(values[0], values[1], values[2]),
                        Vec3::new(values[3], values[4], values[5]),
                    )
                    .map_err(|error| eprintln!("error: invalid sensor: {error}"))?,
                );
            }
            "--albedo" => {
                irradiance.ground_albedo = parse_finite_number(value, "albedo")?;
                if !(0.0..=1.0).contains(&irradiance.ground_albedo) {
                    eprintln!("error: albedo must be between 0 and 1");
                    return Err(());
                }
            }
            "--offset" => {
                irradiance.sensor_offset_meters = parse_finite_number(value, "offset")?;
                if irradiance.sensor_offset_meters < 0.0 {
                    eprintln!("error: offset must be non-negative");
                    return Err(());
                }
            }
            "--max-distance" => {
                irradiance.maximum_distance_meters = parse_finite_number(value, "max-distance")?;
                if irradiance.maximum_distance_meters <= 0.0 {
                    eprintln!("error: max-distance must be positive");
                    return Err(());
                }
            }
            "--category-mask" => {
                irradiance.category_mask = value.parse().map_err(|_| {
                    eprintln!("error: category-mask must be an unsigned 64-bit integer");
                })?;
            }
            "--north" => {
                irradiance.solar.north_rotation_degrees = parse_finite_number(value, "north")?;
            }
            "--delta-t" => {
                irradiance.solar.delta_t_seconds = parse_finite_number(value, "delta-t")?;
            }
            "--pressure" => {
                irradiance.solar.pressure_millibars = parse_finite_number(value, "pressure")?;
            }
            "--temperature" => {
                irradiance.solar.temperature_celsius = parse_finite_number(value, "temperature")?;
            }
            "--min-altitude" => {
                irradiance.solar.minimum_altitude_degrees =
                    parse_finite_number(value, "min-altitude")?;
            }
            "--threads" => {
                thread_count = value.parse().map_err(|_| {
                    eprintln!("error: threads must be a non-negative integer");
                })?;
                if thread_count > 256 {
                    eprintln!("error: threads must be at most 256");
                    return Err(());
                }
            }
            "--timeline-csv" => timeline_csv = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown annual-irradiance option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if sensors.is_empty() {
        eprintln!("error: repeat --sensor x,y,z,nx,ny,nz at least once");
        return Err(());
    }
    Ok(AnnualIrradianceCliOptions {
        sensors,
        irradiance,
        thread_count,
        timeline_csv,
        json,
    })
}

fn write_irradiance_csv(
    path: &str,
    sensors: &[SolarSensor],
    result: &xvarna_hvare::AnnualIrradianceResult,
) -> std::io::Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "sensor_index,sensor_id,weather_index,unix_seconds_utc,state,direct_wh_m2,diffuse_dome_wh_m2,diffuse_circumsolar_wh_m2,diffuse_horizon_wh_m2,diffuse_sky_wh_m2,ground_reflected_wh_m2,global_wh_m2,global_w_m2,attributed_loss_wh_m2,object_id,instance_id,mesh_id,triangle_id,distance_meters"
    )?;
    for (sensor_index, sensor) in sensors.iter().enumerate() {
        let row_start = sensor_index * result.weather_count;
        for (weather_index, entry) in result.timeline[row_start..row_start + result.weather_count]
            .iter()
            .enumerate()
        {
            writeln!(
                writer,
                "{sensor_index},{},{weather_index},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                sensor.id.get(),
                entry.unix_seconds_utc,
                entry.state as u8,
                entry.direct_wh_m2,
                entry.diffuse_dome_wh_m2,
                entry.diffuse_circumsolar_wh_m2,
                entry.diffuse_horizon_wh_m2,
                entry.diffuse_sky_wh_m2,
                entry.ground_reflected_wh_m2,
                entry.global_wh_m2,
                entry.global_w_m2,
                entry.attributed_loss_wh_m2,
                entry.object_id.get(),
                entry.instance_id.get(),
                entry.mesh_id.get(),
                entry.triangle_id,
                if entry.distance_meters.is_finite() {
                    entry.distance_meters.to_string()
                } else {
                    String::new()
                }
            )?;
        }
    }
    writer.flush()
}

fn print_annual_irradiance_report(
    input: &str,
    weather_path: &str,
    weather: &xvarna_zurvan::EpwWeather,
    result: &xvarna_hvare::AnnualIrradianceResult,
    analysis_microseconds: u128,
    timeline_csv: Option<&str>,
) {
    println!("XVARNA MEHR Annual Irradiance {}", engine_version());
    println!("Context: {}", Path::new(input).display());
    println!("Weather: {}", Path::new(weather_path).display());
    println!(
        "EPW: {} interval(s), {} record(s)/hour; location {:.6}, {:.6}, elevation {:.2} m, UTC{:+.2}",
        weather.records.len(),
        weather.data_period.records_per_hour,
        weather.location.solar.latitude_degrees,
        weather.location.solar.longitude_degrees,
        weather.location.solar.elevation_meters,
        weather.location.time_zone_hours
    );
    println!(
        "Sanitized radiation: GHI {}, DNI {}, DHI {}",
        weather.missing_counts.global_horizontal,
        weather.missing_counts.direct_normal,
        weather.missing_counts.diffuse_horizontal
    );
    println!(
        "Model: Perez anisotropic sky; 144 dome + 24 horizon rays/sensor; closest-hit direct/circumsolar attribution; isotropic ground reflection"
    );
    println!(
        "Analysis: {} sensor(s), {} timeline entries, {} ms",
        result.summaries.len(),
        result.timeline.len(),
        format_milliseconds(analysis_microseconds)
    );
    for (index, summary) in result.summaries.iter().enumerate() {
        println!(
            "Sensor {index} / ID {}: global={:.6} kWh/m2, direct={:.6}, diffuse={:.6}, ground={:.6}, peak={:.3} W/m2, dome={:.6}, horizon={:.6}, dominant solar object={} / loss={:.6} kWh/m2",
            summary.sensor_id,
            summary.global_wh_m2 / 1_000.0,
            summary.direct_wh_m2 / 1_000.0,
            summary.diffuse_sky_wh_m2 / 1_000.0,
            summary.ground_reflected_wh_m2 / 1_000.0,
            summary.peak_global_w_m2,
            summary.dome_visibility_ratio,
            summary.horizon_visibility_ratio,
            summary.dominant_solar_occluder_object_id,
            summary.dominant_solar_occluder_loss_wh_m2 / 1_000.0,
        );
    }
    println!("Weather hash: {}", format_hash(&weather.content_hash));
    println!("Result hash: {}", format_hash(&result.content_hash));
    if let Some(path) = timeline_csv {
        println!("Timeline CSV: {}", Path::new(path).display());
    }
}

fn print_annual_irradiance_json(
    input: &str,
    weather_path: &str,
    weather: &xvarna_zurvan::EpwWeather,
    result: &xvarna_hvare::AnnualIrradianceResult,
    analysis_microseconds: u128,
    timeline_csv: Option<&str>,
) {
    print!(
        concat!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"MEHR\",",
            "\"context\":\"{}\",\"weather\":\"{}\",\"weatherCount\":{},",
            "\"sensorCount\":{},\"timelineCount\":{},\"analysisMicroseconds\":{},",
            "\"weatherHash\":\"{}\",\"resultHash\":\"{}\",\"summaries\":["
        ),
        json_escape(input),
        json_escape(weather_path),
        weather.records.len(),
        result.summaries.len(),
        result.timeline.len(),
        analysis_microseconds,
        format_hash(&weather.content_hash),
        format_hash(&result.content_hash),
    );
    for (index, summary) in result.summaries.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!(
            concat!(
                "{{\"sensorId\":{},\"globalKWhM2\":{},\"directKWhM2\":{},",
                "\"diffuseKWhM2\":{},\"groundKWhM2\":{},\"peakWM2\":{},",
                "\"peakUnixSecondsUtc\":{},\"domeVisibility\":{},",
                "\"horizonVisibility\":{},\"dominantSolarObjectId\":{},",
                "\"dominantSolarLossKWhM2\":{}}}"
            ),
            summary.sensor_id.get(),
            summary.global_wh_m2 / 1_000.0,
            summary.direct_wh_m2 / 1_000.0,
            summary.diffuse_sky_wh_m2 / 1_000.0,
            summary.ground_reflected_wh_m2 / 1_000.0,
            summary.peak_global_w_m2,
            summary.peak_unix_seconds_utc,
            summary.dome_visibility_ratio,
            summary.horizon_visibility_ratio,
            summary.dominant_solar_occluder_object_id.get(),
            summary.dominant_solar_occluder_loss_wh_m2 / 1_000.0,
        );
    }
    print!("]");
    if let Some(path) = timeline_csv {
        print!(",\"timelineCsv\":\"{}\"", json_escape(path));
    }
    println!("}}");
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Clone, Debug)]
struct SurfaceGridCliOptions {
    grid: SurfaceGridOptions,
    mesh_obj: Option<String>,
    json: bool,
}

fn run_surface_grid(input: &str, output_csv: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_surface_grid_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let grid = match generate_surface_grid(&imported.mesh, options.grid) {
        Ok(grid) => grid,
        Err(error) => {
            eprintln!("error: surface grid failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_surface_grid_csv(output_csv, &grid.cells) {
        eprintln!(
            "error: cannot write {}: {error}",
            Path::new(output_csv).display()
        );
        return ExitCode::FAILURE;
    }
    if let Some(path) = &options.mesh_obj {
        let mesh = match surface_cell_mesh(&grid.cells) {
            Ok(mesh) => mesh,
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(error) = fs::write(path, write_obj(&mesh)) {
            eprintln!("error: cannot write {}: {error}", Path::new(path).display());
            return ExitCode::FAILURE;
        }
    }
    let elapsed = started.elapsed().as_micros();
    if options.json {
        println!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"surface-grid\",",
                "\"input\":\"{}\",\"csv\":\"{}\",\"sourceFaces\":{},",
                "\"cells\":{},\"skippedFaces\":{},\"sourceArea\":{},",
                "\"sampledArea\":{},\"maximumCellEdge\":{},\"maximumDepth\":{},",
                "\"elapsedMicroseconds\":{},\"hash\":\"{}\"}}"
            ),
            json_escape(input),
            json_escape(output_csv),
            grid.source_face_count,
            grid.cells.len(),
            grid.skipped_face_count,
            grid.source_area,
            grid.sampled_area,
            grid.maximum_cell_edge_length,
            grid.maximum_subdivision_depth,
            elapsed,
            format_hash(&grid.content_hash),
        );
    } else {
        println!("XVARNA deterministic surface grid {}", engine_version());
        println!("Input: {}", Path::new(input).display());
        println!(
            "Grid: {} source face(s), {} cell(s), {} skipped; max edge {:.10}, depth {}",
            grid.source_face_count,
            grid.cells.len(),
            grid.skipped_face_count,
            grid.maximum_cell_edge_length,
            grid.maximum_subdivision_depth
        );
        println!(
            "Area: source {:.12}, sampled {:.12}, delta {:.6e}",
            grid.source_area,
            grid.sampled_area,
            grid.sampled_area - grid.source_area
        );
        println!("Sensors CSV: {}", Path::new(output_csv).display());
        if let Some(path) = &options.mesh_obj {
            println!("Analysis mesh: {}", Path::new(path).display());
        }
        println!("Hash: {}", format_hash(&grid.content_hash));
    }
    ExitCode::SUCCESS
}

fn parse_surface_grid_options(arguments: &[String]) -> Result<SurfaceGridCliOptions, ()> {
    let mut cell_size = None;
    let mut offset = 1.0e-4;
    let mut first_sensor_id = 1_u64;
    let mut maximum_cell_count = 1_000_000_usize;
    let mut mesh_obj = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--json" {
            json = true;
            index += 1;
            continue;
        }
        let argument = arguments[index].as_str();
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--cell-size" => cell_size = Some(parse_finite_number(value, "cell-size")?),
            "--offset" => offset = parse_finite_number(value, "offset")?,
            "--first-id" => {
                first_sensor_id = value.parse().map_err(|_| {
                    eprintln!("error: first-id must be an unsigned 64-bit integer");
                })?;
            }
            "--max-cells" => {
                maximum_cell_count = value.parse().map_err(|_| {
                    eprintln!("error: max-cells must be a positive integer");
                })?;
            }
            "--mesh-obj" => mesh_obj = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown surface-grid option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    let Some(cell_size) = cell_size else {
        eprintln!("error: --cell-size is required");
        return Err(());
    };
    let grid = SurfaceGridOptions::try_new(cell_size, offset, first_sensor_id, maximum_cell_count)
        .map_err(|error| eprintln!("error: invalid surface-grid policy: {error}"))?;
    Ok(SurfaceGridCliOptions {
        grid,
        mesh_obj,
        json,
    })
}

fn write_surface_grid_csv(
    path: &str,
    cells: &[xvarna_geometry::SurfaceCell],
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(fs::File::create(path)?);
    writeln!(
        writer,
        "cell_index,sensor_id,source_face,subdivision_depth,position_x,position_y,position_z,normal_x,normal_y,normal_z,area,a_x,a_y,a_z,b_x,b_y,b_z,c_x,c_y,c_z"
    )?;
    for (index, cell) in cells.iter().enumerate() {
        writeln!(
            writer,
            "{index},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            cell.sensor_id.get(),
            cell.source_face_index,
            cell.subdivision_depth,
            cell.position.x,
            cell.position.y,
            cell.position.z,
            cell.normal.x,
            cell.normal.y,
            cell.normal.z,
            cell.area,
            cell.a.x,
            cell.a.y,
            cell.a.z,
            cell.b.x,
            cell.b.y,
            cell.b.z,
            cell.c.x,
            cell.c.y,
            cell.c.z,
        )?;
    }
    writer.flush()
}

fn surface_cell_mesh(cells: &[xvarna_geometry::SurfaceCell]) -> Result<Mesh, &'static str> {
    let vertex_count = cells
        .len()
        .checked_mul(3)
        .ok_or("analysis mesh vertex count overflowed")?;
    if vertex_count > u32::MAX as usize {
        return Err("analysis mesh exceeds the u32 index contract");
    }
    let mut positions = Vec::with_capacity(vertex_count);
    let mut triangles = Vec::with_capacity(cells.len());
    for cell in cells {
        let base = u32::try_from(positions.len()).map_err(|_| "analysis mesh index overflowed")?;
        positions.extend([cell.a, cell.b, cell.c]);
        triangles.push([base, base + 1, base + 2]);
    }
    Ok(Mesh {
        positions,
        triangles,
    })
}

#[allow(clippy::too_many_lines)]
fn run_scene_benchmark(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_scene_benchmark_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let audit_options = MeshAuditOptions::try_new(1.0e-6, 0.25).expect("fixed options valid");
    let Ok(audit) = audit_mesh(&imported.mesh, audit_options) else {
        eprintln!("error: mesh audit configuration is invalid");
        return ExitCode::FAILURE;
    };
    if !audit.is_analysis_ready {
        eprintln!("error: input requires mesh repair before scene compilation");
        return ExitCode::FAILURE;
    }
    let Some(bounds) = audit.bounds else {
        eprintln!("error: input contains no finite geometry");
        return ExitCode::FAILURE;
    };

    let mut builder = SceneBuilder::new(SceneBuildOptions {
        thread_count: options.thread_count,
        ..SceneBuildOptions::default()
    })
    .expect("CLI benchmark options are valid");
    let mesh_id = match builder.add_mesh(imported.mesh.clone()) {
        Ok(mesh_id) => mesh_id,
        Err(error) => {
            eprintln!("error: scene mesh ingest failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = builder.add_instance(
        mesh_id,
        Transform::IDENTITY,
        ObjectId::new(1),
        InstanceId::new(1),
        u64::MAX,
    ) {
        eprintln!("error: scene instance failed: {error}");
        return ExitCode::FAILURE;
    }
    let scene = match builder.build() {
        Ok(scene) => scene,
        Err(error) => {
            eprintln!("error: scene compile failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let rays = benchmark_rays(bounds, options.ray_count);
    let query_started = Instant::now();
    let hits = scene.trace_closest_batch(&rays);
    let query_duration = query_started.elapsed();
    let reference_count = rays.len().min(2_048);
    let reference_started = Instant::now();
    let mismatch_count = rays[..reference_count]
        .iter()
        .zip(&hits[..reference_count])
        .filter(|(ray, hit)| {
            let reference_ray = Ray::try_new(ray.origin, ray.direction, ray.t_min, ray.t_max)
                .expect("generated benchmark ray is valid");
            let reference = imported
                .mesh
                .triangles
                .iter()
                .filter_map(|indices| {
                    intersect_triangle_reference(
                        reference_ray,
                        Triangle {
                            a: imported.mesh.positions[indices[0] as usize],
                            b: imported.mesh.positions[indices[1] as usize],
                            c: imported.mesh.positions[indices[2] as usize],
                            object_id: ObjectId::new(1),
                        },
                    )
                })
                .min_by(|left, right| left.distance.total_cmp(&right.distance));
            reference.is_some() != hit.hit
                || reference
                    .is_some_and(|expected| (expected.distance - hit.distance).abs() > 1.0e-9)
        })
        .count();
    let reference_duration = reference_started.elapsed();
    let hit_count = hits.iter().filter(|hit| hit.hit).count();
    let rays_per_second = f64::from(options.ray_count) / query_duration.as_secs_f64().max(1.0e-12);
    print_scene_benchmark(
        input,
        options.json,
        scene.stats(),
        options.ray_count,
        hit_count,
        query_duration.as_micros(),
        rays_per_second,
        reference_count,
        mismatch_count,
        reference_duration.as_micros(),
    );
    if mismatch_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneBenchmarkOptions {
    ray_count: u32,
    thread_count: usize,
    json: bool,
}

fn parse_scene_benchmark_options(arguments: &[String]) -> Result<SceneBenchmarkOptions, ()> {
    let mut options = SceneBenchmarkOptions {
        ray_count: 10_000,
        thread_count: 0,
        json: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => {
                options.json = true;
                index += 1;
            }
            "--rays" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --rays requires a value");
                    return Err(());
                };
                options.ray_count = value.parse().map_err(|_| {
                    eprintln!("error: ray count must be an integer");
                })?;
                if !(1..=5_000_000).contains(&options.ray_count) {
                    eprintln!("error: ray count must be between 1 and 5,000,000");
                    return Err(());
                }
                index += 2;
            }
            "--threads" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --threads requires a value");
                    return Err(());
                };
                options.thread_count = value.parse().map_err(|_| {
                    eprintln!("error: thread count must be a non-negative integer");
                })?;
                if options.thread_count > 256 {
                    eprintln!("error: thread count must be at most 256");
                    return Err(());
                }
                index += 2;
            }
            unknown => {
                eprintln!("error: unknown scene-bench option '{unknown}'");
                return Err(());
            }
        }
    }
    Ok(options)
}

fn benchmark_rays(bounds: xvarna_geometry::Aabb, ray_count: u32) -> Vec<QueryRay> {
    let mut side = 1_u32;
    while side.saturating_mul(side) < ray_count {
        side += 1;
    }
    let extent = bounds.extent();
    let height = extent.x.max(extent.y).max(extent.z).max(1.0);
    (0..ray_count)
        .map(|index| {
            let column = index % side;
            let row = index / side;
            let x = ((f64::from(column) + 0.5) / f64::from(side)).mul_add(extent.x, bounds.min.x);
            let y = ((f64::from(row) + 0.5) / f64::from(side)).mul_add(extent.y, bounds.min.y);
            QueryRay::try_new(
                Vec3::new(x, y, bounds.max.z + height),
                Vec3::new(0.0, 0.0, -1.0),
                0.0,
                height.mul_add(2.0, extent.z),
                u64::MAX,
            )
            .expect("generated benchmark ray is valid")
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn print_scene_benchmark(
    input: &str,
    json: bool,
    stats: xvarna_scene::SceneStats,
    ray_count: u32,
    hit_count: usize,
    query_microseconds: u128,
    rays_per_second: f64,
    reference_count: usize,
    mismatch_count: usize,
    reference_microseconds: u128,
) {
    if json {
        println!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"sceneHash\":\"{}\",",
                "\"resources\":{},\"instances\":{},\"uniqueTriangles\":{},",
                "\"instancedTriangles\":{},\"blasNodes\":{},\"tlasNodes\":{},",
                "\"depth\":{},\"memoryBytes\":{},\"buildMicroseconds\":{},",
                "\"workers\":{},\"rays\":{},\"hits\":{},\"queryMicroseconds\":{},",
                "\"raysPerSecond\":{},\"referenceRays\":{},\"mismatches\":{},",
                "\"referenceMicroseconds\":{}}}"
            ),
            format_hash(&stats.content_hash),
            stats.mesh_resource_count,
            stats.instance_count,
            stats.unique_triangle_count,
            stats.instanced_triangle_count,
            stats.blas_node_count,
            stats.tlas_node_count,
            stats.maximum_bvh_depth,
            stats.approximate_memory_bytes,
            stats.build_time_microseconds,
            stats.thread_count,
            ray_count,
            hit_count,
            query_microseconds,
            rays_per_second,
            reference_count,
            mismatch_count,
            reference_microseconds,
        );
        return;
    }
    println!("XVARNA ZAMYAD Scene Engine benchmark");
    println!("File: {}", Path::new(input).display());
    println!(
        "Scene: {} resource(s), {} instance(s), {} unique / {} instanced triangles",
        stats.mesh_resource_count,
        stats.instance_count,
        stats.unique_triangle_count,
        stats.instanced_triangle_count
    );
    println!(
        "Acceleration: {} BLAS nodes, {} TLAS nodes, depth {}, {} worker(s)",
        stats.blas_node_count, stats.tlas_node_count, stats.maximum_bvh_depth, stats.thread_count
    );
    println!(
        "Build: {} ms; memory: {} MiB",
        format_milliseconds(u128::from(stats.build_time_microseconds)),
        format_mebibytes(stats.approximate_memory_bytes)
    );
    println!(
        "Query: {ray_count} rays, {hit_count} hits, {} ms, {:.0} rays/s",
        format_milliseconds(query_microseconds),
        rays_per_second
    );
    println!(
        "Differential validation: {reference_count} ray(s), {mismatch_count} mismatch(es), {} ms",
        format_milliseconds(reference_microseconds)
    );
    println!("Scene hash: {}", format_hash(&stats.content_hash));
}

fn format_milliseconds(microseconds: u128) -> String {
    format!("{}.{:03}", microseconds / 1_000, microseconds % 1_000)
}

fn format_mebibytes(bytes: usize) -> String {
    let bytes = u128::try_from(bytes).expect("usize always fits u128");
    let divisor = 1_048_576_u128;
    format!(
        "{}.{:03}",
        bytes / divisor,
        bytes % divisor * 1_000 / divisor
    )
}

fn run_mesh_check(input: &str, arguments: &[String]) -> ExitCode {
    let Ok((tolerance, json)) = parse_check_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let options = MeshAuditOptions::try_new(tolerance, 0.25).expect("CLI validates tolerance");
    let Ok(report) = audit_mesh(&imported.mesh, options) else {
        eprintln!("error: mesh audit configuration is invalid");
        return ExitCode::from(2);
    };

    if json {
        print_audit_json(&report, imported.triangulated_quad_count);
    } else {
        print_audit_report(input, &report, imported.triangulated_quad_count);
    }

    if report.is_analysis_ready {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run_mesh_repair(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let Ok(tolerance) = parse_repair_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let options = MeshAuditOptions::try_new(tolerance, 0.25).expect("CLI validates tolerance");
    let Ok(repair) = repair_mesh_conservative(&imported.mesh, options) else {
        eprintln!("error: mesh repair configuration is invalid");
        return ExitCode::from(2);
    };
    let serialized = write_obj(&repair.mesh);
    if let Err(error) = fs::write(output, serialized) {
        eprintln!(
            "error: cannot write {}: {error}",
            Path::new(output).display()
        );
        return ExitCode::FAILURE;
    }

    println!("XVARNA conservative mesh repair");
    println!("Input: {}", Path::new(input).display());
    println!("Output: {}", Path::new(output).display());
    println!("Removed faces: {}", repair.removed_face_count);
    println!("Removed vertices: {}", repair.removed_vertex_count);
    println!("Welded vertices: 0");
    println!("Filled holes: 0");
    println!("Before hash: {}", format_hash(&repair.before_hash));
    println!("After hash: {}", format_hash(&repair.after_hash));
    ExitCode::SUCCESS
}

#[derive(Clone, Debug)]
struct IsovistCliOptions {
    viewpoints: Vec<Viewpoint>,
    options: IsovistOptions,
    thread_count: usize,
    boundary_csv: Option<String>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
fn run_isovist(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_isovist_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "isovist") else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let result = match analyze_isovists(&scene, &options.viewpoints, options.options) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: DAENA isovist failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed().as_micros();
    if let Some(path) = &options.boundary_csv
        && let Err(error) = write_isovist_csv(path, &options.viewpoints, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    let total_open = result
        .summaries
        .iter()
        .map(|value| value.open_count)
        .sum::<usize>();
    if options.json {
        print!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA isovist\",",
                "\"input\":\"{}\",\"viewpointCount\":{},\"sampleCount\":{},",
                "\"rayCount\":{},\"occludedCount\":{},\"openCount\":{},",
                "\"analysisMicroseconds\":{},\"hash\":\"{}\",\"summaries\":["
            ),
            json_escape(input),
            options.viewpoints.len(),
            result.sample_count,
            result.rays.len(),
            result.rays.len() - total_open,
            total_open,
            elapsed,
            format_hash(&result.content_hash)
        );
        for (index, summary) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                concat!(
                    "{{\"viewpointId\":{},\"areaM2\":{},\"perimeterM\":{},",
                    "\"meanRadialM\":{},\"minimumRadialM\":{},\"maximumRadialM\":{},",
                    "\"radialStandardDeviationM\":{},\"radialSkewness\":{},",
                    "\"compactness\":{},\"areaConvergenceDeltaM2\":{},",
                    "\"dominantOccluderObjectId\":{},\"dominantOccluderFraction\":{}}}"
                ),
                summary.viewpoint_id.get(),
                summary.area_square_meters,
                summary.perimeter_meters,
                summary.mean_radial_meters,
                summary.minimum_radial_meters,
                summary.maximum_radial_meters,
                summary.radial_standard_deviation_meters,
                summary.radial_skewness,
                summary.compactness,
                summary.area_convergence_delta_square_meters,
                summary.dominant_occluder_object_id.get(),
                summary.dominant_occluder_fraction
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA planar isovist");
        println!("input: {input}");
        println!(
            "study: {} viewpoint(s) x {} samples = {} rays",
            options.viewpoints.len(),
            result.sample_count,
            result.rays.len()
        );
        println!(
            "states: {} occluded; {} open at radial limit",
            result.rays.len() - total_open,
            total_open
        );
        for summary in &result.summaries {
            println!(
                "viewpoint {}: area {:.6} m2; perimeter {:.6} m; compactness {:.6}; mean depth {:.6} m; delta area {:.6} m2; dominant blocker {} ({:.3}%)",
                summary.viewpoint_id,
                summary.area_square_meters,
                summary.perimeter_meters,
                summary.compactness,
                summary.mean_radial_meters,
                summary.area_convergence_delta_square_meters,
                summary.dominant_occluder_object_id,
                summary.dominant_occluder_fraction * 100.0
            );
        }
        println!("analysis: {:.3} ms", micros_to_milliseconds(elapsed));
        println!("hash: {}", format_hash(&result.content_hash));
        if let Some(path) = options.boundary_csv {
            println!("boundary csv: {path}");
        }
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_isovist_options(arguments: &[String]) -> Result<IsovistCliOptions, ()> {
    let mut viewpoints = Vec::new();
    let mut options = IsovistOptions::default();
    let mut thread_count = 0;
    let mut boundary_csv = None;
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
            "--viewpoint" => {
                let values = parse_comma_vector(value, "viewpoint", 9)?;
                let id = usize_id(viewpoints.len(), "viewpoints")?;
                viewpoints.push(
                    Viewpoint::try_new(
                        SensorId::new(id),
                        Vec3::new(values[0], values[1], values[2]),
                        Vec3::new(values[3], values[4], values[5]),
                        Vec3::new(values[6], values[7], values[8]),
                    )
                    .map_err(|error| eprintln!("error: invalid viewpoint: {error}"))?,
                );
            }
            "--samples" => {
                options.sample_count = value
                    .parse()
                    .map_err(|_| eprintln!("error: samples must be an integer"))?;
            }
            "--fov" => {
                let degrees = parse_finite_number(value, "fov")?;
                options.field_of_view_radians = degrees * TAU / 360.0;
            }
            "--max-distance" => {
                options.maximum_distance_meters = parse_positive(value, "max-distance")?;
            }
            "--offset" => options.eye_offset_meters = parse_nonnegative(value, "offset")?,
            "--category-mask" => options.category_mask = parse_category_mask(value)?,
            "--threads" => thread_count = parse_threads(value)?,
            "--boundary-csv" => boundary_csv = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown isovist option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if viewpoints.is_empty() {
        eprintln!("error: repeat --viewpoint x,y,z,nx,ny,nz,fx,fy,fz at least once");
        return Err(());
    }
    Ok(IsovistCliOptions {
        viewpoints,
        options,
        thread_count,
        boundary_csv,
        json,
    })
}

fn write_isovist_csv(
    path: &str,
    viewpoints: &[Viewpoint],
    result: &xvarna_daena::IsovistResult,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(fs::File::create(path)?);
    writeln!(
        writer,
        "viewpoint_id,sample_index,state,direction_x,direction_y,direction_z,endpoint_x_m,endpoint_y_m,endpoint_z_m,distance_m,object_id,instance_id,mesh_id,triangle_id"
    )?;
    for (viewpoint_index, viewpoint) in viewpoints.iter().enumerate() {
        for sample_index in 0..result.sample_count {
            let ray = result.rays[viewpoint_index * result.sample_count + sample_index];
            writeln!(
                writer,
                "{},{},{},{:.17},{:.17},{:.17},{:.17},{:.17},{:.17},{:.17},{},{},{},{}",
                viewpoint.id.get(),
                sample_index,
                u8::from(ray.state == IsovistRayState::OpenAtLimit),
                ray.direction.x,
                ray.direction.y,
                ray.direction.z,
                ray.endpoint.x,
                ray.endpoint.y,
                ray.endpoint.z,
                ray.distance_meters,
                ray.object_id.get(),
                ray.instance_id.get(),
                ray.mesh_id.get(),
                ray.triangle_id
            )?;
        }
    }
    writer.flush()
}

#[derive(Clone, Debug)]
struct IntervisibilityCliOptions {
    observers: Vec<VisibilityObserver>,
    targets: Vec<VisibilityTarget>,
    options: IntervisibilityOptions,
    thread_count: usize,
    matrix_csv: Option<String>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
fn run_intervisibility(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_intervisibility_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "intervisibility") else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let result = match analyze_intervisibility(
        &scene,
        &options.observers,
        &options.targets,
        options.options,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: DAENA intervisibility failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed().as_micros();
    if let Some(path) = &options.matrix_csv
        && let Err(error) =
            write_intervisibility_csv(path, &options.observers, &options.targets, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    let visible = result
        .entries
        .iter()
        .filter(|entry| entry.state == VisibilityState::Visible)
        .count();
    let blocked = result
        .entries
        .iter()
        .filter(|entry| entry.state == VisibilityState::Blocked)
        .count();
    let peak = result
        .entries
        .iter()
        .map(|entry| entry.privacy_risk)
        .fold(0.0, f64::max);
    if options.json {
        print!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA intervisibility\",",
                "\"input\":\"{}\",\"observerCount\":{},\"targetCount\":{},\"entryCount\":{},",
                "\"visibleCount\":{},\"blockedCount\":{},\"peakPrivacyRisk\":{},",
                "\"analysisMicroseconds\":{},\"hash\":\"{}\",\"targetSummaries\":["
            ),
            json_escape(input),
            options.observers.len(),
            options.targets.len(),
            result.entries.len(),
            visible,
            blocked,
            peak,
            elapsed,
            format_hash(&result.content_hash)
        );
        for (index, summary) in result.target_summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"targetId\":{},\"visibleObserverCount\":{},\"exposureFraction\":{},\"cumulativePrivacyRisk\":{},\"combinedPrivacyRisk\":{}}}",
                summary.target_id.get(),
                summary.visible_observer_count,
                summary.exposure_fraction,
                summary.cumulative_privacy_risk,
                summary.combined_privacy_risk
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA directed intervisibility + privacy");
        println!("input: {input}");
        println!(
            "matrix: {} observers x {} targets = {} pairs",
            options.observers.len(),
            options.targets.len(),
            result.entries.len()
        );
        println!(
            "states: {visible} visible; {blocked} blocked; {} excluded/coincident",
            result.entries.len() - visible - blocked
        );
        println!(
            "privacy formula: observer_weight * target_sensitivity * facing^({:.6}) * 1/(1+(distance/{:.6})^2)",
            options.options.facing_exponent, options.options.privacy_reference_distance_meters
        );
        println!("peak pair privacy risk: {peak:.6}");
        println!("analysis: {:.3} ms", micros_to_milliseconds(elapsed));
        println!("hash: {}", format_hash(&result.content_hash));
        if let Some(path) = options.matrix_csv {
            println!("matrix csv: {path}");
        }
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_intervisibility_options(arguments: &[String]) -> Result<IntervisibilityCliOptions, ()> {
    let mut observers = Vec::new();
    let mut targets = Vec::new();
    let mut options = IntervisibilityOptions::default();
    let mut thread_count = 0;
    let mut matrix_csv = None;
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
            "--observer" => {
                let values = parse_comma_values(value, "observer")?;
                if values.len() != 4 {
                    eprintln!("error: observer requires x,y,z,weight");
                    return Err(());
                }
                let id = usize_id(observers.len(), "observers")?;
                observers.push(
                    VisibilityObserver::try_new(
                        SensorId::new(id),
                        Vec3::new(values[0], values[1], values[2]),
                        values[3],
                    )
                    .map_err(|error| eprintln!("error: invalid observer: {error}"))?,
                );
            }
            "--target" => {
                let values = parse_comma_values(value, "target")?;
                if values.len() != 4 && values.len() != 7 {
                    eprintln!(
                        "error: target requires x,y,z,sensitivity or x,y,z,sensitivity,fx,fy,fz"
                    );
                    return Err(());
                }
                let id = usize_id(targets.len(), "targets")?;
                let facing =
                    (values.len() == 7).then(|| Vec3::new(values[4], values[5], values[6]));
                targets.push(
                    VisibilityTarget::try_new(
                        TargetId::new(id),
                        Vec3::new(values[0], values[1], values[2]),
                        facing,
                        values[3],
                    )
                    .map_err(|error| eprintln!("error: invalid target: {error}"))?,
                );
            }
            "--clearance" => {
                options.endpoint_clearance_meters = parse_nonnegative(value, "clearance")?;
            }
            "--max-distance" => {
                options.maximum_distance_meters = parse_positive(value, "max-distance")?;
            }
            "--privacy-reference" => {
                options.privacy_reference_distance_meters =
                    parse_positive(value, "privacy-reference")?;
            }
            "--facing-exponent" => {
                options.facing_exponent = parse_nonnegative(value, "facing-exponent")?;
            }
            "--category-mask" => options.category_mask = parse_category_mask(value)?,
            "--threads" => thread_count = parse_threads(value)?,
            "--matrix-csv" => matrix_csv = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown intervisibility option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if observers.is_empty() || targets.is_empty() {
        eprintln!("error: at least one --observer and one --target are required");
        return Err(());
    }
    Ok(IntervisibilityCliOptions {
        observers,
        targets,
        options,
        thread_count,
        matrix_csv,
        json,
    })
}

fn write_intervisibility_csv(
    path: &str,
    observers: &[VisibilityObserver],
    targets: &[VisibilityTarget],
    result: &xvarna_daena::IntervisibilityResult,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(fs::File::create(path)?);
    writeln!(
        writer,
        "observer_id,target_id,state,distance_m,direction_x,direction_y,direction_z,privacy_risk,facing_factor,distance_factor,blocker_object_id,blocker_instance_id,blocker_mesh_id,blocker_triangle_id"
    )?;
    for (observer_index, observer) in observers.iter().enumerate() {
        for (target_index, target) in targets.iter().enumerate() {
            let entry = result.entries[observer_index * targets.len() + target_index];
            writeln!(
                writer,
                "{},{},{},{:.17},{:.17},{:.17},{:.17},{:.17},{:.17},{:.17},{},{},{},{}",
                observer.id.get(),
                target.id.get(),
                entry.state as u8,
                entry.distance_meters,
                entry.direction.x,
                entry.direction.y,
                entry.direction.z,
                entry.privacy_risk,
                entry.facing_factor,
                entry.distance_factor,
                entry.blocker_object_id.get(),
                entry.blocker_instance_id.get(),
                entry.blocker_mesh_id.get(),
                entry.blocker_triangle_id
            )?;
        }
    }
    writer.flush()
}

#[derive(Clone, Debug)]
struct VisibilityGraphCliOptions {
    nodes: Vec<VisibilityNode>,
    options: VisibilityGraphOptions,
    sparse: bool,
    maximum_neighbors: usize,
    thread_count: usize,
    pairs_csv: Option<String>,
    metrics_csv: Option<String>,
    json: bool,
}

#[allow(clippy::too_many_lines)]
fn run_visibility_graph(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_visibility_graph_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "visibility-graph") else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let result = match if options.sparse {
        analyze_sparse_visibility_graph(
            &scene,
            &options.nodes,
            SparseVisibilityGraphOptions {
                endpoint_clearance_meters: options.options.endpoint_clearance_meters,
                maximum_distance_meters: options.options.maximum_distance_meters,
                category_mask: options.options.category_mask,
                maximum_neighbors: options.maximum_neighbors,
                compute_centrality: options.options.compute_centrality,
            },
        )
    } else {
        analyze_visibility_graph(&scene, &options.nodes, options.options)
    } {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: DAENA visibility graph failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed().as_micros();
    if let Some(path) = &options.pairs_csv
        && let Err(error) = write_visibility_pairs_csv(path, &options.nodes, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    if let Some(path) = &options.metrics_csv
        && let Err(error) = write_visibility_metrics_csv(path, &result)
    {
        eprintln!("error: cannot write {}: {error}", Path::new(path).display());
        return ExitCode::FAILURE;
    }
    let possible = options
        .nodes
        .len()
        .saturating_mul(options.nodes.len().saturating_sub(1))
        / 2;
    let density = if possible == 0 {
        0.0
    } else {
        count_to_f64(result.visible_edge_count) / count_to_f64(possible)
    };
    if options.json {
        print!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA visibility graph\",",
                "\"input\":\"{}\",\"nodeCount\":{},\"pairCount\":{},\"visibleEdgeCount\":{},",
                "\"density\":{},\"connectedComponentCount\":{},\"centralityComputed\":{},",
                "\"sparse\":{},\"maximumNeighbors\":{},",
                "\"analysisMicroseconds\":{},\"hash\":\"{}\",\"metrics\":["
            ),
            json_escape(input),
            options.nodes.len(),
            result.pairs.len(),
            result.visible_edge_count,
            density,
            result.connected_component_count,
            options.options.compute_centrality,
            options.sparse,
            if options.sparse {
                options.maximum_neighbors
            } else {
                0
            },
            elapsed,
            format_hash(&result.content_hash)
        );
        for (index, metric) in result.metrics.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"nodeId\":{},\"degree\":{},\"degreeCentrality\":{},\"componentIndex\":{},\"harmonicCloseness\":{},\"betweennessCentrality\":{}}}",
                metric.node_id.get(),
                metric.degree,
                metric.degree_centrality,
                metric.component_index,
                metric.harmonic_closeness,
                metric.betweenness_centrality
            );
        }
        println!("]}}");
    } else {
        println!(
            "XVARNA DAENA {} visibility graph",
            if options.sparse {
                "spatial sparse"
            } else {
                "exact all-pairs"
            }
        );
        println!("input: {input}");
        println!(
            "graph: {} nodes; {} pairs; {} visible edges; density {:.6}",
            options.nodes.len(),
            result.pairs.len(),
            result.visible_edge_count,
            density
        );
        println!(
            "topology: {} connected component(s); centrality {}",
            result.connected_component_count,
            if options.options.compute_centrality {
                "computed"
            } else {
                "disabled"
            }
        );
        println!("analysis: {:.3} ms", micros_to_milliseconds(elapsed));
        println!("hash: {}", format_hash(&result.content_hash));
        if let Some(path) = options.pairs_csv {
            println!("pairs csv: {path}");
        }
        if let Some(path) = options.metrics_csv {
            println!("metrics csv: {path}");
        }
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
fn parse_visibility_graph_options(arguments: &[String]) -> Result<VisibilityGraphCliOptions, ()> {
    let mut nodes = Vec::new();
    let mut options = VisibilityGraphOptions::default();
    let mut sparse = false;
    let mut maximum_neighbors = 16;
    let mut thread_count = 0;
    let mut pairs_csv = None;
    let mut metrics_csv = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if argument == "--json" {
            json = true;
            index += 1;
            continue;
        }
        if argument == "--no-centrality" {
            options.compute_centrality = false;
            index += 1;
            continue;
        }
        if argument == "--sparse" {
            sparse = true;
            options.compute_centrality = false;
            index += 1;
            continue;
        }
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--node" => {
                let values = parse_comma_vector(value, "node", 3)?;
                let id = usize_id(nodes.len(), "nodes")?;
                nodes.push(
                    VisibilityNode::try_new(
                        SensorId::new(id),
                        Vec3::new(values[0], values[1], values[2]),
                    )
                    .map_err(|error| eprintln!("error: invalid node: {error}"))?,
                );
            }
            "--clearance" => {
                options.endpoint_clearance_meters = parse_nonnegative(value, "clearance")?;
            }
            "--max-distance" => {
                options.maximum_distance_meters = parse_positive(value, "max-distance")?;
            }
            "--category-mask" => options.category_mask = parse_category_mask(value)?,
            "--max-neighbors" => {
                maximum_neighbors = value.parse::<usize>().map_err(|_| {
                    eprintln!("error: max-neighbors must be an unsigned integer");
                })?;
            }
            "--threads" => thread_count = parse_threads(value)?,
            "--pairs-csv" => pairs_csv = Some(value.clone()),
            "--metrics-csv" => metrics_csv = Some(value.clone()),
            unknown => {
                eprintln!("error: unknown visibility-graph option '{unknown}'");
                return Err(());
            }
        }
        index += 2;
    }
    if nodes.is_empty() {
        eprintln!("error: repeat --node x,y,z at least once");
        return Err(());
    }
    if sparse && !options.maximum_distance_meters.is_finite() {
        eprintln!("error: --sparse requires a finite --max-distance");
        return Err(());
    }
    if sparse && !(1..=1_024).contains(&maximum_neighbors) {
        eprintln!("error: --max-neighbors must be 1..1024");
        return Err(());
    }
    Ok(VisibilityGraphCliOptions {
        nodes,
        options,
        sparse,
        maximum_neighbors,
        thread_count,
        pairs_csv,
        metrics_csv,
        json,
    })
}

fn write_visibility_pairs_csv(
    path: &str,
    nodes: &[VisibilityNode],
    result: &xvarna_daena::VisibilityGraphResult,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(fs::File::create(path)?);
    writeln!(
        writer,
        "first_index,first_node_id,second_index,second_node_id,state,distance_m,blocker_object_id"
    )?;
    for pair in &result.pairs {
        writeln!(
            writer,
            "{},{},{},{},{},{:.17},{}",
            pair.first_index,
            nodes[pair.first_index].id.get(),
            pair.second_index,
            nodes[pair.second_index].id.get(),
            pair.state as u8,
            pair.distance_meters,
            pair.blocker_object_id.get()
        )?;
    }
    writer.flush()
}

fn write_visibility_metrics_csv(
    path: &str,
    result: &xvarna_daena::VisibilityGraphResult,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(fs::File::create(path)?);
    writeln!(
        writer,
        "node_id,degree,degree_centrality,component_index,harmonic_closeness,betweenness_centrality"
    )?;
    for metric in &result.metrics {
        writeln!(
            writer,
            "{},{},{:.17},{},{:.17},{:.17}",
            metric.node_id.get(),
            metric.degree,
            metric.degree_centrality,
            metric.component_index,
            metric.harmonic_closeness,
            metric.betweenness_centrality
        )?;
    }
    writer.flush()
}

fn build_cli_scene(input: &str, thread_count: usize, command: &str) -> Option<Scene> {
    let source = std::fs::read_to_string(input)
        .map_err(|error| eprintln!("error: cannot read {input}: {error}"))
        .ok()?;
    let objects = xvarna_io::parse_obj_scene(&source)
        .map_err(|error| eprintln!("error: OBJ import failed: {error}"))
        .ok()?;
    if objects.is_empty() {
        eprintln!("error: input contains no faces for {command}");
        return None;
    }
    let mut builder = match SceneBuilder::new(SceneBuildOptions {
        thread_count,
        ..SceneBuildOptions::default()
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: scene options failed: {error}");
            return None;
        }
    };
    for (index, object) in objects.into_iter().enumerate() {
        let audit = audit_mesh(
            &object.mesh,
            MeshAuditOptions::try_new(1.0e-6, 0.25).expect("fixed options valid"),
        )
        .ok()?;
        if !audit.is_analysis_ready {
            eprintln!(
                "error: object {} requires mesh repair before {command}",
                object.name
            );
            return None;
        }
        let object_id = (index + 1) as u64;
        eprintln!("OBJ object {object_id}: {}", object.name);
        let mesh_id = match builder.add_mesh(object.mesh) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: scene mesh ingest failed: {error}");
                return None;
            }
        };
        if let Err(error) = builder.add_instance(
            mesh_id,
            Transform::IDENTITY,
            ObjectId::new(object_id),
            InstanceId::new(object_id),
            u64::MAX,
        ) {
            eprintln!("error: scene instance failed: {error}");
            return None;
        }
    }
    match builder.build() {
        Ok(value) => Some(value),
        Err(error) => {
            eprintln!("error: scene compile failed: {error}");
            None
        }
    }
}

fn parse_comma_values(value: &str, name: &str) -> Result<Vec<f64>, ()> {
    value
        .split(',')
        .map(|part| parse_finite_number(part.trim(), name))
        .collect()
}

fn parse_positive(value: &str, name: &str) -> Result<f64, ()> {
    let parsed = parse_finite_number(value, name)?;
    if parsed <= 0.0 {
        eprintln!("error: {name} must be positive");
        return Err(());
    }
    Ok(parsed)
}

fn parse_nonnegative(value: &str, name: &str) -> Result<f64, ()> {
    let parsed = parse_finite_number(value, name)?;
    if parsed < 0.0 {
        eprintln!("error: {name} must be non-negative");
        return Err(());
    }
    Ok(parsed)
}

fn parse_category_mask(value: &str) -> Result<u64, ()> {
    if value == "-1" {
        return Ok(u64::MAX);
    }
    value
        .parse()
        .map_err(|_| eprintln!("error: category-mask must be -1 or an unsigned 64-bit integer"))
}

fn parse_threads(value: &str) -> Result<usize, ()> {
    let parsed = value
        .parse()
        .map_err(|_| eprintln!("error: threads must be a non-negative integer"))?;
    if parsed > 256 {
        eprintln!("error: threads must be at most 256");
        return Err(());
    }
    Ok(parsed)
}

fn usize_id(index: usize, name: &str) -> Result<u64, ()> {
    u64::try_from(index + 1).map_err(|_| eprintln!("error: too many {name}"))
}

fn count_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("CLI production caps keep counts within u32"))
}

fn micros_to_milliseconds(value: u128) -> f64 {
    Duration::from_micros(u64::try_from(value).unwrap_or(u64::MAX)).as_secs_f64() * 1000.0
}

fn load_obj(path: &str) -> Option<xvarna_io::ObjImportReport> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: cannot read {}: {error}", Path::new(path).display());
            return None;
        }
    };
    match parse_obj(&source) {
        Ok(report) => Some(report),
        Err(error) => {
            eprintln!("error: {error}");
            None
        }
    }
}

fn parse_check_options(arguments: &[String]) -> Result<(f64, bool), ()> {
    let mut tolerance = 1.0e-6;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--tolerance" => {
                tolerance = parse_tolerance(arguments.get(index + 1))?;
                index += 2;
            }
            unknown => {
                eprintln!("error: unknown mesh-check option '{unknown}'");
                return Err(());
            }
        }
    }
    Ok((tolerance, json))
}

fn parse_repair_options(arguments: &[String]) -> Result<f64, ()> {
    if arguments.is_empty() {
        return Ok(1.0e-6);
    }
    if arguments.len() == 2 && arguments[0] == "--tolerance" {
        return parse_tolerance(arguments.get(1));
    }
    eprintln!("error: mesh-repair accepts only '--tolerance <VALUE>'");
    Err(())
}

fn parse_tolerance(value: Option<&String>) -> Result<f64, ()> {
    let Some(value) = value else {
        eprintln!("error: --tolerance requires a value");
        return Err(());
    };
    let Ok(tolerance) = value.parse::<f64>() else {
        eprintln!("error: tolerance must be a number");
        return Err(());
    };
    if !tolerance.is_finite() || tolerance <= 0.0 {
        eprintln!("error: tolerance must be finite and positive");
        return Err(());
    }
    Ok(tolerance)
}

fn print_audit_report(path: &str, report: &MeshAuditReport, triangulated_quads: usize) {
    println!("XVARNA RASHNU Mesh Doctor {}", engine_version());
    println!("File: {}", Path::new(path).display());
    println!(
        "Status: {}",
        if report.is_analysis_ready {
            "analysis-ready"
        } else {
            "repair required"
        }
    );
    println!(
        "Topology: {}",
        if report.is_watertight {
            "watertight"
        } else {
            "open or inconsistent"
        }
    );
    println!(
        "Vertices: {}; triangles: {}; accepted: {}; triangulated quads: {}",
        report.vertex_count, report.face_count, report.accepted_face_count, triangulated_quads
    );
    println!(
        "Degenerate: {}; duplicate: {}; invalid index: {}; non-finite faces: {}",
        report.degenerate_face_count,
        report.duplicate_face_count,
        report.invalid_index_face_count,
        report.non_finite_face_count
    );
    println!(
        "Boundary edges: {}; non-manifold: {}; winding conflicts: {}",
        report.boundary_edge_count,
        report.non_manifold_edge_count,
        report.inconsistent_winding_edge_count
    );
    println!(
        "Isolated vertices: {}; connected components: {}",
        report.isolated_vertex_count, report.connected_component_count
    );
    println!("Surface area: {:.10}", report.surface_area);
    println!("Signed volume: {:.10}", report.signed_volume);
    println!("Estimated f32 error: {:.6e}", report.estimated_f32_error);
    println!(
        "Precision budget: {}",
        if report.exceeds_f32_precision_budget {
            "exceeded"
        } else {
            "within budget"
        }
    );
    println!("Geometry hash: {}", format_hash(&report.content_hash));
}

fn print_audit_json(report: &MeshAuditReport, triangulated_quads: usize) {
    println!(
        concat!(
            "{{\"schemaVersion\":\"0.16.0\",\"analysisReady\":{},\"watertight\":{},",
            "\"vertices\":{},\"triangles\":{},\"acceptedTriangles\":{},\"triangulatedQuads\":{},",
            "\"degenerateFaces\":{},\"duplicateFaces\":{},\"invalidIndexFaces\":{},",
            "\"nonFiniteFaces\":{},\"isolatedVertices\":{},\"boundaryEdges\":{},",
            "\"nonManifoldEdges\":{},\"windingConflicts\":{},\"components\":{},",
            "\"surfaceArea\":{},\"signedVolume\":{},\"estimatedF32Error\":{},",
            "\"f32PrecisionRisk\":{},\"contentHash\":\"{}\"}}"
        ),
        report.is_analysis_ready,
        report.is_watertight,
        report.vertex_count,
        report.face_count,
        report.accepted_face_count,
        triangulated_quads,
        report.degenerate_face_count,
        report.duplicate_face_count,
        report.invalid_index_face_count,
        report.non_finite_face_count,
        report.isolated_vertex_count,
        report.boundary_edge_count,
        report.non_manifold_edge_count,
        report.inconsistent_winding_edge_count,
        report.connected_component_count,
        report.surface_area,
        report.signed_volume,
        report.estimated_f32_error,
        report.exceeds_f32_precision_budget,
        format_hash(&report.content_hash)
    );
}

fn format_hash(hash: &[u8; 32]) -> String {
    let mut formatted = String::with_capacity(64);
    for byte in hash {
        write!(&mut formatted, "{byte:02x}").expect("writing to String cannot fail");
    }
    formatted
}

fn print_info(json: bool) {
    if json {
        println!(
            "{{\"engineVersion\":\"{}\",\"abiVersion\":\"{}\",\"os\":\"{}\",\"arch\":\"{}\"}}",
            engine_version(),
            ABI_VERSION,
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    } else {
        println!("XVARNA engine: {}", engine_version());
        println!("Native ABI: {ABI_VERSION}");
        println!("Host: {}-{}", std::env::consts::OS, std::env::consts::ARCH);
    }
}

fn run_self_test() -> ExitCode {
    let ray = Ray::try_new(
        Vec3::new(0.25, 0.25, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
        0.0,
        10.0,
    );
    let triangle = Triangle {
        a: Vec3::new(0.0, 0.0, 0.0),
        b: Vec3::new(1.0, 0.0, 0.0),
        c: Vec3::new(0.0, 1.0, 0.0),
        object_id: ObjectId::new(1),
    };

    let Ok(ray) = ray else {
        eprintln!("self-test failed: reference ray is invalid");
        return ExitCode::FAILURE;
    };
    let Some(hit) = intersect_triangle_reference(ray, triangle) else {
        eprintln!("self-test failed: expected reference hit");
        return ExitCode::FAILURE;
    };
    if (hit.distance - 1.0).abs() > 1.0e-12 || hit.object_id != ObjectId::new(1) {
        eprintln!("self-test failed: unexpected reference result {hit:?}");
        return ExitCode::FAILURE;
    }

    println!("self-test passed: reference triangle distance = 1.0 m");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_position_cli_parses_explicit_offset_and_scientific_options() {
        let arguments = [
            "--latitude",
            "39.742476",
            "--longitude",
            "-105.1786",
            "--elevation",
            "1830.14",
            "--time",
            "2003-10-17T19:30:30Z",
            "--delta-t",
            "67",
            "--pressure",
            "820",
            "--temperature",
            "11",
            "--json",
        ]
        .map(str::to_owned);
        let options = parse_sun_position_options(&arguments).expect("options are valid");
        assert_eq!(options.times, vec![1_066_419_030]);
        assert!((options.elevation_meters - 1_830.14).abs() < f64::EPSILON);
        assert!((options.solar.delta_t_seconds - 67.0).abs() < f64::EPSILON);
        assert!((options.solar.pressure_millibars - 820.0).abs() < f64::EPSILON);
        assert!(options.json);
    }

    #[test]
    fn sun_position_cli_requires_location_and_time() {
        let arguments = ["--latitude", "35.0"].map(str::to_owned);
        assert!(parse_sun_position_options(&arguments).is_err());
    }

    #[test]
    fn sky_view_cli_parses_repeatable_sensors_and_export_policy() {
        let arguments = [
            "--sensor",
            "1,2,3,0,0,1",
            "--sensor",
            "4,5,6,0,1,0",
            "--samples",
            "8192",
            "--seed",
            "42",
            "--threads",
            "8",
            "--mask-csv",
            "mask.csv",
            "--json",
        ]
        .map(str::to_owned);

        let options = parse_sky_view_options(&arguments).expect("options are valid");

        assert_eq!(options.sensors.len(), 2);
        assert_eq!(options.sensors[0].id, SensorId::new(1));
        assert_eq!(options.sensors[1].normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(options.sky.sample_count, 8_192);
        assert_eq!(options.sky.seed, 42);
        assert_eq!(options.thread_count, 8);
        assert_eq!(options.mask_csv.as_deref(), Some("mask.csv"));
        assert!(options.json);
    }

    #[test]
    fn sky_view_cli_rejects_missing_or_invalid_sensors() {
        assert!(parse_sky_view_options(&[]).is_err());
        let invalid = ["--sensor", "0,0,0,0,0,0"].map(str::to_owned);
        assert!(parse_sky_view_options(&invalid).is_err());
        let too_few_samples = ["--sensor", "0,0,0,0,0,1", "--samples", "8"].map(str::to_owned);
        assert!(parse_sky_view_options(&too_few_samples).is_err());
    }

    #[test]
    fn annual_cli_parses_scientific_export_and_execution_policy() {
        let arguments = [
            "--sensor",
            "1,2,3,0,0,1",
            "--sensor",
            "4,5,6,0,1,0",
            "--albedo",
            "0.35",
            "--no-weather-albedo",
            "--north",
            "15",
            "--threads",
            "8",
            "--timeline-csv",
            "annual.csv",
            "--json",
        ]
        .map(str::to_owned);

        let options = parse_annual_irradiance_options(&arguments).expect("options valid");

        assert_eq!(options.sensors.len(), 2);
        assert_eq!(options.sensors[1].normal, Vec3::new(0.0, 1.0, 0.0));
        assert!((options.irradiance.ground_albedo - 0.35).abs() < f64::EPSILON);
        assert!(!options.irradiance.use_weather_albedo);
        assert!((options.irradiance.solar.north_rotation_degrees - 15.0).abs() < f64::EPSILON);
        assert_eq!(options.thread_count, 8);
        assert_eq!(options.timeline_csv.as_deref(), Some("annual.csv"));
        assert!(options.json);
    }

    #[test]
    fn annual_cli_requires_sensor_and_valid_albedo() {
        assert!(parse_annual_irradiance_options(&[]).is_err());
        let invalid = ["--sensor", "0,0,0,0,0,1", "--albedo", "1.5"].map(str::to_owned);
        assert!(parse_annual_irradiance_options(&invalid).is_err());
    }

    #[test]
    fn surface_grid_cli_requires_resolution_and_parses_resource_policy() {
        let arguments = [
            "--cell-size",
            "0.25",
            "--offset",
            "0.002",
            "--first-id",
            "100",
            "--max-cells",
            "50000",
            "--mesh-obj",
            "grid.obj",
            "--json",
        ]
        .map(str::to_owned);
        let options = parse_surface_grid_options(&arguments).expect("options valid");
        assert!((options.grid.target_edge_length - 0.25).abs() < f64::EPSILON);
        assert!((options.grid.sensor_offset - 0.002).abs() < f64::EPSILON);
        assert_eq!(options.grid.first_sensor_id, 100);
        assert_eq!(options.grid.maximum_cell_count, 50_000);
        assert_eq!(options.mesh_obj.as_deref(), Some("grid.obj"));
        assert!(options.json);
        assert!(parse_surface_grid_options(&[]).is_err());
    }

    #[test]
    fn daena_isovist_cli_parses_frames_resolution_and_export() {
        let arguments = [
            "--viewpoint",
            "0,0,1.6,0,0,1,1,0,0",
            "--viewpoint",
            "5,0,1.6,0,0,1,0,1,0",
            "--samples",
            "1440",
            "--fov",
            "270",
            "--max-distance",
            "50",
            "--boundary-csv",
            "isovist.csv",
            "--threads",
            "4",
            "--json",
        ]
        .map(str::to_owned);
        let options = parse_isovist_options(&arguments).expect("DAENA isovist options valid");
        assert_eq!(options.viewpoints.len(), 2);
        assert_eq!(options.options.sample_count, 1440);
        assert!(
            1.5_f64
                .mul_add(
                    -core::f64::consts::PI,
                    options.options.field_of_view_radians
                )
                .abs()
                < 1.0e-12
        );
        assert_eq!(options.boundary_csv.as_deref(), Some("isovist.csv"));
        assert_eq!(options.thread_count, 4);
        assert!(options.json);
    }

    #[test]
    fn daena_intervisibility_cli_parses_directional_privacy_contract() {
        let arguments = [
            "--observer",
            "0,0,1.6,0.8",
            "--observer",
            "5,0,1.6,1",
            "--target",
            "10,0,1.6,0.9,-1,0,0",
            "--privacy-reference",
            "8",
            "--facing-exponent",
            "2",
            "--matrix-csv",
            "matrix.csv",
            "--json",
        ]
        .map(str::to_owned);
        let options =
            parse_intervisibility_options(&arguments).expect("DAENA matrix options valid");
        assert_eq!(options.observers.len(), 2);
        assert_eq!(options.targets.len(), 1);
        assert!(options.targets[0].facing.is_some());
        assert!((options.options.privacy_reference_distance_meters - 8.0).abs() < f64::EPSILON);
        assert!((options.options.facing_exponent - 2.0).abs() < f64::EPSILON);
        assert_eq!(options.matrix_csv.as_deref(), Some("matrix.csv"));
    }

    #[test]
    fn daena_visibility_graph_cli_parses_topology_policy() {
        let arguments = [
            "--node",
            "0,0,1.6",
            "--node",
            "1,0,1.6",
            "--node",
            "2,0,1.6",
            "--max-distance",
            "25",
            "--sparse",
            "--max-neighbors",
            "12",
            "--no-centrality",
            "--pairs-csv",
            "pairs.csv",
            "--metrics-csv",
            "metrics.csv",
        ]
        .map(str::to_owned);
        let options =
            parse_visibility_graph_options(&arguments).expect("DAENA graph options valid");
        assert_eq!(options.nodes.len(), 3);
        assert!(options.sparse);
        assert_eq!(options.maximum_neighbors, 12);
        assert!(!options.options.compute_centrality);
        assert_eq!(options.pairs_csv.as_deref(), Some("pairs.csv"));
        assert_eq!(options.metrics_csv.as_deref(), Some("metrics.csv"));
        assert!(parse_visibility_graph_options(&[]).is_err());
    }
}

#[cfg(test)]
mod obj_scene_regression {
    use super::*;
    #[test]
    fn cli_preserves_ids_used_by_optical_material_assignments() {
        let path =
            std::env::temp_dir().join(format!("xvarna object identity {}.obj", std::process::id()));
        std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nv 1 0 1\nv 0 1 1\no opaque roof\nf 1 2 3\no glazing\nf 4 5 6\n").unwrap();
        let scene = build_cli_scene(path.to_str().unwrap(), 1, "test").expect("valid scene");
        std::fs::remove_file(path).unwrap();
        let triangles = scene.world_triangles();
        assert_eq!(triangles.len(), 2);
        assert_eq!(triangles[0].object_id.get(), 1);
        assert_eq!(triangles[1].object_id.get(), 2);
        assert_ne!(triangles[0].mesh_id, triangles[1].mesh_id);
    }
}
