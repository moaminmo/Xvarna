# XVARNA 0.16 native and managed SDK

## Managed daylight and Radiance interchange

The 0.19 SDK retains the typed RASHNU Evidence Passports, generated Saltelli/Jansen and Morris experiments, robust uncertainty analysis, and cost-aware Fast/Reference scheduling introduced in 0.18. The same managed contracts now drive both 49-component GH1 and GH2 connectors; see [the Evidence API guide](sdk/evidence.md) and [GH2 connector validation](validation/grasshopper2-connector.md). The 0.16 `StudyPlatformWorkspace` retains durable external batches, exact checkpoint/resume, scalar/vector result commit, baseline delta maps, and JSON/Parquet/glTF/HTML reporting; see [the Study platform API guide](sdk/study-platform.md). Daylight/Radiance APIs remain documented in [the daylight API guide](sdk/daylight.md).

This Windows x64 SDK exposes the same Rust engine used by the Rhino packages and standalone CLI. It is intended for custom .NET applications, C/C++ hosts, research pipelines, and future host connectors.

## Contents

```text
include/xvarna.h                 Stable C ABI declarations
native/win-x64/xvarna_core.dll   Rust 1.96 MSVC native engine
managed/net8.0/Xvarna.Native.dll .NET 8 API
managed/net10.0/Xvarna.Native.dll .NET 10 API
```

Keep `xvarna_core.dll` beside the consuming executable or in an explicit native-library search path. Choose exactly one managed target matching the application runtime.

## Incremental scene lifecycle

Built scenes are immutable snapshots. A delta transaction creates a new snapshot while the old one stays queryable, shares unchanged BLAS resources, and independently refits or rebuilds the static and dynamic TLAS layers according to a declared quality policy:

```csharp
SceneUpdateReport update = scene.ApplyDeltas(
    [SceneInstanceDelta.Update(
        instanceId: 7,
        rowMajorTransform: movedTransform,
        layer: SceneLayer.Dynamic)],
    SceneUpdatePolicy.Default);

Console.WriteLine($"{update.DynamicTlasUpdate}; " +
                  $"reused BLAS={update.ReusedBlasCount}; {update.CurrentHash}");
```

Use `SceneInstanceDelta.Add`, `Update`, and `Remove` for atomic instance changes. `ReplaceMesh` is the explicit geometry-change path and rebuilds only the affected BLAS before producing a new snapshot. `SceneUpdateReport` records layer changes, BLAS reuse/rebuild, TLAS decisions, refit-quality ratio, time, and before/after hashes.

## Cache and shared scheduler

```csharp
XvarnaCache.Configure(new CacheConfiguration(
    @"D:\xvarna-cache",
    MemoryBudgetBytes: 256UL * 1024 * 1024,
    DiskBudgetBytes: 4UL * 1024 * 1024 * 1024));
CacheStatistics cache = XvarnaCache.GetStatistics();

XvarnaAnalysisScheduler.Configure(new AnalysisSchedulerOptions(
    MaximumConcurrency: Math.Max(1, Environment.ProcessorCount - 1),
    MaximumQueuedJobs: 256));
using ScheduledAnalysis<MyResult> job = XvarnaAnalysisScheduler.Schedule(
    "My analysis", "document/component-guid", 100,
    (cancellation, progress) => RunAnalysis(cancellation, progress));
MyResult result = await job.Completion;
```

Cache files carry a schema, key, length, and BLAKE3 checksum; writes use a same-directory temporary file plus atomic rename. Invalid entries are rejected and removed. Both memory and disk tiers enforce byte budgets and expose hits, misses, writes, eviction, corruption, occupancy, and policy telemetry. `Clear` removes only XVARNA `.xvc` entries.

The managed scheduler bounds concurrent and queued work, links cancellation tokens, reports phase/fraction progress, and suppresses a completed result if a newer generation for the same scope exists. Queue cancellation is immediate; work already submitted to a native/GPU phase observes cancellation at its next safe boundary and any superseded output is discarded.

## Managed VAYU compute

