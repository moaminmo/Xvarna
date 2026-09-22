using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class SceneTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void SceneDeduplicatesResourcesAndPreservesAttributionInModelUnits()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong firstMesh = builder.AddMesh(TrianglePositionsMillimeters(), [0, 1, 2]);
        ulong duplicateMesh = builder.AddMesh(TrianglePositionsMillimeters(), [0, 1, 2]);
        Assert.Equal(firstMesh, duplicateMesh);
        builder.AddInstance(firstMesh, Identity, objectId: 41, instanceId: 73, categoryMask: 4);

        using XvarnaScene scene = builder.Build();
        Assert.Equal(1ul, scene.Statistics.MeshResourceCount);
        Assert.Equal(1ul, scene.Statistics.InstanceCount);
        Assert.Equal(1ul, scene.Statistics.UniqueTriangleCount);
        Assert.Equal(64, scene.Statistics.ContentHash.Length);

        SceneRay ray = new(250.0, 250.0, 1000.0, 0.0, 0.0, -1.0, 0.0, 2000.0, 4);
        SceneHit hit = Assert.Single(scene.TraceClosest([ray]));
        Assert.True(hit.Hit);
        Assert.Equal(1000.0, hit.Distance, 9);
        Assert.Equal(41ul, hit.ObjectId);
        Assert.Equal(73ul, hit.InstanceId);
        Assert.Equal(firstMesh, hit.MeshId);
        Assert.Equal(0u, hit.TriangleId);

        Assert.True(Assert.Single(scene.TraceAny([ray])));
        Assert.False(Assert.Single(scene.TraceAny([ray with { CategoryMask = 2 }])));
    }

    [Fact]
    public void CompiledSceneSupportsConcurrentOrderedBatches()
    {
        using XvarnaSceneBuilder builder = new(SceneBuildOptions.Default with { ThreadCount = 2 });
        ulong meshId = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(meshId, Identity, 1, 1);
        using XvarnaScene scene = builder.Build();
        SceneRay[] rays = Enumerable.Range(0, 1024)
            .Select(index => new SceneRay(
                (index % 32) / 31.0,
                (index / 32) / 31.0,
                1.0,
                0.0,
                0.0,
                -1.0,
                0.0,
                2.0))
            .ToArray();

        SceneHit[][] batches = new SceneHit[4][];
        Parallel.For(0, batches.Length, index => batches[index] = scene.TraceClosest(rays));

        Assert.All(batches, batch => Assert.Equal(rays.Length, batch.Length));
        for (int index = 0; index < rays.Length; index++)
        {
            Assert.Equal(batches[0][index].Hit, batches[3][index].Hit);
        }
    }

    [Fact]
    public void BuilderRejectsBrokenGeometryAcrossNativeBoundary()
    {
        using XvarnaSceneBuilder builder = new(SceneBuildOptions.Default);
        XvarnaNativeException exception = Assert.Throws<XvarnaNativeException>(() =>
            builder.AddMesh(
                [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0],
                [0, 1, 2]));
        Assert.Equal(3, exception.StatusCode);
    }

    [Fact]
    public void DynamicDeltaRefitsOnlyDynamicTlasAndInvalidatesSceneRevision()
    {
        using XvarnaSceneBuilder builder = new(SceneBuildOptions.Default);
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 10, 1, 1, SceneLayer.Static);
        double[] dynamicTransform = (double[])Identity.Clone();
        dynamicTransform[3] = 2.0;
        builder.AddInstance(mesh, dynamicTransform, 20, 2, 2, SceneLayer.Dynamic);
        using XvarnaScene scene = builder.Build();
        string previousHash = scene.Statistics.ContentHash;

        double[] moved = (double[])dynamicTransform.Clone();
        moved[3] = 3.0;
        SceneUpdateReport report = scene.ApplyDeltas(
            [SceneInstanceDelta.Update(2, rowMajorTransform: moved)]);

        Assert.Equal(HierarchyUpdateKind.Reused, report.StaticTlasUpdate);
        Assert.Equal(HierarchyUpdateKind.Refit, report.DynamicTlasUpdate);
        Assert.Equal(1ul, report.ReusedBlasCount);
        Assert.Equal(0ul, report.RebuiltBlasCount);
        Assert.Equal(1ul, scene.Revision);
        Assert.Equal(previousHash, report.PreviousHash);
        Assert.Equal(scene.Statistics.ContentHash, report.CurrentHash);
        Assert.NotEqual(previousHash, scene.Statistics.ContentHash);
    }

    [Fact]
    public void ComputeSessionRetainsPriorImmutableSnapshotAcrossSceneDelta()
    {
        using XvarnaSceneBuilder builder = new(SceneBuildOptions.Default);
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1, 1, SceneLayer.Dynamic);
        using XvarnaScene scene = builder.Build();
        using XvarnaComputeSession oldSession = XvarnaComputeSession.Create(
            scene,
            new ComputeOptions(Preference: ComputePreference.Cpu));
        SceneRay originalRay = new(0.25, 0.25, 1.0, 0, 0, -1, 0, 2, 1);
        Assert.True(Assert.Single(oldSession.TraceClosest([originalRay]).Hits).Hit);

        double[] moved = (double[])Identity.Clone();
        moved[3] = 10.0;
        scene.ApplyDeltas([SceneInstanceDelta.Update(1, rowMajorTransform: moved)]);

        Assert.True(Assert.Single(oldSession.TraceClosest([originalRay]).Hits).Hit);
        Assert.False(Assert.Single(scene.TraceClosest([originalRay])).Hit);
    }

    private static double[] TrianglePositionsMillimeters() =>
    [
        0.0, 0.0, 0.0,
        1000.0, 0.0, 0.0,
        0.0, 1000.0, 0.0,
    ];
}
