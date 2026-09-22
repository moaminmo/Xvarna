//! XVARNA 0.15 photometric daylight and Radiance reference workflows.

use std::{fmt::Write as _, fs, path::Path, process::ExitCode};
use xvarna_geometry::Vec3;
use xvarna_hvare::{
    AnnualDaylightOptions, DaylightMatrixOptions, DaylightMoment, DaylightSensor, DaylightSkyModel,
    OpticalMaterial, OpticalMaterialKind, OpticalMaterialLibrary, RadianceRunOptions,
    RadianceToolchain, analyze_annual_daylight, analyze_daylight_factor, analyze_point_illuminance,
    compare_daylight_results, export_radiance_bundle, run_radiance_annual_matrix,
    run_radiance_point, write_radiance_bundle,
};
use xvarna_types::{ObjectId, SensorId};
use xvarna_zurvan::parse_epw;

use super::{build_cli_scene, format_hash};

#[derive(Clone, Debug, Default)]
struct DaylightInputs {
    sensors: Vec<DaylightSensor>,
    materials: Vec<OpticalMaterial>,
    assignments: Vec<(ObjectId, u64)>,
    threads: usize,
    json: bool,
    simulation_year: Option<i32>,
}

impl DaylightInputs {
    fn library(&self) -> Result<OpticalMaterialLibrary, ExitCode> {
        OpticalMaterialLibrary::try_new(self.materials.clone(), self.assignments.clone()).map_err(
            |error| {
                eprintln!("error: invalid optical material catalog: {error}");
                ExitCode::from(2)
            },
        )
    }
}

