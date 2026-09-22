using System.Runtime.InteropServices;
using System.Text;

namespace Xvarna.Native;

/// <summary>Backend selection policy for a VAYU compute session.</summary>
public enum ComputePreference
{
    /// <summary>Prefer portable GPU and fall back to canonical f64 CPU explicitly.</summary>
    Auto = 0,
    /// <summary>Require portable GPU; creation or dispatch failures are returned.</summary>
    RequirePortableGpu = 1,
    /// <summary>Use the canonical Rayon/f64 CPU backend.</summary>
    Cpu = 2,
}

/// <summary>Adapter power policy.</summary>
public enum ComputePowerPreference
{
    /// <summary>Prefer a high-performance adapter.</summary>
    HighPerformance = 0,
    /// <summary>Prefer a low-power adapter.</summary>
    LowPower = 1,
}

/// <summary>Backend that produced a session or batch.</summary>
public enum ComputeBackend
{
    /// <summary>Canonical f64 CPU traversal.</summary>
    Cpu = 0,
    /// <summary>VAYU wgpu/WGSL f32 traversal.</summary>
    PortableGpu = 1,
}

/// <summary>Validated resource and precision policy for VAYU.</summary>
public readonly record struct ComputeOptions(
    ComputePreference Preference = ComputePreference.Auto,
    ComputePowerPreference PowerPreference = ComputePowerPreference.HighPerformance,
    double MaximumPrecisionErrorMeters = 0.00025,
    ulong MaximumExpandedTriangleCount = 20_000_000,
    ulong MaximumRaysPerDispatch = 1_048_576,
    ulong MaximumGeometryChunkBytes = 512UL * 1024 * 1024,
    ulong MaximumGpuMemoryBytes = 1024UL * 1024 * 1024,
    bool EnableSceneCache = true,
    string? AdapterNameFilter = null)
{
    /// <summary>Balanced automatic portable-GPU policy with canonical CPU fallback.</summary>
    public static ComputeOptions Default { get; } = new(
        ComputePreference.Auto,
        ComputePowerPreference.HighPerformance,
        0.00025,
        20_000_000,
        1_048_576,
        512UL * 1024 * 1024,
        1024UL * 1024 * 1024,
        true,
        null);
}

/// <summary>One portable adapter visible to VAYU.</summary>
public sealed record ComputeAdapterInfo
{
    /// <summary>Human-readable adapter name.</summary>
    public required string Name { get; init; }
    /// <summary>Driver name/detail.</summary>
    public required string Driver { get; init; }
    /// <summary>Graphics API backend code: 1 Vulkan, 2 Metal, 3 DX12, 4 GL, 5 WebGPU.</summary>
    public required uint BackendCode { get; init; }
    /// <summary>Device class code: 1 integrated, 2 discrete, 3 virtual, 4 CPU.</summary>
    public required uint DeviceTypeCode { get; init; }
    /// <summary>Vendor identifier reported by the driver.</summary>
    public required uint VendorId { get; init; }
    /// <summary>Device identifier reported by the driver.</summary>
    public required uint DeviceId { get; init; }
}

/// <summary>Immutable backend, adapter, memory, precision, and fallback provenance.</summary>
public sealed record ComputeSessionInfo
{
    /// <summary>Selected backend.</summary>
    public required ComputeBackend Backend { get; init; }
    /// <summary>True when Auto creation fell back to CPU.</summary>
    public required bool UsedCreationFallback { get; init; }
    /// <summary>Selected adapter name, empty for CPU.</summary>
    public required string AdapterName { get; init; }
    /// <summary>Creation status or exact fallback reason.</summary>
    public required string Message { get; init; }
    /// <summary>Graphics API backend code.</summary>
    public required uint AdapterBackendCode { get; init; }
    /// <summary>Adapter device class code.</summary>
    public required uint DeviceTypeCode { get; init; }
    /// <summary>Vendor identifier.</summary>
    public required uint VendorId { get; init; }
    /// <summary>Device identifier.</summary>
    public required uint DeviceId { get; init; }
    /// <summary>Effective triangles represented by the backend.</summary>
    public required ulong TriangleCount { get; init; }
    /// <summary>Active hierarchy node count.</summary>
    public required ulong NodeCount { get; init; }
    /// <summary>Approximate portable GPU payload bytes.</summary>
    public required ulong ApproximateGpuBytes { get; init; }
    /// <summary>Initialization time in microseconds.</summary>
    public required ulong InitializationMicroseconds { get; init; }
    /// <summary>Largest observed scene conversion error in metres.</summary>
    public required double ScenePrecisionErrorMeters { get; init; }
    /// <summary>Number of bounded two-level geometry chunks.</summary>
    public required uint GeometryChunkCount { get; init; }
    /// <summary>True when the portable plan was restored from validated cache.</summary>
    public required bool SceneCacheHit { get; init; }
    /// <summary>True when chunks are streamed through one bounded working set.</summary>
    public required bool StreamedGeometry { get; init; }
}

