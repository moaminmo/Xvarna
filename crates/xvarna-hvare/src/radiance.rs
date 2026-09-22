//! Radiance scene mapping, cancellable reference execution, and matrix reuse.

use core::fmt;
use std::{
    ffi::OsStr,
    fmt::Write as _,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use xvarna_scene::Scene;
use xvarna_zurvan::EpwWeather;

use crate::{
    DaylightMoment, DaylightSensor, OpticalMaterial, OpticalMaterialKind, OpticalMaterialLibrary,
};

const RADIANCE_LUMINOUS_EFFICACY: f64 = 179.0;
const SOLAR_DISK_SOLID_ANGLE: f64 = 6.796_7e-5;

/// Complete deterministic text bundle accepted by Radiance tools.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadianceExportBundle {
    /// Material primitives.
    pub materials_rad: String,
    /// World-space polygon geometry in metres.
    pub geometry_rad: String,
    /// Sensor positions and normals for `rtrace -I+` or `rfluxmtx -I+`.
    pub sensors_pts: String,
    /// Machine-readable provenance manifest.
    pub manifest_json: String,
    /// Stable scene/material/sensor/export identity.
    pub content_hash: [u8; 32],
}

/// Exports one immutable ZAMYAD scene with exact Object-ID optical mapping.
pub fn export_radiance_bundle(
    scene: &Scene,
    sensors: &[DaylightSensor],
    materials: &OpticalMaterialLibrary,
) -> Result<RadianceExportBundle, RadianceError> {
    if sensors.is_empty() {
        return Err(RadianceError::InvalidInput);
    }
    let mut materials_rad = String::from(
        "# XVARNA Radiance material export v1\n\
         void plastic xv_implicit_opaque\n0\n0\n5 0.2 0.2 0.2 0 0\n\n",
    );
    for material in materials.materials() {
        write_material(&mut materials_rad, material)?;
    }
    let mut geometry_rad = String::from("# XVARNA canonical world geometry in metres\n");
    let triangles = scene.world_triangles();
    for triangle in &triangles {
        let modifier = materials
            .material_for_object(triangle.object_id)
            .map_or_else(
                || "xv_implicit_opaque".to_owned(),
                |value| material_name(value.id),
            );
        writeln!(
            geometry_rad,
            "{modifier} polygon xv_o{}_i{}_t{}\n0\n0\n9\n  {:.17} {:.17} {:.17}\n  {:.17} {:.17} {:.17}\n  {:.17} {:.17} {:.17}\n",
            triangle.object_id.get(),
            triangle.instance_id.get(),
            triangle.triangle_id,
            triangle.first.x,
            triangle.first.y,
            triangle.first.z,
            triangle.second.x,
            triangle.second.y,
            triangle.second.z,
            triangle.third.x,
            triangle.third.y,
            triangle.third.z,
        )
        .map_err(|_| RadianceError::Export)?;
    }
    let mut sensors_pts = String::new();
    for sensor in sensors {
        writeln!(
            sensors_pts,
            "{:.17} {:.17} {:.17} {:.17} {:.17} {:.17}",
            sensor.position.x,
            sensor.position.y,
            sensor.position.z,
            sensor.normal.x,
            sensor.normal.y,
            sensor.normal.z,
        )
        .map_err(|_| RadianceError::Export)?;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_RADIANCE_EXPORT_V1\0");
    hasher.update(&scene.stats().content_hash);
    hasher.update(&materials.content_hash());
    hasher.update(materials_rad.as_bytes());
    hasher.update(geometry_rad.as_bytes());
    hasher.update(sensors_pts.as_bytes());
    let content_hash = *hasher.finalize().as_bytes();
    let manifest_json = format!(
        "{{\"schemaVersion\":\"0.16.0\",\"generator\":\"XVARNA\",\"units\":\"metres\",\"sceneHash\":\"{}\",\"materialHash\":\"{}\",\"exportHash\":\"{}\",\"triangles\":{},\"sensors\":{},\"unassignedMaterial\":\"xv_implicit_opaque\"}}",
        format_hash(scene.stats().content_hash),
        format_hash(materials.content_hash()),
        format_hash(content_hash),
        triangles.len(),
        sensors.len(),
    );
    Ok(RadianceExportBundle {
        materials_rad,
        geometry_rad,
        sensors_pts,
        manifest_json,
        content_hash,
    })
}

/// Writes a bundle into an existing or newly created output directory.
pub fn write_radiance_bundle(
    bundle: &RadianceExportBundle,
    output_directory: &Path,
) -> Result<RadianceBundlePaths, RadianceError> {
    if output_directory.as_os_str().is_empty() {
        return Err(RadianceError::InvalidInput);
    }
    fs::create_dir_all(output_directory).map_err(|_| RadianceError::Io)?;
    let paths = RadianceBundlePaths {
        directory: output_directory.to_path_buf(),
        materials: output_directory.join("materials.rad"),
        geometry: output_directory.join("geometry.rad"),
        sensors: output_directory.join("sensors.pts"),
        manifest: output_directory.join("manifest.json"),
    };
    write_text(&paths.materials, &bundle.materials_rad)?;
    write_text(&paths.geometry, &bundle.geometry_rad)?;
    write_text(&paths.sensors, &bundle.sensors_pts)?;
    write_text(&paths.manifest, &bundle.manifest_json)?;
    Ok(paths)
}

/// Files produced by [`write_radiance_bundle`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadianceBundlePaths {
    /// Root bundle directory.
    pub directory: PathBuf,
    /// Material file.
    pub materials: PathBuf,
    /// Geometry file.
    pub geometry: PathBuf,
    /// Sensor file.
    pub sensors: PathBuf,
    /// Provenance manifest.
    pub manifest: PathBuf,
}

