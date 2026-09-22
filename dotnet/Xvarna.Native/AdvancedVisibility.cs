using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Source-model 3D point used by advanced visibility APIs.</summary>
public readonly record struct SpatialPoint(double X, double Y, double Z);

/// <summary>Oriented observer camera with an explicit importance weight.</summary>
public readonly record struct ViewCamera(
    ulong ObserverId, SpatialPoint Position, SpatialPoint Forward, SpatialPoint Up, double Weight = 1.0);

/// <summary>Triangular target patch; repeated target IDs are aggregated natively.</summary>
public readonly record struct ViewTargetTriangle(
    ulong TargetId, SpatialPoint First, SpatialPoint Second, SpatialPoint Third,
    ulong CategoryMask, double CategoryWeight = 1.0);

/// <summary>Scientific Target/Weighted/Green View policy.</summary>
public readonly record struct TargetViewOptions(
    uint SamplesPerPatch = 16,
    double HorizontalFieldOfViewDegrees = 90.0,
    double VerticalFieldOfViewDegrees = 60.0,
    double MaximumDistance = 1000.0,
    double EndpointClearance = 1.0e-4,
    ulong OccluderCategoryMask = ulong.MaxValue,
    ulong TargetCategoryMask = ulong.MaxValue,
    ulong GreenCategoryMask = 0,
    double DistanceReference = 25.0,
    double DistanceExponent = 2.0,
    double DirectionExponent = 1.0,
    bool TwoSidedTargets = false)
{
    /// <summary>Production default Target/Weighted/Green View policy.</summary>
    public static TargetViewOptions Default { get; } = new(16, 90.0, 60.0, 1000.0, 1.0e-4,
        ulong.MaxValue, ulong.MaxValue, 0, 25.0, 2.0, 1.0, false);
}

/// <summary>One observer/logical-target solid-angle and weighted contribution.</summary>
public readonly record struct TargetViewEntry(
    ulong ObserverId, ulong TargetId, ulong CategoryMask,
    double PotentialSolidAngleSteradians, double VisibleSolidAngleSteradians,
    double VisibilityFraction, double FieldOfViewFraction, double WeightedFieldOfViewScore,
    double ConvergenceDeltaSteradians, ulong EligibleSampleCount, ulong VisibleSampleCount,
    ulong DominantBlockerObjectId, double DominantBlockedSolidAngleSteradians);

/// <summary>Raw Target View, Weighted View Quality, and Green View for one camera.</summary>
public readonly record struct TargetViewSummary(
    ulong ObserverId, double FieldOfViewSolidAngleSteradians,
    double PotentialTargetSolidAngleSteradians, double VisibleTargetSolidAngleSteradians,
    double TargetViewFraction, double TargetUniverseVisibilityFraction,
    double WeightedViewScore, double GreenViewIndex, double GreenShareOfVisibleTargets,
    ulong DominantTargetId, double ConvergenceDeltaSteradians);

/// <summary>Backend used by an aggregated advanced DAENA analysis.</summary>
public enum DaenaExecutionBackend
{
    /// Canonical f64 CPU traversal.
    Cpu = 0,
    /// Portable f32 GPU traversal.
    PortableGpu = 1,
    /// GPU and CPU both produced batches after runtime fallback.
    Mixed = 2,
}

/// <summary>Immutable backend, adapter, transfer, precision, and fallback provenance.</summary>
public sealed record DaenaExecutionReport
{
    /// <summary>Aggregate backend.</summary>
    public required DaenaExecutionBackend Backend { get; init; }
    /// <summary>Selected adapter, empty for CPU.</summary>
    public required string AdapterName { get; init; }
    /// <summary>Whether portable session creation fell back to CPU.</summary>
    public required bool UsedCreationFallback { get; init; }
    /// <summary>Whether one or more domain batches used VAYU's persistent scratch arena.</summary>
    public required bool UsedReusableBuffers { get; init; }
    /// <summary>Number of domain batches retried on CPU.</summary>
    public required ulong RuntimeFallbackBatchCount { get; init; }
    /// <summary>Domain query batch count.</summary>
    public required ulong BatchCount { get; init; }
    /// <summary>Total ordered ray count.</summary>
    public required ulong RayCount { get; init; }
    /// <summary>Total portable dispatch count.</summary>
    public required ulong DispatchCount { get; init; }
    /// <summary>Total upload/encoding microseconds.</summary>
    public required ulong UploadMicroseconds { get; init; }
    /// <summary>Total execution/wait microseconds.</summary>
    public required ulong ExecutionMicroseconds { get; init; }
    /// <summary>Total readback/decode microseconds.</summary>
    public required ulong ReadbackMicroseconds { get; init; }
    /// <summary>Maximum measured f32 conversion error in metres.</summary>
    public required double MaximumPrecisionErrorMeters { get; init; }
    /// <summary>First creation/runtime fallback reason.</summary>
    public required string FallbackReason { get; init; }
}

