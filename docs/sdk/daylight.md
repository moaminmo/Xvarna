# .NET daylight API — 0.15.0

`Xvarna.Native` exposes one source-unit-aware API on .NET 8 and .NET 10.

```csharp
OpticalMaterialLibrary optical = new(
    [new OpticalMaterial(1, OpticalMaterialKind.Glass,
        0.08, 0.08, 0.08, 0.65, 0.67, 0.70,
        RefractiveIndex: 1.52)],
    [new MaterialAssignment(ObjectId: 17, MaterialId: 1)]);

DaylightSensor[] sensors =
[
    new(1, 0, 0, 0.8, 0, 0, 1, Area: 1.0),
];

DaylightMatrixOptions matrix = new(SkyPatchCount: 2304);
DaylightMoment moment = new(DateTimeOffset.UtcNow, 0.4, 0.2, 0.89, 80000, 12000);

PointIlluminanceResult point = await scene.AnalyzePointIlluminanceAsync(
    sensors, optical, moment, matrix, cancellationToken);

DaylightFactorResult df = scene.AnalyzeDaylightFactor(
    sensors, optical, 10000, matrix);

using DaylightAnalysisJob<AnnualDaylightResult> job = scene.BeginAnnualDaylight(
    sensors, EpwWeatherFile.Load("weather.epw"), optical,
    AnnualDaylightOptions.Default with { Matrix = matrix }, cancellationToken);

AnnualIrradianceProgress progress = job.Progress;
AnnualDaylightResult annual = await job.Completion;

RadianceExportBundle bundle = scene.ExportRadiance(sensors, optical);
bundle.Write("radiance-bundle");

DaylightValidationReport agreement = DaylightValidation.Compare(
    fastLux, radianceLux, absoluteToleranceLux: 50, relativeTolerance: 0.10);
```

Sensor positions, offsets, maximum distances, and areas use the scene's source unit. The native ABI and Radiance bundle use metres. `MatrixReused`, `MatrixHash`, analysis duration, and `ContentHash` are first-class result provenance. Unassigned objects are opaque.

The managed connector intentionally exports Radiance bundles but does not bundle Radiance executables. The Rust/CLI runner performs cancellable external execution and records exact commands and tool versions.