/// Discovered Radiance command-line installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadianceToolchain {
    /// `oconv` executable.
    pub oconv: PathBuf,
    /// `rtrace` executable.
    pub rtrace: PathBuf,
    /// Optional `rfluxmtx` executable.
    pub rfluxmtx: Option<PathBuf>,
    /// Optional `gendaymtx` executable.
    pub gendaymtx: Option<PathBuf>,
    /// Optional `dctimestep` executable.
    pub dctimestep: Option<PathBuf>,
    /// Version text returned by `rtrace -version` when available.
    pub version: String,
}

impl RadianceToolchain {
    /// Discovers tools in an explicit directory or the process `PATH`.
    pub fn discover(bin_directory: Option<&Path>) -> Result<Self, RadianceError> {
        let oconv = find_tool("oconv", bin_directory).ok_or(RadianceError::ToolUnavailable)?;
        let rtrace = find_tool("rtrace", bin_directory).ok_or(RadianceError::ToolUnavailable)?;
        let version = Command::new(&rtrace)
            .arg("-version")
            .output()
            .ok()
            .map(|output| {
                let value = if output.stdout.is_empty() {
                    output.stderr
                } else {
                    output.stdout
                };
                String::from_utf8_lossy(&value).trim().to_owned()
            })
            .unwrap_or_default();
        Ok(Self {
            oconv,
            rtrace,
            rfluxmtx: find_tool("rfluxmtx", bin_directory),
            gendaymtx: find_tool("gendaymtx", bin_directory),
            dctimestep: find_tool("dctimestep", bin_directory),
            version,
        })
    }

    /// True when the three annual matrix utilities are present.
    #[must_use]
    pub const fn supports_annual_matrix(&self) -> bool {
        self.rfluxmtx.is_some() && self.gendaymtx.is_some() && self.dctimestep.is_some()
    }
}

/// Radiance ambient and execution controls with explicit fidelity/runtime trade-offs.
#[derive(Clone, Debug, PartialEq)]
pub struct RadianceRunOptions {
    /// Ambient bounce count.
    pub ambient_bounces: u32,
    /// Ambient divisions.
    pub ambient_divisions: u32,
    /// Ambient super-samples.
    pub ambient_supersamples: u32,
    /// Ambient accuracy.
    pub ambient_accuracy: f64,
    /// Optional persistent annual matrix cache root.
    pub matrix_cache_directory: Option<PathBuf>,
    /// Retain temporary files after point-in-time execution.
    pub keep_temporary_files: bool,
}

impl Default for RadianceRunOptions {
    fn default() -> Self {
        Self {
            ambient_bounces: 5,
            ambient_divisions: 4_096,
            ambient_supersamples: 1_024,
            ambient_accuracy: 0.1,
            matrix_cache_directory: None,
            keep_temporary_files: false,
        }
    }
}

