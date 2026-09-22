//! Headless interoperability, scene-document, and weather-policy commands.

use std::{fmt::Write, fs, path::Path, process::ExitCode};
use xvarna_geometry::{Aabb, Mesh, MeshHealOptions, Vec3, clip_mesh_to_aabb, heal_mesh};
use xvarna_io::{
    parse_glb, parse_gltf, parse_obj, parse_ply, parse_stl, write_glb, write_gltf_embedded,
    write_obj, write_ply_binary, write_stl_binary,
};
use xvarna_scene::{SceneBuildOptions, SceneBuilder, SceneDocument, SceneLayer, Transform};
use xvarna_types::{InstanceId, ObjectId};
use xvarna_zurvan::{
    GapPolicy, LeapDayPolicy, MissingRadiationPolicy, WeatherPolicy, apply_weather_policy,
    parse_epw, parse_wea, parse_weather_csv, write_wea, write_weather_csv,
};

#[derive(Clone, Copy, Debug, Default)]
struct MeshPolicy {
    weld_tolerance: f64,
    repair_winding: bool,
    clip: Option<Aabb>,
    json: bool,
}

/// Converts supported mesh/scene exchange formats with explicit geometry policy.
pub fn run_mesh_convert(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let Ok(policy) = parse_mesh_policy(arguments, true) else {
        return ExitCode::from(2);
    };
    let mut mesh = match load_mesh(input) {
        Ok(mesh) => mesh,
        Err(error) => return fail(&error),
    };
    let source_vertices = mesh.positions.len();
    let source_triangles = mesh.triangles.len();
    let mut welded = 0;
    let mut flipped = 0;
    if policy.weld_tolerance > 0.0 || policy.repair_winding {
        let healed = match heal_mesh(
            &mesh,
            MeshHealOptions {
                weld_tolerance: policy.weld_tolerance,
                repair_winding: policy.repair_winding,
                ..MeshHealOptions::default()
            },
        ) {
            Ok(value) => value,
            Err(error) => return fail(&format!("mesh healing failed: {error}")),
        };
        welded = healed.welded_vertex_count;
        flipped = healed.consistency_flip_count;
        mesh = healed.mesh;
    }
    if let Some(bounds) = policy.clip {
        mesh = match clip_mesh_to_aabb(&mesh, bounds, 1.0e-9) {
            Ok(report) => report.mesh,
            Err(error) => return fail(&format!("ROI clipping failed: {error}")),
        };
    }
    if let Err(error) = write_mesh(output, &mesh) {
        return fail(&error);
    }
    if policy.json {
        println!(
            "{{\"input\":{},\"output\":{},\"sourceVertices\":{},\"sourceTriangles\":{},\"outputVertices\":{},\"outputTriangles\":{},\"weldedVertices\":{},\"windingFlips\":{}}}",
            json_string(input),
            json_string(output),
            source_vertices,
            source_triangles,
            mesh.positions.len(),
            mesh.triangles.len(),
            welded,
            flipped
        );
    } else {
        println!("Converted {input} -> {output}");
        println!("Vertices: {source_vertices} -> {}", mesh.positions.len());
        println!("Triangles: {source_triangles} -> {}", mesh.triangles.len());
        println!("Welded vertices: {welded}; winding flips: {flipped}");
    }
    ExitCode::SUCCESS
}