pub fn run_daylight_point(input: &str, arguments: &[String]) -> ExitCode {
    let mut common = DaylightInputs::default();
    let mut options = DaylightMatrixOptions::default();
    let mut sky_model = DaylightSkyModel::Isotropic;
    let mut moment = None;
    let result = parse_arguments(arguments, &mut common, |argument, value| match argument {
        "--moment" => parse_moment(value).map(|parsed| moment = Some(parsed)),
        "--sky" => parse_sky(value).map(|parsed| sky_model = parsed),
        _ => parse_matrix_option(argument, value, &mut options),
    });
    if result.is_err() {
        return ExitCode::from(2);
    }
    let Some(moment) = moment else {
        eprintln!(
            "error: --moment unix,dx,dy,dz,direct-normal-lux,diffuse-horizontal-lux is required"
        );
        return ExitCode::from(2);
    };
    let Some(scene) = checked_scene(input, &common, "daylight-point") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let result = match analyze_point_illuminance(
        &scene,
        &common.sensors,
        &materials,
        moment,
        sky_model,
        options,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: point illuminance failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if common.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"HVARE daylight fast path\",\"matrixReused\":{},\"matrixHash\":\"{}\",\"resultHash\":\"{}\",\"entries\":[",
            result.matrix_reused,
            format_hash(&result.matrix_hash),
            format_hash(&result.content_hash)
        );
        for (index, value) in result.entries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"sensorId\":{},\"directLux\":{},\"diffuseLux\":{},\"totalLux\":{},\"directTransmission\":{}}}",
                value.sensor_id,
                value.direct_lux,
                value.diffuse_lux,
                value.total_lux,
                value.direct_transmission
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA HVARE Point-in-Time Daylight 0.16.0");
        println!(
            "matrix: {} / reused={}",
            format_hash(&result.matrix_hash),
            result.matrix_reused
        );
        for value in &result.entries {
            println!(
                "sensor {}: total={:.3} lux; direct={:.3}; diffuse={:.3}; transmission={:.6}",
                value.sensor_id,
                value.total_lux,
                value.direct_lux,
                value.diffuse_lux,
                value.direct_transmission
            );
        }
        println!("hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

pub fn run_daylight_factor(input: &str, arguments: &[String]) -> ExitCode {
    let mut common = DaylightInputs::default();
    let mut options = DaylightMatrixOptions::default();
    let mut exterior_lux = 10_000.0;
    let parsed = parse_arguments(arguments, &mut common, |argument, value| match argument {
        "--exterior-lux" => parse_positive(value, "exterior-lux").map(|v| exterior_lux = v),
        _ => parse_matrix_option(argument, value, &mut options),
    });
    if parsed.is_err() {
        return ExitCode::from(2);
    }
    let Some(scene) = checked_scene(input, &common, "daylight-factor") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let result =
        match analyze_daylight_factor(&scene, &common.sensors, &materials, exterior_lux, options) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: daylight factor failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    if common.json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"exteriorLux\":{},\"matrixReused\":{},\"matrixHash\":\"{}\",\"resultHash\":\"{}\",\"entries\":[",
            exterior_lux,
            result.matrix_reused,
            format_hash(&result.matrix_hash),
            format_hash(&result.content_hash)
        );
        for (index, value) in result.entries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"sensorId\":{},\"interiorLux\":{},\"daylightFactorPercent\":{}}}",
                value.sensor_id, value.interior_illuminance_lux, value.daylight_factor_percent
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA CIE Overcast Daylight Factor 0.16.0");
        for value in &result.entries {
            println!(
                "sensor {}: {:.4}% / {:.3} lux",
                value.sensor_id, value.daylight_factor_percent, value.interior_illuminance_lux
            );
        }
        println!("matrix reused: {}", result.matrix_reused);
        println!("hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
pub fn run_annual_daylight(input: &str, weather_path: &str, arguments: &[String]) -> ExitCode {
    let mut common = DaylightInputs::default();
    let mut options = AnnualDaylightOptions::default();
    let mut timeline_csv = None;
    let parsed = parse_arguments(arguments, &mut common, |argument, value| match argument {
        "--timeline-csv" => {
            timeline_csv = Some(value.to_owned());
            Ok(())
        }
        "--sky" => parse_sky(value).map(|v| options.sky_model = v),
        "--direct-efficacy" => parse_positive(value, "direct-efficacy")
            .map(|v| options.direct_luminous_efficacy_lm_per_w = v),
        "--diffuse-efficacy" => parse_positive(value, "diffuse-efficacy")
            .map(|v| options.diffuse_luminous_efficacy_lm_per_w = v),
        "--occupied-start" => {
            parse_number(value, "occupied-start").map(|v| options.occupied_start_hour = v)
        }
        "--occupied-end" => {
            parse_number(value, "occupied-end").map(|v| options.occupied_end_hour = v)
        }
        "--sda-lux" => parse_positive(value, "sda-lux").map(|v| options.sda_threshold_lux = v),
        "--sda-fraction" => {
            parse_fraction(value, "sda-fraction").map(|v| options.sda_required_fraction = v)
        }
        "--ase-lux" => parse_positive(value, "ase-lux").map(|v| options.ase_threshold_lux = v),
        "--ase-hours" => {
            parse_nonnegative(value, "ase-hours").map(|v| options.ase_maximum_hours = v)
        }
        "--udi-lower" => parse_positive(value, "udi-lower").map(|v| options.udi_lower_lux = v),
        "--udi-preferred" => {
            parse_positive(value, "udi-preferred").map(|v| options.udi_preferred_lux = v)
        }
        "--udi-upper" => parse_positive(value, "udi-upper").map(|v| options.udi_upper_lux = v),
        "--north" => parse_number(value, "north").map(|v| options.solar.north_rotation_degrees = v),
        _ => parse_matrix_option(argument, value, &mut options.matrix),
    });
    if parsed.is_err() {
        return ExitCode::from(2);
    }
    let Some(scene) = checked_scene(input, &common, "annual-daylight") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let Some(weather) = load_weather(weather_path, common.simulation_year) else {
        return ExitCode::FAILURE;
    };
    let result =
        match analyze_annual_daylight(&scene, &common.sensors, &weather, &materials, options) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: annual daylight failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    if let Some(path) = timeline_csv.as_deref()
        && let Err(error) = write_annual_csv(path, &common.sensors, &result)
    {
        eprintln!("error: cannot write timeline: {error}");
        return ExitCode::FAILURE;
    }
    if common.json {
        println!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"HVARE annual daylight\",\"sensorCount\":{},\"weatherCount\":{},\"timelineCount\":{},\"matrixReused\":{},\"matrixHash\":\"{}\",\"resultHash\":\"{}\",\"project\":{{\"sdaAreaPercent\":{},\"aseAreaPercent\":{},\"udiBelowPercent\":{},\"udiSupplementalPercent\":{},\"udiUsefulPercent\":{},\"udiExceededPercent\":{},\"totalSensorAreaM2\":{}}}}}",
            result.summaries.len(),
            result.weather_count,
            result.timeline.len(),
            result.matrix_reused,
            format_hash(&result.matrix_hash),
            format_hash(&result.content_hash),
            result.project.sda_area_percent,
            result.project.ase_area_percent,
            result.project.udi_below_percent,
            result.project.udi_supplemental_percent,
            result.project.udi_useful_percent,
            result.project.udi_exceeded_percent,
            result.project.total_sensor_area_square_meters
        );
    } else {
        println!("XVARNA HVARE Annual Daylight 0.16.0");
        println!(
            "sDA={:.3}%  ASE={:.3}%  UDI useful={:.3}%  UDI exceeded={:.3}%",
            result.project.sda_area_percent,
            result.project.ase_area_percent,
            result.project.udi_useful_percent,
            result.project.udi_exceeded_percent
        );
        for value in &result.summaries {
            println!(
                "sensor {}: sDA={:.3}%; ASE={:.3}h (fails={}); UDI useful={:.3}%; mean={:.3} lux",
                value.sensor_id,
                value.sda_occupied_fraction * 100.0,
                value.ase_exceedance_hours,
                value.ase_fails,
                value.udi_useful_fraction * 100.0,
                value.mean_occupied_lux
            );
        }
        println!(
            "matrix: {} / reused={}",
            format_hash(&result.matrix_hash),
            result.matrix_reused
        );
        println!("hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

pub fn run_radiance_export(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let mut common = DaylightInputs::default();
    if parse_arguments(arguments, &mut common, |argument, _| {
        eprintln!("error: unknown radiance-export option '{argument}'");
        Err(())
    })
    .is_err()
    {
        return ExitCode::from(2);
    }
    let Some(scene) = checked_scene(input, &common, "radiance-export") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let bundle = match export_radiance_bundle(&scene, &common.sensors, &materials) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: Radiance export failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_radiance_bundle(&bundle, Path::new(output)) {
        eprintln!("error: cannot write Radiance bundle: {error}");
        return ExitCode::FAILURE;
    }
    println!("Radiance bundle: {}", Path::new(output).display());
    println!("export hash: {}", format_hash(&bundle.content_hash));
    ExitCode::SUCCESS
}

pub fn run_radiance_point_reference(input: &str, arguments: &[String]) -> ExitCode {
    let mut common = DaylightInputs::default();
    let mut moment = None;
    let mut bin = None;
    let mut run = RadianceRunOptions::default();
    let parsed = parse_arguments(arguments, &mut common, |argument, value| match argument {
        "--moment" => parse_moment(value).map(|v| moment = Some(v)),
        "--radiance-bin" => {
            bin = Some(value.to_owned());
            Ok(())
        }
        "--ambient-bounces" => parse_u32(value, "ambient-bounces").map(|v| run.ambient_bounces = v),
        "--ambient-divisions" => {
            parse_u32(value, "ambient-divisions").map(|v| run.ambient_divisions = v)
        }
        "--ambient-supersamples" => {
            parse_u32(value, "ambient-supersamples").map(|v| run.ambient_supersamples = v)
        }
        "--ambient-accuracy" => {
            parse_fraction(value, "ambient-accuracy").map(|v| run.ambient_accuracy = v)
        }
        "--keep-temp" if value == "true" || value == "false" => {
            run.keep_temporary_files = value == "true";
            Ok(())
        }
        unknown => {
            eprintln!("error: unknown radiance-point option '{unknown}'");
            Err(())
        }
    });
    if parsed.is_err() {
        return ExitCode::from(2);
    }
    let Some(moment) = moment else {
        eprintln!("error: --moment is required");
        return ExitCode::from(2);
    };
    let Some(scene) = checked_scene(input, &common, "radiance-point") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let Ok(bundle) = export_radiance_bundle(&scene, &common.sensors, &materials) else {
        eprintln!("error: Radiance export failed");
        return ExitCode::FAILURE;
    };
    let toolchain = match RadianceToolchain::discover(bin.as_deref().map(Path::new)) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: {error}; install Radiance or pass --radiance-bin");
            return ExitCode::FAILURE;
        }
    };
    let result = match run_radiance_point(&toolchain, &bundle, moment, &run, None) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: Radiance reference failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if common.json {
        println!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"Radiance\",\"toolVersion\":\"{}\",\"elapsedMicroseconds\":{},\"runHash\":\"{}\",\"illuminanceLux\":{:?}}}",
            escape_json(&result.provenance.tool_version),
            result.provenance.elapsed_microseconds,
            format_hash(&result.provenance.content_hash),
            result.illuminance_lux
        );
    } else {
        println!("XVARNA Radiance Point Reference 0.16.0");
        println!("Radiance: {}", result.provenance.tool_version);
        for (index, lux) in result.illuminance_lux.iter().enumerate() {
            println!("sensor {}: {:.3} lux", index + 1, lux);
        }
        for command in &result.provenance.commands {
            println!("command: {command}");
        }
        println!("hash: {}", format_hash(&result.provenance.content_hash));
    }
    ExitCode::SUCCESS
}

pub fn run_radiance_annual_reference(
    input: &str,
    weather_path: &str,
    arguments: &[String],
) -> ExitCode {
    let mut common = DaylightInputs::default();
    let mut bin = None;
    let mut run = RadianceRunOptions::default();
    let parsed = parse_arguments(arguments, &mut common, |argument, value| match argument {
        "--radiance-bin" => {
            bin = Some(value.to_owned());
            Ok(())
        }
        "--matrix-cache" => {
            run.matrix_cache_directory = Some(value.into());
            Ok(())
        }
        "--ambient-bounces" => parse_u32(value, "ambient-bounces").map(|v| run.ambient_bounces = v),
        "--ambient-divisions" => {
            parse_u32(value, "ambient-divisions").map(|v| run.ambient_divisions = v)
        }
        "--ambient-supersamples" => {
            parse_u32(value, "ambient-supersamples").map(|v| run.ambient_supersamples = v)
        }
        "--ambient-accuracy" => {
            parse_fraction(value, "ambient-accuracy").map(|v| run.ambient_accuracy = v)
        }
        unknown => {
            eprintln!("error: unknown radiance-annual option '{unknown}'");
            Err(())
        }
    });
    if parsed.is_err() {
        return ExitCode::from(2);
    }
    let Some(scene) = checked_scene(input, &common, "radiance-annual") else {
        return ExitCode::FAILURE;
    };
    let Some(weather) = load_weather(weather_path, common.simulation_year) else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = common.library() else {
        return ExitCode::from(2);
    };
    let Ok(bundle) = export_radiance_bundle(&scene, &common.sensors, &materials) else {
        eprintln!("error: Radiance export failed");
        return ExitCode::FAILURE;
    };
    let toolchain = match RadianceToolchain::discover(bin.as_deref().map(Path::new)) {
        Ok(value) if value.supports_annual_matrix() => value,
        Ok(_) => {
            eprintln!("error: rfluxmtx, gendaymtx, and dctimestep are required");
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match run_radiance_annual_matrix(&toolchain, &bundle, &weather, &run, None) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: Radiance annual reference failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("XVARNA Radiance Annual Matrix 0.16.0");
    println!(
        "shape: {} sensors x {} timesteps = {} lux values",
        result.sensor_count,
        result.timestep_count,
        result.illuminance_lux.len()
    );
    println!("matrix reused: {}", result.provenance.matrix_reused);
    println!("matrix directory: {}", result.matrix_directory.display());
    println!("hash: {}", format_hash(&result.provenance.content_hash));
    ExitCode::SUCCESS
}

pub fn run_daylight_compare(
    fast_path: &str,
    reference_path: &str,
    arguments: &[String],
) -> ExitCode {
    let mut absolute = 50.0;
    let mut relative = 0.1;
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
        let parsed = match arguments[index].as_str() {
            "--absolute-lux" => parse_nonnegative(value, "absolute-lux").map(|v| absolute = v),
            "--relative" => parse_fraction(value, "relative").map(|v| relative = v),
            unknown => {
                eprintln!("error: unknown daylight-compare option '{unknown}'");
                Err(())
            }
        };
        if parsed.is_err() {
            return ExitCode::from(2);
        }
        index += 2;
    }
    let Some(fast) = load_numeric_values(fast_path) else {
        return ExitCode::FAILURE;
    };
    let Some(reference) = load_numeric_values(reference_path) else {
        return ExitCode::FAILURE;
    };
    let report = match compare_daylight_results(&fast, &reference, absolute, relative) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: daylight comparison failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!(
            "{{\"schemaVersion\":\"0.16.0\",\"count\":{},\"accepted\":{},\"acceptedFraction\":{},\"meanBiasLux\":{},\"maeLux\":{},\"rmseLux\":{},\"mape\":{},\"maximumAbsoluteErrorLux\":{},\"rSquared\":{},\"hash\":\"{}\"}}",
            report.count,
            report.accepted,
            report.accepted_fraction,
            report.mean_bias_lux,
            report.mean_absolute_error_lux,
            report.root_mean_square_error_lux,
            report.mean_absolute_percentage_error,
            report.maximum_absolute_error_lux,
            report.r_squared,
            format_hash(&report.content_hash)
        );
    } else {
        println!("XVARNA Fast Path vs Radiance Validation 0.16.0");
        println!(
            "n={}; bias={:.3} lux; MAE={:.3}; RMSE={:.3}; MAPE={:.3}%; max={:.3}; R2={:.6}",
            report.count,
            report.mean_bias_lux,
            report.mean_absolute_error_lux,
            report.root_mean_square_error_lux,
            report.mean_absolute_percentage_error * 100.0,
            report.maximum_absolute_error_lux,
            report.r_squared
        );
        println!(
            "accepted: {} ({:.3}%)",
            report.accepted,
            report.accepted_fraction * 100.0
        );
        println!("hash: {}", format_hash(&report.content_hash));
    }
    if report.accepted {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn parse_arguments(
    arguments: &[String],
    common: &mut DaylightInputs,
    mut specific: impl FnMut(&str, &str) -> Result<(), ()>,
) -> Result<(), ()> {
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--json" {
            common.json = true;
            index += 1;
            continue;
        }
        let argument = arguments[index].as_str();
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("error: {argument} requires a value");
            return Err(());
        };
        match argument {
            "--sensor" => common
                .sensors
                .push(parse_sensor(value, common.sensors.len())?),
            "--material" => common.materials.push(parse_material(value)?),
            "--assign" => common.assignments.push(parse_assignment(value)?),
            "--threads" => common.threads = parse_usize(value, "threads")?,
            "--simulation-year" => {
                common.simulation_year = Some(
                    value
                        .parse::<i32>()
                        .map_err(|_| eprintln!("error: invalid simulation year"))?,
                );
            }
            _ => specific(argument, value)?,
        }
        index += 2;
    }
    if common.sensors.is_empty() {
        eprintln!("error: repeat --sensor id,x,y,z,nx,ny,nz,area at least once");
        return Err(());
    }
    Ok(())
}

fn parse_matrix_option(
    argument: &str,
    value: &str,
    options: &mut DaylightMatrixOptions,
) -> Result<(), ()> {
    match argument {
        "--patches" => parse_usize(value, "patches").map(|v| options.sky_patch_count = v),
        "--offset" => parse_nonnegative(value, "offset").map(|v| options.sensor_offset_meters = v),
        "--max-distance" => {
            parse_positive(value, "max-distance").map(|v| options.maximum_distance_meters = v)
        }
        "--category-mask" => value
            .parse()
            .map(|v| options.category_mask = v)
            .map_err(|_| eprintln!("error: category-mask must be an unsigned integer")),
        "--material-layers" => {
            parse_usize(value, "material-layers").map(|v| options.maximum_material_layers = v)
        }
        "--min-transmission" => {
            parse_fraction(value, "min-transmission").map(|v| options.minimum_transmission = v)
        }
        unknown => {
            eprintln!("error: unknown daylight option '{unknown}'");
            Err(())
        }
    }
}

fn checked_scene(
    input: &str,
    common: &DaylightInputs,
    command: &str,
) -> Option<xvarna_scene::Scene> {
    if common.threads > 256 {
        eprintln!("error: threads must be at most 256");
        return None;
    }
    build_cli_scene(input, common.threads, command)
}

fn parse_sensor(value: &str, index: usize) -> Result<DaylightSensor, ()> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != 8 {
        eprintln!("error: sensor requires id,x,y,z,nx,ny,nz,area");
        return Err(());
    }
    let id = values[0]
        .parse::<u64>()
        .map_err(|_| eprintln!("error: sensor id must be an unsigned integer"))?;
    let numbers = values[1..]
        .iter()
        .map(|v| parse_number(v, "sensor"))
        .collect::<Result<Vec<_>, _>>()?;
    DaylightSensor::try_new(
        SensorId::new(id.max(u64::try_from(index + 1).unwrap_or(1))),
        Vec3::new(numbers[0], numbers[1], numbers[2]),
        Vec3::new(numbers[3], numbers[4], numbers[5]),
        numbers[6],
    )
    .map_err(|error| eprintln!("error: invalid sensor: {error}"))
}