/// <summary>Per-batch backend, transfer, timing, precision, and fallback provenance.</summary>
public sealed record ComputeBatchStatistics
{
    /// <summary>Backend that produced the output.</summary>
    public required ComputeBackend Backend { get; init; }
    /// <summary>True when Auto mode retried this batch on CPU.</summary>
    public required bool UsedRuntimeFallback { get; init; }
    /// <summary>True when VAYU served this batch from its persistent scratch arena.</summary>
    public required bool UsedReusableBuffers { get; init; }
    /// <summary>Ordered ray count.</summary>
    public required ulong RayCount { get; init; }
    /// <summary>Bounded GPU dispatch count.</summary>
    public required ulong DispatchCount { get; init; }
    /// <summary>Upload/encoding time in microseconds.</summary>
    public required ulong UploadMicroseconds { get; init; }
    /// <summary>Execution/wait time in microseconds.</summary>
    public required ulong ExecutionMicroseconds { get; init; }
    /// <summary>Readback/decode time in microseconds.</summary>
    public required ulong ReadbackMicroseconds { get; init; }
    /// <summary>Largest observed scene/ray conversion error in metres.</summary>
    public required double PrecisionErrorMeters { get; init; }
}

/// <summary>Cumulative VAYU throughput, memory-reuse, fallback, and device-health telemetry.</summary>
public sealed record ComputeRuntimeStatistics
{
    /// <summary>Closest-hit and any-hit calls received.</summary>
    public required ulong TraceCount { get; init; }
    /// <summary>Total rays received, including CPU fallback.</summary>
    public required ulong RayCount { get; init; }
    /// <summary>Total portable GPU dispatches.</summary>
    public required ulong DispatchCount { get; init; }
    /// <summary>Successful batches served from reusable buffers.</summary>
    public required ulong ReusableBufferBatchCount { get; init; }
    /// <summary>Auto-mode batches retried on CPU.</summary>
    public required ulong RuntimeFallbackCount { get; init; }
    /// <summary>Scratch allocation/growth generations.</summary>
    public required ulong ScratchGenerationCount { get; init; }
    /// <summary>Current ray capacity of each scratch slot.</summary>
    public required ulong ScratchCapacityRays { get; init; }
    /// <summary>Current bytes retained by both scratch slots.</summary>
    public required ulong ScratchGpuBytes { get; init; }
    /// <summary>Cumulative queue-write bytes.</summary>
    public required ulong UploadedBytes { get; init; }
    /// <summary>Cumulative readback bytes.</summary>
    public required ulong ReadbackBytes { get; init; }
    /// <summary>Device-loss callbacks observed.</summary>
    public required ulong DeviceLossCount { get; init; }
    /// <summary>Execution errors observed.</summary>
    public required ulong ExecutionErrorCount { get; init; }
    /// <summary>Geometry chunks uploaded after initialization.</summary>
    public required ulong GeometryUploadCount { get; init; }
    /// <summary>Streamed geometry bytes uploaded after initialization.</summary>
    public required ulong GeometryUploadedBytes { get; init; }
    /// <summary>Whether the retained logical device remains usable.</summary>
    public required bool DeviceHealthy { get; init; }
    /// <summary>Most recent exact device/runtime diagnostic.</summary>
    public required string LastError { get; init; }
}

/// <summary>Closest-hit result with its execution provenance.</summary>
public sealed record ComputeTraceResult(SceneHit[] Hits, ComputeBatchStatistics Statistics);