/// Builds one complete canonical scene and writes JSON or XVSCN binary.
pub fn run_scene_pack(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let dynamic = arguments.iter().any(|argument| argument == "--dynamic");
    let filtered = arguments
        .iter()
        .filter(|argument| argument.as_str() != "--dynamic")
        .cloned()
        .collect::<Vec<_>>();
    let Ok(policy) = parse_mesh_policy(&filtered, false) else {
        return ExitCode::from(2);
    };
    let source = match load_mesh(input) {
        Ok(mesh) => mesh,
        Err(error) => return fail(&error),
    };
    let healed = match heal_mesh(
        &source,
        MeshHealOptions {
            weld_tolerance: policy.weld_tolerance,
            repair_winding: policy.repair_winding,
            ..MeshHealOptions::default()
        },
    ) {
        Ok(report) => report.mesh,
        Err(error) => return fail(&format!("mesh canonicalization failed: {error}")),
    };
    let mut builder = match SceneBuilder::new(SceneBuildOptions::default()) {
        Ok(builder) => builder,
        Err(error) => return fail(&error.to_string()),
    };
    let mesh_id = match builder.add_mesh(healed) {
        Ok(id) => id,
        Err(error) => return fail(&error.to_string()),
    };
    if let Err(error) = builder.add_instance_in_layer(
        mesh_id,
        Transform::IDENTITY,
        ObjectId::new(1),
        InstanceId::new(1),
        u64::MAX,
        if dynamic {
            SceneLayer::Dynamic
        } else {
            SceneLayer::Static
        },
    ) {
        return fail(&error.to_string());
    }
    let scene = match builder.build() {
        Ok(scene) => scene,
        Err(error) => return fail(&error.to_string()),
    };
    let document = scene.to_document();
    let write_result = if extension(output).as_deref() == Some("json") {
        document
            .to_json_pretty()
            .map_err(|error| error.to_string())
            .and_then(|text| fs::write(output, text).map_err(|error| error.to_string()))
    } else {
        document
            .to_binary()
            .map_err(|error| error.to_string())
            .and_then(|bytes| fs::write(output, bytes).map_err(|error| error.to_string()))
    };
    if let Err(error) = write_result {
        return fail(&format!("scene serialization failed: {error}"));
    }
    if policy.json {
        println!(
            "{{\"output\":{},\"sceneHash\":{},\"meshes\":{},\"instances\":{},\"triangles\":{}}}",
            json_string(output),
            json_string(&hex(&scene.stats().content_hash)),
            scene.stats().mesh_resource_count,
            scene.stats().instance_count,
            scene.stats().unique_triangle_count
        );
    } else {
        println!("Packed scene: {output}");
        println!("Scene hash: {}", hex(&scene.stats().content_hash));
    }
    ExitCode::SUCCESS
}

/// Verifies a complete scene document, recompiles it, and expands world geometry.
pub fn run_scene_unpack(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let json = match arguments {
        [] => false,
        [argument] if argument == "--json" => true,
        _ => {
            eprintln!("error: scene-unpack accepts only --json");
            return ExitCode::from(2);
        }
    };
    let bytes = match fs::read(input) {
        Ok(bytes) => bytes,
        Err(error) => return fail(&format!("cannot read {input}: {error}")),
    };
    let document = if bytes.first().is_some_and(|byte| *byte == b'{') {
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) => return fail(&format!("scene JSON is not UTF-8: {error}")),
        };
        match SceneDocument::from_json(text) {
            Ok(document) => document,
            Err(error) => return fail(&format!("scene JSON failed verification: {error}")),
        }
    } else {
        match SceneDocument::from_binary(&bytes) {
            Ok(document) => document,
            Err(error) => return fail(&format!("scene binary failed verification: {error}")),
        }
    };
    let scene = match document.compile() {
        Ok(scene) => scene,
        Err(error) => return fail(&format!("scene compilation failed: {error}")),
    };
    let triangles = scene.world_triangles();
    let mut mesh = Mesh {
        positions: Vec::with_capacity(triangles.len().saturating_mul(3)),
        triangles: Vec::with_capacity(triangles.len()),
    };
    for triangle in triangles {
        let Ok(base) = u32::try_from(mesh.positions.len()) else {
            return fail("expanded scene exceeds u32 vertex range");
        };
        mesh.positions
            .extend([triangle.first, triangle.second, triangle.third]);
        mesh.triangles.push([base, base + 1, base + 2]);
    }
    if let Err(error) = write_mesh(output, &mesh) {
        return fail(&error);
    }
    if json {
        println!(
            "{{\"input\":{},\"output\":{},\"instances\":{},\"expandedTriangles\":{}}}",
            json_string(input),
            json_string(output),
            scene.stats().instance_count,
            mesh.triangles.len()
        );
    } else {
        println!("Unpacked verified scene: {input} -> {output}");
    }
    ExitCode::SUCCESS
}