/// <summary>Immutable advanced DAENA target-view result.</summary>
public sealed record TargetViewResult
{
    /// <summary>Source cameras.</summary>
    public required IReadOnlyList<ViewCamera> Observers { get; init; }
    /// <summary>Source triangular target patches.</summary>
    public required IReadOnlyList<ViewTargetTriangle> TargetPatches { get; init; }
    /// <summary>Sorted logical target IDs.</summary>
    public required IReadOnlyList<ulong> TargetIds { get; init; }
    /// <summary>Observer-major target entries.</summary>
    public required IReadOnlyList<TargetViewEntry> Entries { get; init; }
    /// <summary>One summary per observer.</summary>
    public required IReadOnlyList<TargetViewSummary> Summaries { get; init; }
    /// <summary>Transparent scientific policy.</summary>
    public required TargetViewOptions Options { get; init; }
    /// <summary>Native analysis runtime.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic result identity.</summary>
    public required string ContentHash { get; init; }
    /// <summary>Execution provenance when a backend session produced the result.</summary>
    public DaenaExecutionReport? Execution { get; init; }
}

/// <summary>Cone-like protected corridor terminating in a circular target aperture.</summary>
public readonly record struct ViewCorridorDefinition(
    ulong CorridorId, SpatialPoint Origin, SpatialPoint Target, SpatialPoint Up, double TargetRadius);

/// <summary>Protected-corridor sampling policy.</summary>
public readonly record struct ViewCorridorOptions(
    uint SampleCount = 512, double EndpointClearance = 1.0e-4, ulong CategoryMask = ulong.MaxValue);

/// <summary>One target-aperture sample with exact first-blocker identity.</summary>
public readonly record struct ViewCorridorSample(
    ulong CorridorId, VisibilityState State, SpatialPoint AperturePoint, SpatialPoint Direction,
    double ApertureDistance, double FirstHitDistance, ulong BlockerObjectId,
    ulong BlockerInstanceId, ulong BlockerMeshId, uint BlockerTriangleId);

/// <summary>Protected-corridor open aperture and conflict summary.</summary>
public readonly record struct ViewCorridorSummary(
    ulong CorridorId, double ApertureSolidAngleSteradians, ulong OpenSampleCount,
    ulong BlockedSampleCount, double OpenFraction, double OpenSolidAngleSteradians,
    double ConvergenceDelta, ulong DominantBlockerObjectId, double DominantBlockerFraction,
    double NearestBlockerDistance);

/// <summary>Immutable DAENA protected-corridor result.</summary>
public sealed record ViewCorridorResult
{
    /// <summary>Source corridors.</summary>
    public required IReadOnlyList<ViewCorridorDefinition> Corridors { get; init; }
    /// <summary>One aggregate per corridor.</summary>
    public required IReadOnlyList<ViewCorridorSummary> Summaries { get; init; }
    /// <summary>Corridor-major aperture samples.</summary>
    public required IReadOnlyList<ViewCorridorSample> Samples { get; init; }
    /// <summary>Sampling policy.</summary>
    public required ViewCorridorOptions Options { get; init; }
    /// <summary>Native analysis runtime.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic result identity.</summary>
    public required string ContentHash { get; init; }
    /// <summary>Execution provenance when a backend session produced the result.</summary>
    public DaenaExecutionReport? Execution { get; init; }
}

/// <summary>Tangent-facing polyline path definition.</summary>
public sealed record ObserverPathDefinition
{
    /// <summary>Stable path identifier.</summary>
    public required ulong PathId { get; init; }
    /// <summary>Ordered source-model polyline vertices.</summary>
    public required IReadOnlyList<SpatialPoint> Vertices { get; init; }
    /// <summary>Camera-up direction.</summary>
    public SpatialPoint Up { get; init; } = new(0.0, 0.0, 1.0);
    /// <summary>Observer importance.</summary>
    public double Weight { get; init; } = 1.0;
}

