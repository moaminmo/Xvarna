using System.Runtime.InteropServices;
using System.Text;

namespace Xvarna.Native;

/// <summary>Bounded process-wide memory/disk cache policy.</summary>
public readonly record struct CacheConfiguration(
    string Directory,
    ulong MemoryBudgetBytes = 256UL * 1024 * 1024,
    ulong DiskBudgetBytes = 4UL * 1024 * 1024 * 1024);

/// <summary>Observable cache counters, occupancy, policy, and recovery state.</summary>
public sealed record CacheStatistics
{
    /// <summary>Entries served from the process memory LRU.</summary>
    public required ulong MemoryHitCount { get; init; }
    /// <summary>Entries restored from the checksummed disk cache.</summary>
    public required ulong DiskHitCount { get; init; }
    /// <summary>Lookups not present in either tier.</summary>
    public required ulong MissCount { get; init; }
    /// <summary>Successful atomic disk writes.</summary>
    public required ulong WriteCount { get; init; }
    /// <summary>Entries evicted to enforce memory or disk budgets.</summary>
    public required ulong EvictionCount { get; init; }
    /// <summary>Invalid entries quarantined and removed during recovery.</summary>
    public required ulong CorruptionCount { get; init; }
    /// <summary>Current memory-tier occupancy.</summary>
    public required ulong MemoryBytes { get; init; }
    /// <summary>Current disk-tier occupancy.</summary>
    public required ulong DiskBytes { get; init; }
    /// <summary>Current number of memory-tier entries.</summary>
    public required ulong MemoryEntryCount { get; init; }
    /// <summary>Current number of disk-tier entries.</summary>
    public required ulong DiskEntryCount { get; init; }
    /// <summary>Configured memory-tier byte budget.</summary>
    public required ulong MemoryBudgetBytes { get; init; }
    /// <summary>Configured disk-tier byte budget.</summary>
    public required ulong DiskBudgetBytes { get; init; }
    /// <summary>Canonical active cache directory.</summary>
    public required string Directory { get; init; }
    /// <summary>Most recent cache or recovery event.</summary>
    public required string LastEvent { get; init; }
}

/// <summary>Configures and inspects XVARNA's checksummed LRU cache.</summary>
public static class XvarnaCache
{
    private const uint ExpectedOptionsSize = 24;
    private const uint ExpectedStatsSize = 616;

    /// <summary>Atomically replaces the cache directory and budgets.</summary>
    public static unsafe void Configure(CacheConfiguration configuration)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(configuration.Directory);
        byte[] directory = Encoding.UTF8.GetBytes(Path.GetFullPath(configuration.Directory));
        if (directory.Length is 0 or > 32_768)
        {
            throw new ArgumentOutOfRangeException(nameof(configuration), "Cache directory must contain 1..32768 UTF-8 bytes.");
        }
        NativeLibraryBootstrap.Initialize();
        NativeCacheOptions options = new()
        {
            StructureSize = ExpectedOptionsSize,
            Flags = 0,
            MemoryBudgetBytes = configuration.MemoryBudgetBytes,
            DiskBudgetBytes = configuration.DiskBudgetBytes,
        };
        fixed (byte* directoryPointer = directory)
        {
            ThrowIfFailed(NativeMethods.CacheConfigure(&options, directoryPointer, (nuint)directory.Length));
        }
    }

    /// <summary>Reads telemetry without resetting counters.</summary>
    public static unsafe CacheStatistics GetStatistics()
    {
        NativeLibraryBootstrap.Initialize();
        NativeCacheStats value = default;
        ThrowIfFailed(NativeMethods.CacheGetStats(&value));
        if (value.StructureSize != ExpectedStatsSize || Marshal.SizeOf<NativeCacheStats>() != ExpectedStatsSize)
        {
            throw new InvalidOperationException($"Native cache-statistics layout mismatch: {value.StructureSize}.");
        }
        return new()
        {
            MemoryHitCount = value.MemoryHitCount,
            DiskHitCount = value.DiskHitCount,
            MissCount = value.MissCount,
            WriteCount = value.WriteCount,
            EvictionCount = value.EvictionCount,
            CorruptionCount = value.CorruptionCount,
            MemoryBytes = value.MemoryBytes,
            DiskBytes = value.DiskBytes,
            MemoryEntryCount = value.MemoryEntryCount,
            DiskEntryCount = value.DiskEntryCount,
            MemoryBudgetBytes = value.MemoryBudgetBytes,
            DiskBudgetBytes = value.DiskBudgetBytes,
            Directory = Decode(value.Directory, 256),
            LastEvent = Decode(value.LastEvent, 256),
        };
    }

    /// <summary>Clears only `.xvc` entries inside the active cache directory.</summary>
    public static void Clear()
    {
        NativeLibraryBootstrap.Initialize();
        ThrowIfFailed(NativeMethods.CacheClear());
    }

    private static unsafe string Decode(byte* value, int capacity)
    {
        int length = 0;
        while (length < capacity && value[length] != 0)
        {
            length++;
        }
        return Encoding.UTF8.GetString(value, length);
    }

    private static void ThrowIfFailed(NativeStatus status)
    {
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
    }
}
