//! DAENA/HVARE 0.14 product-level intelligence command workflows.

use std::process::ExitCode;
use xvarna_daena::{
    Isovist3dOptions, Landmark, LandmarkObserver, LandmarkVisibilityOptions, SpatialViewpoint,
    analyze_isovist_3d, analyze_landmark_visibility,
};
use xvarna_geometry::Vec3;
use xvarna_hvare::{
    EnvelopeCandidate, SolarEnvelopeOptions, SolarScenario, SolarScenarioOptions, SolarSensor,
    analyze_solar_envelope, compare_solar_scenarios,
};
use xvarna_scene::{AnalysisMaterial, MaterialLibrary};
use xvarna_types::{ObjectId, SensorId, TargetId};
use xvarna_zurvan::{SolarLocation, SolarOptions, SunSample, SunSet};

use super::{build_cli_scene, format_hash, parse_category_mask, parse_nonnegative, parse_positive};

#[derive(Clone, Debug, Default)]
struct MaterialInputs {
    materials: Vec<AnalysisMaterial>,
    assignments: Vec<(ObjectId, u64)>,
}

impl MaterialInputs {
    fn build(&self) -> Result<MaterialLibrary, ()> {
        MaterialLibrary::try_new(self.materials.clone(), self.assignments.clone()).map_err(
            |error| {
                eprintln!("error: material library is invalid: {error}");
            },
        )
    }
}