/// <summary>Dynamic path spacing and nested Target View policy.</summary>
public readonly record struct ObserverPathOptions(double Spacing = 1.0, TargetViewOptions View = default)
{
    /// <summary>Production default path and nested view policy.</summary>
    public static ObserverPathOptions Default { get; } = new(1.0, TargetViewOptions.Default);
}

/// <summary>One ordered path sample and its view-quality metrics.</summary>
public readonly record struct ObserverPathSample(
    ulong PathId, ulong SampleIndex, double DistanceAlongPath, SpatialPoint Position,
    SpatialPoint Forward, double TargetViewFraction, double WeightedViewScore,
    double GreenViewIndex, ulong DominantTargetId, double ConvergenceDeltaSteradians);

/// <summary>Distance-weighted dynamic-path summary and best/worst locations.</summary>
public readonly record struct ObserverPathSummary(
    ulong PathId, double PathLength, ulong SampleCount, double MeanTargetViewFraction,
    double MeanWeightedViewScore, double MeanGreenViewIndex, double MinimumWeightedViewScore,
    double MaximumWeightedViewScore, ulong WorstSampleIndex, ulong BestSampleIndex);

/// <summary>Immutable Dynamic Observer Path result.</summary>
public sealed record ObserverPathResult
{
    /// <summary>Source paths.</summary>
    public required IReadOnlyList<ObserverPathDefinition> Paths { get; init; }
    /// <summary>One aggregate per path.</summary>
    public required IReadOnlyList<ObserverPathSummary> Summaries { get; init; }
    /// <summary>Path-major samples.</summary>
    public required IReadOnlyList<ObserverPathSample> Samples { get; init; }
    /// <summary>Source target patches.</summary>
    public required IReadOnlyList<ViewTargetTriangle> TargetPatches { get; init; }
    /// <summary>Sampling and view policy.</summary>
    public required ObserverPathOptions Options { get; init; }
    /// <summary>Native analysis runtime.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic result identity.</summary>
    public required string ContentHash { get; init; }
    /// <summary>Execution provenance when a backend session produced the result.</summary>
    public DaenaExecutionReport? Execution { get; init; }
}

public sealed partial class XvarnaScene
{
    private const uint ExpectedTargetViewOptionsSize = 96;
    private const uint ExpectedTargetViewEntrySize = 112;
    private const uint ExpectedTargetViewSummarySize = 96;
    private const uint ExpectedTargetViewMetadataSize = 72;
    private const uint ExpectedCorridorOptionsSize = 24;
    private const uint ExpectedCorridorSummarySize = 88;
    private const uint ExpectedCorridorSampleSize = 112;
    private const uint ExpectedCorridorMetadataSize = 72;
    private const uint ExpectedPathOptionsSize = 112;
    private const uint ExpectedPathSampleSize = 120;
    private const uint ExpectedPathSummarySize = 88;
    private const uint ExpectedPathMetadataSize = 64;

