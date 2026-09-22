//! Real external Radiance acceptance: sky normalization and diffuse interreflection.
use std::path::Path;
use xvarna_geometry::{Mesh, Vec3};
use xvarna_hvare::{
    DaylightMoment, DaylightSensor, OpticalMaterial, OpticalMaterialKind, OpticalMaterialLibrary,
    RadianceRunOptions, RadianceToolchain, export_radiance_bundle, run_radiance_point,
};
use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
use xvarna_types::{InstanceId, ObjectId, SensorId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Supply the installed Radiance bin directory")?;
    let tools = RadianceToolchain::discover(Some(Path::new(&path)))?;
    let mut builder = SceneBuilder::new(SceneBuildOptions::default())?;
    let mesh = builder.add_mesh(Mesh {
        positions: vec![
            Vec3::new(-1000.0, -1000.0, 0.0),
            Vec3::new(1000.0, -1000.0, 0.0),
            Vec3::new(1000.0, 1000.0, 0.0),
            Vec3::new(-1000.0, 1000.0, 0.0),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    })?;
    builder.add_instance(
        mesh,
        Transform::IDENTITY,
        ObjectId::new(1),
        InstanceId::new(1),
        1,
    )?;
    let scene = builder.build()?;
    let surface = OpticalMaterial::try_new(
        1,
        OpticalMaterialKind::Plastic,
        [0.5; 3],
        [0.0; 3],
        0.0,
        0.0,
        1.52,
    )?;
    let materials = OpticalMaterialLibrary::try_new([surface], [(ObjectId::new(1), 1)])?;
    let sensors = [
        DaylightSensor::try_new(
            SensorId::new(1),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
        )?,
        DaylightSensor::try_new(
            SensorId::new(2),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            1.0,
        )?,
    ];
    let bundle = export_radiance_bundle(&scene, &sensors, &materials)?;
    let moment = DaylightMoment::try_new(0, Vec3::new(0.0, 0.0, 1.0), 0.0, 10_000.0)?;
    let options = RadianceRunOptions {
        ambient_bounces: 2,
        ambient_accuracy: 0.0,
        ambient_divisions: 8192,
        ambient_supersamples: 0,
        ..Default::default()
    };
    let reflected = run_radiance_point(&tools, &bundle, moment, &options, None)?;
    let direct = run_radiance_point(
        &tools,
        &bundle,
        moment,
        &RadianceRunOptions {
            ambient_bounces: 0,
            ..options
        },
        None,
    )?;
    println!("Radiance: {}", tools.version);
    println!(
        "No ambient: {:?}; two bounces: {:?}",
        direct.illuminance_lux, reflected.illuminance_lux
    );
    // Uniform sky integral is pi*L; a large Lambertian ground returns rho*E.
    assert!((reflected.illuminance_lux[0] - 10_000.0).abs() < 200.0);
    assert!((reflected.illuminance_lux[1] - 5_000.0).abs() < 250.0);
    assert!(direct.illuminance_lux[1].abs() < 1.0);
    println!(
        "PASS: real production export/runner, sky photometry and reflected light within analytical tolerances."
    );
    if let Some(output) = std::env::args().nth(2) {
        std::fs::write(
            output,
            format!(
                "quantity,reference_lux,observed_lux\nsky,10000,{}\nreflection,5000,{}\nambient_disabled,0,{}\n",
                reflected.illuminance_lux[0],
                reflected.illuminance_lux[1],
                direct.illuminance_lux[1]
            ),
        )?;
    }
    Ok(())
}
