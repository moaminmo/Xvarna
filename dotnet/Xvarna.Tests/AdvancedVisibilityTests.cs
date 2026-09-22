using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class AdvancedVisibilityTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void TargetWeightedAndGreenViewCrossNativeBoundary()
    {
        using XvarnaScene scene = BuildDistantScene();
        ViewCamera observer = new(1, new(0, 0, 0), new(1, 0, 0), new(0, 0, 1));
        ViewTargetTriangle target = new(7,
            new(5000, -1000, -1000), new(5000, 0, 1000), new(5000, 1000, -1000), 2, 0.8);

        TargetViewResult result = scene.AnalyzeTargetView([observer], [target],
            TargetViewOptions.Default with
            {
                SamplesPerPatch = 64,
                GreenCategoryMask = 2,
                TwoSidedTargets = true,
                MaximumDistance = 100_000,
                DistanceReference = 25_000,
            });

        TargetViewSummary summary = Assert.Single(result.Summaries);
        Assert.True(summary.TargetViewFraction > 0.0);
        Assert.Equal(1.0, summary.GreenShareOfVisibleTargets, 12);
        Assert.True(summary.WeightedViewScore < summary.TargetViewFraction);
        Assert.Equal(7ul, summary.DominantTargetId);
        Assert.Single(result.Entries);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void CorridorReportsPartialConflictAndSourceObject()
    {
        using XvarnaScene scene = BuildDividerScene();
        ViewCorridorDefinition corridor = new(9, new(0, 0, 0), new(4000, 0, 0), new(0, 0, 1), 1500);

        ViewCorridorResult result = scene.AnalyzeViewCorridors([corridor], new(2048, 0.1, 1));

        ViewCorridorSummary summary = Assert.Single(result.Summaries);
        Assert.InRange(summary.OpenFraction, 0.01, 0.99);
        Assert.Equal(42ul, summary.DominantBlockerObjectId);
        Assert.Equal(2048, result.Samples.Count);
    }

    [Fact]
    public void ObserverPathReturnsOrderedDistanceWeightedViewSeries()
    {
        using XvarnaScene scene = BuildDistantScene();
        ObserverPathDefinition path = new()
        {
            PathId = 5,
            Vertices = [new(0, 0, 0), new(10000, 0, 0)],
            Up = new(0, 0, 1),
        };
        ViewTargetTriangle target = new(7,
            new(15000, -1000, -1000), new(15000, 0, 1000), new(15000, 1000, -1000), 2, 1.0);

        ObserverPathResult result = scene.AnalyzeObserverPaths([path], [target],
            new ObserverPathOptions(2000, TargetViewOptions.Default with
            {
                GreenCategoryMask = 2,
                TwoSidedTargets = true,
                MaximumDistance = 100_000,
                DistanceReference = 25_000,
            }));

        Assert.Equal(6, result.Samples.Count);
        ObserverPathSummary summary = Assert.Single(result.Summaries);
        Assert.Equal(10000.0, summary.PathLength, 8);
        Assert.True(summary.MeanGreenViewIndex > 0.0);
        Assert.True(summary.MaximumWeightedViewScore > summary.MinimumWeightedViewScore);
    }

    [Fact]
    public void CpuComputeSessionExecutesEveryAdvancedVisibilityDomain()
    {
        using XvarnaScene distantScene = BuildDistantScene();
        using XvarnaComputeSession distantCompute = XvarnaComputeSession.Create(
            distantScene,
            new ComputeOptions(Preference: ComputePreference.Cpu));
        ViewCamera observer = new(1, new(0, 0, 0), new(1, 0, 0), new(0, 0, 1));
        ViewTargetTriangle target = new(7,
            new(5000, -1000, -1000), new(5000, 0, 1000), new(5000, 1000, -1000), 2, 0.8);
        TargetViewOptions viewOptions = TargetViewOptions.Default with
        {
            SamplesPerPatch = 64,
            GreenCategoryMask = 2,
            TwoSidedTargets = true,
            MaximumDistance = 100_000,
            DistanceReference = 25_000,
        };

        TargetViewResult targetResult = distantCompute.AnalyzeTargetView([observer], [target], viewOptions);
        DaenaExecutionReport targetExecution = Assert.IsType<DaenaExecutionReport>(targetResult.Execution);
        Assert.Equal(DaenaExecutionBackend.Cpu, targetExecution.Backend);
        Assert.Equal(1ul, targetExecution.BatchCount);
        Assert.Equal(64ul, targetExecution.RayCount);
        Assert.Equal(0ul, targetExecution.DispatchCount);

        ObserverPathDefinition path = new()
        {
            PathId = 5,
            Vertices = [new(0, 0, 0), new(4000, 0, 0)],
            Up = new(0, 0, 1),
        };
        ObserverPathResult pathResult = distantCompute.AnalyzeObserverPaths(
            [path], [target], new ObserverPathOptions(2000, viewOptions));
        DaenaExecutionReport pathExecution = Assert.IsType<DaenaExecutionReport>(pathResult.Execution);
        Assert.Equal(DaenaExecutionBackend.Cpu, pathExecution.Backend);
        Assert.Equal(1ul, pathExecution.BatchCount);
        Assert.InRange(pathExecution.RayCount, 1ul, 192ul);

        using XvarnaScene dividerScene = BuildDividerScene();
        using XvarnaComputeSession dividerCompute = XvarnaComputeSession.Create(
            dividerScene,
            new ComputeOptions(Preference: ComputePreference.Cpu));
        ViewCorridorResult corridorResult = dividerCompute.AnalyzeViewCorridors(
            [new ViewCorridorDefinition(9, new(0, 0, 0), new(4000, 0, 0), new(0, 0, 1), 1500)],
            new ViewCorridorOptions(256, 0.1, 1));
        DaenaExecutionReport corridorExecution = Assert.IsType<DaenaExecutionReport>(corridorResult.Execution);
        Assert.Equal(DaenaExecutionBackend.Cpu, corridorExecution.Backend);
        Assert.Equal(1ul, corridorExecution.BatchCount);
        Assert.Equal(256ul, corridorExecution.RayCount);
    }

    [Fact]
    public void PortableGpuExecutesTargetViewWhenAnAdapterIsAvailable()
    {
        ComputeAdapterInfo? adapter = XvarnaComputeSession.EnumerateAdapters().FirstOrDefault();
        if (adapter is null)
        {
            return;
        }

        using XvarnaScene scene = BuildDistantScene();
        XvarnaComputeSession compute;
        try
        {
            compute = XvarnaComputeSession.Create(scene, new ComputeOptions(
                Preference: ComputePreference.RequirePortableGpu,
                MaximumPrecisionErrorMeters: 0.001,
                MaximumRaysPerDispatch: 16,
                AdapterNameFilter: adapter.Name));
        }
        catch (XvarnaNativeException exception) when (exception.StatusCode == 5)
        {
            return;
        }
        using (compute)
        {
            ViewCamera[] observers = [new(1, new(0, 0, 0), new(1, 0, 0), new(0, 0, 1))];
            ViewTargetTriangle[] targets = [new(7,
                new(5000, -1000, -1000), new(5000, 0, 1000), new(5000, 1000, -1000), 2, 1.0)];
            TargetViewOptions options = TargetViewOptions.Default with
            {
                SamplesPerPatch = 64,
                GreenCategoryMask = 2,
                TwoSidedTargets = true,
                MaximumDistance = 100_000,
            };
            TargetViewResult reference = scene.AnalyzeTargetView(observers, targets, options);
            TargetViewResult result = compute.AnalyzeTargetView(observers, targets, options);

            DaenaExecutionReport execution = Assert.IsType<DaenaExecutionReport>(result.Execution);
            Assert.Equal(DaenaExecutionBackend.PortableGpu, execution.Backend);
            Assert.Equal(adapter.Name, execution.AdapterName);
            Assert.Equal(64ul, execution.RayCount);
            Assert.Equal(4ul, execution.DispatchCount);
            Assert.Equal(0ul, execution.RuntimeFallbackBatchCount);
            Assert.True(execution.UsedReusableBuffers);
            Assert.Equal(reference.ContentHash, result.ContentHash);
            Assert.Equal(reference.Entries[0].VisibleSampleCount, result.Entries[0].VisibleSampleCount);
            Assert.Equal(reference.Entries[0].DominantBlockerObjectId, result.Entries[0].DominantBlockerObjectId);
            Assert.InRange(
                Math.Abs(reference.Summaries[0].WeightedViewScore - result.Summaries[0].WeightedViewScore),
                0.0,
                1.0e-12);
        }
    }

    private static XvarnaScene BuildDistantScene()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [-100000, -100000, -100000, -100000, 100000, -100000, -100000, 0, 100000],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 99, 1, 1);
        return builder.Build();
    }

    private static XvarnaScene BuildDividerScene()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [2000, -500, -500, 2000, 500, -500, 2000, 500, 500, 2000, -500, 500],
            [0, 1, 2, 0, 2, 3]);
        builder.AddInstance(mesh, Identity, 42, 1, 1);
        return builder.Build();
    }
}
