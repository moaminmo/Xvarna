using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_cache_configure", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus CacheConfigure(
        NativeCacheOptions* options,
        byte* directoryUtf8,
        nuint directoryLength);

    [DllImport(LibraryName, EntryPoint = "xv_cache_get_stats", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus CacheGetStats(NativeCacheStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_cache_clear", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus CacheClear();

    [DllImport(LibraryName, EntryPoint = "xv_compute_enumerate_adapters", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeEnumerateAdapters(
        NativeAdapterInfo* output,
        nuint capacity,
        nuint* outputCount);

    [DllImport(LibraryName, EntryPoint = "xv_compute_create", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeCreate(
        SafeSceneHandle sceneHandle,
        NativeComputeOptions* options,
        ulong* outputHandle,
        NativeComputeInfo* outputInfo);

    [DllImport(LibraryName, EntryPoint = "xv_compute_create_for_adapter", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeCreateForAdapter(
        SafeSceneHandle sceneHandle,
        NativeComputeOptions* options,
        byte* adapterNameFilter,
        nuint adapterNameFilterLength,
        ulong* outputHandle,
        NativeComputeInfo* outputInfo);

    [DllImport(LibraryName, EntryPoint = "xv_compute_trace_closest", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeTraceClosest(
        SafeComputeHandle handle,
        NativeRay* rays,
        nuint rayCount,
        NativeHit* outputHits,
        nuint hitCapacity,
        NativeComputeBatchStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_compute_trace_any", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeTraceAny(
        SafeComputeHandle handle,
        NativeRay* rays,
        nuint rayCount,
        byte* outputHits,
        nuint hitCapacity,
        NativeComputeBatchStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_compute_get_runtime_stats", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeGetRuntimeStats(
        SafeComputeHandle handle,
        NativeComputeRuntimeStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_compute_release", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeRelease(ulong handle);
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeComputeOptions
{
    internal uint StructureSize;
    internal uint Preference;
    internal uint PowerPreference;
    internal uint Flags;
    internal double MaximumPrecisionErrorMeters;
    internal ulong MaximumExpandedTriangleCount;
    internal ulong MaximumRaysPerDispatch;
    internal ulong MaximumGeometryChunkBytes;
    internal ulong MaximumGpuMemoryBytes;
    internal ulong Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeComputeInfo
{
    internal uint StructureSize;
    internal uint Backend;
    internal uint Flags;
    internal uint AdapterBackend;
    internal uint DeviceType;
    internal uint VendorId;
    internal uint DeviceId;
    internal uint GeometryChunkCount;
    internal ulong TriangleCount;
    internal ulong NodeCount;
    internal ulong ApproximateGpuBytes;
    internal ulong InitializationMicroseconds;
    internal double ScenePrecisionErrorMeters;
    internal fixed byte AdapterName[128];
    internal fixed byte Message[256];
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeComputeBatchStats
{
    internal uint StructureSize;
    internal uint Backend;
    internal uint Flags;
    internal uint Reserved;
    internal ulong RayCount;
    internal ulong DispatchCount;
    internal ulong UploadMicroseconds;
    internal ulong ExecutionMicroseconds;
    internal ulong ReadbackMicroseconds;
    internal double PrecisionErrorMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeComputeRuntimeStats
{
    internal uint StructureSize;
    internal uint Flags;
    internal uint Reserved0;
    internal uint Reserved1;
    internal ulong TraceCount;
    internal ulong RayCount;
    internal ulong DispatchCount;
    internal ulong ReusableBufferBatchCount;
    internal ulong RuntimeFallbackCount;
    internal ulong ScratchGenerationCount;
    internal ulong ScratchCapacityRays;
    internal ulong ScratchGpuBytes;
    internal ulong UploadedBytes;
    internal ulong ReadbackBytes;
    internal ulong DeviceLossCount;
    internal ulong ExecutionErrorCount;
    internal ulong GeometryUploadCount;
    internal ulong GeometryUploadedBytes;
    internal fixed byte LastError[256];
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeCacheOptions
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong MemoryBudgetBytes;
    internal ulong DiskBudgetBytes;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeCacheStats
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong MemoryHitCount;
    internal ulong DiskHitCount;
    internal ulong MissCount;
    internal ulong WriteCount;
    internal ulong EvictionCount;
    internal ulong CorruptionCount;
    internal ulong MemoryBytes;
    internal ulong DiskBytes;
    internal ulong MemoryEntryCount;
    internal ulong DiskEntryCount;
    internal ulong MemoryBudgetBytes;
    internal ulong DiskBudgetBytes;
    internal fixed byte Directory[256];
    internal fixed byte LastEvent[256];
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeAdapterInfo
{
    internal uint StructureSize;
    internal uint Backend;
    internal uint DeviceType;
    internal uint Flags;
    internal uint VendorId;
    internal uint DeviceId;
    internal uint Reserved0;
    internal uint Reserved1;
    internal fixed byte Name[128];
    internal fixed byte Driver[64];
}