/// <summary>Any-hit result with its execution provenance.</summary>
public sealed record ComputeVisibilityResult(bool[] Hits, ComputeBatchStatistics Statistics);

/// <summary>Thread-safe VAYU compute session retaining native scene ownership.</summary>
public sealed partial class XvarnaComputeSession : IDisposable
{
    private const uint ExpectedOptionsSize = 64;
    private const uint ExpectedInfoSize = 456;
    private const uint ExpectedBatchStatsSize = 64;
    private const uint ExpectedRuntimeStatsSize = 384;
    private readonly SafeComputeHandle handle;
    private readonly XvarnaScene scene;

    private unsafe XvarnaComputeSession(XvarnaScene scene, ComputeOptions options)
    {
        ArgumentNullException.ThrowIfNull(scene);
        Validate(options);
        NativeComputeOptions nativeOptions = new()
        {
            StructureSize = ExpectedOptionsSize,
            Preference = (uint)options.Preference,
            PowerPreference = (uint)options.PowerPreference,
            Flags = options.EnableSceneCache ? 1U : 0U,
            MaximumPrecisionErrorMeters = options.MaximumPrecisionErrorMeters,
            MaximumExpandedTriangleCount = options.MaximumExpandedTriangleCount,
            MaximumRaysPerDispatch = options.MaximumRaysPerDispatch,
            MaximumGeometryChunkBytes = options.MaximumGeometryChunkBytes,
            MaximumGpuMemoryBytes = options.MaximumGpuMemoryBytes,
            Reserved = 0,
        };
        ulong rawHandle = 0;
        NativeComputeInfo nativeInfo = default;
        if (string.IsNullOrWhiteSpace(options.AdapterNameFilter))
        {
            ThrowIfFailed(NativeMethods.ComputeCreate(
                scene.NativeHandle,
                &nativeOptions,
                &rawHandle,
                &nativeInfo));
        }
        else
        {
            byte[] adapterFilter = Encoding.UTF8.GetBytes(options.AdapterNameFilter.Trim());
            fixed (byte* adapterFilterPointer = adapterFilter)
            {
                ThrowIfFailed(NativeMethods.ComputeCreateForAdapter(
                    scene.NativeHandle,
                    &nativeOptions,
                    adapterFilterPointer,
                    (nuint)adapterFilter.Length,
                    &rawHandle,
                    &nativeInfo));
            }
        }
        handle = new SafeComputeHandle(rawHandle);
        this.scene = scene;
        Info = ConvertInfo(nativeInfo);
    }

    /// <summary>Selected backend and immutable creation provenance.</summary>
    public ComputeSessionInfo Info { get; }

    /// <summary>Creates an Auto, required-GPU, or CPU session over an immutable scene.</summary>
    public static XvarnaComputeSession Create(XvarnaScene scene, ComputeOptions options = default) =>
        new(scene, NormalizeDefault(options));

    /// <summary>Enumerates portable adapters without creating a scene/session.</summary>
    public static unsafe IReadOnlyList<ComputeAdapterInfo> EnumerateAdapters()
    {
        NativeLibraryBootstrap.Initialize();
        nuint count = 0;
        NativeStatus countStatus = NativeMethods.ComputeEnumerateAdapters(null, 0, &count);
        if (countStatus is not NativeStatus.Success and not NativeStatus.InvalidLength)
        {
            ThrowIfFailed(countStatus);
        }
        if (count == 0)
        {
            return [];
        }
        int length = checked((int)count);
        NativeAdapterInfo[] native = new NativeAdapterInfo[length];
        fixed (NativeAdapterInfo* pointer = native)
        {
            ThrowIfFailed(NativeMethods.ComputeEnumerateAdapters(pointer, count, &count));
        }
        ComputeAdapterInfo[] adapters = new ComputeAdapterInfo[length];
        for (int index = 0; index < length; index++)
        {
            NativeAdapterInfo value = native[index];
            if (value.StructureSize != 224)
            {
                throw new InvalidOperationException($"Native adapter layout mismatch: {value.StructureSize}.");
            }
            adapters[index] = new()
            {
                Name = Decode(value.Name, 128),
                Driver = Decode(value.Driver, 64),
                BackendCode = value.Backend,
                DeviceTypeCode = value.DeviceType,
                VendorId = value.VendorId,
                DeviceId = value.DeviceId,
            };
        }
        return adapters;
    }