#[allow(clippy::too_many_lines)]
pub fn run_isovist_3d(input: &str, arguments: &[String]) -> ExitCode {
    let mut viewpoints = Vec::new();
    let mut options = Isovist3dOptions::default();
    let mut materials = MaterialInputs::default();
    let mut threads = 0;
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
            return ExitCode::from(2);
        };
        let parsed = match argument {
            "--viewpoint" => {
                parse_spatial_viewpoint(value, viewpoints.len()).map(|value| viewpoints.push(value))
            }
            "--material" => parse_material(value).map(|value| materials.materials.push(value)),
            "--assign" => parse_assignment(value).map(|value| materials.assignments.push(value)),
            "--samples" => parse_usize(value, "samples").map(|value| options.sample_count = value),
            "--max-distance" => parse_positive(value, "max-distance")
                .map(|value| options.maximum_distance_meters = value),
            "--offset" => {
                parse_nonnegative(value, "offset").map(|value| options.eye_offset_meters = value)
            }
            "--category-mask" => {
                parse_category_mask(value).map(|value| options.category_mask = value)
            }
            "--top-k" => parse_usize(value, "top-k").map(|value| options.top_k = value),
            "--counterfactual" => parse_usize(value, "counterfactual")
                .map(|value| options.counterfactual_count = value),
            "--material-layers" => parse_usize(value, "material-layers")
                .map(|value| options.maximum_material_layers = value),
            "--min-transmission" => parse_fraction(value, "min-transmission")
                .map(|value| options.minimum_transmission = value),
            "--threads" => parse_usize(value, "threads").map(|value| threads = value),
            unknown => {
                eprintln!("error: unknown isovist-3d option '{unknown}'");
                Err(())
            }
        };
        if parsed.is_err() {
            return ExitCode::from(2);
        }
        index += 2;
    }
    if viewpoints.is_empty() {
        eprintln!("error: repeat --viewpoint at least once");
        return ExitCode::from(2);
    }
    let Some(scene) = build_cli_scene(input, threads, "isovist-3d") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = materials.build() else {
        return ExitCode::from(2);
    };
    let result = match analyze_isovist_3d(&scene, &viewpoints, &materials, options) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: isovist-3d failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA isovist-3d\",\"hash\":\"{}\",\"viewpoints\":[",
            format_hash(&result.content_hash)
        );
        for (index, value) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"id\":{},\"volume\":{},\"radialSurface\":{},\"openness\":{},\"visibleSolidAngle\":{},\"convergence\":{},\"dominantObject\":{}}}",
                value.viewpoint_id.get(),
                value.volume_cubic_meters,
                value.radial_surface_square_meters,
                value.openness_ratio,
                value.visible_solid_angle_steradians,
                value.volume_convergence_delta_cubic_meters,
                value.dominant_occluder_object_id.get()
            );
        }
        println!(
            "],\"attributionCount\":{},\"categoryCount\":{}}}",
            result.attribution.len(),
            result.categories.len()
        );
    } else {
        println!("XVARNA DAENA 3D Isovist 0.14");
        println!(
            "Study: {} viewpoint(s) x {} rays; top {}; exact remove-one {}",
            viewpoints.len(),
            options.sample_count,
            options.top_k,
            options.counterfactual_count
        );
        for value in &result.summaries {
            println!(
                "view {}  volume={:.8}  surface={:.8}  openness={:.8}  mean-radius={:.8}  dominant={}",
                value.viewpoint_id.get(),
                value.volume_cubic_meters,
                value.radial_surface_square_meters,
                value.openness_ratio,
                value.mean_radial_meters,
                value.dominant_occluder_object_id.get()
            );
        }
        println!(
            "Attribution: {} rows; category partitions: {} rows",
            result.attribution.len(),
            result.categories.len()
        );
        println!("Result hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
pub fn run_landmark_visibility(input: &str, arguments: &[String]) -> ExitCode {
    let mut observers = Vec::new();
    let mut landmarks = Vec::new();
    let mut options = LandmarkVisibilityOptions::default();
    let mut materials = MaterialInputs::default();
    let mut threads = 0;
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
            return ExitCode::from(2);
        };
        let parsed = match argument {
            "--observer" => parse_landmark_observer(value).map(|value| observers.push(value)),
            "--landmark" => parse_landmark(value).map(|value| landmarks.push(value)),
            "--material" => parse_material(value).map(|value| materials.materials.push(value)),
            "--assign" => parse_assignment(value).map(|value| materials.assignments.push(value)),
            "--samples" => parse_usize(value, "samples").map(|value| options.sample_count = value),
            "--horizontal-fov" => parse_positive(value, "horizontal-fov")
                .map(|value| options.horizontal_field_of_view_radians = value.to_radians()),
            "--vertical-fov" => parse_positive(value, "vertical-fov")
                .map(|value| options.vertical_field_of_view_radians = value.to_radians()),
            "--max-distance" => parse_positive(value, "max-distance")
                .map(|value| options.maximum_distance_meters = value),
            "--clearance" => parse_nonnegative(value, "clearance")
                .map(|value| options.endpoint_clearance_meters = value),
            "--category-mask" => {
                parse_category_mask(value).map(|value| options.category_mask = value)
            }
            "--top-k" => parse_usize(value, "top-k").map(|value| options.top_k = value),
            "--material-layers" => parse_usize(value, "material-layers")
                .map(|value| options.maximum_material_layers = value),
            "--min-transmission" => parse_fraction(value, "min-transmission")
                .map(|value| options.minimum_transmission = value),
            "--threads" => parse_usize(value, "threads").map(|value| threads = value),
            unknown => {
                eprintln!("error: unknown landmark-visibility option '{unknown}'");
                Err(())
            }
        };
        if parsed.is_err() {
            return ExitCode::from(2);
        }
        index += 2;
    }
    if observers.is_empty() || landmarks.is_empty() {
        eprintln!("error: landmark-visibility requires --observer and --landmark");
        return ExitCode::from(2);
    }
    let Some(scene) = build_cli_scene(input, threads, "landmark-visibility") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = materials.build() else {
        return ExitCode::from(2);
    };
    let result =
        match analyze_landmark_visibility(&scene, &observers, &landmarks, &materials, options) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: landmark visibility failed: {error}");
                return ExitCode::FAILURE;
            }
        };
    if json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"DAENA landmark-visibility\",\"hash\":\"{}\",\"entries\":[",
            format_hash(&result.content_hash)
        );
        for (index, value) in result.entries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"observer\":{},\"landmark\":{},\"insideFov\":{},\"visibleFraction\":{},\"visibleSolidAngle\":{},\"weightedScore\":{},\"dominantObject\":{}}}",
                value.observer_id.get(),
                value.landmark_id.get(),
                value.inside_field_of_view,
                value.visible_fraction,
                value.visible_solid_angle_steradians,
                value.weighted_visibility_score,
                value.dominant_blocker_object_id.get()
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA DAENA Landmark Visibility 0.14");
        println!(
            "Matrix: {} observer(s) x {} landmark(s) x {} samples",
            observers.len(),
            landmarks.len(),
            options.sample_count
        );
        for value in &result.entries {
            println!(
                "observer {} landmark {}  visible={:.8}  omega={:.8} sr  score={:.8}  dominant={}",
                value.observer_id.get(),
                value.landmark_id.get(),
                value.visible_fraction,
                value.visible_solid_angle_steradians,
                value.weighted_visibility_score,
                value.dominant_blocker_object_id.get()
            );
        }
        println!("Result hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

// Solar commands are kept here so the same manual schedule grammar and material validation
// drive both envelope screening and scenario comparison.

#[allow(clippy::too_many_lines)]
pub fn run_solar_envelope(input: &str, arguments: &[String]) -> ExitCode {
    let mut sensors = Vec::new();
    let mut candidates = Vec::new();
    let mut suns = Vec::new();
    let mut materials = MaterialInputs::default();
    let mut options = SolarEnvelopeOptions::default();
    let mut threads = 0;
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
            return ExitCode::from(2);
        };
        let parsed = match argument {
            "--sensor" => parse_solar_sensor(value).map(|value| sensors.push(value)),
            "--candidate" => parse_candidate(value).map(|value| candidates.push(value)),
            "--sun" => parse_sun(value).map(|value| suns.push(value)),
            "--material" => parse_material(value).map(|value| materials.materials.push(value)),
            "--assign" => parse_assignment(value).map(|value| materials.assignments.push(value)),
            "--radius" => {
                parse_positive(value, "radius").map(|value| options.candidate_radius_meters = value)
            }
            "--preserve" => parse_fraction(value, "preserve")
                .map(|value| options.required_preserved_fraction = value),
            "--shade" => {
                parse_fraction(value, "shade").map(|value| options.target_shaded_fraction = value)
            }
            "--vertical-clearance" => parse_nonnegative(value, "vertical-clearance")
                .map(|value| options.vertical_clearance_meters = value),
            "--category-mask" => {
                parse_category_mask(value).map(|value| options.category_mask = value)
            }
            "--threads" => parse_usize(value, "threads").map(|value| threads = value),
            unknown => {
                eprintln!("error: unknown solar-envelope option '{unknown}'");
                Err(())
            }
        };
        if parsed.is_err() {
            return ExitCode::from(2);
        }
        index += 2;
    }
    if sensors.is_empty() || candidates.is_empty() || suns.is_empty() {
        eprintln!("error: solar-envelope requires --sensor, --candidate, and --sun");
        return ExitCode::from(2);
    }
    let Some(scene) = build_cli_scene(input, threads, "solar-envelope") else {
        return ExitCode::FAILURE;
    };
    let Ok(materials) = materials.build() else {
        return ExitCode::from(2);
    };
    let sun_set = manual_sun_set(suns);
    let result = match analyze_solar_envelope(
        &scene,
        &sensors,
        &candidates,
        &sun_set,
        &materials,
        options,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: solar envelope failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"HVARE solar-envelope\",\"hash\":\"{}\",\"cells\":[",
            format_hash(&result.content_hash)
        );
        for (index, value) in result.cells.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            let axis = candidates[index].axis;
            print!(
                "{{\"id\":{},\"axis\":[{},{},{}],\"accessElevation\":{},\"accessAxisDistance\":{},\"shadingElevation\":{},\"shadingAxisDistance\":{},\"hours\":{},\"constraints\":{}}}",
                value.candidate_id,
                axis.x,
                axis.y,
                axis.z,
                value.maximum_solar_access_elevation_meters,
                value.maximum_solar_access_height_meters,
                value.minimum_shading_elevation_meters,
                value.minimum_shading_height_meters,
                value.considered_baseline_sun_hours,
                value.constraint_count
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA HVARE Freeform Solar/Shading Envelope 0.17");
        for (value, candidate) in result.cells.iter().zip(&candidates) {
            println!(
                "candidate {}  axis=({:.5},{:.5},{:.5})  access-distance={:.8} (z={:.8})  shade-distance={:.8} (z={:.8})  hours={:.5}  constraints={}",
                value.candidate_id,
                candidate.axis.x,
                candidate.axis.y,
                candidate.axis.z,
                value.maximum_solar_access_height_meters,
                value.maximum_solar_access_elevation_meters,
                value.minimum_shading_height_meters,
                value.minimum_shading_elevation_meters,
                value.considered_baseline_sun_hours,
                value.constraint_count
            );
        }
        println!(
            "Scope: freeform oriented-column early-massing screening, not final code compliance or radiosity."
        );
        println!("Result hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_lines)]
pub fn run_solar_scenarios(input: &str, arguments: &[String]) -> ExitCode {
    let mut sensors = Vec::new();
    let mut suns = Vec::new();
    let mut scenarios: Vec<(u64, MaterialInputs)> = Vec::new();
    let mut options = SolarScenarioOptions::default();
    let mut threads = 0;
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
            return ExitCode::from(2);
        };
        let parsed = match argument {
            "--sensor" => parse_solar_sensor(value).map(|value| sensors.push(value)),
            "--sun" => parse_sun(value).map(|value| suns.push(value)),
            "--scenario" => parse_u64(value, "scenario id")
                .map(|id| scenarios.push((id, MaterialInputs::default()))),
            "--material" => scenarios
                .last_mut()
                .ok_or_else(|| {
                    eprintln!("error: --material must follow --scenario");
                })
                .and_then(|scenario| {
                    parse_material(value).map(|value| scenario.1.materials.push(value))
                }),
            "--assign" => scenarios
                .last_mut()
                .ok_or_else(|| {
                    eprintln!("error: --assign must follow --scenario");
                })
                .and_then(|scenario| {
                    parse_assignment(value).map(|value| scenario.1.assignments.push(value))
                }),
            "--top-k" => parse_usize(value, "top-k").map(|value| options.top_k = value),
            "--material-layers" => parse_usize(value, "material-layers")
                .map(|value| options.maximum_material_layers = value),
            "--min-transmission" => parse_fraction(value, "min-transmission")
                .map(|value| options.minimum_transmission = value),
            "--category-mask" => {
                parse_category_mask(value).map(|value| options.category_mask = value)
            }
            "--threads" => parse_usize(value, "threads").map(|value| threads = value),
            unknown => {
                eprintln!("error: unknown solar-scenarios option '{unknown}'");
                Err(())
            }
        };
        if parsed.is_err() {
            return ExitCode::from(2);
        }
        index += 2;
    }
    if sensors.is_empty() || suns.is_empty() || scenarios.is_empty() {
        eprintln!("error: solar-scenarios requires --sensor, --sun, and --scenario");
        return ExitCode::from(2);
    }
    let Some(scene) = build_cli_scene(input, threads, "solar-scenarios") else {
        return ExitCode::FAILURE;
    };
    let scenario_values = scenarios
        .into_iter()
        .map(|(id, value)| {
            value.build().and_then(|library| {
                SolarScenario::try_new(id, format!("scenario-{id}"), library).map_err(|error| {
                    eprintln!("error: invalid scenario: {error}");
                })
            })
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(scenario_values) = scenario_values else {
        return ExitCode::from(2);
    };
    let result = match compare_solar_scenarios(
        &scene,
        &sensors,
        &manual_sun_set(suns),
        &scenario_values,
        options,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: solar scenario comparison failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        print!(
            "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"HVARE solar-scenarios\",\"hash\":\"{}\",\"summaries\":[",
            format_hash(&result.content_hash)
        );
        for (index, value) in result.summaries.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"scenario\":{},\"sensor\":{},\"received\":{},\"lost\":{},\"access\":{},\"dominantObject\":{}}}",
                value.scenario_id,
                value.sensor_id.get(),
                value.received_sun_hours,
                value.lost_sun_hours,
                value.solar_access_ratio,
                value.dominant_occluder_object_id.get()
            );
        }
        println!(
            "],\"attributionCount\":{},\"categoryCount\":{},\"deltaCount\":{}}}",
            result.attribution.len(),
            result.categories.len(),
            result.deltas.len()
        );
    } else {
        println!("XVARNA HVARE Solar Scenario Comparison 0.14");
        for value in &result.summaries {
            println!(
                "scenario {} sensor {}  received={:.6}h  lost={:.6}h  access={:.8}  dominant={}",
                value.scenario_id,
                value.sensor_id.get(),
                value.received_sun_hours,
                value.lost_sun_hours,
                value.solar_access_ratio,
                value.dominant_occluder_object_id.get()
            );
        }
        println!(
            "Attribution: {}; categories: {}; deltas: {}",
            result.attribution.len(),
            result.categories.len(),
            result.deltas.len()
        );
        println!("Result hash: {}", format_hash(&result.content_hash));
    }
    ExitCode::SUCCESS
}

