using System.Buffers.Binary;
using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Validated options used to compile an immutable native scene.</summary>
public readonly record struct SceneBuildOptions(
    double UnitScaleToMeters,
    double AbsoluteToleranceMeters,
    uint MaximumLeafSize = 4,
    uint ThreadCount = 0)
{
    /// <summary>Default options for models already expressed in metres.</summary>
    public static SceneBuildOptions Default { get; } = new(1.0, 1.0e-6);
}

/// <summary>One model-space ray with an optional 64-bit instance-category filter.</summary>
public readonly record struct SceneRay(
    double OriginX,
    double OriginY,
    double OriginZ,
    double DirectionX,
    double DirectionY,
    double DirectionZ,
    double MinimumDistance,
    double MaximumDistance,
    ulong CategoryMask = ulong.MaxValue);

/// <summary>Closest-hit result with stable source attribution and model-space distance.</summary>
public readonly record struct SceneHit(
    bool Hit,
    double Distance,
    double BarycentricU,
    double BarycentricV,
    ulong ObjectId,
    ulong InstanceId,
    ulong MeshId,
    uint TriangleId,
    bool FrontFace);

/// <summary>Static or interactively updated scene acceleration layer.</summary>
public enum SceneLayer
{
    /// <summary>Rarely changed geometry retained across dynamic edits.</summary>
    Static = 0,
    /// <summary>Interactive geometry optimized for bounded TLAS refits.</summary>
    Dynamic = 1,
}

/// <summary>TLAS action selected after a delta transaction.</summary>
public enum HierarchyUpdateKind
{
    /// <summary>The hierarchy remained byte-for-byte reusable.</summary>
    Reused = 0,
    /// <summary>Existing topology was refitted to changed bounds.</summary>
    Refit = 1,
    /// <summary>Topology was rebuilt because structure or quality required it.</summary>
    Rebuilt = 2,
}

/// <summary>Refit quality and periodic rebuild policy.</summary>
public readonly record struct SceneUpdatePolicy(
    double MaximumRefitQualityRatio = 1.35,
    uint MaximumConsecutiveRefits = 32)
{
    /// <summary>Balanced quality guard with a periodic rebuild after 32 refits.</summary>
    public static SceneUpdatePolicy Default { get; } = new(1.35, 32);
}

/// <summary>Instance delta operation.</summary>
public enum SceneDeltaOperation
{
    /// <summary>Change transform, identity, category, or layer.</summary>
    Update = 0,
    /// <summary>Add a new instance of an existing mesh resource.</summary>
    Add = 1,
    /// <summary>Remove an existing instance.</summary>
    Remove = 2,
}

/// <summary>One atomic instance add, update, or removal.</summary>
public sealed record SceneInstanceDelta
{
    /// <summary>Operation applied by the immutable transaction.</summary>
    public required SceneDeltaOperation Operation { get; init; }
    /// <summary>Stable instance identity.</summary>
    public required ulong InstanceId { get; init; }
    /// <summary>Existing mesh resource identity, required for adds.</summary>
    public ulong MeshId { get; init; }
    /// <summary>Optional replacement source-object identity.</summary>
    public ulong? ObjectId { get; init; }
    /// <summary>Optional replacement category filter mask.</summary>
    public ulong? CategoryMask { get; init; }
    /// <summary>Optional replacement acceleration layer.</summary>
    public SceneLayer? Layer { get; init; }
    /// <summary>Optional affine row-major model transform.</summary>
    public double[]? RowMajorTransform { get; init; }

    /// <summary>Creates a partial update for one existing instance.</summary>
    public static SceneInstanceDelta Update(
        ulong instanceId,
        double[]? rowMajorTransform = null,
        ulong? objectId = null,
        ulong? categoryMask = null,
        SceneLayer? layer = null) =>
        new()
        {
            Operation = SceneDeltaOperation.Update,
            InstanceId = instanceId,
            RowMajorTransform = rowMajorTransform,
            ObjectId = objectId,
            CategoryMask = categoryMask,
            Layer = layer,
        };

    /// <summary>Creates an add operation referencing an existing mesh resource.</summary>
    public static SceneInstanceDelta Add(
        ulong meshId,
        double[] rowMajorTransform,
        ulong objectId,
        ulong instanceId,
        ulong categoryMask,
        SceneLayer layer) =>
        new()
        {
            Operation = SceneDeltaOperation.Add,
            InstanceId = instanceId,
            MeshId = meshId,
            RowMajorTransform = rowMajorTransform,
            ObjectId = objectId,
            CategoryMask = categoryMask,
            Layer = layer,
        };

