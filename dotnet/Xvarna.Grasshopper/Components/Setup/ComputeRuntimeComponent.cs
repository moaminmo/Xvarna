using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Reads live throughput, scratch reuse, fallback, and device-health telemetry.</summary>
public sealed class ComputeRuntimeComponent : GH_Component
{
    /// <summary>Initializes the VAYU runtime telemetry component.</summary>
    public ComputeRuntimeComponent()
        : base(
            "XVARNA Runtime",
            "XV Runtime",
            "Reads cumulative VAYU dispatch, persistent-arena, transfer, fallback, and device-health telemetry.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("5d0d2f8c-2ff5-485f-a23a-54537da8671d");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Backend", "B", "Cached session produced by XV Backend.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddIntegerParameter("Traces", "T", "Cumulative closest-hit and any-hit calls.", GH_ParamAccess.item);
        output.AddNumberParameter("Rays", "R", "Cumulative rays accepted by this session.", GH_ParamAccess.item);
        output.AddNumberParameter("Dispatches", "D", "Cumulative portable GPU dispatches.", GH_ParamAccess.item);
        output.AddNumberParameter("Reused Batches", "RB", "Batches served by persistent scratch buffers.", GH_ParamAccess.item);
        output.AddIntegerParameter("Arena Generations", "G", "First allocation plus capacity-growth count.", GH_ParamAccess.item);
        output.AddNumberParameter("Arena Capacity", "C", "Ray capacity of each persistent slot.", GH_ParamAccess.item);
        output.AddNumberParameter("Arena Memory MB", "MB", "Dynamic memory retained by both scratch slots.", GH_ParamAccess.item);
        output.AddIntegerParameter("Fallbacks", "F", "Runtime batches retried on CPU.", GH_ParamAccess.item);
        output.AddBooleanParameter("Healthy", "OK", "True while the retained GPU device is usable.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Info", "Complete transfer, error, and device-health report.", GH_ParamAccess.item);
        output.AddNumberParameter("Geometry Uploads", "GU", "Geometry chunk uploads, including streaming passes.", GH_ParamAccess.item);
        output.AddNumberParameter("Geometry Upload MB", "GMB", "Cumulative geometry bytes uploaded.", GH_ParamAccess.item);
        output.AddTextParameter("Cache", "Cache", "Global memory/disk cache occupancy, hit/miss, eviction, and recovery telemetry.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        object? wrapped = null;
        if (!dataAccess.GetData(0, ref wrapped))
        {
            return;
        }
        XvarnaComputeSession? session = wrapped switch
        {
            XvarnaComputeSession value => value,
            GH_ObjectWrapper { Value: XvarnaComputeSession value } => value,
            _ => null,
        };
        if (session is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Input must come from XV Backend.");
            return;
        }
        try
        {
            ComputeRuntimeStatistics stats = session.GetRuntimeStatistics();
            CacheStatistics cache = XvarnaCache.GetStatistics();
            dataAccess.SetData(0, checked((int)Math.Min(stats.TraceCount, int.MaxValue)));
            dataAccess.SetData(1, (double)stats.RayCount);
            dataAccess.SetData(2, (double)stats.DispatchCount);
            dataAccess.SetData(3, (double)stats.ReusableBufferBatchCount);
            dataAccess.SetData(4, checked((int)Math.Min(stats.ScratchGenerationCount, int.MaxValue)));
            dataAccess.SetData(5, (double)stats.ScratchCapacityRays);
            dataAccess.SetData(6, stats.ScratchGpuBytes / 1_048_576.0);
            dataAccess.SetData(7, checked((int)Math.Min(stats.RuntimeFallbackCount, int.MaxValue)));
            dataAccess.SetData(8, stats.DeviceHealthy);
            dataAccess.SetData(9, BuildReport(stats, cache));
            dataAccess.SetData(10, (double)stats.GeometryUploadCount);
            dataAccess.SetData(11, stats.GeometryUploadedBytes / 1_048_576.0);
            dataAccess.SetData(12, BuildCacheReport(cache));
            if (!stats.DeviceHealthy)
            {
                AddRuntimeMessage(GH_RuntimeMessageLevel.Error, stats.LastError);
            }
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ObjectDisposedException
            or InvalidOperationException
            or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static string BuildReport(ComputeRuntimeStatistics stats, CacheStatistics cache)
    {
        string errorLine = string.IsNullOrWhiteSpace(stats.LastError)
            ? string.Empty
            : $"{Environment.NewLine}Last error: {stats.LastError}";
        return string.Create(
            CultureInfo.InvariantCulture,
            $"Requests: {stats.TraceCount:N0}; rays: {stats.RayCount:N0}; dispatches: {stats.DispatchCount:N0}{Environment.NewLine}Persistent arena: {stats.ScratchCapacityRays:N0} rays/slot; {stats.ScratchGpuBytes / 1_048_576.0:N3} MiB; generations: {stats.ScratchGenerationCount:N0}{Environment.NewLine}Reuse: {stats.ReusableBufferBatchCount:N0} batches; fallbacks: {stats.RuntimeFallbackCount:N0}{Environment.NewLine}Transfer: {stats.UploadedBytes / 1_048_576.0:N3} MiB rays; {stats.ReadbackBytes / 1_048_576.0:N3} MiB read back; {stats.GeometryUploadedBytes / 1_048_576.0:N3} MiB geometry/{stats.GeometryUploadCount:N0} uploads{Environment.NewLine}Cache: {cache.MemoryHitCount:N0} memory + {cache.DiskHitCount:N0} disk hits; {cache.MissCount:N0} misses; {cache.EvictionCount:N0} evictions; {cache.CorruptionCount:N0} recoveries{Environment.NewLine}Device healthy: {stats.DeviceHealthy}; losses: {stats.DeviceLossCount:N0}; execution errors: {stats.ExecutionErrorCount:N0}{errorLine}");
    }

    private static string BuildCacheReport(CacheStatistics cache) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"Hits: {cache.MemoryHitCount:N0} memory; {cache.DiskHitCount:N0} disk; misses: {cache.MissCount:N0}{Environment.NewLine}" +
            $"Entries: {cache.MemoryEntryCount:N0} memory/{cache.DiskEntryCount:N0} disk; writes: {cache.WriteCount:N0}; evictions: {cache.EvictionCount:N0}{Environment.NewLine}" +
            $"Occupancy: {cache.MemoryBytes / 1_048_576.0:N3}/{cache.MemoryBudgetBytes / 1_048_576.0:N3} MiB memory; {cache.DiskBytes / 1_073_741_824.0:N3}/{cache.DiskBudgetBytes / 1_073_741_824.0:N3} GiB disk{Environment.NewLine}" +
            $"Recovery: {cache.CorruptionCount:N0}; last: {cache.LastEvent}{Environment.NewLine}Directory: {cache.Directory}");
}
