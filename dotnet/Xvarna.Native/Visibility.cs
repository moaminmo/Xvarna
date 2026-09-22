using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>A source-model eye point with an oriented planar frame.</summary>
public readonly record struct PlanarViewpoint(
    ulong ViewpointId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double PlaneNormalX,
    double PlaneNormalY,
    double PlaneNormalZ,
    double ForwardX,
    double ForwardY,
    double ForwardZ);

/// <summary>Designer-facing planar isovist policy.</summary>
public readonly record struct IsovistOptions(
    uint SampleCount = 720,
    double FieldOfViewDegrees = 360.0,
    double MaximumDistance = 100.0,
    double EyeOffset = 1.0e-4,
    ulong CategoryMask = ulong.MaxValue);

/// <summary>State of one sampled isovist direction.</summary>
public enum IsovistRayState
{
    /// <summary>Scene geometry was hit.</summary>
    Occluded = 0,
    /// <summary>No geometry was hit before the radial limit.</summary>
    OpenAtLimit = 1,
}

/// <summary>One ordered boundary sample with stable first-hit identity.</summary>
public readonly record struct IsovistRay(
    IsovistRayState State,
    double DirectionX,
    double DirectionY,
    double DirectionZ,
    double EndpointX,
    double EndpointY,
    double EndpointZ,
    double Distance,
    ulong ObjectId,
    ulong InstanceId,
    ulong MeshId,
    uint TriangleId);

/// <summary>Spatial, radial, convergence, and dominant-occluder metrics.</summary>
public readonly record struct IsovistSummary(
    ulong ViewpointId,
    double Area,
    double Perimeter,
    double CentroidDistance,
    double MeanRadial,
    double MinimumRadial,
    double MaximumRadial,
    double RadialStandardDeviation,
    double RadialSkewness,
    double Compactness,
    double AreaConvergenceDelta,
    ulong OccludedCount,
    ulong OpenCount,
    ulong DominantOccluderObjectId,
    double DominantOccluderFraction);