    /// <summary>Creates a removal for one existing instance.</summary>
    public static SceneInstanceDelta Remove(ulong instanceId) =>
        new() { Operation = SceneDeltaOperation.Remove, InstanceId = instanceId };
}

/// <summary>Complete provenance for one immutable scene transition.</summary>
public sealed record SceneUpdateReport
{
    /// <summary>Number of requested atomic deltas.</summary>
    public required ulong DeltaCount { get; init; }
    /// <summary>Changes affecting the static layer.</summary>
    public required ulong StaticChangeCount { get; init; }
    /// <summary>Changes affecting the dynamic layer.</summary>
    public required ulong DynamicChangeCount { get; init; }
    /// <summary>Bottom-level hierarchies shared from the prior snapshot.</summary>
    public required ulong ReusedBlasCount { get; init; }
    /// <summary>Bottom-level hierarchies rebuilt for changed mesh resources.</summary>
    public required ulong RebuiltBlasCount { get; init; }
    /// <summary>Static-layer TLAS decision.</summary>
    public required HierarchyUpdateKind StaticTlasUpdate { get; init; }
    /// <summary>Dynamic-layer TLAS decision.</summary>
    public required HierarchyUpdateKind DynamicTlasUpdate { get; init; }
    /// <summary>Largest post-refit cost ratio observed in this transaction.</summary>
    public required double MaximumRefitQualityRatio { get; init; }
    /// <summary>Total native transaction duration.</summary>
    public required ulong UpdateTimeMicroseconds { get; init; }
    /// <summary>Content hash of the prior immutable snapshot.</summary>
    public required string PreviousHash { get; init; }
    /// <summary>Content hash of the resulting immutable snapshot.</summary>
    public required string CurrentHash { get; init; }
}

/// <summary>Build, memory, hierarchy, precision, and identity metrics for a scene snapshot.</summary>
public sealed record SceneStatistics
{
    /// <summary>Unique mesh resources after content deduplication.</summary>
    public required ulong MeshResourceCount { get; init; }
    /// <summary>Geometry occurrence count.</summary>
    public required ulong InstanceCount { get; init; }
    /// <summary>Unique stored triangle count.</summary>
    public required ulong UniqueTriangleCount { get; init; }
    /// <summary>Effective instanced triangle count.</summary>
    public required ulong InstancedTriangleCount { get; init; }
    /// <summary>Total bottom-level acceleration nodes.</summary>
    public required ulong BlasNodeCount { get; init; }
    /// <summary>Top-level acceleration nodes.</summary>
    public required ulong TlasNodeCount { get; init; }
    /// <summary>Maximum hierarchy depth.</summary>
    public required ulong MaximumBvhDepth { get; init; }
    /// <summary>Approximate native CPU memory.</summary>
    public required ulong ApproximateMemoryBytes { get; init; }
    /// <summary>Compilation duration in microseconds.</summary>
    public required ulong BuildTimeMicroseconds { get; init; }
    /// <summary>Ray-query worker count.</summary>
    public required uint ThreadCount { get; init; }
    /// <summary>Internal canonical origin rebase in metres.</summary>
    public required (double X, double Y, double Z) RebaseOriginMeters { get; init; }
    /// <summary>Deterministic BLAKE3 scene digest.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Builds a native scene from reusable mesh resources and transformed occurrences.</summary>
public sealed class XvarnaSceneBuilder : IDisposable
{
    private const uint ExpectedOptionsSize = 32;
    private SafeSceneHandle? handle;
    private readonly double unitScaleToMeters;

    /// <summary>Creates an empty builder with validated unit, tolerance, BVH, and thread settings.</summary>
    public unsafe XvarnaSceneBuilder(SceneBuildOptions options)
    {
        ValidateOptions(options);
        NativeLibraryBootstrap.Initialize();
        NativeSceneOptions native = new()
        {
            StructureSize = ExpectedOptionsSize,
            MaximumLeafSize = options.MaximumLeafSize,
            ThreadCount = options.ThreadCount,
            Reserved = 0,
            UnitScaleToMeters = options.UnitScaleToMeters,
            AbsoluteToleranceMeters = options.AbsoluteToleranceMeters,
        };
        ulong rawHandle = 0;
        NativeStatus status = NativeMethods.SceneCreate(&native, &rawHandle);
        ThrowIfFailed(status);
        handle = new SafeSceneHandle(rawHandle);
        unitScaleToMeters = options.UnitScaleToMeters;
    }