impl RadianceRunOptions {
    fn validate(&self) -> Result<(), RadianceError> {
        if self.ambient_bounces > 32
            || self.ambient_divisions == 0
            || self.ambient_supersamples > self.ambient_divisions
            || !self.ambient_accuracy.is_finite()
            || !(0.0..=1.0).contains(&self.ambient_accuracy)
        {
            return Err(RadianceError::InvalidInput);
        }
        Ok(())
    }
}

/// Complete external-run provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadianceProvenance {
    /// Radiance version string.
    pub tool_version: String,
    /// Ordered executable and argument records.
    pub commands: Vec<String>,
    /// Wall-clock execution duration.
    pub elapsed_microseconds: u64,
    /// True when an annual daylight coefficient matrix was reused.
    pub matrix_reused: bool,
    /// Export or annual-run identity.
    pub content_hash: [u8; 32],
}

/// Reference point-in-time illuminance returned by `rtrace -I+`.
#[derive(Clone, Debug, PartialEq)]
pub struct RadiancePointResult {
    /// Sensor-aligned photopic illuminance in lux.
    pub illuminance_lux: Vec<f64>,
    /// External execution and identity evidence.
    pub provenance: RadianceProvenance,
    /// Retained working directory when requested.
    pub retained_directory: Option<PathBuf>,
}

/// Runs a point-in-time Radiance reference using an isotropic sky and finite solar source.
pub fn run_radiance_point(
    toolchain: &RadianceToolchain,
    bundle: &RadianceExportBundle,
    moment: DaylightMoment,
    options: &RadianceRunOptions,
    cancelled: Option<&AtomicBool>,
) -> Result<RadiancePointResult, RadianceError> {
    options.validate()?;
    check_cancelled(cancelled)?;
    let started = Instant::now();
    let working = temporary_directory("point")?;
    let paths = write_radiance_bundle(bundle, &working)?;
    let sky_path = working.join("sky.rad");
    write_text(&sky_path, &radiance_sky(moment)?)?;
    let octree_path = working.join("scene.oct");
    let result_path = working.join("point.rgb");
    let mut commands = Vec::new();
    let oconv_arguments = vec![
        paths.materials.as_os_str().to_owned(),
        paths.geometry.as_os_str().to_owned(),
        sky_path.as_os_str().to_owned(),
    ];
    commands.push(command_text(&toolchain.oconv, &oconv_arguments));
    run_command_to_file(
        &toolchain.oconv,
        &oconv_arguments,
        None,
        &octree_path,
        cancelled,
    )?;
    let rtrace_arguments = radiance_trace_arguments(options, &octree_path);
    commands.push(command_text(&toolchain.rtrace, &rtrace_arguments));
    run_command_to_file(
        &toolchain.rtrace,
        &rtrace_arguments,
        Some(&paths.sensors),
        &result_path,
        cancelled,
    )?;
    let illuminance_lux = parse_rgb_illuminance(&result_path)?;
    let sensor_count = bundle.sensors_pts.lines().count();
    if illuminance_lux.len() != sensor_count {
        return Err(RadianceError::InvalidOutput);
    }
    let content_hash = run_hash(bundle.content_hash, &commands, &illuminance_lux);
    let retained_directory = if options.keep_temporary_files {
        Some(working)
    } else {
        remove_exact_working_directory(&working)?;
        None
    };
    Ok(RadiancePointResult {
        illuminance_lux,
        provenance: RadianceProvenance {
            tool_version: toolchain.version.clone(),
            commands,
            elapsed_microseconds: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            matrix_reused: false,
            content_hash,
        },
        retained_directory,
    })
}

/// Annual sensor-major Radiance illuminance matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct RadianceAnnualResult {
    /// Sensor-major, time-major illuminance in lux.
    pub illuminance_lux: Vec<f64>,
    /// Sensor count.
    pub sensor_count: usize,
    /// Weather timestep count.
    pub timestep_count: usize,
    /// External execution and matrix-reuse evidence.
    pub provenance: RadianceProvenance,
    /// Directory containing reusable matrices and manifests.
    pub matrix_directory: PathBuf,
}