fn parse_material(value: &str) -> Result<OpticalMaterial, ()> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != 11 {
        eprintln!("error: material requires id,kind,Rr,Rg,Rb,Tr,Tg,Tb,specularity,roughness,ior");
        return Err(());
    }
    let id = values[0]
        .parse()
        .map_err(|_| eprintln!("error: material id must be an unsigned integer"))?;
    let kind = match values[1].to_ascii_lowercase().as_str() {
        "plastic" => OpticalMaterialKind::Plastic,
        "glass" => OpticalMaterialKind::Glass,
        "metal" => OpticalMaterialKind::Metal,
        "trans" => OpticalMaterialKind::Trans,
        "mirror" => OpticalMaterialKind::Mirror,
        _ => {
            eprintln!("error: material kind must be plastic|glass|metal|trans|mirror");
            return Err(());
        }
    };
    let n = values[2..]
        .iter()
        .map(|v| parse_number(v, "material"))
        .collect::<Result<Vec<_>, _>>()?;
    OpticalMaterial::try_new(
        id,
        kind,
        [n[0], n[1], n[2]],
        [n[3], n[4], n[5]],
        n[6],
        n[7],
        n[8],
    )
    .map_err(|error| eprintln!("error: invalid material: {error}"))
}

fn parse_assignment(value: &str) -> Result<(ObjectId, u64), ()> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != 2 {
        eprintln!("error: assignment requires object-id,material-id");
        return Err(());
    }
    Ok((
        ObjectId::new(
            values[0]
                .parse()
                .map_err(|_| eprintln!("error: invalid object id"))?,
        ),
        values[1]
            .parse()
            .map_err(|_| eprintln!("error: invalid material id"))?,
    ))
}