    /// <summary>Adds a packed XYZ/triangle mesh and returns its deduplicated resource identifier.</summary>
    public unsafe ulong AddMesh(double[] positions, uint[] triangles)
    {
        ArgumentNullException.ThrowIfNull(positions);
        ArgumentNullException.ThrowIfNull(triangles);
        if (positions.Length == 0 || positions.Length % 3 != 0)
        {
            throw new ArgumentException("Position buffer must contain one or more XYZ triples.", nameof(positions));
        }
        if (triangles.Length == 0 || triangles.Length % 3 != 0)
        {
            throw new ArgumentException("Triangle buffer must contain one or more index triples.", nameof(triangles));
        }

        ulong meshId = 0;
        fixed (double* positionsPointer = positions)
        fixed (uint* trianglesPointer = triangles)
        {
            NativeStatus status = NativeMethods.SceneAddMesh(
                GetHandle(),
                positionsPointer,
                (nuint)(positions.Length / 3),
                trianglesPointer,
                (nuint)(triangles.Length / 3),
                &meshId);
            ThrowIfFailed(status);
        }
        return meshId;
    }

    /// <summary>Adds an affine row-major transformed occurrence of a mesh resource.</summary>
    public unsafe void AddInstance(
        ulong meshId,
        double[] rowMajorTransform,
        ulong objectId,
        ulong instanceId,
        ulong categoryMask = ulong.MaxValue,
        SceneLayer layer = SceneLayer.Static)
    {
        ArgumentNullException.ThrowIfNull(rowMajorTransform);
        if (rowMajorTransform.Length != 16)
        {
            throw new ArgumentException("Transform must contain exactly sixteen row-major values.", nameof(rowMajorTransform));
        }
        if (!Enum.IsDefined(layer))
        {
            throw new ArgumentOutOfRangeException(nameof(layer));
        }
        fixed (double* transformPointer = rowMajorTransform)
        {
            ThrowIfFailed(NativeMethods.SceneAddInstanceLayered(
                GetHandle(),
                meshId,
                transformPointer,
                objectId,
                instanceId,
                categoryMask,
                (uint)layer));
        }
    }

    /// <summary>Compiles and transfers ownership into an immutable, thread-safe scene.</summary>
    public unsafe XvarnaScene Build()
    {
        SafeSceneHandle ownedHandle = GetHandle();
        NativeSceneStats native = default;
        ThrowIfFailed(NativeMethods.SceneBuild(ownedHandle, &native));
        SceneStatistics statistics = SceneConversions.ToStatistics(native);
        handle = null;
        return new XvarnaScene(ownedHandle, unitScaleToMeters, statistics);
    }

    /// <inheritdoc />
    public void Dispose()
    {
        handle?.Dispose();
        handle = null;
    }

    private SafeSceneHandle GetHandle() =>
        handle is null || handle.IsClosed || handle.IsInvalid
            ? throw new ObjectDisposedException(nameof(XvarnaSceneBuilder))
            : handle;