fn parse_spatial_viewpoint(value: &str, index: usize) -> Result<SpatialViewpoint, ()> {
    let values = numbers(value, 3, "viewpoint")?;
    SpatialViewpoint::try_new(
        SensorId::new(u64::try_from(index + 1).map_err(|_| ())?),
        Vec3::new(values[0], values[1], values[2]),
    )
    .map_err(|error| eprintln!("error: invalid viewpoint: {error}"))
}

fn parse_landmark_observer(value: &str) -> Result<LandmarkObserver, ()> {
    let fields = fields(value, 10, "observer", "id,x,y,z,fx,fy,fz,ux,uy,uz")?;
    let id = parse_u64(fields[0], "observer id")?;
    let values = numeric_fields(&fields[1..], "observer")?;
    LandmarkObserver::try_new(
        SensorId::new(id),
        Vec3::new(values[0], values[1], values[2]),
        Vec3::new(values[3], values[4], values[5]),
        Vec3::new(values[6], values[7], values[8]),
    )
    .map_err(|error| eprintln!("error: invalid observer: {error}"))
}

fn parse_landmark(value: &str) -> Result<Landmark, ()> {
    let fields = fields(value, 6, "landmark", "id,x,y,z,radius,weight")?;
    let id = parse_u64(fields[0], "landmark id")?;
    let values = numeric_fields(&fields[1..], "landmark")?;
    Landmark::try_new(
        TargetId::new(id),
        Vec3::new(values[0], values[1], values[2]),
        values[3],
        values[4],
    )
    .map_err(|error| eprintln!("error: invalid landmark: {error}"))
}