```csharp
using Xvarna.Native;

IReadOnlyList<ComputeAdapterInfo> devices =
    XvarnaComputeSession.EnumerateAdapters();

using XvarnaComputeSession compute = XvarnaComputeSession.Create(
    scene,
    new ComputeOptions(
        Preference: ComputePreference.Auto,
        MaximumPrecisionErrorMeters: 0.00025,
        MaximumExpandedTriangleCount: 20_000_000,
        MaximumRaysPerDispatch: 1_048_576,
        MaximumGeometryChunkBytes: 512UL * 1024 * 1024,
        MaximumGpuMemoryBytes: 1024UL * 1024 * 1024,
        EnableSceneCache: true,
        AdapterNameFilter: "NVIDIA"));

ComputeTraceResult closest = compute.TraceClosest(rays);
ComputeVisibilityResult visible = compute.TraceAny(rays);
ComputeRuntimeStatistics runtime = compute.GetRuntimeStatistics();

Console.WriteLine($"{compute.Info.Backend}: {compute.Info.AdapterName}");
Console.WriteLine($"{closest.Statistics.DispatchCount} dispatches; " +
                  $"reused={closest.Statistics.UsedReusableBuffers}; " +
                  $"fallback={closest.Statistics.UsedRuntimeFallback}");
Console.WriteLine($"arena={runtime.ScratchGpuBytes / 1_048_576.0:N2} MiB; " +
                  $"generations={runtime.ScratchGenerationCount}; healthy={runtime.DeviceHealthy}");
```

The session retains native scene ownership and can be reused for ordered batches. VAYU preserves the scene's two-level BLAS/instance structure and streams deterministic geometry chunks when the declared geometry or VRAM budget requires it. Its two-slot arena retains dynamic buffers and bind groups after warm-up; runtime statistics are cumulative and do not reset or synchronize the device. `ComputeSessionInfo` exposes chunk count, cache hit, streaming, identity counts, and payload bytes; runtime statistics expose geometry uploads and host snapshot memory. `Auto` exposes creation and runtime fallback in immutable provenance; `RequirePortableGpu` returns an error rather than changing backend; `Cpu` selects the canonical f64 implementation. Source coordinates, distances, and tolerances remain in scene model units at the managed API while the precision budget is explicitly in metres. See [the portable compute contract](validation/portable-gpu.md), [throughput contract](validation/vayu-production-throughput.md), and [0.13 lifecycle contract](validation/zamyad-vayu-lifecycle.md).

## Managed DAENA visibility

```csharp
PlanarViewpoint eye = new(
    ViewpointId: 1,
    PositionX: 0, PositionY: 0, PositionZ: 1.6,
    PlaneNormalX: 0, PlaneNormalY: 0, PlaneNormalZ: 1,
    ForwardX: 1, ForwardY: 0, ForwardZ: 0);

IsovistResult isovist = scene.AnalyzeIsovists(
    [eye],
    new IsovistOptions(SampleCount: 1440, MaximumDistance: 50));

IntervisibilityResult privacy = scene.AnalyzeIntervisibility(
    [new VisibilityObserverPoint(1, 0, 0, 1.6, Weight: 1)],
    [new VisibilityTargetPoint(1, 10, 0, 1.6,
        Sensitivity: 0.9, HasFacing: true, FacingX: -1)],
    new IntervisibilityOptions(PrivacyReferenceDistance: 8));

VisibilityGraphResult graph = scene.AnalyzeVisibilityGraph(
    [new(1, 0, 0, 1.6), new(2, 2, 0, 1.6), new(3, 4, 0, 1.6)],
    new VisibilityGraphOptions(MaximumDistance: 25));
```

All positions and public distances use the scene's source model unit; isovist area and area convergence use its squared unit. Results retain complete rays/pairs, first blockers, aggregates, runtime, and deterministic identity. See [the scientific contract](validation/spatial-visibility-and-privacy.md).

## Managed DAENA/HVARE intelligence