    /// <summary>Traces an ordered closest-hit batch in source model units.</summary>
    public unsafe ComputeTraceResult TraceClosest(IReadOnlyList<SceneRay> rays)
    {
        ArgumentNullException.ThrowIfNull(rays);
        ThrowIfDisposed();
        NativeRay[] nativeRays = scene.ConvertRays(rays);
        NativeHit[] nativeHits = new NativeHit[nativeRays.Length];
        NativeComputeBatchStats nativeStats = default;
        fixed (NativeRay* raysPointer = nativeRays)
        fixed (NativeHit* hitsPointer = nativeHits)
        {
            ThrowIfFailed(NativeMethods.ComputeTraceClosest(
                handle,
                raysPointer,
                (nuint)nativeRays.Length,
                hitsPointer,
                (nuint)nativeHits.Length,
                &nativeStats));
        }
        return new(scene.ConvertHits(nativeHits), ConvertStats(nativeStats));
    }

    /// <summary>Traces an ordered early-exit visibility batch in source model units.</summary>
    public unsafe ComputeVisibilityResult TraceAny(IReadOnlyList<SceneRay> rays)
    {
        ArgumentNullException.ThrowIfNull(rays);
        ThrowIfDisposed();
        NativeRay[] nativeRays = scene.ConvertRays(rays);
        byte[] nativeHits = new byte[nativeRays.Length];
        NativeComputeBatchStats nativeStats = default;
        fixed (NativeRay* raysPointer = nativeRays)
        fixed (byte* hitsPointer = nativeHits)
        {
            ThrowIfFailed(NativeMethods.ComputeTraceAny(
                handle,
                raysPointer,
                (nuint)nativeRays.Length,
                hitsPointer,
                (nuint)nativeHits.Length,
                &nativeStats));
        }
        return new(Array.ConvertAll(nativeHits, value => value != 0), ConvertStats(nativeStats));
    }

    /// <summary>Reads cumulative telemetry without resetting counters or synchronizing the GPU.</summary>
    public unsafe ComputeRuntimeStatistics GetRuntimeStatistics()
    {
        ThrowIfDisposed();
        NativeComputeRuntimeStats native = default;
        ThrowIfFailed(NativeMethods.ComputeGetRuntimeStats(handle, &native));
        if (native.StructureSize != ExpectedRuntimeStatsSize
            || Marshal.SizeOf<NativeComputeRuntimeStats>() != ExpectedRuntimeStatsSize)
        {
            throw new InvalidOperationException($"Native compute runtime-stats layout mismatch: {native.StructureSize}.");
        }
        return new()
        {
            TraceCount = native.TraceCount,
            RayCount = native.RayCount,
            DispatchCount = native.DispatchCount,
            ReusableBufferBatchCount = native.ReusableBufferBatchCount,
            RuntimeFallbackCount = native.RuntimeFallbackCount,
            ScratchGenerationCount = native.ScratchGenerationCount,
            ScratchCapacityRays = native.ScratchCapacityRays,
            ScratchGpuBytes = native.ScratchGpuBytes,
            UploadedBytes = native.UploadedBytes,
            ReadbackBytes = native.ReadbackBytes,
            DeviceLossCount = native.DeviceLossCount,
            ExecutionErrorCount = native.ExecutionErrorCount,
            GeometryUploadCount = native.GeometryUploadCount,
            GeometryUploadedBytes = native.GeometryUploadedBytes,
            DeviceHealthy = (native.Flags & 1) != 0,
            LastError = Decode(native.LastError, 256),
        };
    }

    /// <inheritdoc />
    public void Dispose() => handle.Dispose();

    private static ComputeOptions NormalizeDefault(ComputeOptions options) =>
        options == default ? ComputeOptions.Default : options;