/// Runs the Radiance daylight-coefficient workflow (`rfluxmtx` + `gendaymtx` + `dctimestep`).
///
/// The geometry/sensor daylight coefficient matrix is keyed independently from weather, so a
/// second climate run over the same scene, materials, sensors, and ambient policy reuses it.
#[allow(clippy::too_many_lines)]
pub fn run_radiance_annual_matrix(
    toolchain: &RadianceToolchain,
    bundle: &RadianceExportBundle,
    weather: &EpwWeather,
    options: &RadianceRunOptions,
    cancelled: Option<&AtomicBool>,
) -> Result<RadianceAnnualResult, RadianceError> {
    options.validate()?;
    if !toolchain.supports_annual_matrix() || weather.records.is_empty() {
        return Err(RadianceError::ToolUnavailable);
    }
    check_cancelled(cancelled)?;
    let started = Instant::now();
    let mut key_hasher = blake3::Hasher::new();
    // The cache contract includes the matrix basis and Radiance implementation.  A matrix
    // produced by another receiver basis or tool version must never be silently reused.
    key_hasher.update(b"XVARNA_RADIANCE_DC_MATRIX_V3\0");
    key_hasher.update(sky_receiver_text().as_bytes());
    key_hasher.update(toolchain.version.as_bytes());
    key_hasher.update(&bundle.content_hash);
    key_hasher.update(&options.ambient_bounces.to_le_bytes());
    key_hasher.update(&options.ambient_divisions.to_le_bytes());
    key_hasher.update(&options.ambient_supersamples.to_le_bytes());
    key_hasher.update(&options.ambient_accuracy.to_bits().to_le_bytes());
    let matrix_key = *key_hasher.finalize().as_bytes();
    let root = options
        .matrix_cache_directory
        .clone()
        .unwrap_or(temporary_directory("annual")?);
    fs::create_dir_all(&root).map_err(|_| RadianceError::Io)?;
    let matrix_directory = root.join(format!("dc-{}", &format_hash(matrix_key)[..16]));
    fs::create_dir_all(&matrix_directory).map_err(|_| RadianceError::Io)?;
    let paths = write_radiance_bundle(bundle, &matrix_directory)?;
    let sky_receiver = matrix_directory.join("sky-receiver.rad");
    write_text(&sky_receiver, sky_receiver_text())?;
    let octree = matrix_directory.join("scene.oct");
    let coefficient_matrix = matrix_directory.join("daylight.mtx");
    let matrix_manifest = matrix_directory.join("daylight.key");
    let key_text = format_hash(matrix_key);
    let matrix_reused = coefficient_matrix.is_file()
        && fs::read_to_string(&matrix_manifest).is_ok_and(|value| value.trim() == key_text);
    let mut commands = Vec::new();
    if !matrix_reused {
        let oconv_arguments = vec![
            paths.materials.as_os_str().to_owned(),
            paths.geometry.as_os_str().to_owned(),
        ];
        commands.push(command_text(&toolchain.oconv, &oconv_arguments));
        run_command_to_file(&toolchain.oconv, &oconv_arguments, None, &octree, cancelled)?;
        let rflux = toolchain
            .rfluxmtx
            .as_ref()
            .ok_or(RadianceError::ToolUnavailable)?;
        let rflux_arguments = vec![
            "-I+".into(),
            "-y".into(),
            paths.sensors_pts_count().to_string().into(),
            "-ab".into(),
            options.ambient_bounces.to_string().into(),
            "-ad".into(),
            options.ambient_divisions.to_string().into(),
            "-as".into(),
            options.ambient_supersamples.to_string().into(),
            "-aa".into(),
            options.ambient_accuracy.to_string().into(),
            "-lw".into(),
            (0.1 / f64::from(options.ambient_divisions))
                .to_string()
                .into(),
            "-".into(),
            "sky-receiver.rad".into(),
            "-i".into(),
            "scene.oct".into(),
        ];
        commands.push(format!(
            "cwd={} {}",
            matrix_directory.display(),
            command_text(rflux, &rflux_arguments)
        ));
        run_command_to_file_in_dir(
            rflux,
            &rflux_arguments,
            Some(&paths.sensors),
            &coefficient_matrix,
            cancelled,
            Some(&matrix_directory),
        )?;
        write_text(&matrix_manifest, &key_text)?;
    }
    let wea = matrix_directory.join("weather.wea");
    write_wea(weather, &wea)?;
    let sky_matrix = matrix_directory.join(format!(
        "sky-{}.mtx",
        &format_hash(weather.content_hash)[..16]
    ));
    let gendaymtx = toolchain
        .gendaymtx
        .as_ref()
        .ok_or(RadianceError::ToolUnavailable)?;
    let genday_arguments = vec!["-m".into(), "1".into(), wea.as_os_str().to_owned()];
    commands.push(command_text(gendaymtx, &genday_arguments));
    run_command_to_file(gendaymtx, &genday_arguments, None, &sky_matrix, cancelled)?;
    let annual_rgb = matrix_directory.join("annual.rgb");
    let dctimestep = toolchain
        .dctimestep
        .as_ref()
        .ok_or(RadianceError::ToolUnavailable)?;
    let dc_arguments = vec![
        "-h".into(),
        coefficient_matrix.as_os_str().to_owned(),
        sky_matrix.as_os_str().to_owned(),
    ];
    commands.push(command_text(dctimestep, &dc_arguments));
    run_command_to_file(dctimestep, &dc_arguments, None, &annual_rgb, cancelled)?;
    let illuminance_lux = parse_rgb_illuminance(&annual_rgb)?;
    let sensor_count = bundle.sensors_pts.lines().count();
    let expected = sensor_count
        .checked_mul(weather.records.len())
        .ok_or(RadianceError::InvalidOutput)?;
    if illuminance_lux.len() != expected {
        return Err(RadianceError::InvalidOutput);
    }
    let content_hash = run_hash(bundle.content_hash, &commands, &illuminance_lux);
    Ok(RadianceAnnualResult {
        illuminance_lux,
        sensor_count,
        timestep_count: weather.records.len(),
        provenance: RadianceProvenance {
            tool_version: toolchain.version.clone(),
            commands,
            elapsed_microseconds: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            matrix_reused,
            content_hash,
        },
        matrix_directory,
    })
}