fn parse_moment(value: &str) -> Result<DaylightMoment, ()> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != 6 {
        eprintln!("error: moment requires unix,dx,dy,dz,direct-normal-lux,diffuse-horizontal-lux");
        return Err(());
    }
    let timestamp = values[0]
        .parse()
        .map_err(|_| eprintln!("error: moment unix timestamp is invalid"))?;
    let n = values[1..]
        .iter()
        .map(|v| parse_number(v, "moment"))
        .collect::<Result<Vec<_>, _>>()?;
    DaylightMoment::try_new(timestamp, Vec3::new(n[0], n[1], n[2]), n[3], n[4])
        .map_err(|error| eprintln!("error: invalid moment: {error}"))
}

fn parse_sky(value: &str) -> Result<DaylightSkyModel, ()> {
    match value {
        "isotropic" => Ok(DaylightSkyModel::Isotropic),
        "cie-overcast" => Ok(DaylightSkyModel::CieOvercast),
        _ => {
            eprintln!("error: sky must be isotropic or cie-overcast");
            Err(())
        }
    }
}

fn load_weather(path: &str, simulation_year: Option<i32>) -> Option<xvarna_zurvan::EpwWeather> {
    let source = fs::read_to_string(path)
        .map_err(|error| eprintln!("error: cannot read {path}: {error}"))
        .ok()?;
    let weather = parse_epw(&source)
        .map_err(|error| eprintln!("error: invalid EPW: {error}"))
        .ok()?;
    match simulation_year {
        Some(year) => {
            eprintln!("weather: explicit simulation year {year}; original source years retained");
            xvarna_zurvan::with_simulation_year(&weather, year)
                .map_err(|error| eprintln!("error: simulation calendar: {error}"))
                .ok()
        }
        None => Some(weather),
    }
}