    private static void Validate(ComputeOptions options)
    {
        if (!Enum.IsDefined(options.Preference) || !Enum.IsDefined(options.PowerPreference))
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Compute enum value is invalid.");
        }
        if (!double.IsFinite(options.MaximumPrecisionErrorMeters) || options.MaximumPrecisionErrorMeters <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Precision error budget must be finite and positive.");
        }
        if (options.MaximumExpandedTriangleCount == 0
            || options.MaximumRaysPerDispatch == 0
            || options.MaximumGeometryChunkBytes == 0
            || options.MaximumGpuMemoryBytes == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Triangle, dispatch, geometry-chunk, and GPU-memory limits must be positive.");
        }
        if (options.MaximumGeometryChunkBytes > options.MaximumGpuMemoryBytes)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Geometry chunk bytes cannot exceed the GPU-memory budget.");
        }
        if (options.AdapterNameFilter is not null)
        {
            string filter = options.AdapterNameFilter.Trim();
            if (filter.Length == 0 || Encoding.UTF8.GetByteCount(filter) > 256)
            {
                throw new ArgumentOutOfRangeException(nameof(options), "Adapter filter must contain 1..256 UTF-8 bytes.");
            }
            if (options.Preference == ComputePreference.Cpu)
            {
                throw new ArgumentException("An adapter filter cannot be combined with the CPU backend.", nameof(options));
            }
        }
    }

    private static unsafe ComputeSessionInfo ConvertInfo(NativeComputeInfo value)
    {
        if (value.StructureSize != ExpectedInfoSize || Marshal.SizeOf<NativeComputeInfo>() != ExpectedInfoSize)
        {
            throw new InvalidOperationException($"Native compute-info layout mismatch: {value.StructureSize}.");
        }
        return new()
        {
            Backend = (ComputeBackend)value.Backend,
            UsedCreationFallback = (value.Flags & 1) != 0,
            AdapterName = Decode(value.AdapterName, 128),
            Message = Decode(value.Message, 256),
            AdapterBackendCode = value.AdapterBackend,
            DeviceTypeCode = value.DeviceType,
            VendorId = value.VendorId,
            DeviceId = value.DeviceId,
            TriangleCount = value.TriangleCount,
            NodeCount = value.NodeCount,
            ApproximateGpuBytes = value.ApproximateGpuBytes,
            InitializationMicroseconds = value.InitializationMicroseconds,
            ScenePrecisionErrorMeters = value.ScenePrecisionErrorMeters,
            GeometryChunkCount = value.GeometryChunkCount,
            SceneCacheHit = (value.Flags & 4) != 0,
            StreamedGeometry = (value.Flags & 8) != 0,
        };
    }

    private static ComputeBatchStatistics ConvertStats(NativeComputeBatchStats value)
    {
        if (value.StructureSize != ExpectedBatchStatsSize || Marshal.SizeOf<NativeComputeBatchStats>() != ExpectedBatchStatsSize)
        {
            throw new InvalidOperationException($"Native compute-stats layout mismatch: {value.StructureSize}.");
        }
        return new()
        {
            Backend = (ComputeBackend)value.Backend,
            UsedRuntimeFallback = (value.Flags & 1) != 0,
            UsedReusableBuffers = (value.Flags & 2) != 0,
            RayCount = value.RayCount,
            DispatchCount = value.DispatchCount,
            UploadMicroseconds = value.UploadMicroseconds,
            ExecutionMicroseconds = value.ExecutionMicroseconds,
            ReadbackMicroseconds = value.ReadbackMicroseconds,
            PrecisionErrorMeters = value.PrecisionErrorMeters,
        };
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

    private void ThrowIfDisposed()
    {
        if (handle.IsClosed || handle.IsInvalid)
        {
            throw new ObjectDisposedException(nameof(XvarnaComputeSession));
        }
    }

    private static void ThrowIfFailed(NativeStatus status)
    {
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
    }
}

internal sealed class SafeComputeHandle : SafeHandle
{
    internal SafeComputeHandle(ulong handleValue)
        : base(nint.Zero, true)
    {
        SetHandle(checked((nint)handleValue));
    }

    public override bool IsInvalid => handle == nint.Zero;

    protected override bool ReleaseHandle() =>
        NativeMethods.ComputeRelease(unchecked((ulong)handle)) == NativeStatus.Success;
}
