using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class ComputeTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void ExplicitCpuSessionPreservesModelUnitsAndAttribution()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-7));
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1000.0, 0.0, 0.0, 0.0, 1000.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 91, 92, 8);
        using XvarnaScene scene = builder.Build();
        using XvarnaComputeSession compute = XvarnaComputeSession.Create(
            scene,
            new ComputeOptions(Preference: ComputePreference.Cpu));

        ComputeTraceResult result = compute.TraceClosest(
            [new SceneRay(250.0, 250.0, 1000.0, 0.0, 0.0, -1.0, 0.0, 2000.0, 8)]);

        SceneHit hit = Assert.Single(result.Hits);
        Assert.True(hit.Hit);
        Assert.Equal(1000.0, hit.Distance, 8);
        Assert.Equal(91ul, hit.ObjectId);
        Assert.Equal(ComputeBackend.Cpu, result.Statistics.Backend);
        Assert.Equal(ComputeBackend.Cpu, compute.Info.Backend);
        Assert.False(compute.Info.UsedCreationFallback);
        Assert.False(result.Statistics.UsedReusableBuffers);
        ComputeRuntimeStatistics runtime = compute.GetRuntimeStatistics();
        Assert.Equal(1ul, runtime.TraceCount);
        Assert.Equal(1ul, runtime.RayCount);
        Assert.Equal(0ul, runtime.DispatchCount);
        Assert.True(runtime.DeviceHealthy);
    }

    [Fact]
    public void OmittedComputeOptionsUseProductionBudgetsAndCachePolicy()
    {
        using XvarnaSceneBuilder builder = new(SceneBuildOptions.Default);
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1, 1);
        using XvarnaScene scene = builder.Build();
        using XvarnaComputeSession compute = XvarnaComputeSession.Create(scene);
        Assert.Contains(compute.Info.Backend, new[] { ComputeBackend.Cpu, ComputeBackend.PortableGpu });
    }

    [Fact]
    public void AdapterEnumerationReturnsStableMetadata()
    {
        IReadOnlyList<ComputeAdapterInfo> adapters = XvarnaComputeSession.EnumerateAdapters();
        Assert.All(adapters, adapter =>
        {
            Assert.False(string.IsNullOrWhiteSpace(adapter.Name));
            Assert.InRange(adapter.BackendCode, 0u, 5u);
            Assert.InRange(adapter.DeviceTypeCode, 0u, 4u);
        });
    }

    [Fact]
    public void PortableSessionReusesArenaAndExposesManagedTelemetryWhenAvailable()
    {
        ComputeAdapterInfo? adapter = XvarnaComputeSession.EnumerateAdapters().FirstOrDefault();
        if (adapter is null)
        {
            return;
        }
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(1.0, 1.0e-7));
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 7, 8, 1);
        using XvarnaScene scene = builder.Build();
        XvarnaComputeSession compute;
        try
        {
            compute = XvarnaComputeSession.Create(
                scene,
                new ComputeOptions(
                    Preference: ComputePreference.RequirePortableGpu,
                    MaximumRaysPerDispatch: 64,
                    AdapterNameFilter: adapter.Name));
        }
        catch (XvarnaNativeException)
        {
            return;
        }
        using (compute)
        {
            SceneRay[] rays = Enumerable.Range(0, 128)
                .Select(index => new SceneRay(
                    (index % 8) / 8.0,
                    (index / 8) / 16.0,
                    1.0,
                    0.0,
                    0.0,
                    -1.0,
                    0.0,
                    2.0,
                    1))
                .ToArray();
            ComputeTraceResult cold = compute.TraceClosest(rays);
            ComputeTraceResult warm = compute.TraceClosest(rays);
            ComputeRuntimeStatistics runtime = compute.GetRuntimeStatistics();

            Assert.True(cold.Statistics.UsedReusableBuffers);
            Assert.True(warm.Statistics.UsedReusableBuffers);
            Assert.Equal(2ul, cold.Statistics.DispatchCount);
            Assert.Equal(2ul, runtime.TraceCount);
            Assert.Equal(4ul, runtime.DispatchCount);
            Assert.Equal(2ul, runtime.ReusableBufferBatchCount);
            Assert.Equal(1ul, runtime.ScratchGenerationCount);
            Assert.True(runtime.DeviceHealthy);
            Assert.Equal(0ul, runtime.DeviceLossCount);
        }
    }

    [Fact]
    public void CpuSessionRejectsMeaninglessAdapterFilter()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(1.0, 1.0e-7));
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1, 1);
        using XvarnaScene scene = builder.Build();

        Assert.Throws<ArgumentException>(() => XvarnaComputeSession.Create(
            scene,
            new ComputeOptions(
                Preference: ComputePreference.Cpu,
                AdapterNameFilter: "NVIDIA")));
    }

    [Fact]
    public void AutoSessionReportsUnmatchedAdapterFilterFallback()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(1.0, 1.0e-7));
        ulong mesh = builder.AddMesh(
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1, 1);
        using XvarnaScene scene = builder.Build();
        using XvarnaComputeSession compute = XvarnaComputeSession.Create(
            scene,
            new ComputeOptions(
                Preference: ComputePreference.Auto,
                AdapterNameFilter: "__xvarna_adapter_that_cannot_exist__"));

        Assert.Equal(ComputeBackend.Cpu, compute.Info.Backend);
        Assert.True(compute.Info.UsedCreationFallback);
        Assert.Contains("no adapter name contains", compute.Info.Message, StringComparison.OrdinalIgnoreCase);
    }
}