fn write_annual_csv(
    path: &str,
    sensors: &[DaylightSensor],
    result: &xvarna_hvare::AnnualDaylightResult,
) -> std::io::Result<()> {
    let mut output = String::from(
        "sensor_id,weather_index,unix_seconds_utc,occupied,direct_lux,diffuse_lux,total_lux\n",
    );
    for (sensor_index, sensor) in sensors.iter().enumerate() {
        let start = sensor_index * result.weather_count;
        for (weather_index, value) in result.timeline[start..start + result.weather_count]
            .iter()
            .enumerate()
        {
            writeln!(
                output,
                "{},{},{},{},{},{},{}",
                sensor.id,
                weather_index,
                value.unix_seconds_utc,
                value.occupied,
                value.direct_lux,
                value.diffuse_lux,
                value.total_lux
            )
            .expect("String writes cannot fail");
        }
    }
    fs::write(path, output)
}

fn load_numeric_values(path: &str) -> Option<Vec<f64>> {
    let source = fs::read_to_string(path)
        .map_err(|error| eprintln!("error: cannot read {path}: {error}"))
        .ok()?;
    source
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| eprintln!("error: {path} contains non-numeric data"))
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn parse_number(value: &str, name: &str) -> Result<f64, ()> {
    let value = value
        .parse::<f64>()
        .map_err(|_| eprintln!("error: {name} must be numeric"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        eprintln!("error: {name} must be finite");
        Err(())
    }
}

fn parse_positive(value: &str, name: &str) -> Result<f64, ()> {
    let value = parse_number(value, name)?;
    if value > 0.0 {
        Ok(value)
    } else {
        eprintln!("error: {name} must be positive");
        Err(())
    }
}

fn parse_nonnegative(value: &str, name: &str) -> Result<f64, ()> {
    let value = parse_number(value, name)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        eprintln!("error: {name} must be non-negative");
        Err(())
    }
}

fn parse_fraction(value: &str, name: &str) -> Result<f64, ()> {
    let value = parse_number(value, name)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        eprintln!("error: {name} must be in [0,1]");
        Err(())
    }
}

fn parse_usize(value: &str, name: &str) -> Result<usize, ()> {
    value
        .parse()
        .map_err(|_| eprintln!("error: {name} must be a non-negative integer"))
}

fn parse_u32(value: &str, name: &str) -> Result<u32, ()> {
    value
        .parse()
        .map_err(|_| eprintln!("error: {name} must be a non-negative 32-bit integer"))
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
