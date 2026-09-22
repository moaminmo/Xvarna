using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Configures and exposes the checksummed two-tier VAYU scene cache.</summary>
public sealed class CacheComponent : GH_Component
{
    private CacheConfiguration? appliedConfiguration;
    private bool previousClear;

    /// <summary>Initializes the cache policy and telemetry component.</summary>
    public CacheComponent()
        : base(
            "XVARNA Cache",
            "XV Cache",
            "Configures bounded memory/disk scene caching and reports hits, misses, eviction, and corruption recovery.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("c84040e1-739c-4a65-8eef-9c57277bbb23");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Directory", "Dir", "Cache directory. Empty uses LocalAppData/XVARNA/cache/v1.", GH_ParamAccess.item, string.Empty);
        input.AddNumberParameter("Memory MB", "RAM", "Process-memory LRU budget in MiB.", GH_ParamAccess.item, 256.0);
        input.AddNumberParameter("Disk GB", "Disk", "Persistent disk budget in GiB.", GH_ParamAccess.item, 4.0);
        input.AddBooleanParameter("Apply", "Apply", "Apply policy when its values change.", GH_ParamAccess.item, true);
        input.AddBooleanParameter("Clear", "Clear", "Rising edge atomically clears XVARNA .xvc entries only.", GH_ParamAccess.item, false);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddNumberParameter("Memory Hits", "MH", "Process-memory cache hits.", GH_ParamAccess.item);
        output.AddNumberParameter("Disk Hits", "DH", "Validated persistent cache hits.", GH_ParamAccess.item);
        output.AddNumberParameter("Misses", "Miss", "Cache misses.", GH_ParamAccess.item);
        output.AddNumberParameter("Writes", "W", "Successful atomic disk writes.", GH_ParamAccess.item);
        output.AddNumberParameter("Evictions", "E", "Memory and disk budget evictions.", GH_ParamAccess.item);
        output.AddNumberParameter("Recoveries", "C", "Corrupt entries rejected and removed.", GH_ParamAccess.item);
        output.AddNumberParameter("Memory Used MB", "RAM", "Current memory-tier occupancy.", GH_ParamAccess.item);
        output.AddNumberParameter("Disk Used GB", "Disk", "Current disk-tier occupancy.", GH_ParamAccess.item);
        output.AddTextParameter("Directory", "Dir", "Canonical active cache directory.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Info", "Complete cache schema, budget, occupancy, and recovery telemetry.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        string directory = string.Empty;
        double memoryMb = 256.0;
        double diskGb = 4.0;
        bool apply = true;
        bool clear = false;
        dataAccess.GetData(0, ref directory);
        dataAccess.GetData(1, ref memoryMb);
        dataAccess.GetData(2, ref diskGb);
        dataAccess.GetData(3, ref apply);
        dataAccess.GetData(4, ref clear);
        try
        {
            string resolvedDirectory = string.IsNullOrWhiteSpace(directory)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "XVARNA", "cache", "v1")
                : Path.GetFullPath(directory.Trim());
            CacheConfiguration configuration = new(
                resolvedDirectory,
                ToBytes(memoryMb, 1_048_576.0, "Memory MB"),
                ToBytes(diskGb, 1_073_741_824.0, "Disk GB"));
            if (apply && appliedConfiguration != configuration)
            {
                XvarnaCache.Configure(configuration);
                appliedConfiguration = configuration;
            }
            if (clear && !previousClear)
            {
                XvarnaCache.Clear();
                AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, "XVARNA cache entries cleared.");
            }
            previousClear = clear;
            CacheStatistics stats = XvarnaCache.GetStatistics();
            dataAccess.SetData(0, (double)stats.MemoryHitCount);
            dataAccess.SetData(1, (double)stats.DiskHitCount);
            dataAccess.SetData(2, (double)stats.MissCount);
            dataAccess.SetData(3, (double)stats.WriteCount);
            dataAccess.SetData(4, (double)stats.EvictionCount);
            dataAccess.SetData(5, (double)stats.CorruptionCount);
            dataAccess.SetData(6, stats.MemoryBytes / 1_048_576.0);
            dataAccess.SetData(7, stats.DiskBytes / 1_073_741_824.0);
            dataAccess.SetData(8, stats.Directory);
            dataAccess.SetData(9, BuildReport(stats));
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ArgumentException
            or InvalidOperationException
            or OverflowException
            or IOException)
        {
            previousClear = clear;
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static ulong ToBytes(double value, double multiplier, string name)
    {
        if (!double.IsFinite(value) || value < 0.0 || value > ulong.MaxValue / multiplier)
        {
            throw new ArgumentOutOfRangeException(name, "Cache budget must be finite and non-negative.");
        }
        return checked((ulong)Math.Ceiling(value * multiplier));
    }

    private static string BuildReport(CacheStatistics stats) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"Schema: XVARNA cache v1; checksum: BLAKE3; writes: atomic rename{Environment.NewLine}" +
            $"Hits: {stats.MemoryHitCount:N0} memory + {stats.DiskHitCount:N0} disk; misses: {stats.MissCount:N0}; writes: {stats.WriteCount:N0}{Environment.NewLine}" +
            $"Memory: {stats.MemoryEntryCount:N0} entries; {stats.MemoryBytes / 1_048_576.0:N3}/{stats.MemoryBudgetBytes / 1_048_576.0:N3} MiB{Environment.NewLine}" +
            $"Disk: {stats.DiskEntryCount:N0} entries; {stats.DiskBytes / 1_073_741_824.0:N3}/{stats.DiskBudgetBytes / 1_073_741_824.0:N3} GiB{Environment.NewLine}" +
            $"Evictions: {stats.EvictionCount:N0}; corruption recoveries: {stats.CorruptionCount:N0}; last event: {stats.LastEvent}{Environment.NewLine}" +
            $"Directory: {stats.Directory}");
}