```csharp
AnalysisMaterialLibrary materials = new(
    [new AnalysisMaterial(1, VisibleTransmittance: 0.7,
        SolarTransmittance: 0.5, Reflectance: 0.2)],
    [new MaterialAssignment(ObjectId: 17, MaterialId: 1)]);

Isovist3dResult spatial = scene.AnalyzeIsovists3d(
    [new SpatialViewpoint(1, 0, 0, 1.6)], materials,
    new Isovist3dOptions());

LandmarkVisibilityResult landmark = scene.AnalyzeLandmarkVisibility(
    [new LandmarkObserver(1, 0, 0, 1.6, 1, 0, 0, 0, 0, 1)],
    [new Landmark(7, 20, 0, 8, Radius: 2, Weight: 1)],
    materials, new LandmarkVisibilityOptions());

SpatialScenarioComparison delta = DaenaScenarios.Compare(
    "baseline", spatial, "candidate", candidateSpatial);
```

`CompareSolarScenarios` accepts named `SolarMaterialScenario` rows and returns a complete scenario/sensor/time matrix plus loss attribution and deltas. `AnalyzeSolarEnvelope` accepts `EnvelopeCandidate` columns and returns access/shading thresholds plus controlling sensor/time records. Managed coordinates and returned lengths remain in scene source units; the native ABI uses canonical metres. See [the 0.14 scientific contract](validation/daena-spatial-solar-intelligence.md).

## Managed surface grid and PV proxy

```csharp
using Xvarna.Native;

SurfaceGridResult grid = SurfaceGridGenerator.Generate(
    positionsXYZ,
    triangleIndices,
    new SurfaceGridOptions(TargetEdgeLength: 0.5, SensorOffset: 0.001));

Dictionary<ulong, SensorIrradianceSummary> byId = annual.Summaries
    .ToDictionary(x => x.SensorId);
double[] annualWhM2 = grid.Cells
    .Select(cell => byId[cell.SensorId].GlobalWhM2)
    .ToArray();

PvPotentialResult potential = SurfacePotentialAnalyzer.Analyze(
    grid,
    annualWhM2,
    new PvPotentialOptions(
        MinimumIrradianceKWhM2: 800,
        ModuleEfficiency: 0.22,
        CoverageRatio: 0.85,
        SystemLossFraction: 0.14),
    unitScaleToMeters: 1.0);
```

Grid coordinates remain in source model units; `unitScaleToMeters` converts cell area to m² for energy and capacity math. The result exposes aligned cells, edge-connected regions, weighted mean/P10/P50/P90, area, incident energy, kWp, kWh, kWh/kWp, assumptions, and hash. Treat the output as a screening proxy, not bankable PV yield.

## Managed annual irradiance

```csharp
using Xvarna.Native;

EpwWeatherFile weather = EpwWeatherFile.Load("weather.epw");
SolarSensor[] sensors =
[
    new(1, 0.0, 0.0, 1.2, 0.0, 0.0, 1.0),
];

using AnnualIrradianceAnalysisJob job = scene.BeginAnnualIrradiance(
    sensors,
    weather,
    AnnualIrradianceOptions.Default,
    cancellationToken);

AnnualIrradianceProgress progress = job.Progress;
AnnualIrradianceResult annual = await job.Completion;
double globalKWhM2 = annual.Summaries[0].GlobalWhM2 / 1000.0;
IrradianceTimelineEntry firstInterval = annual.Timeline[0];
```

`EpwWeatherFile.Parse` accepts in-memory text. `AnalyzeAnnualIrradiance` and `AnalyzeAnnualIrradianceAsync` provide simpler sync/async entry points. The result retains annual totals, peak time/load, dome/horizon visibility, dominant obstruction metrics, full component timeline, first-hit source identity, EPW diagnostics, and deterministic hashes.

## Managed Sky View

```csharp
using Xvarna.Native;

using XvarnaSceneBuilder builder = new(new SceneBuildOptions(
    UnitScaleToMeters: 1.0,
    AbsoluteToleranceMeters: 1e-6,
    ThreadCount: 0));

ulong mesh = builder.AddMesh(positionsXYZ, triangleIndices);
builder.AddInstance(mesh, rowMajorTransform, objectId: 10, instanceId: 1);
using XvarnaScene scene = builder.Build();

SkySensor[] sensors =
[
    new(1, 0.0, 0.0, 1.2, 0.0, 0.0, 1.0),
];

using SkyViewAnalysisJob job = scene.BeginSkyView(
    sensors,
    SkyViewOptions.Default with { SampleCount = 8192 },
    cancellationToken);

SkyViewProgress progress = job.Progress;
SkyViewResult result = await job.Completion;
double svf = result.Summaries[0].CosineWeightedSkyViewFactor;
```