impl RadianceBundlePaths {
    fn sensors_pts_count(&self) -> usize {
        fs::read_to_string(&self.sensors).map_or(0, |value| value.lines().count())
    }
}

/// Radiance export, toolchain, execution, or parsing failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadianceError {
    /// Inputs or numerical controls are invalid.
    InvalidInput,
    /// Text mapping failed.
    Export,
    /// File IO failed.
    Io,
    /// Required Radiance executables are unavailable.
    ToolUnavailable,
    /// A Radiance command returned failure.
    CommandFailed(String),
    /// Radiance output shape or values are invalid.
    InvalidOutput,
    /// Cancellation was observed and the child process was terminated.
    Cancelled,
}

impl fmt::Display for RadianceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => formatter.write_str("Radiance inputs or controls are invalid"),
            Self::Export => formatter.write_str("Radiance text export failed"),
            Self::Io => formatter.write_str("Radiance working-file IO failed"),
            Self::ToolUnavailable => {
                formatter.write_str("required Radiance command-line tools are unavailable")
            }
            Self::CommandFailed(message) => write!(formatter, "Radiance command failed: {message}"),
            Self::InvalidOutput => formatter.write_str("Radiance returned invalid output"),
            Self::Cancelled => formatter.write_str("Radiance execution was cancelled"),
        }
    }
}

impl std::error::Error for RadianceError {}

fn write_material(output: &mut String, material: OpticalMaterial) -> Result<(), RadianceError> {
    let name = material_name(material.id);
    let r = material.reflectance;
    let t = material.transmittance;
    match material.kind {
        OpticalMaterialKind::Plastic => writeln!(
            output,
            "void plastic {name}\n0\n0\n5 {:.9} {:.9} {:.9} {:.9} {:.9}\n",
            r[0], r[1], r[2], material.specularity, material.roughness,
        ),
        OpticalMaterialKind::Metal => writeln!(
            output,
            "void metal {name}\n0\n0\n5 {:.9} {:.9} {:.9} {:.9} {:.9}\n",
            r[0], r[1], r[2], material.specularity, material.roughness,
        ),
        OpticalMaterialKind::Mirror => writeln!(
            output,
            "void mirror {name}\n0\n0\n3 {:.9} {:.9} {:.9}\n",
            r[0], r[1], r[2],
        ),
        OpticalMaterialKind::Glass => writeln!(
            output,
            "void glass {name}\n0\n0\n4 {:.9} {:.9} {:.9} {:.9}\n",
            radiance_transmissivity(t[0]),
            radiance_transmissivity(t[1]),
            radiance_transmissivity(t[2]),
            material.refractive_index,
        ),
        OpticalMaterialKind::Trans => {
            let transmitted = material.photopic_transmittance();
            let total = (material.photopic_reflectance() + transmitted).min(1.0);
            let transmitted_fraction = if total > 0.0 {
                transmitted / total
            } else {
                0.0
            };
            writeln!(
                output,
                "void trans {name}\n0\n0\n7 {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} 0\n",
                r[0], r[1], r[2], material.specularity, material.roughness, transmitted_fraction,
            )
        }
    }
    .map_err(|_| RadianceError::Export)
}