fn parse_material(value: &str) -> Result<AnalysisMaterial, ()> {
    let fields = fields(value, 4, "material", "id,visible,solar,reflectance")?;
    AnalysisMaterial::try_new(
        parse_u64(fields[0], "material id")?,
        parse_fraction(fields[1], "visible")?,
        parse_fraction(fields[2], "solar")?,
        parse_fraction(fields[3], "reflectance")?,
    )
    .map_err(|error| eprintln!("error: invalid material: {error}"))
}

fn parse_assignment(value: &str) -> Result<(ObjectId, u64), ()> {
    let fields = fields(value, 2, "assignment", "object,material")?;
    Ok((
        ObjectId::new(parse_u64(fields[0], "object id")?),
        parse_u64(fields[1], "material id")?,
    ))
}

fn parse_solar_sensor(value: &str) -> Result<SolarSensor, ()> {
    let fields = fields(value, 7, "sensor", "id,x,y,z,nx,ny,nz")?;
    let id = parse_u64(fields[0], "sensor id")?;
    let values = numeric_fields(&fields[1..], "sensor")?;
    SolarSensor::try_new(
        SensorId::new(id),
        Vec3::new(values[0], values[1], values[2]),
        Vec3::new(values[3], values[4], values[5]),
    )
    .map_err(|error| eprintln!("error: invalid sensor: {error}"))
}