    private static void ValidateOptions(SceneBuildOptions options)
    {
        if (!double.IsFinite(options.UnitScaleToMeters) || options.UnitScaleToMeters <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Unit scale must be finite and positive.");
        }
        if (!double.IsFinite(options.AbsoluteToleranceMeters) || options.AbsoluteToleranceMeters <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Tolerance must be finite and positive.");
        }
        if (options.MaximumLeafSize is < 1 or > 64)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Maximum leaf size must be between 1 and 64.");
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

/// <summary>Immutable, thread-safe native scene snapshot supporting batched CPU ray queries.</summary>
public sealed partial class XvarnaScene : IDisposable
{
    private const uint ExpectedHitSize = 64;
    private readonly SafeSceneHandle handle;
    private readonly double unitScaleToMeters;

    internal XvarnaScene(SafeSceneHandle handle, double unitScaleToMeters, SceneStatistics statistics)
    {
        this.handle = handle;
        this.unitScaleToMeters = unitScaleToMeters;
        Statistics = statistics;
    }

    /// <summary>Immutable snapshot statistics and deterministic content identity.</summary>
    public SceneStatistics Statistics { get; private set; }

    /// <summary>Monotonic managed revision incremented after every successful delta transaction.</summary>
    public ulong Revision { get; private set; }

    internal SafeSceneHandle NativeHandle => handle;

    internal double UnitScaleToMeters => unitScaleToMeters;

    /// <summary>Applies an ordered atomic instance transaction and publishes a new native snapshot.</summary>
    public unsafe SceneUpdateReport ApplyDeltas(
        IReadOnlyList<SceneInstanceDelta> deltas,
        SceneUpdatePolicy policy = default)
    {
        ArgumentNullException.ThrowIfNull(deltas);
        ThrowIfDisposed();
        policy = policy == default ? SceneUpdatePolicy.Default : policy;
        ValidateUpdatePolicy(policy);
        NativeSceneInstanceDelta[] native = new NativeSceneInstanceDelta[deltas.Count];
        for (int index = 0; index < deltas.Count; index++)
        {
            SceneInstanceDelta delta = deltas[index] ?? throw new ArgumentException("Scene delta cannot be null.", nameof(deltas));
            NativeSceneInstanceDelta value = new()
            {
                StructureSize = 176,
                Operation = (uint)delta.Operation,
                Flags = 0,
                Layer = (uint)(delta.Layer ?? SceneLayer.Static),
                InstanceId = delta.InstanceId,
                MeshId = delta.MeshId,
                ObjectId = delta.ObjectId ?? 0,
                CategoryMask = delta.CategoryMask ?? 0,
            };
            switch (delta.Operation)
            {
                case SceneDeltaOperation.Update:
                    value.Flags = (delta.RowMajorTransform is null ? 0U : 1U)
                        | (delta.ObjectId.HasValue ? 2U : 0U)
                        | (delta.CategoryMask.HasValue ? 4U : 0U)
                        | (delta.Layer.HasValue ? 8U : 0U);
                    break;
                case SceneDeltaOperation.Add:
                    if (delta.RowMajorTransform is null || !delta.ObjectId.HasValue
                        || !delta.CategoryMask.HasValue || !delta.Layer.HasValue)
                    {
                        throw new ArgumentException("Add deltas require transform, object, category, and layer.", nameof(deltas));
                    }
                    break;
                case SceneDeltaOperation.Remove:
                    if (delta.RowMajorTransform is not null || delta.ObjectId.HasValue
                        || delta.CategoryMask.HasValue || delta.Layer.HasValue || delta.MeshId != 0)
                    {
                        throw new ArgumentException("Remove deltas accept only InstanceId.", nameof(deltas));
                    }
                    break;
                default:
                    throw new ArgumentOutOfRangeException(nameof(deltas), "Unknown scene delta operation.");
            }
            if (delta.RowMajorTransform is not null)
            {
                ValidateTransform(delta.RowMajorTransform);
                fixed (double* source = delta.RowMajorTransform)
                {
                    double* destination = value.Transform;
                    Buffer.MemoryCopy(source, destination, 16 * sizeof(double), 16 * sizeof(double));
                }
            }
            native[index] = value;
        }
        NativeSceneUpdateOptions nativePolicy = new()
        {
            StructureSize = 16,
            MaximumConsecutiveRefits = policy.MaximumConsecutiveRefits,
            MaximumRefitQualityRatio = policy.MaximumRefitQualityRatio,
        };
        NativeSceneUpdateReport nativeReport = default;
        NativeSceneStats nativeStats = default;
        fixed (NativeSceneInstanceDelta* deltaPointer = native)
        {
            ThrowIfFailed(NativeMethods.SceneApplyInstanceDeltas(
                handle,
                deltaPointer,
                (nuint)native.Length,
                &nativePolicy,
                &nativeReport,
                &nativeStats));
        }
        Statistics = SceneConversions.ToStatistics(nativeStats);
        Revision = checked(Revision + 1);
        return ConvertUpdateReport(nativeReport);
    }

    /// <summary>Replaces one shared resource while preserving its MeshId.</summary>
    public unsafe SceneUpdateReport ReplaceMesh(
        ulong meshId,
        double[] positions,
        uint[] triangles,
        SceneUpdatePolicy policy = default)
    {
        ArgumentNullException.ThrowIfNull(positions);
        ArgumentNullException.ThrowIfNull(triangles);
        ThrowIfDisposed();
        if (positions.Length == 0 || positions.Length % 3 != 0
            || triangles.Length == 0 || triangles.Length % 3 != 0)
        {
            throw new ArgumentException("Mesh buffers must contain non-empty XYZ/index triples.");
        }
        policy = policy == default ? SceneUpdatePolicy.Default : policy;
        ValidateUpdatePolicy(policy);
        NativeSceneUpdateOptions nativePolicy = new()
        {
            StructureSize = 16,
            MaximumConsecutiveRefits = policy.MaximumConsecutiveRefits,
            MaximumRefitQualityRatio = policy.MaximumRefitQualityRatio,
        };
        NativeSceneUpdateReport nativeReport = default;
        NativeSceneStats nativeStats = default;
        fixed (double* positionsPointer = positions)
        fixed (uint* trianglesPointer = triangles)
        {
            ThrowIfFailed(NativeMethods.SceneReplaceMesh(
                handle,
                meshId,
                positionsPointer,
                (nuint)(positions.Length / 3),
                trianglesPointer,
                (nuint)(triangles.Length / 3),
                &nativePolicy,
                &nativeReport,
                &nativeStats));
        }
        Statistics = SceneConversions.ToStatistics(nativeStats);
        Revision = checked(Revision + 1);
        return ConvertUpdateReport(nativeReport);
    }

    /// <summary>Traces an ordered closest-hit batch. Inputs and distances use source model units.</summary>
    public unsafe SceneHit[] TraceClosest(IReadOnlyList<SceneRay> rays)
    {
        ArgumentNullException.ThrowIfNull(rays);
        ThrowIfDisposed();
        NativeRay[] nativeRays = ConvertRays(rays);
        NativeHit[] nativeHits = new NativeHit[nativeRays.Length];
        fixed (NativeRay* raysPointer = nativeRays)
        fixed (NativeHit* hitsPointer = nativeHits)
        {
            ThrowIfFailed(NativeMethods.SceneTraceClosest(
                handle,
                raysPointer,
                (nuint)nativeRays.Length,
                hitsPointer,
                (nuint)nativeHits.Length));
        }

        return ConvertHits(nativeHits);
    }

    internal SceneHit[] ConvertHits(NativeHit[] nativeHits)
    {
        SceneHit[] hits = new SceneHit[nativeHits.Length];
        for (int index = 0; index < hits.Length; index++)
        {
            NativeHit hit = nativeHits[index];
            if (hit.StructureSize != ExpectedHitSize)
            {
                throw new InvalidOperationException($"Native hit layout mismatch: {hit.StructureSize}.");
            }
            hits[index] = new(
                (hit.Flags & 1) != 0,
                hit.Distance / unitScaleToMeters,
                hit.BarycentricU,
                hit.BarycentricV,
                hit.ObjectId,
                hit.InstanceId,
                hit.MeshId,
                hit.TriangleId,
                (hit.Flags & 2) != 0);
        }
        return hits;
    }

    /// <summary>Traces an ordered visibility batch. Inputs use source model units.</summary>
    public unsafe bool[] TraceAny(IReadOnlyList<SceneRay> rays)
    {
        ArgumentNullException.ThrowIfNull(rays);
        ThrowIfDisposed();
        NativeRay[] nativeRays = ConvertRays(rays);
        byte[] nativeHits = new byte[nativeRays.Length];
        fixed (NativeRay* raysPointer = nativeRays)
        fixed (byte* hitsPointer = nativeHits)
        {
            ThrowIfFailed(NativeMethods.SceneTraceAny(
                handle,
                raysPointer,
                (nuint)nativeRays.Length,
                hitsPointer,
                (nuint)nativeHits.Length));
        }
        return Array.ConvertAll(nativeHits, value => value != 0);
    }

    /// <inheritdoc />
    public void Dispose() => handle.Dispose();

    internal NativeRay[] ConvertRays(IReadOnlyList<SceneRay> rays)
    {
        NativeRay[] converted = new NativeRay[rays.Count];
        for (int index = 0; index < rays.Count; index++)
        {
            SceneRay ray = rays[index];
            converted[index] = new()
            {
                OriginX = ray.OriginX * unitScaleToMeters,
                OriginY = ray.OriginY * unitScaleToMeters,
                OriginZ = ray.OriginZ * unitScaleToMeters,
                DirectionX = ray.DirectionX,
                DirectionY = ray.DirectionY,
                DirectionZ = ray.DirectionZ,
                TMin = ray.MinimumDistance * unitScaleToMeters,
                TMax = ray.MaximumDistance * unitScaleToMeters,
                CategoryMask = ray.CategoryMask,
            };
        }
        return converted;
    }

    private static void ValidateTransform(double[] transform)
    {
        if (transform.Length != 16 || transform.Any(value => !double.IsFinite(value)))
        {
            throw new ArgumentException("Transform must contain sixteen finite row-major values.", nameof(transform));
        }
    }

    private static void ValidateUpdatePolicy(SceneUpdatePolicy policy)
    {
        if (!double.IsFinite(policy.MaximumRefitQualityRatio)
            || policy.MaximumRefitQualityRatio < 1.0
            || policy.MaximumConsecutiveRefits == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(policy));
        }
    }

    private static unsafe SceneUpdateReport ConvertUpdateReport(NativeSceneUpdateReport value)
    {
        if (value.StructureSize != 128 || Marshal.SizeOf<NativeSceneUpdateReport>() != 128)
        {
            throw new InvalidOperationException($"Native scene-update report layout mismatch: {value.StructureSize}.");
        }
        return new()
        {
            DeltaCount = value.DeltaCount,
            StaticChangeCount = value.StaticChangeCount,
            DynamicChangeCount = value.DynamicChangeCount,
            ReusedBlasCount = value.ReusedBlasCount,
            RebuiltBlasCount = value.RebuiltBlasCount,
            UpdateTimeMicroseconds = value.UpdateTimeMicroseconds,
            MaximumRefitQualityRatio = value.MaximumRefitQualityRatio,
            StaticTlasUpdate = (HierarchyUpdateKind)(value.Flags & 3),
            DynamicTlasUpdate = (HierarchyUpdateKind)((value.Flags >> 2) & 3),
            PreviousHash = ConvertHash(value.PreviousHash),
            CurrentHash = ConvertHash(value.CurrentHash),
        };
    }

    private static unsafe string ConvertHash(ulong* words)
    {
        Span<byte> hash = stackalloc byte[32];
        for (int index = 0; index < 4; index++)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(hash.Slice(index * 8, 8), words[index]);
        }
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    private void ThrowIfDisposed()
    {
        if (handle.IsClosed || handle.IsInvalid)
        {
            throw new ObjectDisposedException(nameof(XvarnaScene));
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

internal sealed class SafeSceneHandle : SafeHandle
{
    internal SafeSceneHandle(ulong handleValue)
        : base(nint.Zero, true)
    {
        SetHandle(checked((nint)handleValue));
    }

    public override bool IsInvalid => handle == nint.Zero;

    protected override bool ReleaseHandle() =>
        NativeMethods.SceneRelease(unchecked((ulong)handle)) == NativeStatus.Success;
}

internal static class SceneConversions
{
    private const uint ExpectedStatisticsSize = 136;

    internal static SceneStatistics ToStatistics(NativeSceneStats native)
    {
        if (native.StructureSize != ExpectedStatisticsSize || native.StructureSize != Marshal.SizeOf<NativeSceneStats>())
        {
            throw new InvalidOperationException($"Native scene-statistics layout mismatch: native {native.StructureSize}, managed {Marshal.SizeOf<NativeSceneStats>()}.");
        }
        Span<byte> hash = stackalloc byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(hash[0..8], native.ContentHash0);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[8..16], native.ContentHash1);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[16..24], native.ContentHash2);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[24..32], native.ContentHash3);
        return new()
        {
            MeshResourceCount = native.MeshResourceCount,
            InstanceCount = native.InstanceCount,
            UniqueTriangleCount = native.UniqueTriangleCount,
            InstancedTriangleCount = native.InstancedTriangleCount,
            BlasNodeCount = native.BlasNodeCount,
            TlasNodeCount = native.TlasNodeCount,
            MaximumBvhDepth = native.MaximumBvhDepth,
            ApproximateMemoryBytes = native.ApproximateMemoryBytes,
            BuildTimeMicroseconds = native.BuildTimeMicroseconds,
            ThreadCount = native.ThreadCount,
            RebaseOriginMeters = (native.RebaseOriginX, native.RebaseOriginY, native.RebaseOriginZ),
            ContentHash = Convert.ToHexString(hash).ToLowerInvariant(),
        };
    }
}