fn radiance_sky(moment: DaylightMoment) -> Result<String, RadianceError> {
    if moment.direct_normal_illuminance_lux < 0.0
        || moment.diffuse_horizontal_illuminance_lux < 0.0
        || !moment.sun_direction.is_finite()
    {
        return Err(RadianceError::InvalidInput);
    }
    let sky_radiance = moment.diffuse_horizontal_illuminance_lux
        / (RADIANCE_LUMINOUS_EFFICACY * core::f64::consts::PI);
    let solar_radiance = moment.direct_normal_illuminance_lux
        / (RADIANCE_LUMINOUS_EFFICACY * SOLAR_DISK_SOLID_ANGLE);
    Ok(format!(
        "# XVARNA isotropic point-in-time sky\n\
         void glow xv_sky_glow\n0\n0\n4 {sky_radiance:.12} {sky_radiance:.12} {sky_radiance:.12} 0\n\
         xv_sky_glow source xv_sky\n0\n0\n4 0 0 1 180\n\n\
         void light xv_solar\n0\n0\n3 {solar_radiance:.12} {solar_radiance:.12} {solar_radiance:.12}\n\
         xv_solar source xv_sun\n0\n0\n4 {:.12} {:.12} {:.12} 0.533\n",
        moment.sun_direction.x, moment.sun_direction.y, moment.sun_direction.z,
    ))
}

const fn sky_receiver_text() -> &'static str {
    "#@rfluxmtx h=u u=Y\n\
     void glow xv_ground_glow\n0\n0\n4 1 1 1 0\n\
     xv_ground_glow source xv_ground\n0\n0\n4 0 0 -1 180\n\n\
     #@rfluxmtx h=r1 u=Y\n\
     void glow xv_sky_glow\n0\n0\n4 1 1 1 0\n\
     xv_sky_glow source xv_sky\n0\n0\n4 0 0 1 180\n"
}

fn radiance_trace_arguments(
    options: &RadianceRunOptions,
    octree: &Path,
) -> Vec<std::ffi::OsString> {
    vec![
        "-I+".into(),
        "-h".into(),
        "-ov".into(),
        "-ab".into(),
        options.ambient_bounces.to_string().into(),
        "-ad".into(),
        options.ambient_divisions.to_string().into(),
        "-as".into(),
        options.ambient_supersamples.to_string().into(),
        "-aa".into(),
        options.ambient_accuracy.to_string().into(),
        "-lw".into(),
        (0.1 / f64::from(options.ambient_divisions))
            .to_string()
            .into(),
        octree.as_os_str().to_owned(),
    ]
}

fn run_command_to_file(
    program: &Path,
    arguments: &[std::ffi::OsString],
    stdin_path: Option<&Path>,
    stdout_path: &Path,
    cancelled: Option<&AtomicBool>,
) -> Result<(), RadianceError> {
    run_command_to_file_in_dir(program, arguments, stdin_path, stdout_path, cancelled, None)
}