fn parse_candidate(value: &str) -> Result<EnvelopeCandidate, ()> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 4 && fields.len() != 7 {
        eprintln!("error: candidate needs id,x,y,z[,axis-x,axis-y,axis-z]");
        return Err(());
    }
    let values = numeric_fields(&fields[1..], "candidate")?;
    let id = parse_u64(fields[0], "candidate id")?;
    let position = Vec3::new(values[0], values[1], values[2]);
    let candidate = if values.len() == 3 {
        EnvelopeCandidate::try_new(id, position)
    } else {
        EnvelopeCandidate::try_new_oriented(
            id,
            position,
            Vec3::new(values[3], values[4], values[5]),
        )
    };
    candidate.map_err(|error| eprintln!("error: invalid candidate: {error}"))
}

fn parse_sun(value: &str) -> Result<SunSample, ()> {
    let fields = fields(value, 6, "sun", "unix,dx,dy,dz,duration,weight")?;
    let timestamp = fields[0]
        .parse::<i64>()
        .map_err(|_| eprintln!("error: sun unix timestamp is invalid"))?;
    let values = numeric_fields(&fields[1..], "sun")?;
    let direction = Vec3::new(values[0], values[1], values[2])
        .normalized()
        .ok_or_else(|| eprintln!("error: sun direction is invalid"))?;
    if values[3] <= 0.0 || values[4] < 0.0 {
        eprintln!("error: sun duration must be positive and weight non-negative");
        return Err(());
    }
    Ok(SunSample {
        unix_seconds_utc: timestamp,
        direction,
        altitude_degrees: direction.z.asin().to_degrees(),
        azimuth_degrees: direction
            .x
            .atan2(direction.y)
            .to_degrees()
            .rem_euclid(360.0),
        duration_hours: values[3],
        weight: values[4],
        is_active: direction.z > 0.0 && values[4] > 0.0,
    })
}