/// Converts EPW/WEA/CSV through an explicit missing/leap/gap policy.
pub fn run_weather_convert(input: &str, output: &str, arguments: &[String]) -> ExitCode {
    let Ok((policy, json, simulation_year)) = parse_weather_policy(arguments) else {
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(input) {
        Ok(source) => source,
        Err(error) => return fail(&format!("cannot read {input}: {error}")),
    };
    let weather = match extension(input).as_deref() {
        Some("epw") => parse_epw(&source).map_err(|error| error.to_string()),
        Some("wea") => parse_wea(&source).map_err(|error| error.to_string()),
        Some("csv") => parse_weather_csv(&source).map_err(|error| error.to_string()),
        _ => Err("weather input extension must be .epw, .wea, or .csv".to_owned()),
    };
    let weather = match weather {
        Ok(weather) => weather,
        Err(error) => return fail(&format!("weather import failed: {error}")),
    };
    let weather = if let Some(year) = simulation_year {
        match xvarna_zurvan::with_simulation_year(&weather, year) {
            Ok(value) => value,
            Err(error) => return fail(&format!("simulation calendar: {error}")),
        }
    } else {
        weather
    };
    let normalized = match apply_weather_policy(&weather, policy) {
        Ok(report) => report,
        Err(error) => return fail(&format!("weather policy failed: {error}")),
    };
    let content = match extension(output).as_deref() {
        Some("wea") => Ok(write_wea(&normalized.weather)),
        Some("csv") => Ok(write_weather_csv(&normalized.weather)),
        _ => Err("weather output extension must be .wea or .csv"),
    };
    let content = match content {
        Ok(content) => content,
        Err(error) => return fail(error),
    };
    if let Err(error) = fs::write(output, content) {
        return fail(&format!("cannot write {output}: {error}"));
    }
    if json {
        println!(
            "{{\"input\":{},\"output\":{},\"records\":{},\"gaps\":{},\"droppedLeap\":{},\"interpolated\":[{},{},{}],\"weatherHash\":{}}}",
            json_string(input),
            json_string(output),
            normalized.weather.records.len(),
            normalized.gap_count,
            normalized.dropped_leap_day_count,
            normalized.interpolated_global_count,
            normalized.interpolated_direct_count,
            normalized.interpolated_diffuse_count,
            json_string(&hex(&normalized.weather.content_hash))
        );
    } else {
        println!("Normalized weather: {input} -> {output}");
        println!(
            "Records: {}; gaps: {}; leap rows dropped: {}",
            normalized.weather.records.len(),
            normalized.gap_count,
            normalized.dropped_leap_day_count
        );
    }
    ExitCode::SUCCESS
}

fn parse_mesh_policy(arguments: &[String], allow_clip: bool) -> Result<MeshPolicy, ()> {
    let mut policy = MeshPolicy::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--repair-winding" => {
                policy.repair_winding = true;
                index += 1;
            }
            "--json" => {
                policy.json = true;
                index += 1;
            }
            "--weld" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --weld requires metres");
                    return Err(());
                };
                policy.weld_tolerance = value.parse().map_err(|_| {
                    eprintln!("error: --weld must be a finite non-negative number");
                })?;
                index += 2;
            }
            "--clip" if allow_clip => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --clip requires minx,miny,minz,maxx,maxy,maxz");
                    return Err(());
                };
                policy.clip = Some(parse_bounds(value)?);
                index += 2;
            }
            unknown => {
                eprintln!("error: unknown interoperability option '{unknown}'");
                return Err(());
            }
        }
    }
    if !policy.weld_tolerance.is_finite() || policy.weld_tolerance < 0.0 {
        eprintln!("error: --weld must be finite and non-negative");
        return Err(());
    }
    Ok(policy)
}