fn run_command_to_file_in_dir(
    program: &Path,
    arguments: &[std::ffi::OsString],
    stdin_path: Option<&Path>,
    stdout_path: &Path,
    cancelled: Option<&AtomicBool>,
    working_directory: Option<&Path>,
) -> Result<(), RadianceError> {
    check_cancelled(cancelled)?;
    let stdout = File::create(stdout_path).map_err(|_| RadianceError::Io)?;
    let stderr_path = stdout_path.with_extension("stderr.log");
    let stderr = File::create(&stderr_path).map_err(|_| RadianceError::Io)?;
    let mut command = Command::new(program);
    if let Some(directory) = working_directory {
        command.current_dir(directory);
    }
    // rfluxmtx launches sibling Radiance programs internally. Resolve them from
    // the explicitly selected toolchain, even when it is not on the host PATH.
    if let Some(directory) = program.parent() {
        let mut paths = vec![directory.to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(value) = std::env::join_paths(paths) {
            command.env("PATH", value);
        }
    }
    command
        .args(arguments)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(path) = stdin_path {
        command.stdin(Stdio::from(
            File::open(path).map_err(|_| RadianceError::Io)?,
        ));
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command
        .spawn()
        .map_err(|_| RadianceError::ToolUnavailable)?;
    loop {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RadianceError::Cancelled);
        }
        if let Some(status) = child.try_wait().map_err(|_| RadianceError::Io)? {
            if status.success() {
                let _ = fs::remove_file(&stderr_path);
                return Ok(());
            }
            let message = fs::read_to_string(&stderr_path).unwrap_or_default();
            return Err(RadianceError::CommandFailed(message.trim().to_owned()));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_rgb_illuminance(path: &Path) -> Result<Vec<f64>, RadianceError> {
    let text = fs::read_to_string(path).map_err(|_| RadianceError::Io)?;
    let values = text
        .split_whitespace()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| RadianceError::InvalidOutput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() % 3 != 0 || values.iter().any(|value| !value.is_finite()) {
        return Err(RadianceError::InvalidOutput);
    }
    Ok(values
        .chunks_exact(3)
        .map(|rgb| {
            RADIANCE_LUMINOUS_EFFICACY
                * (0.265_f64.mul_add(rgb[0], 0.670_f64.mul_add(rgb[1], 0.065 * rgb[2])))
        })
        .collect())
}

fn write_wea(weather: &EpwWeather, path: &Path) -> Result<(), RadianceError> {
    let mut output = String::new();
    writeln!(output, "place {}", weather.location.city).map_err(|_| RadianceError::Export)?;
    writeln!(
        output,
        "latitude {:.8}",
        weather.location.solar.latitude_degrees
    )
    .map_err(|_| RadianceError::Export)?;
    writeln!(
        output,
        "longitude {:.8}",
        -weather.location.solar.longitude_degrees
    )
    .map_err(|_| RadianceError::Export)?;
    writeln!(
        output,
        "time_zone {:.8}",
        -15.0 * weather.location.time_zone_hours
    )
    .map_err(|_| RadianceError::Export)?;
    writeln!(
        output,
        "site_elevation {:.8}",
        weather.location.solar.elevation_meters
    )
    .map_err(|_| RadianceError::Export)?;
    writeln!(output, "weather_data_file_units 1").map_err(|_| RadianceError::Export)?;
    for record in &weather.records {
        let end_hour = f64::from(record.hour - 1) + f64::from(record.minute) / 60.0;
        let midpoint = end_hour - record.duration_hours / 2.0;
        writeln!(
            output,
            "{} {} {:.6} {:.6} {:.6}",
            record.month,
            record.day,
            midpoint,
            record.direct_normal_wh_m2 / record.duration_hours,
            record.diffuse_horizontal_wh_m2 / record.duration_hours,
        )
        .map_err(|_| RadianceError::Export)?;
    }
    write_text(path, &output)
}

fn find_tool(name: &str, directory: Option<&Path>) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_owned()]
    } else {
        vec![name.to_owned()]
    };
    if let Some(directory) = directory {
        for candidate in &candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir().ok()?.join(path)
                });
            }
        }
        return None;
    }
    let path_value = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_value) {
        for candidate in &candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir().ok()?.join(path)
                });
            }
        }
    }
    None
}

fn temporary_directory(label: &str) -> Result<PathBuf, RadianceError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RadianceError::Io)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "xvarna-radiance-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).map_err(|_| RadianceError::Io)?;
    Ok(directory)
}

fn remove_exact_working_directory(path: &Path) -> Result<(), RadianceError> {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return Err(RadianceError::Io);
    };
    if !name.starts_with("xvarna-radiance-")
        || path.parent() != Some(std::env::temp_dir().as_path())
    {
        return Err(RadianceError::Io);
    }
    fs::remove_dir_all(path).map_err(|_| RadianceError::Io)
}

fn write_text(path: &Path, value: &str) -> Result<(), RadianceError> {
    let mut file = File::create(path).map_err(|_| RadianceError::Io)?;
    file.write_all(value.as_bytes())
        .map_err(|_| RadianceError::Io)
}

fn command_text(program: &Path, arguments: &[std::ffi::OsString]) -> String {
    let mut value = program.display().to_string();
    for argument in arguments {
        value.push(' ');
        value.push_str(&argument.to_string_lossy());
    }
    value
}