`AnalyzeSkyView` and `AnalyzeSkyViewAsync` are simpler alternatives when progress polling is unnecessary. Positions and distances use the source model unit declared by `UnitScaleToMeters`; the native core stores canonical metres.

## Managed advanced view

```csharp
ViewCamera camera = new(1, new(0, 0, 1.6), new(1, 0, 0), new(0, 0, 1));
ViewTargetTriangle tree = new(7,
    new(15, -2, 0), new(15, 0, 4), new(15, 2, 0),
    CategoryMask: 2, CategoryWeight: 0.9);

using XvarnaComputeSession compute = XvarnaComputeSession.Create(
    scene,
    new ComputeOptions(
        Preference: ComputePreference.Auto,
        MaximumPrecisionErrorMeters: 0.00025));

TargetViewResult view = compute.AnalyzeTargetView(
    [camera], [tree],
    TargetViewOptions.Default with
    {
        GreenCategoryMask = 2,
        SamplesPerPatch = 64,
        TwoSidedTargets = true,
    });

double rawTargetView = view.Summaries[0].TargetViewFraction;
double weightedView = view.Summaries[0].WeightedViewScore;
double greenView = view.Summaries[0].GreenViewIndex;
DaenaExecutionReport execution = view.Execution!;
```

The same methods remain available directly on `XvarnaScene` for canonical CPU execution. `XvarnaComputeSession.AnalyzeViewCorridors` and `AnalyzeObserverPaths` expose accelerated protected-aperture and moving-camera workflows. `Execution` records the aggregate backend, adapter, batches/rays/dispatches, upload/execute/readback timing, maximum precision error, and fallback state. All source positions and distances use the scene's source model units; direction vectors and steradians are dimensionless.

## Managed VAHMAN Study

```csharp
StudyProblem problem = new()
{
    Variables =
    [
        new(1, DesignVariableKind.Continuous, 0, 1),
        new(2, DesignVariableKind.Integer, 1, 20),
    ],
    ObjectiveDirections = [ObjectiveDirection.Minimize, ObjectiveDirection.Maximize],
    ConstraintCount = 1,
};

using XvarnaOptimizer optimizer = new(problem,
    MultiObjectiveOptimizerOptions.Default with { Seed = 42 });

OptimizerBatch batch = optimizer.Ask();
// Evaluate every batch.Candidates item through the application model.
OptimizerSnapshot state = optimizer.Tell(evaluations);
```

`StudyAnalyzer.Rank`, `SpearmanSensitivity`, and `Hypervolume2d` operate on completed study tables. `XvarnaOptimizer` owns a safe native handle; every `Ask` must be followed by exactly one aligned `Tell` before another `Ask`.

## Persistent VAHMAN Study platform

`StudyPlatformWorkspace.Create` validates a typed `StudyPlatformManifest` against the native 0.16 registry contract. `BeginBatch` returns or resumes an exact journaled external batch; `CommitBatch` atomically accepts complete grouped scalar/vector objectives, residual constraints, optional metric fields, runtime, and provenance. `GenerateReport` runs constraint-aware ranking, bootstrap/permutation/Holm-qualified rank sensitivity, baseline comparison, and standards-based export.

The corresponding native functions are `xv_study_manifest_schema_json`, `xv_study_workspace_create`, `xv_study_workspace_status`, `xv_study_workspace_begin_batch`, `xv_study_workspace_commit_batch`, and `xv_study_workspace_report`. They use the standard two-call UTF-8 sizing contract and caller-owned paths/JSON. See [the full API and lifecycle guide](sdk/study-platform.md).

## C ABI lifecycle

