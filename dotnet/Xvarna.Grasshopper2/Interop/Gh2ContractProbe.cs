using Xvarna.Grasshopper2.Components;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Interop;

/// <summary>Runs a small end-to-end connector contract without requiring a canvas document.</summary>
public static class Gh2ContractProbe
{
    /// <summary>Validates every tracked Grasshopper 2 request example against its runtime DTO.</summary>
    public static string ValidateExamples(string repositoryRoot)
    {
        string root = Path.Combine(repositoryRoot, "examples", "grasshopper2");
        IsovistRequest isovist = Gh2Json.Read<IsovistRequest>(File.ReadAllText(Path.Combine(root, "isovist-request.json")));
        TargetViewRequest view = Gh2Json.Read<TargetViewRequest>(File.ReadAllText(Path.Combine(root, "target-view-request.json")));
        AnnualDaylightRequest daylight = Gh2Json.Read<AnnualDaylightRequest>(File.ReadAllText(Path.Combine(root, "annual-daylight-request.json")));
        StudyRankRequest study = Gh2Json.Read<StudyRankRequest>(File.ReadAllText(Path.Combine(root, "study-rank-request.json")));
        if (isovist.Viewpoints.Count == 0 || view.Cameras.Count == 0 || view.Targets.Count == 0
            || daylight.Sensors.Count == 0 || study.Candidates.Count == 0 || study.Evaluations.Count == 0)
            throw new InvalidOperationException("One or more GH2 examples deserialize to an empty request.");
        return $"isovist {isovist.Viewpoints.Count}; target view {view.Cameras.Count}×{view.Targets.Count}; daylight {daylight.Sensors.Count}; study {study.Candidates.Count}";
    }

    /// <summary>Validates native loading, scene traversal and evidence sealing in the GH2 load context.</summary>
    public static string Run()
    {
        double[] positions = [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0];
        uint[] triangles = [0, 1, 2, 0, 2, 3];
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(1.0, 1e-6, 4, 1));
        ulong meshId = builder.AddMesh(positions, triangles);
        builder.AddInstance(meshId, Identity, 7, 1, ulong.MaxValue, SceneLayer.Static);
        using XvarnaScene scene = builder.Build();
        SceneHit hit = scene.TraceClosest([new SceneRay(0.25, 0.25, 1.0, 0, 0, -1, 0, 10, ulong.MaxValue)])[0];
        if (!hit.Hit || hit.ObjectId != 7 || Math.Abs(hit.Distance - 1.0) > 1e-10)
            throw new InvalidOperationException("GH2 geometry-to-native ray contract returned an unexpected result.");
        using XvarnaComputeSession compute = XvarnaComputeSession.Create(
            scene,
            ComputeOptions.Default with { Preference = ComputePreference.Cpu });
        ComputeTraceResult computed = compute.TraceClosest(
            [new SceneRay(0.25, 0.25, 1.0, 0, 0, -1, 0, 10, ulong.MaxValue)]);
        if (computed.Statistics.Backend != ComputeBackend.Cpu || !computed.Hits[0].Hit || computed.Hits[0].ObjectId != 7)
            throw new InvalidOperationException("GH2 VAYU-backend contract failed to preserve ordered CPU attribution.");

        SurfaceGridResult grid = SurfaceGridGenerator.Generate(positions, triangles, new(0.5, 0.01, 100, 1000));
        if (Math.Abs(grid.SampledArea - 1.0) > 1e-12 || grid.Cells.Count <= 2)
            throw new InvalidOperationException("GH2 surface-grid contract did not preserve the analytic unit area.");

        SkyViewResult sky = scene.AnalyzeSkyView(
            [new SkySensor(1, 0.5, 0.5, 1, 0, 0, 1)],
            new SkyViewOptions(64, 42, 1e-4, 100, ulong.MaxValue));
        if (Math.Abs(sky.Summaries[0].CosineWeightedSkyViewFactor - 1.0) > 1e-12)
            throw new InvalidOperationException("GH2 sky-view contract failed on the analytic open hemisphere.");

        DaylightSensor[] daylightSensors = [new(1, 0.5, 0.5, 1, 0, 0, 1, 1)];
        DaylightMoment moment = new(DateTimeOffset.FromUnixTimeSeconds(1_718_966_400), 0, 0, 1, 50_000, 10_000);
        PointIlluminanceResult daylight = scene.AnalyzePointIlluminance(
            daylightSensors, OpticalMaterialLibrary.Opaque, moment,
            new DaylightMatrixOptions(SkyPatchCount: 64));
        if (Math.Abs(daylight.Entries[0].TotalLux - 60_000) > 1e-7)
            throw new InvalidOperationException("GH2 daylight contract failed on the analytic open-sky case.");