/// <summary>Immutable viewpoint-major DAENA isovist result.</summary>
public sealed record IsovistResult
{
    /// <summary>Source viewpoints.</summary>
    public required IReadOnlyList<PlanarViewpoint> Viewpoints { get; init; }
    /// <summary>One aggregate per viewpoint.</summary>
    public required IReadOnlyList<IsovistSummary> Summaries { get; init; }
    /// <summary>Ordered viewpoint-major boundary samples.</summary>
    public required IReadOnlyList<IsovistRay> Rays { get; init; }
    /// <summary>Sampling and query policy.</summary>
    public required IsovistOptions Options { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>A weighted eye point in a directed visibility study.</summary>
public readonly record struct VisibilityObserverPoint(
    ulong ObserverId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double Weight = 1.0);

/// <summary>A privacy/view target with optional directional facing.</summary>
public readonly record struct VisibilityTargetPoint(
    ulong TargetId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double Sensitivity = 1.0,
    bool HasFacing = false,
    double FacingX = 0.0,
    double FacingY = 0.0,
    double FacingZ = 0.0);

/// <summary>Directed intervisibility and privacy policy.</summary>
public readonly record struct IntervisibilityOptions(
    double EndpointClearance = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    double PrivacyReferenceDistance = 10.0,
    double FacingExponent = 1.0,
    ulong CategoryMask = ulong.MaxValue);

/// <summary>Classification of one endpoint pair.</summary>
public enum VisibilityState
{
    /// <summary>The direct segment is unobstructed.</summary>
    Visible = 0,
    /// <summary>The direct segment is blocked.</summary>
    Blocked = 1,
    /// <summary>The endpoint distance exceeds the policy.</summary>
    OutOfRange = 2,
    /// <summary>The endpoints are coincident.</summary>
    Coincident = 3,
}

/// <summary>One row-major directed observer-target entry.</summary>
public readonly record struct IntervisibilityEntry(
    VisibilityState State,
    double Distance,
    double DirectionX,
    double DirectionY,
    double DirectionZ,
    double PrivacyRisk,
    double FacingFactor,
    double DistanceFactor,
    ulong BlockerObjectId,
    ulong BlockerInstanceId,
    ulong BlockerMeshId,
    uint BlockerTriangleId);

/// <summary>One observer-row aggregate.</summary>
public readonly record struct ObserverVisibilitySummary(
    ulong ObserverId,
    ulong VisibleCount,
    double VisibleFraction,
    double TotalPrivacyRisk,
    double PeakPrivacyRisk);

/// <summary>One target-column exposure aggregate.</summary>
public readonly record struct TargetVisibilitySummary(
    ulong TargetId,
    ulong VisibleObserverCount,
    double ExposureFraction,
    double CumulativePrivacyRisk,
    double CombinedPrivacyRisk);

/// <summary>Immutable directed visibility and privacy matrix.</summary>
public sealed record IntervisibilityResult
{
    /// <summary>Source observer rows.</summary>
    public required IReadOnlyList<VisibilityObserverPoint> Observers { get; init; }
    /// <summary>Source target columns.</summary>
    public required IReadOnlyList<VisibilityTargetPoint> Targets { get; init; }
    /// <summary>Row-major matrix entries.</summary>
    public required IReadOnlyList<IntervisibilityEntry> Entries { get; init; }
    /// <summary>One aggregate per observer.</summary>
    public required IReadOnlyList<ObserverVisibilitySummary> ObserverSummaries { get; init; }
    /// <summary>One aggregate per target.</summary>
    public required IReadOnlyList<TargetVisibilitySummary> TargetSummaries { get; init; }
    /// <summary>Query and risk policy.</summary>
    public required IntervisibilityOptions Options { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>A finite source-model visibility graph node.</summary>
public readonly record struct VisibilityNodePoint(
    ulong NodeId,
    double PositionX,
    double PositionY,
    double PositionZ);

/// <summary>Undirected visibility-graph query and topology policy.</summary>
public readonly record struct VisibilityGraphOptions(
    double EndpointClearance = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    ulong CategoryMask = ulong.MaxValue,
    bool ComputeCentrality = true);

/// <summary>Spatially indexed radius/degree policy that avoids all-pairs materialization.</summary>
public readonly record struct SparseVisibilityGraphOptions(
    double EndpointClearance = 1.0e-4,
    double MaximumDistance = 25.0,
    ulong CategoryMask = ulong.MaxValue,
    uint MaximumNeighbors = 16,
    bool ComputeCentrality = false);

/// <summary>One unordered pair with obstruction attribution.</summary>
public readonly record struct VisibilityGraphPair(
    ulong FirstIndex,
    ulong SecondIndex,
    VisibilityState State,
    double Distance,
    ulong BlockerObjectId);

/// <summary>Per-node topology and centrality metrics.</summary>
public readonly record struct VisibilityNodeMetrics(
    ulong NodeId,
    ulong Degree,
    double DegreeCentrality,
    ulong ComponentIndex,
    double HarmonicCloseness,
    double BetweennessCentrality);

/// <summary>Immutable all-pairs DAENA visibility graph.</summary>
public sealed record VisibilityGraphResult
{
    /// <summary>Source nodes.</summary>
    public required IReadOnlyList<VisibilityNodePoint> Nodes { get; init; }
    /// <summary>Every unordered pair, including blockers and distance exclusions.</summary>
    public required IReadOnlyList<VisibilityGraphPair> Pairs { get; init; }
    /// <summary>One topology record per node.</summary>
    public required IReadOnlyList<VisibilityNodeMetrics> Metrics { get; init; }
    /// <summary>Directly visible edge count.</summary>
    public required ulong VisibleEdgeCount { get; init; }
    /// <summary>Connected component count.</summary>
    public required ulong ConnectedComponentCount { get; init; }
    /// <summary>Query and topology policy.</summary>
    public required VisibilityGraphOptions Options { get; init; }
    /// <summary>True when pairs are a spatially indexed radius/degree subset.</summary>
    public bool IsSparse { get; init; }
    /// <summary>Candidate degree cap used by sparse discovery; zero for exact all-pairs.</summary>
    public uint MaximumNeighbors { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

public sealed partial class XvarnaScene
{
    private const uint ExpectedIsovistOptionsSize = 40;
    private const uint ExpectedIsovistSummarySize = 128;
    private const uint ExpectedIsovistRaySize = 96;
    private const uint ExpectedIsovistMetadataSize = 72;
    private const uint ExpectedIntervisibilityOptionsSize = 48;
    private const uint ExpectedIntervisibilityEntrySize = 96;
    private const uint ExpectedVisibilitySummarySize = 48;
    private const uint ExpectedIntervisibilityMetadataSize = 72;
    private const uint ExpectedGraphOptionsSize = 32;
    private const uint ExpectedSparseGraphOptionsSize = 40;
    private const uint ExpectedGraphPairSize = 40;
    private const uint ExpectedGraphMetricsSize = 56;
    private const uint ExpectedGraphMetadataSize = 80;

    /// <summary>Computes complete sampled planar isovists in the native DAENA engine.</summary>
    public unsafe IsovistResult AnalyzeIsovists(
        IReadOnlyList<PlanarViewpoint> viewpoints,
        IsovistOptions options)
    {
        ArgumentNullException.ThrowIfNull(viewpoints);
        if (viewpoints.Count == 0) throw new ArgumentException("At least one viewpoint is required.", nameof(viewpoints));
        ThrowIfDisposed();
        int rayCount = checked(viewpoints.Count * checked((int)options.SampleCount));
        NativeViewpoint[] nativeViewpoints = viewpoints.Select(value => new NativeViewpoint
        {
            ViewpointId = value.ViewpointId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
            PlaneNormalX = value.PlaneNormalX,
            PlaneNormalY = value.PlaneNormalY,
            PlaneNormalZ = value.PlaneNormalZ,
            ForwardX = value.ForwardX,
            ForwardY = value.ForwardY,
            ForwardZ = value.ForwardZ,
        }).ToArray();
        NativeIsovistOptions nativeOptions = new()
        {
            StructureSize = ExpectedIsovistOptionsSize,
            SampleCount = options.SampleCount,
            FieldOfViewRadians = options.FieldOfViewDegrees * Math.PI / 180.0,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            EyeOffsetMeters = options.EyeOffset * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
        };
        NativeIsovistSummary[] nativeSummaries = new NativeIsovistSummary[viewpoints.Count];
        NativeIsovistRay[] nativeRays = new NativeIsovistRay[rayCount];
        NativeIsovistMetadata metadata = default;
        fixed (NativeViewpoint* viewpointPointer = nativeViewpoints)
        fixed (NativeIsovistSummary* summaryPointer = nativeSummaries)
        fixed (NativeIsovistRay* rayPointer = nativeRays)
        {
            ThrowIfFailed(NativeMethods.SceneIsovist(handle, viewpointPointer, (nuint)nativeViewpoints.Length,
                &nativeOptions, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                rayPointer, (nuint)nativeRays.Length));
        }
        ValidateLayouts(metadata, nativeSummaries, nativeRays);
        double inverseScale = 1.0 / unitScaleToMeters;
        double inverseAreaScale = inverseScale * inverseScale;
        return new()
        {
            Viewpoints = viewpoints.ToArray(),
            Options = options,
            Summaries = nativeSummaries.Select(value => new IsovistSummary(value.ViewpointId,
                value.AreaSquareMeters * inverseAreaScale, value.PerimeterMeters * inverseScale,
                value.CentroidDistanceMeters * inverseScale, value.MeanRadialMeters * inverseScale,
                value.MinimumRadialMeters * inverseScale, value.MaximumRadialMeters * inverseScale,
                value.RadialStandardDeviationMeters * inverseScale, value.RadialSkewness, value.Compactness,
                value.AreaConvergenceDeltaSquareMeters * inverseAreaScale, value.OccludedCount, value.OpenCount,
                value.DominantOccluderObjectId, value.DominantOccluderFraction)).ToArray(),
            Rays = nativeRays.Select(value => new IsovistRay((IsovistRayState)value.State,
                value.DirectionX, value.DirectionY, value.DirectionZ, value.EndpointX * inverseScale,
                value.EndpointY * inverseScale, value.EndpointZ * inverseScale, value.DistanceMeters * inverseScale,
                value.ObjectId, value.InstanceId, value.MeshId, value.TriangleId)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Builds a radius-indexed graph with a deterministic hard candidate-degree cap.</summary>
    public unsafe VisibilityGraphResult AnalyzeSparseVisibilityGraph(
        IReadOnlyList<VisibilityNodePoint> nodes,
        SparseVisibilityGraphOptions options)
    {
        ArgumentNullException.ThrowIfNull(nodes);
        if (nodes.Count == 0) throw new ArgumentException("At least one graph node is required.", nameof(nodes));
        if (!double.IsFinite(options.MaximumDistance) || options.MaximumDistance <= 0.0)
            throw new ArgumentOutOfRangeException(nameof(options), "Sparse graph maximum distance must be finite and positive.");
        if (options.MaximumNeighbors is 0 or > 1024)
            throw new ArgumentOutOfRangeException(nameof(options), "Sparse graph maximum neighbors must be 1..1024.");
        ThrowIfDisposed();
        int pairCapacity = checked((int)Math.Min(
            16_000_000ul,
            checked((ulong)nodes.Count * options.MaximumNeighbors / 2)));
        NativeVisibilityNode[] nativeNodes = nodes.Select(value => new NativeVisibilityNode
        {
            NodeId = value.NodeId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
        }).ToArray();
        NativeSparseVisibilityGraphOptions nativeOptions = new()
        {
            StructureSize = ExpectedSparseGraphOptionsSize,
            Flags = options.ComputeCentrality ? 1U : 0U,
            EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
            MaximumNeighbors = options.MaximumNeighbors,
        };
        NativeVisibilityNodeMetrics[] nativeMetrics = new NativeVisibilityNodeMetrics[nodes.Count];
        NativeVisibilityGraphPair[] nativePairCapacity = new NativeVisibilityGraphPair[pairCapacity];
        NativeVisibilityGraphMetadata metadata = default;
        fixed (NativeVisibilityNode* nodePointer = nativeNodes)
        fixed (NativeVisibilityNodeMetrics* metricPointer = nativeMetrics)
        fixed (NativeVisibilityGraphPair* pairPointer = nativePairCapacity)
        {
            ThrowIfFailed(NativeMethods.SceneSparseVisibilityGraph(handle, nodePointer, (nuint)nativeNodes.Length,
                &nativeOptions, &metadata, metricPointer, (nuint)nativeMetrics.Length,
                pairPointer, (nuint)nativePairCapacity.Length));
        }
        int pairCount = checked((int)metadata.PairCount);
        NativeVisibilityGraphPair[] nativePairs = nativePairCapacity.Take(pairCount).ToArray();
        ValidateLayouts(metadata, nativeMetrics, nativePairs);
        double inverseScale = 1.0 / unitScaleToMeters;
        return new()
        {
            Nodes = nodes.ToArray(),
            Options = new(options.EndpointClearance, options.MaximumDistance,
                options.CategoryMask, options.ComputeCentrality),
            IsSparse = true,
            MaximumNeighbors = options.MaximumNeighbors,
            Metrics = nativeMetrics.Select(value => new VisibilityNodeMetrics(value.NodeId, value.Degree,
                value.DegreeCentrality, value.ComponentIndex, value.HarmonicCloseness,
                value.BetweennessCentrality)).ToArray(),
            Pairs = nativePairs.Select(value => new VisibilityGraphPair(value.FirstIndex, value.SecondIndex,
                (VisibilityState)value.State, value.DistanceMeters * inverseScale,
                value.BlockerObjectId)).ToArray(),
            VisibleEdgeCount = metadata.VisibleEdgeCount,
            ConnectedComponentCount = metadata.ConnectedComponentCount,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1,
                metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Computes directed intervisibility plus transparent distance/direction-weighted privacy risk.</summary>
    public unsafe IntervisibilityResult AnalyzeIntervisibility(
        IReadOnlyList<VisibilityObserverPoint> observers,
        IReadOnlyList<VisibilityTargetPoint> targets,
        IntervisibilityOptions options)
    {
        ArgumentNullException.ThrowIfNull(observers);
        ArgumentNullException.ThrowIfNull(targets);
        if (observers.Count == 0 || targets.Count == 0) throw new ArgumentException("Observers and targets must be non-empty.");
        ThrowIfDisposed();
        int entryCount = checked(observers.Count * targets.Count);
        NativeVisibilityObserver[] nativeObservers = observers.Select(value => new NativeVisibilityObserver
        {
            ObserverId = value.ObserverId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
            Weight = value.Weight,
        }).ToArray();
        NativeVisibilityTarget[] nativeTargets = targets.Select(value => new NativeVisibilityTarget
        {
            TargetId = value.TargetId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
            Sensitivity = value.Sensitivity,
            Flags = value.HasFacing ? 1U : 0U,
            FacingX = value.FacingX,
            FacingY = value.FacingY,
            FacingZ = value.FacingZ,
        }).ToArray();
        NativeIntervisibilityOptions nativeOptions = new()
        {
            StructureSize = ExpectedIntervisibilityOptionsSize,
            EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            PrivacyReferenceDistanceMeters = options.PrivacyReferenceDistance * unitScaleToMeters,
            FacingExponent = options.FacingExponent,
            CategoryMask = options.CategoryMask,
        };
        NativeObserverVisibilitySummary[] nativeObserverSummaries = new NativeObserverVisibilitySummary[observers.Count];
        NativeTargetVisibilitySummary[] nativeTargetSummaries = new NativeTargetVisibilitySummary[targets.Count];
        NativeIntervisibilityEntry[] nativeEntries = new NativeIntervisibilityEntry[entryCount];
        NativeIntervisibilityMetadata metadata = default;
        fixed (NativeVisibilityObserver* observerPointer = nativeObservers)
        fixed (NativeVisibilityTarget* targetPointer = nativeTargets)
        fixed (NativeObserverVisibilitySummary* observerSummaryPointer = nativeObserverSummaries)
        fixed (NativeTargetVisibilitySummary* targetSummaryPointer = nativeTargetSummaries)
        fixed (NativeIntervisibilityEntry* entryPointer = nativeEntries)
        {
            ThrowIfFailed(NativeMethods.SceneIntervisibility(handle, observerPointer, (nuint)nativeObservers.Length,
                targetPointer, (nuint)nativeTargets.Length, &nativeOptions, &metadata,
                observerSummaryPointer, (nuint)nativeObserverSummaries.Length,
                targetSummaryPointer, (nuint)nativeTargetSummaries.Length, entryPointer, (nuint)nativeEntries.Length));
        }
        ValidateLayouts(metadata, nativeObserverSummaries, nativeTargetSummaries, nativeEntries);
        double inverseScale = 1.0 / unitScaleToMeters;
        return new()
        {
            Observers = observers.ToArray(),
            Targets = targets.ToArray(),
            Options = options,
            ObserverSummaries = nativeObserverSummaries.Select(value => new ObserverVisibilitySummary(
                value.ObserverId, value.VisibleCount, value.VisibleFraction, value.TotalPrivacyRisk, value.PeakPrivacyRisk)).ToArray(),
            TargetSummaries = nativeTargetSummaries.Select(value => new TargetVisibilitySummary(
                value.TargetId, value.VisibleObserverCount, value.ExposureFraction,
                value.CumulativePrivacyRisk, value.CombinedPrivacyRisk)).ToArray(),
            Entries = nativeEntries.Select(value => new IntervisibilityEntry((VisibilityState)value.State,
                value.DistanceMeters * inverseScale, value.DirectionX, value.DirectionY, value.DirectionZ,
                value.PrivacyRisk, value.FacingFactor, value.DistanceFactor, value.BlockerObjectId,
                value.BlockerInstanceId, value.BlockerMeshId, value.BlockerTriangleId)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Builds an exact all-pairs visibility graph with components and centrality.</summary>
    public unsafe VisibilityGraphResult AnalyzeVisibilityGraph(
        IReadOnlyList<VisibilityNodePoint> nodes,
        VisibilityGraphOptions options)
    {
        ArgumentNullException.ThrowIfNull(nodes);
        if (nodes.Count == 0) throw new ArgumentException("At least one graph node is required.", nameof(nodes));
        ThrowIfDisposed();
        int pairCount = checked(nodes.Count * (nodes.Count - 1) / 2);
        NativeVisibilityNode[] nativeNodes = nodes.Select(value => new NativeVisibilityNode
        {
            NodeId = value.NodeId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
        }).ToArray();
        NativeVisibilityGraphOptions nativeOptions = new()
        {
            StructureSize = ExpectedGraphOptionsSize,
            Flags = options.ComputeCentrality ? 1U : 0U,
            EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
        };
        NativeVisibilityNodeMetrics[] nativeMetrics = new NativeVisibilityNodeMetrics[nodes.Count];
        NativeVisibilityGraphPair[] nativePairs = new NativeVisibilityGraphPair[pairCount];
        NativeVisibilityGraphMetadata metadata = default;
        fixed (NativeVisibilityNode* nodePointer = nativeNodes)
        fixed (NativeVisibilityNodeMetrics* metricPointer = nativeMetrics)
        fixed (NativeVisibilityGraphPair* pairPointer = nativePairs)
        {
            ThrowIfFailed(NativeMethods.SceneVisibilityGraph(handle, nodePointer, (nuint)nativeNodes.Length,
                &nativeOptions, &metadata, metricPointer, (nuint)nativeMetrics.Length,
                pairPointer, (nuint)nativePairs.Length));
        }
        ValidateLayouts(metadata, nativeMetrics, nativePairs);
        double inverseScale = 1.0 / unitScaleToMeters;
        return new()
        {
            Nodes = nodes.ToArray(),
            Options = options,
            IsSparse = false,
            MaximumNeighbors = 0,
            Metrics = nativeMetrics.Select(value => new VisibilityNodeMetrics(value.NodeId, value.Degree,
                value.DegreeCentrality, value.ComponentIndex, value.HarmonicCloseness, value.BetweennessCentrality)).ToArray(),
            Pairs = nativePairs.Select(value => new VisibilityGraphPair(value.FirstIndex, value.SecondIndex,
                (VisibilityState)value.State, value.DistanceMeters * inverseScale, value.BlockerObjectId)).ToArray(),
            VisibleEdgeCount = metadata.VisibleEdgeCount,
            ConnectedComponentCount = metadata.ConnectedComponentCount,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    private static string FormatHash(ulong first, ulong second, ulong third, ulong fourth) =>
        SolarConversions.FormatHash(first, second, third, fourth);

    private static void ValidateLayouts(NativeIsovistMetadata metadata, IReadOnlyList<NativeIsovistSummary> summaries, IReadOnlyList<NativeIsovistRay> rays)
    {
        if (metadata.StructureSize != ExpectedIsovistMetadataSize || metadata.StructureSize != Marshal.SizeOf<NativeIsovistMetadata>()
            || summaries.Any(value => value.StructureSize != ExpectedIsovistSummarySize)
            || rays.Any(value => value.StructureSize != ExpectedIsovistRaySize))
            throw new InvalidOperationException("Native DAENA isovist ABI layout mismatch.");
    }

    private static void ValidateLayouts(NativeIntervisibilityMetadata metadata,
        IReadOnlyList<NativeObserverVisibilitySummary> observers, IReadOnlyList<NativeTargetVisibilitySummary> targets,
        IReadOnlyList<NativeIntervisibilityEntry> entries)
    {
        if (metadata.StructureSize != ExpectedIntervisibilityMetadataSize || metadata.StructureSize != Marshal.SizeOf<NativeIntervisibilityMetadata>()
            || observers.Any(value => value.StructureSize != ExpectedVisibilitySummarySize)
            || targets.Any(value => value.StructureSize != ExpectedVisibilitySummarySize)
            || entries.Any(value => value.StructureSize != ExpectedIntervisibilityEntrySize))
            throw new InvalidOperationException("Native DAENA intervisibility ABI layout mismatch.");
    }

    private static void ValidateLayouts(NativeVisibilityGraphMetadata metadata,
        IReadOnlyList<NativeVisibilityNodeMetrics> metrics, IReadOnlyList<NativeVisibilityGraphPair> pairs)
    {
        if (metadata.StructureSize != ExpectedGraphMetadataSize || metadata.StructureSize != Marshal.SizeOf<NativeVisibilityGraphMetadata>()
            || metrics.Any(value => value.StructureSize != ExpectedGraphMetricsSize)
            || pairs.Any(value => value.StructureSize != ExpectedGraphPairSize))
            throw new InvalidOperationException("Native DAENA visibility-graph ABI layout mismatch.");
    }
}