    /// <summary>Computes solid-angle Target/Weighted/Green View in native Rust.</summary>
    public unsafe TargetViewResult AnalyzeTargetView(
        IReadOnlyList<ViewCamera> observers,
        IReadOnlyList<ViewTargetTriangle> targetPatches,
        TargetViewOptions options)
    {
        ArgumentNullException.ThrowIfNull(observers);
        ArgumentNullException.ThrowIfNull(targetPatches);
        if (observers.Count == 0 || targetPatches.Count == 0)
            throw new ArgumentException("Observers and target patches must be non-empty.");
        ThrowIfDisposed();
        NativeViewObserver[] nativeObservers = observers.Select(ToNative).ToArray();
        NativeViewTargetPatch[] nativePatches = ConvertPatches(targetPatches);
        ulong[] targetIds = targetPatches
            .Where(patch => (patch.CategoryMask & options.TargetCategoryMask) != 0)
            .Select(patch => patch.TargetId).Distinct().Order().ToArray();
        if (targetIds.Length == 0) throw new ArgumentException("Target category filter excludes every patch.", nameof(options));
        int entryCount = checked(observers.Count * targetIds.Length);
        NativeTargetViewSummary[] nativeSummaries = new NativeTargetViewSummary[observers.Count];
        NativeTargetViewEntry[] nativeEntries = new NativeTargetViewEntry[entryCount];
        NativeTargetViewMetadata metadata = default;
        NativeTargetViewOptions nativeOptions = ToNative(options);
        fixed (NativeViewObserver* observerPointer = nativeObservers)
        fixed (NativeViewTargetPatch* patchPointer = nativePatches)
        fixed (NativeTargetViewSummary* summaryPointer = nativeSummaries)
        fixed (NativeTargetViewEntry* entryPointer = nativeEntries)
        {
            ThrowIfFailed(NativeMethods.SceneTargetView(handle, observerPointer, (nuint)nativeObservers.Length,
                patchPointer, (nuint)nativePatches.Length, &nativeOptions, &metadata,
                summaryPointer, (nuint)nativeSummaries.Length, entryPointer, (nuint)nativeEntries.Length));
        }
        if (metadata.StructureSize != ExpectedTargetViewMetadataSize || metadata.TargetCount != (ulong)targetIds.Length
            || nativeSummaries.Any(value => value.StructureSize != ExpectedTargetViewSummarySize)
            || nativeEntries.Any(value => value.StructureSize != ExpectedTargetViewEntrySize))
            throw new InvalidOperationException("Native DAENA Target View ABI layout mismatch.");
        return new()
        {
            Observers = observers.ToArray(),
            TargetPatches = targetPatches.ToArray(),
            TargetIds = targetIds,
            Options = options,
            Summaries = nativeSummaries.Select(value => new TargetViewSummary(value.ObserverId,
                value.FovSolidAngleSteradians, value.PotentialTargetSolidAngleSteradians,
                value.VisibleTargetSolidAngleSteradians, value.TargetViewFraction,
                value.TargetUniverseVisibilityFraction, value.WeightedViewScore, value.GreenViewIndex,
                value.GreenShareOfVisibleTargets, value.DominantTargetId, value.ConvergenceDeltaSteradians)).ToArray(),
            Entries = nativeEntries.Select(value => new TargetViewEntry(value.ObserverId, value.TargetId,
                value.CategoryMask, value.PotentialSolidAngleSteradians, value.VisibleSolidAngleSteradians,
                value.VisibilityFraction, value.FovFraction, value.WeightedFovScore,
                value.ConvergenceDeltaSteradians, value.EligibleSampleCount, value.VisibleSampleCount,
                value.DominantBlockerObjectId, value.DominantBlockedSolidAngleSteradians)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Computes aperture coverage and first-blocker conflicts for protected view corridors.</summary>
    public unsafe ViewCorridorResult AnalyzeViewCorridors(
        IReadOnlyList<ViewCorridorDefinition> corridors, ViewCorridorOptions options)
    {
        ArgumentNullException.ThrowIfNull(corridors);
        if (corridors.Count == 0) throw new ArgumentException("At least one corridor is required.", nameof(corridors));
        ThrowIfDisposed();
        int sampleCount = checked(corridors.Count * checked((int)options.SampleCount));
        NativeViewCorridor[] nativeCorridors = corridors.Select(value => new NativeViewCorridor
        {
            CorridorId = value.CorridorId,
            OriginX = value.Origin.X * unitScaleToMeters,
            OriginY = value.Origin.Y * unitScaleToMeters,
            OriginZ = value.Origin.Z * unitScaleToMeters,
            TargetX = value.Target.X * unitScaleToMeters,
            TargetY = value.Target.Y * unitScaleToMeters,
            TargetZ = value.Target.Z * unitScaleToMeters,
            UpX = value.Up.X,
            UpY = value.Up.Y,
            UpZ = value.Up.Z,
            TargetRadiusMeters = value.TargetRadius * unitScaleToMeters,
        }).ToArray();
        NativeViewCorridorOptions nativeOptions = new()
        {
            StructureSize = ExpectedCorridorOptionsSize,
            SampleCount = options.SampleCount,
            EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
        };
        NativeViewCorridorSummary[] nativeSummaries = new NativeViewCorridorSummary[corridors.Count];
        NativeViewCorridorSample[] nativeSamples = new NativeViewCorridorSample[sampleCount];
        NativeViewCorridorMetadata metadata = default;
        fixed (NativeViewCorridor* corridorPointer = nativeCorridors)
        fixed (NativeViewCorridorSummary* summaryPointer = nativeSummaries)
        fixed (NativeViewCorridorSample* samplePointer = nativeSamples)
        {
            ThrowIfFailed(NativeMethods.SceneViewCorridor(handle, corridorPointer, (nuint)nativeCorridors.Length,
                &nativeOptions, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                samplePointer, (nuint)nativeSamples.Length));
        }
        if (metadata.StructureSize != ExpectedCorridorMetadataSize
            || nativeSummaries.Any(value => value.StructureSize != ExpectedCorridorSummarySize)
            || nativeSamples.Any(value => value.StructureSize != ExpectedCorridorSampleSize))
            throw new InvalidOperationException("Native DAENA View Corridor ABI layout mismatch.");
        double inverse = 1.0 / unitScaleToMeters;
        return new()
        {
            Corridors = corridors.ToArray(),
            Options = options,
            Summaries = nativeSummaries.Select(value => new ViewCorridorSummary(value.CorridorId,
                value.ApertureSolidAngleSteradians, value.OpenSampleCount, value.BlockedSampleCount,
                value.OpenFraction, value.OpenSolidAngleSteradians, value.ConvergenceDelta,
                value.DominantBlockerObjectId, value.DominantBlockerFraction,
                value.NearestBlockerDistanceMeters * inverse)).ToArray(),
            Samples = nativeSamples.Select(value => new ViewCorridorSample(value.CorridorId,
                (VisibilityState)value.State, new(value.ApertureX * inverse, value.ApertureY * inverse, value.ApertureZ * inverse),
                new(value.DirectionX, value.DirectionY, value.DirectionZ), value.ApertureDistanceMeters * inverse,
                value.FirstHitDistanceMeters * inverse, value.BlockerObjectId, value.BlockerInstanceId,
                value.BlockerMeshId, value.BlockerTriangleId)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Evaluates tangent-facing Target/Weighted/Green View along polyline paths.</summary>
    public unsafe ObserverPathResult AnalyzeObserverPaths(
        IReadOnlyList<ObserverPathDefinition> paths,
        IReadOnlyList<ViewTargetTriangle> targetPatches,
        ObserverPathOptions options)
    {
        ArgumentNullException.ThrowIfNull(paths);
        ArgumentNullException.ThrowIfNull(targetPatches);
        if (paths.Count == 0 || targetPatches.Count == 0)
            throw new ArgumentException("Paths and target patches must be non-empty.");
        if (!double.IsFinite(options.Spacing) || options.Spacing <= 0.0)
            throw new ArgumentOutOfRangeException(nameof(options), "Path spacing must be finite and positive.");
        ThrowIfDisposed();
        List<NativePoint3> vertices = [];
        NativeObserverPath[] nativePaths = new NativeObserverPath[paths.Count];
        int expectedSamples = 0;
        for (int index = 0; index < paths.Count; index++)
        {
            ObserverPathDefinition path = paths[index];
            if (path.Vertices is null || path.Vertices.Count < 2) throw new ArgumentException("Every path needs at least two vertices.", nameof(paths));
            int offset = vertices.Count;
            vertices.AddRange(path.Vertices.Select(point => new NativePoint3
            {
                X = point.X * unitScaleToMeters,
                Y = point.Y * unitScaleToMeters,
                Z = point.Z * unitScaleToMeters,
            }));
            nativePaths[index] = new()
            {
                PathId = path.PathId,
                VertexOffset = checked((ulong)offset),
                VertexCount = checked((ulong)path.Vertices.Count),
                UpX = path.Up.X,
                UpY = path.Up.Y,
                UpZ = path.Up.Z,
                Weight = path.Weight,
            };
            double length = path.Vertices.Zip(path.Vertices.Skip(1), (first, second) => Distance(first, second)).Sum();
            expectedSamples = checked(expectedSamples + Math.Max(2, checked((int)Math.Ceiling(length / options.Spacing) + 1)));
        }
        NativeViewTargetPatch[] nativePatches = ConvertPatches(targetPatches);
        NativeObserverPathOptions nativeOptions = new()
        {
            StructureSize = ExpectedPathOptionsSize,
            SpacingMeters = options.Spacing * unitScaleToMeters,
            View = ToNative(options.View),
        };
        NativeObserverPathSummary[] nativeSummaries = new NativeObserverPathSummary[paths.Count];
        NativeObserverPathSample[] nativeSamples = new NativeObserverPathSample[expectedSamples];
        NativeObserverPathMetadata metadata = default;
        NativePoint3[] nativeVertices = vertices.ToArray();
        fixed (NativeObserverPath* pathPointer = nativePaths)
        fixed (NativePoint3* vertexPointer = nativeVertices)
        fixed (NativeViewTargetPatch* patchPointer = nativePatches)
        fixed (NativeObserverPathSummary* summaryPointer = nativeSummaries)
        fixed (NativeObserverPathSample* samplePointer = nativeSamples)
        {
            ThrowIfFailed(NativeMethods.SceneObserverPath(handle, pathPointer, (nuint)nativePaths.Length,
                vertexPointer, (nuint)nativeVertices.Length, patchPointer, (nuint)nativePatches.Length,
                &nativeOptions, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                samplePointer, (nuint)nativeSamples.Length));
        }
        if (metadata.StructureSize != ExpectedPathMetadataSize || metadata.SampleCount != (ulong)expectedSamples
            || nativeSummaries.Any(value => value.StructureSize != ExpectedPathSummarySize)
            || nativeSamples.Any(value => value.StructureSize != ExpectedPathSampleSize))
            throw new InvalidOperationException("Native DAENA Observer Path ABI layout mismatch.");
        double inverse = 1.0 / unitScaleToMeters;
        return new()
        {
            Paths = paths.ToArray(),
            TargetPatches = targetPatches.ToArray(),
            Options = options,
            Summaries = nativeSummaries.Select(value => new ObserverPathSummary(value.PathId,
                value.PathLengthMeters * inverse, value.SampleCount, value.MeanTargetViewFraction,
                value.MeanWeightedViewScore, value.MeanGreenViewIndex, value.MinimumWeightedViewScore,
                value.MaximumWeightedViewScore, value.WorstSampleIndex, value.BestSampleIndex)).ToArray(),
            Samples = nativeSamples.Select(value => new ObserverPathSample(value.PathId, value.SampleIndex,
                value.DistanceAlongPathMeters * inverse, new(value.PositionX * inverse, value.PositionY * inverse, value.PositionZ * inverse),
                new(value.ForwardX, value.ForwardY, value.ForwardZ), value.TargetViewFraction,
                value.WeightedViewScore, value.GreenViewIndex, value.DominantTargetId,
                value.ConvergenceDeltaSteradians)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    private NativeViewObserver ToNative(ViewCamera value) => new()
    {
        ObserverId = value.ObserverId,
        PositionX = value.Position.X * unitScaleToMeters,
        PositionY = value.Position.Y * unitScaleToMeters,
        PositionZ = value.Position.Z * unitScaleToMeters,
        ForwardX = value.Forward.X,
        ForwardY = value.Forward.Y,
        ForwardZ = value.Forward.Z,
        UpX = value.Up.X,
        UpY = value.Up.Y,
        UpZ = value.Up.Z,
        Weight = value.Weight,
    };

    private NativeViewTargetPatch[] ConvertPatches(IReadOnlyList<ViewTargetTriangle> values) => values.Select(value => new NativeViewTargetPatch
    {
        TargetId = value.TargetId,
        FirstX = value.First.X * unitScaleToMeters,
        FirstY = value.First.Y * unitScaleToMeters,
        FirstZ = value.First.Z * unitScaleToMeters,
        SecondX = value.Second.X * unitScaleToMeters,
        SecondY = value.Second.Y * unitScaleToMeters,
        SecondZ = value.Second.Z * unitScaleToMeters,
        ThirdX = value.Third.X * unitScaleToMeters,
        ThirdY = value.Third.Y * unitScaleToMeters,
        ThirdZ = value.Third.Z * unitScaleToMeters,
        CategoryMask = value.CategoryMask,
        CategoryWeight = value.CategoryWeight,
    }).ToArray();

    private NativeTargetViewOptions ToNative(TargetViewOptions options) => new()
    {
        StructureSize = ExpectedTargetViewOptionsSize,
        Flags = options.TwoSidedTargets ? 1U : 0U,
        SamplesPerPatch = options.SamplesPerPatch,
        HorizontalFovRadians = options.HorizontalFieldOfViewDegrees * Math.PI / 180.0,
        VerticalFovRadians = options.VerticalFieldOfViewDegrees * Math.PI / 180.0,
        MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
        EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
        DistanceReferenceMeters = options.DistanceReference * unitScaleToMeters,
        DistanceExponent = options.DistanceExponent,
        DirectionExponent = options.DirectionExponent,
        OccluderCategoryMask = options.OccluderCategoryMask,
        TargetCategoryMask = options.TargetCategoryMask,
        GreenCategoryMask = options.GreenCategoryMask,
    };

    private static double Distance(SpatialPoint first, SpatialPoint second)
    {
        double x = second.X - first.X, y = second.Y - first.Y, z = second.Z - first.Z;
        return Math.Sqrt(x * x + y * y + z * z);
    }
}