        TargetViewRequest viewRequest = new()
        {
            Cameras = [new ViewCamera(1, new(0, 0, 1), new(1, 0, 0), new(0, 0, 1), 1)],
            Targets = [new ViewTargetTriangle(9, new(2, -0.5, 0.5), new(2, 0, 1.5), new(2, 0.5, 0.5), 2, 1)],
            Options = TargetViewOptions.Default with { SamplesPerPatch = 16, TwoSidedTargets = true, GreenCategoryMask = 2 },
        };
        TargetViewRequest decodedView = Gh2Json.Read<TargetViewRequest>(Gh2Json.Write(viewRequest));
        TargetViewResult view = scene.AnalyzeTargetView(decodedView.Cameras, decodedView.Targets, decodedView.Options);
        if (view.Summaries.Count != 1 || view.Summaries[0].TargetViewFraction <= 0)
            throw new InvalidOperationException("GH2 target-view JSON/native contract returned no visible target.");

        double[] moved = (double[])Identity.Clone();
        moved[3] = 10;
        SceneUpdateReport update = scene.ApplyDeltas([SceneInstanceDelta.Update(1, rowMajorTransform: moved)]);
        if (update.StaticTlasUpdate != HierarchyUpdateKind.Refit || update.DynamicTlasUpdate != HierarchyUpdateKind.Reused
            || update.ReusedBlasCount != 1 || update.RebuiltBlasCount != 0 || scene.Revision != 1)
            throw new InvalidOperationException("GH2 scene-delta contract did not select the expected TLAS refit/BLAS reuse path.");
        if (scene.TraceClosest([new SceneRay(0.25, 0.25, 1.0, 0, 0, -1, 0, 10, ulong.MaxValue)])[0].Hit)
            throw new InvalidOperationException("GH2 scene-delta contract did not commit the moved occurrence.");

        StudyProblem problem = new()
        {
            Variables = [new(1, DesignVariableKind.Continuous, 0, 1)],
            ObjectiveDirections = [ObjectiveDirection.Minimize, ObjectiveDirection.Maximize],
            ConstraintCount = 0,
        };
        StudyCandidate[] candidates =
        [
            new() { CandidateId = 1, Parameters = [0.1] },
            new() { CandidateId = 2, Parameters = [0.9] },
        ];
        StudyEvaluation[] evaluations =
        [
            new() { CandidateId = 1, Objectives = [0.1, 0.2] },
            new() { CandidateId = 2, Objectives = [0.9, 0.8] },
        ];
        RankedStudyResult ranked = StudyAnalyzer.Rank(problem, candidates, evaluations);
        if (ranked.ParetoCandidateIds.Count != 2)
            throw new InvalidOperationException("GH2 Study contract failed to preserve the analytic Pareto trade-off.");
        EvidencePassport evidence = EvidenceEngine.Seal(new EvidencePassport
        {
            ResultHash = scene.Statistics.ContentHash,
            SceneHash = scene.Statistics.ContentHash,
            Method = "GH2 contract probe",
            Fidelity = EvidenceFidelity.Reference,
            Backend = "CPU f64",
            SampleCount = 1,
            ElapsedMicroseconds = scene.Statistics.BuildTimeMicroseconds,
            Assumptions = ["Analytic unit quad"],
            Limitations = ["Connector contract probe, not a performance benchmark"],
            Validation = new EvidenceValidation { ParityTested = true, MaximumParityDelta = 0 },
        });
        if (evidence.ContentHash.Length != 64) throw new InvalidOperationException("Evidence passport was not sealed.");
        return $"scene {scene.Statistics.ContentHash}; ray object {hit.ObjectId} at {hit.Distance:G17}; VAYU {computed.Statistics.Backend}; delta {update.StaticTlasUpdate}/revision {scene.Revision}; " +
            $"grid {grid.Cells.Count} cells; SVF {sky.Summaries[0].CosineWeightedSkyViewFactor:G17}; " +
            $"daylight {daylight.Entries[0].TotalLux:G17} lux; target view {view.Summaries[0].TargetViewFraction:G17}; " +
            $"Pareto {ranked.ParetoCandidateIds.Count}; evidence {evidence.ContentHash}";
    }

    private static readonly double[] Identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
}