fn manual_sun_set(samples: Vec<SunSample>) -> SunSet {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_CLI_MANUAL_SUN_V1\0");
    for value in &samples {
        hasher.update(&value.unix_seconds_utc.to_le_bytes());
        for number in [
            value.direction.x,
            value.direction.y,
            value.direction.z,
            value.duration_hours,
            value.weight,
        ] {
            hasher.update(&number.to_bits().to_le_bytes());
        }
    }
    SunSet {
        location: SolarLocation::try_new(0.0, 0.0, 0.0).expect("zero location is valid"),
        options: SolarOptions::default(),
        samples,
        content_hash: *hasher.finalize().as_bytes(),
    }
}

fn fields<'a>(value: &'a str, count: usize, name: &str, grammar: &str) -> Result<Vec<&'a str>, ()> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != count {
        eprintln!("error: {name} needs {grammar}");
        return Err(());
    }
    Ok(fields)
}

fn numeric_fields(values: &[&str], name: &str) -> Result<Vec<f64>, ()> {
    values
        .iter()
        .map(|item| {
            item.parse::<f64>().map_err(|_| {
                eprintln!("error: {name} contains a non-number");
            })
        })
        .collect()
}

fn numbers(value: &str, count: usize, name: &str) -> Result<Vec<f64>, ()> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != count {
        eprintln!("error: {name} needs {count} comma-separated values");
        return Err(());
    }
    numeric_fields(&values, name)
}

fn parse_u64(value: &str, name: &str) -> Result<u64, ()> {
    value
        .parse::<u64>()
        .map_err(|_| eprintln!("error: {name} must be an unsigned integer"))
}

fn parse_usize(value: &str, name: &str) -> Result<usize, ()> {
    value
        .parse::<usize>()
        .map_err(|_| eprintln!("error: {name} must be a non-negative integer"))
}

fn parse_fraction(value: &str, name: &str) -> Result<f64, ()> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| eprintln!("error: {name} must be a number"))?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        eprintln!("error: {name} must be in [0,1]");
        return Err(());
    }
    Ok(parsed)
}