fn run_hash(export_hash: [u8; 32], commands: &[String], values: &[f64]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_RADIANCE_RUN_V1\0");
    hasher.update(&export_hash);
    for command in commands {
        hasher.update(command.as_bytes());
        hasher.update(&[0]);
    }
    for value in values {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn check_cancelled(cancelled: Option<&AtomicBool>) -> Result<(), RadianceError> {
    if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        Err(RadianceError::Cancelled)
    } else {
        Ok(())
    }
}

fn material_name(id: u64) -> String {
    format!("xv_mat_{id}")
}

fn radiance_transmissivity(transmittance: f64) -> f64 {
    if transmittance <= 0.0 {
        0.0
    } else {
        (((0.007_252_223_9 * transmittance).mul_add(transmittance, 0.840_252_843_5)).sqrt()
            - 0.916_653_066_1)
            / (0.003_626_111_9 * transmittance)
    }
}

fn format_hash(hash: [u8; 32]) -> String {
    hash.iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DaylightError, OpticalMaterialKind};
    use xvarna_geometry::{Mesh, Vec3};
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::{InstanceId, ObjectId, SensorId};

    fn scene() -> Scene {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("builder");
        let mesh = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                triangles: vec![[0, 1, 2]],
            })
            .expect("mesh");
        builder
            .add_instance(
                mesh,
                Transform::IDENTITY,
                ObjectId::new(7),
                InstanceId::new(8),
                1,
            )
            .expect("instance");
        builder.build().expect("scene")
    }

    #[test]
    fn export_maps_exact_material_object_and_sensor_identity() {
        let material = OpticalMaterial::try_new(
            3,
            OpticalMaterialKind::Glass,
            [0.1, 0.1, 0.1],
            [0.6, 0.65, 0.7],
            0.0,
            0.0,
            1.52,
        )
        .expect("material");
        let library =
            OpticalMaterialLibrary::try_new([material], [(ObjectId::new(7), material.id)])
                .expect("library");
        let sensor = DaylightSensor::try_new(
            SensorId::new(1),
            Vec3::new(0.2, 0.2, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
        )
        .expect("sensor");
        let bundle = export_radiance_bundle(&scene(), &[sensor], &library).expect("export");
        assert!(bundle.materials_rad.contains("void glass xv_mat_3"));
        assert!(bundle.geometry_rad.contains("xv_mat_3 polygon xv_o7_i8_t0"));
        assert!(bundle.sensors_pts.contains("0.20000000000000001"));
        assert!(
            bundle
                .manifest_json
                .contains("\"schemaVersion\":\"0.16.0\"")
        );
    }

    #[test]
    fn glass_mapping_is_bounded_and_sky_generation_is_photometric() {
        assert!(radiance_transmissivity(0.0).abs() < f64::EPSILON);
        assert!(radiance_transmissivity(0.6) > 0.6);
        assert!(radiance_transmissivity(0.6) < 1.0);
        let moment = DaylightMoment::try_new(0, Vec3::new(0.0, 0.0, 1.0), 50_000.0, 10_000.0)
            .expect("moment");
        let sky = radiance_sky(moment).expect("sky");
        assert!(sky.contains("xv_sky_glow"));
        assert!(sky.contains("xv_sun"));
        let receiver = sky_receiver_text();
        assert!(receiver.contains("h=u u=Y"));
        assert!(receiver.contains("h=r1 u=Y"));
        assert!(
            receiver.find("xv_ground").expect("ground") < receiver.find("xv_sky").expect("sky")
        );
        let _ = DaylightError::InvalidOptions;
    }

    #[test]
    fn cancelled_external_run_fails_before_tool_launch() {
        let cancelled = AtomicBool::new(true);
        let result = run_radiance_point(
            &RadianceToolchain {
                oconv: "missing-oconv".into(),
                rtrace: "missing-rtrace".into(),
                rfluxmtx: None,
                gendaymtx: None,
                dctimestep: None,
                version: String::new(),
            },
            &export_radiance_bundle(
                &scene(),
                &[DaylightSensor::try_new(
                    SensorId::new(1),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    1.0,
                )
                .expect("sensor")],
                &OpticalMaterialLibrary::default(),
            )
            .expect("bundle"),
            DaylightMoment::try_new(0, Vec3::new(0.0, 0.0, 1.0), 1.0, 1.0).expect("moment"),
            &RadianceRunOptions::default(),
            Some(&cancelled),
        );
        assert!(matches!(result, Err(RadianceError::Cancelled)));
    }
}