1. Compare `xv_abi_version_*` with the required ABI (`0.16.x`).
2. Create a scene with `xv_scene_create`.
3. Add mesh resources and static/dynamic instances, then freeze it with `xv_scene_build`.
4. Create an `xv_job_handle`, allocate summary/timeline buffers, and call `xv_scene_sky_view`.
5. Poll `xv_job_progress` or call `xv_job_cancel` from another thread.
6. Release job and scene handles exactly once.

For incremental updates, submit a complete `xv_scene_instance_delta` array to `xv_scene_apply_instance_deltas`, or replace one mesh through `xv_scene_replace_mesh`. Both atomically publish the new immutable snapshot under the existing handle and return `xv_scene_update_report` plus current statistics. Queries that already cloned the prior snapshot finish safely against it; later calls through the handle see the new revision. The policy controls maximum refit-quality degradation and periodic rebuild count.

For portable compute, call `xv_compute_enumerate_adapters`, create an immutable session with `xv_compute_create` or the case-insensitive UTF-8-filtered `xv_compute_create_for_adapter`, reuse it through raw `xv_compute_trace_closest` / `xv_compute_trace_any` or high-level `xv_compute_target_view` / `xv_compute_view_corridor` / `xv_compute_observer_path`, inspect cumulative health/reuse/transfer counters with `xv_compute_get_runtime_stats`, then call `xv_compute_release` exactly once. `xv_compute_options` includes geometry-chunk, GPU-memory, and scene-cache policy. Raw traces return `xv_compute_batch_stats`; domain calls return `xv_daena_execution_info` with aggregate backend, adapter, dispatch, transfer, precision, fallback, and persistent-arena provenance. A required-GPU session never silently executes on CPU.

Configure process-wide cache policy with `xv_cache_configure`, read the fixed telemetry contract with `xv_cache_get_stats`, and use `xv_cache_clear` for scoped `.xvc` removal. Cache functions are thread-safe and never grant ownership of caller strings.

For annual analysis, call `xv_epw_inspect` first to obtain `weather_count`, allocate `sensor_count * weather_count` timeline entries, then call `xv_scene_annual_irradiance` with a job handle. The EPW buffer is caller-owned UTF-8 and only needs to remain valid for the duration of each call.

`xv_surface_grid` and `xv_surface_pv_potential` use a two-pass caller-owned-buffer contract. Call first with null variable-length outputs and zero capacities to receive counts in metadata, allocate exact buffers, then call again. Surface-grid area uses squared input model units; PV options declare `unit_scale_to_meters`, and all PV area/energy/capacity outputs use m²/kWh/kWp.

Every extensible output begins with `structure_size`; validate it before interpreting the rest of the structure. Functions return `xv_status`, contain Rust panics at the ABI boundary, and never transfer ownership of caller buffers. `XV_STATUS_CANCELLED` is a contained cooperative result, not a crash.

DAENA output dimensions are known from the inputs: isovist rays are `viewpoint_count * sample_count`, directed entries are `observer_count * target_count`, and graph pairs are `node_count * (node_count - 1) / 2`. Allocate exact caller-owned buffers, then call `xv_scene_isovist`, `xv_scene_intervisibility`, or `xv_scene_visibility_graph`.

Advanced DAENA adds `xv_scene_target_view`, `xv_scene_view_corridor`, and `xv_scene_observer_path`. VAHMAN completed-study functions are stateless. Optimizer handles follow `xv_optimizer_create` → alternating `xv_optimizer_ask`/`xv_optimizer_tell` → `xv_optimizer_release`; the caller owns all row-major buffers.

## Scientific semantics

Annual EPW/Perez equations, shading ratios, timestamp policy, attribution, tests, and limitations are specified in `docs/validation/annual-irradiance.md`. Sky View semantics remain in `docs/validation/sky-view-and-shadow-mask.md`; Direct Sun and solar-position semantics are specified separately. Do not relabel annual plane-of-array irradiation as daylight compliance, illuminance, thermal load, photovoltaic yield, or a multi-bounce Radiance result.

The SDK is Apache-2.0 licensed. Preserve `LICENSE` and `NOTICE` when redistributing binaries.
