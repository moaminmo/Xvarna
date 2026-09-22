using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class SkyTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public async Task SkyViewCrossesNativeManagedAndAsyncBoundaries()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        SkySensor sensor = new(5, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);

        SkyViewResult result = await scene.AnalyzeSkyViewAsync(
            [sensor],
            SkyViewOptions.Default with { SampleCount = 1024, Seed = 42 });

        SensorSkySummary summary = Assert.Single(result.Summaries);
        Assert.Equal(1.0, summary.CosineWeightedSkyViewFactor, 12);
        Assert.Equal(1.0, summary.VisibleHemisphereFraction, 12);
        Assert.Equal(1024ul, summary.VisibleCount);
        Assert.Equal(0ul, summary.BlockedCount);
        Assert.Equal(1024, result.Timeline.Count);
        Assert.All(result.Timeline, entry => Assert.Equal(SkyRayState.Visible, entry.State));
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void SkyViewReportsFullBlockerAttributionAndModelUnitDistance()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        const double extent = 1_000_000_000.0;
        ulong meshId = builder.AddMesh(
            [
                -extent, -extent, 1000.0,
                extent, -extent, 1000.0,
                extent, extent, 1000.0,
                -extent, extent, 1000.0,
            ],
            [0, 1, 2, 0, 2, 3]);
        builder.AddInstance(meshId, Identity, objectId: 99, instanceId: 7, categoryMask: 4);
        using XvarnaScene scene = builder.Build();
        SkySensor sensor = new(11, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);

        SkyViewResult result = scene.AnalyzeSkyView(
            [sensor],
            new SkyViewOptions(1024, 7, 0.1, double.PositiveInfinity, 4));

        SensorSkySummary summary = Assert.Single(result.Summaries);
        Assert.Equal(0.0, summary.CosineWeightedSkyViewFactor, 12);
        Assert.Equal(0.0, summary.VisibleHemisphereFraction, 12);
        Assert.Equal(99ul, summary.DominantOccluderObjectId);
        Assert.Equal(1.0, summary.DominantOccluderProjectedFraction, 12);
        Assert.All(result.Timeline, entry =>
        {
            Assert.Equal(SkyRayState.Blocked, entry.State);
            Assert.Equal(99ul, entry.ObjectId);
            Assert.Equal(7ul, entry.InstanceId);
            Assert.True(entry.Distance >= 999.9);
        });
    }

    [Fact]
    public async Task SkyViewObservesCancellationToken()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        using CancellationTokenSource cancellation = new();
        cancellation.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            scene.AnalyzeSkyViewAsync(
                [new SkySensor(1, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0)],
                SkyViewOptions.Default,
                cancellation.Token));
    }

    [Fact]
    public async Task ObservableSkyJobFinishesWithExactProgress()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        using SkyViewAnalysisJob job = scene.BeginSkyView(
            [new SkySensor(1, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0)],
            SkyViewOptions.Default with { SampleCount = 4096 });

        SkyViewResult result = await job.Completion;
        SkyViewProgress progress = job.Progress;

        Assert.Equal(4096, result.Timeline.Count);
        Assert.Equal(4096ul, progress.TotalRays);
        Assert.Equal(4096ul, progress.CompletedRays);
        Assert.Equal(1.0, progress.Fraction);
        Assert.False(progress.CancellationRequested);
    }

    [Fact]
    public void SkyViewRejectsInvalidSamplingAndSensors()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        SkySensor valid = new(1, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        Assert.Throws<XvarnaNativeException>(() =>
            scene.AnalyzeSkyView([valid], SkyViewOptions.Default with { SampleCount = 8 }));
        Assert.Throws<XvarnaNativeException>(() =>
            scene.AnalyzeSkyView(
                [valid with { NormalZ = 0.0 }],
                SkyViewOptions.Default));
    }

    private static XvarnaScene BuildOpenMillimetreScene()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong meshId = builder.AddMesh(
            [
                -1000.0, -1000.0, -1000.0,
                1000.0, -1000.0, -1000.0,
                0.0, 1000.0, -1000.0,
            ],
            [0, 1, 2]);
        builder.AddInstance(meshId, Identity, objectId: 1, instanceId: 1, categoryMask: 1);
        return builder.Build();
    }
}