fn parse_bounds(value: &str) -> Result<Aabb, ()> {
    let values = value
        .split(',')
        .map(str::trim)
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| eprintln!("error: --clip contains an invalid coordinate"))?;
    if values.len() != 6 || values.iter().any(|value| !value.is_finite()) {
        eprintln!("error: --clip requires six finite coordinates");
        return Err(());
    }
    Ok(Aabb::new(
        Vec3::new(values[0], values[1], values[2]),
        Vec3::new(values[3], values[4], values[5]),
    ))
}

fn parse_weather_policy(arguments: &[String]) -> Result<(WeatherPolicy, bool, Option<i32>), ()> {
    let mut policy = WeatherPolicy::default();
    let mut json = false;
    let mut simulation_year = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--simulation-year" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --simulation-year requires a calendar year");
                    return Err(());
                };
                simulation_year = Some(
                    value
                        .parse::<i32>()
                        .map_err(|_| eprintln!("error: invalid simulation year"))?,
                );
                index += 2;
            }
            "--drop-leap" => {
                policy.leap_day = LeapDayPolicy::Drop;
                index += 1;
            }
            "--preserve-gaps" => {
                policy.gaps = GapPolicy::Preserve;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--missing" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: --missing requires reject, zero, or interpolate");
                    return Err(());
                };
                policy.missing_radiation = match value.as_str() {
                    "reject" => MissingRadiationPolicy::Reject,
                    "zero" => MissingRadiationPolicy::Zero,
                    "interpolate" => MissingRadiationPolicy::LinearInterpolation,
                    _ => {
                        eprintln!("error: --missing requires reject, zero, or interpolate");
                        return Err(());
                    }
                };
                index += 2;
            }
            unknown => {
                eprintln!("error: unknown weather option '{unknown}'");
                return Err(());
            }
        }
    }
    Ok((policy, json, simulation_year))
}

fn load_mesh(path: &str) -> Result<Mesh, String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    match extension(path).as_deref() {
        Some("obj") => std::str::from_utf8(&bytes)
            .map_err(|error| error.to_string())
            .and_then(|source| {
                parse_obj(source)
                    .map(|report| report.mesh)
                    .map_err(|error| error.to_string())
            }),
        Some("stl") => parse_stl(&bytes)
            .map(|report| report.mesh)
            .map_err(|error| error.to_string()),
        Some("ply") => parse_ply(&bytes)
            .map(|report| report.mesh)
            .map_err(|error| error.to_string()),
        Some("gltf") => std::str::from_utf8(&bytes)
            .map_err(|error| error.to_string())
            .and_then(|source| {
                parse_gltf(source)
                    .map(|report| report.mesh)
                    .map_err(|error| error.to_string())
            }),
        Some("glb") => parse_glb(&bytes)
            .map(|report| report.mesh)
            .map_err(|error| error.to_string()),
        _ => Err("mesh extension must be .obj, .stl, .ply, .gltf, or .glb".to_owned()),
    }
}

fn write_mesh(path: &str, mesh: &Mesh) -> Result<(), String> {
    match extension(path).as_deref() {
        Some("obj") => fs::write(path, write_obj(mesh)).map_err(|error| error.to_string()),
        Some("stl") => write_stl_binary(mesh)
            .map_err(|error| error.to_string())
            .and_then(|bytes| fs::write(path, bytes).map_err(|error| error.to_string())),
        Some("ply") => write_ply_binary(mesh)
            .map_err(|error| error.to_string())
            .and_then(|bytes| fs::write(path, bytes).map_err(|error| error.to_string())),
        Some("gltf") => write_gltf_embedded(mesh)
            .map_err(|error| error.to_string())
            .and_then(|text| fs::write(path, text).map_err(|error| error.to_string())),
        Some("glb") => write_glb(mesh)
            .map_err(|error| error.to_string())
            .and_then(|bytes| fs::write(path, bytes).map_err(|error| error.to_string())),
        _ => Err("mesh output extension must be .obj, .stl, .ply, .gltf, or .glb".to_owned()),
    }
}

fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string JSON serialization cannot fail")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}
