using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Physically bounded optical properties used by DAENA and HVARE screening.</summary>
public readonly record struct AnalysisMaterial(
    ulong MaterialId,
    double VisibleTransmittance,
    double SolarTransmittance,
    double Reflectance);

/// <summary>Assigns one source object to one analysis material.</summary>
public readonly record struct MaterialAssignment(ulong ObjectId, ulong MaterialId);

/// <summary>Immutable validated material catalog. Unassigned scene objects are opaque.</summary>
public sealed class AnalysisMaterialLibrary
{
    /// <summary>Creates and validates a complete catalog.</summary>
    public AnalysisMaterialLibrary(
        IReadOnlyList<AnalysisMaterial>? materials = null,
        IReadOnlyList<MaterialAssignment>? assignments = null)
    {
        Materials = materials?.ToArray() ?? [];
        Assignments = assignments?.ToArray() ?? [];
        HashSet<ulong> ids = [];
        foreach (AnalysisMaterial material in Materials)
        {
            if (material.MaterialId == 0 || !ids.Add(material.MaterialId)
                || !Fraction(material.VisibleTransmittance)
                || !Fraction(material.SolarTransmittance)
                || !Fraction(material.Reflectance)
                || material.VisibleTransmittance + material.Reflectance > 1.0 + 1.0e-12
                || material.SolarTransmittance + material.Reflectance > 1.0 + 1.0e-12)
                throw new ArgumentException("Materials require unique non-zero IDs and energy-conserving values in [0,1].", nameof(materials));
        }
        HashSet<ulong> objects = [];
        foreach (MaterialAssignment assignment in Assignments)
        {
            if (!ids.Contains(assignment.MaterialId) || !objects.Add(assignment.ObjectId))
                throw new ArgumentException("Assignments require unique object IDs and known material IDs.", nameof(assignments));
        }
    }

    /// <summary>Explicit material definitions.</summary>
    public IReadOnlyList<AnalysisMaterial> Materials { get; }
    /// <summary>Object-to-material assignments.</summary>
    public IReadOnlyList<MaterialAssignment> Assignments { get; }
    /// <summary>Canonical all-opaque catalog.</summary>
    public static AnalysisMaterialLibrary Opaque { get; } = new();

    internal NativeAnalysisMaterial[] NativeMaterials() => Materials.Select(value => new NativeAnalysisMaterial
    {
        MaterialId = value.MaterialId,
        VisibleTransmittance = value.VisibleTransmittance,
        SolarTransmittance = value.SolarTransmittance,
        Reflectance = value.Reflectance,
    }).ToArray();

    internal NativeMaterialAssignment[] NativeAssignments() => Assignments.Select(value => new NativeMaterialAssignment
    {
        ObjectId = value.ObjectId,
        MaterialId = value.MaterialId,
    }).ToArray();

    private static bool Fraction(double value) => double.IsFinite(value) && value is >= 0.0 and <= 1.0;
}

/// <summary>Source-model eye point for an omnidirectional 3D isovist.</summary>
public readonly record struct SpatialViewpoint(ulong ViewpointId, double X, double Y, double Z);

/// <summary>Sampling, optical, attribution, and exact counterfactual policy.</summary>
public readonly record struct Isovist3dOptions(
    uint SampleCount = 4096,
    double MaximumDistance = 100.0,
    double EyeOffset = 1.0e-4,
    ulong CategoryMask = ulong.MaxValue,
    uint TopK = 10,
    uint CounterfactualCount = 5,
    uint MaximumMaterialLayers = 8,
    double MinimumTransmission = 1.0e-4);

/// <summary>One spherical sample with optical and source attribution.</summary>
public readonly record struct Isovist3dRay(
    double DirectionX, double DirectionY, double DirectionZ,
    double EndpointX, double EndpointY, double EndpointZ,
    double Distance, double Transmission,
    ulong ObjectId, ulong InstanceId, ulong MeshId, uint TriangleId, ulong CategoryMask);

/// <summary>Volumetric, radial, solid-angle, convergence, and dominant-source metrics.</summary>
public readonly record struct Isovist3dSummary(
    ulong ViewpointId, double Volume, double RadialSurface, double MeanRadial,
    double MinimumRadial, double MaximumRadial, double VisibleSolidAngle,
    double OpennessRatio, double VolumeConvergenceDelta,
    ulong DominantOccluderObjectId, double DominantOccluderFraction);

/// <summary>Reusable ranked obstruction attribution for 3D-isovist and landmark analyses.</summary>
public readonly record struct ObstructionAttribution(
    ulong ObserverId, ulong TargetId, uint Rank, ulong ObjectId, ulong CategoryMask,
    double BlockedWeight, double Fraction, double MeanDistance,
    double CounterfactualRecoveredWeight, double CounterfactualVolumeDelta);

/// <summary>Exact non-overlapping category-mask combination.</summary>
public readonly record struct CategoryBreakdown(
    ulong ObserverId, ulong TargetId, ulong CategoryMask, double BlockedWeight, double Fraction);

/// <summary>Complete immutable equal-solid-angle 3D-isovist result.</summary>
public sealed record Isovist3dResult
{
    /// <summary>Ordered source viewpoints.</summary>
    public required IReadOnlyList<SpatialViewpoint> Viewpoints { get; init; }
    /// <summary>One aggregate per viewpoint.</summary>
    public required IReadOnlyList<Isovist3dSummary> Summaries { get; init; }
    /// <summary>Viewpoint-major spherical ray matrix.</summary>
    public required IReadOnlyList<Isovist3dRay> Rays { get; init; }
    /// <summary>Ranked per-viewpoint object obstruction.</summary>
    public required IReadOnlyList<ObstructionAttribution> Attribution { get; init; }
    /// <summary>Exact per-viewpoint category partitions.</summary>
    public required IReadOnlyList<CategoryBreakdown> Categories { get; init; }
    /// <summary>Sampling and optical policy.</summary>
    public required Isovist3dOptions Options { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Per-viewpoint metric difference between aligned 3D-isovist scenarios.</summary>
public readonly record struct SpatialScenarioDelta(
    ulong ViewpointId, double VolumeDelta, double OpennessDelta, double MeanRadialDelta);

/// <summary>Per-object obstruction-share difference between aligned scenarios.</summary>
public readonly record struct AttributionDelta(
    ulong ViewpointId, ulong ObjectId, double BaselineFraction, double CandidateFraction, double DeltaFraction);

/// <summary>Aligned metric and source-attribution scenario comparison.</summary>
public sealed record SpatialScenarioComparison(
    string BaselineName, string CandidateName,
    IReadOnlyList<SpatialScenarioDelta> Metrics,
    IReadOnlyList<AttributionDelta> AttributionDeltas);

/// <summary>Oriented source-model landmark observer.</summary>
public readonly record struct LandmarkObserver(
    ulong ObserverId, double PositionX, double PositionY, double PositionZ,
    double ForwardX, double ForwardY, double ForwardZ,
    double UpX, double UpY, double UpZ);

/// <summary>Spherical source-model landmark proxy.</summary>
public readonly record struct Landmark(
    ulong LandmarkId, double PositionX, double PositionY, double PositionZ, double Radius, double Weight = 1.0);

/// <summary>Camera, sampling, and material policy for landmark visibility.</summary>
public readonly record struct LandmarkVisibilityOptions(
    uint SampleCount = 64,
    double HorizontalFieldOfViewDegrees = 90.0,
    double VerticalFieldOfViewDegrees = 90.0,
    double MaximumDistance = 1000.0,
    double EndpointClearance = 1.0e-4,
    ulong CategoryMask = ulong.MaxValue,
    uint TopK = 10,
    uint MaximumMaterialLayers = 8,
    double MinimumTransmission = 1.0e-4);

/// <summary>One row-major observer/landmark visibility cell.</summary>
public readonly record struct LandmarkVisibilityEntry(
    ulong ObserverId, ulong LandmarkId, double Distance, double ApparentSolidAngle,
    double VisibleFraction, double VisibleSolidAngle, double WeightedVisibilityScore,
    bool InsideFieldOfView, ulong DominantBlockerObjectId, double DominantBlockerFraction);

/// <summary>Complete landmark matrix with general attribution and category partitions.</summary>
public sealed record LandmarkVisibilityResult
{
    /// <summary>Row-major observer/landmark matrix.</summary>
    public required IReadOnlyList<LandmarkVisibilityEntry> Entries { get; init; }
    /// <summary>Ranked object attribution per matrix cell.</summary>
    public required IReadOnlyList<ObstructionAttribution> Attribution { get; init; }
    /// <summary>Exact category partitions per matrix cell.</summary>
    public required IReadOnlyList<CategoryBreakdown> Categories { get; init; }
    /// <summary>Landmark columns per observer row.</summary>
    public required int LandmarkCount { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Scenario-level comparison utilities that preserve object attribution.</summary>
public static class DaenaScenarios
{
    /// <summary>Compares aligned 3D-isovist results.</summary>
    public static SpatialScenarioComparison Compare(
        string baselineName, Isovist3dResult baseline,
        string candidateName, Isovist3dResult candidate)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(baselineName);
        ArgumentException.ThrowIfNullOrWhiteSpace(candidateName);
        ArgumentNullException.ThrowIfNull(baseline);
        ArgumentNullException.ThrowIfNull(candidate);
        if (baseline.Summaries.Count != candidate.Summaries.Count
            || baseline.Summaries.Where((value, index) => value.ViewpointId != candidate.Summaries[index].ViewpointId).Any())
            throw new ArgumentException("Scenario results must contain the same ordered viewpoint IDs.");
        SpatialScenarioDelta[] metrics = baseline.Summaries.Zip(candidate.Summaries, (left, right) =>
            new SpatialScenarioDelta(left.ViewpointId, right.Volume - left.Volume,
                right.OpennessRatio - left.OpennessRatio, right.MeanRadial - left.MeanRadial)).ToArray();
        Dictionary<(ulong Viewpoint, ulong Object), (double Baseline, double Candidate)> values = [];
        foreach (ObstructionAttribution item in baseline.Attribution)
            values[(item.ObserverId, item.ObjectId)] = (item.Fraction, 0.0);
        foreach (ObstructionAttribution item in candidate.Attribution)
        {
            values.TryGetValue((item.ObserverId, item.ObjectId), out var current);
            values[(item.ObserverId, item.ObjectId)] = (current.Baseline, item.Fraction);
        }
        AttributionDelta[] deltas = values.OrderBy(value => value.Key.Viewpoint).ThenBy(value => value.Key.Object)
            .Select(value => new AttributionDelta(value.Key.Viewpoint, value.Key.Object,
                value.Value.Baseline, value.Value.Candidate, value.Value.Candidate - value.Value.Baseline)).ToArray();
        return new(baselineName, candidateName, metrics, deltas);
    }
}

public sealed partial class XvarnaScene
{
    /// <summary>Computes material-aware equal-solid-angle 3D isovists.</summary>
    public unsafe Isovist3dResult AnalyzeIsovists3d(
        IReadOnlyList<SpatialViewpoint> viewpoints,
        AnalysisMaterialLibrary materials,
        Isovist3dOptions options)
    {
        ArgumentNullException.ThrowIfNull(viewpoints);
        ArgumentNullException.ThrowIfNull(materials);
        if (viewpoints.Count == 0) throw new ArgumentException("At least one viewpoint is required.", nameof(viewpoints));
        ThrowIfDisposed();
        int rayCount = checked(viewpoints.Count * (int)options.SampleCount);
        int attributionCapacity = checked(viewpoints.Count * (int)options.TopK);
        int categoryCapacity = checked(viewpoints.Count * Math.Max(1, checked((int)Statistics.InstanceCount)));
        NativeSpatialViewpoint[] nativeViewpoints = viewpoints.Select(value => new NativeSpatialViewpoint
        {
            ViewpointId = value.ViewpointId,
            PositionX = value.X * unitScaleToMeters,
            PositionY = value.Y * unitScaleToMeters,
            PositionZ = value.Z * unitScaleToMeters,
        }).ToArray();
        NativeAnalysisMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeIsovist3dOptions nativeOptions = new()
        {
            StructureSize = 56,
            SampleCount = options.SampleCount,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            EyeOffsetMeters = options.EyeOffset * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
            TopK = options.TopK,
            CounterfactualCount = options.CounterfactualCount,
            MaximumMaterialLayers = options.MaximumMaterialLayers,
            Reserved = 0,
            MinimumTransmission = options.MinimumTransmission,
        };
        NativeIsovist3dSummary[] nativeSummaries = new NativeIsovist3dSummary[viewpoints.Count];
        NativeIsovist3dRay[] nativeRays = new NativeIsovist3dRay[rayCount];
        NativeAttributionEntry[] nativeAttribution = new NativeAttributionEntry[attributionCapacity];
        NativeCategoryBreakdown[] nativeCategories = new NativeCategoryBreakdown[categoryCapacity];
        NativeIsovist3dMetadata metadata = default;
        fixed (NativeSpatialViewpoint* viewpointPointer = nativeViewpoints)
        fixed (NativeAnalysisMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (NativeIsovist3dSummary* summaryPointer = nativeSummaries)
        fixed (NativeIsovist3dRay* rayPointer = nativeRays)
        fixed (NativeAttributionEntry* attributionPointer = nativeAttribution)
        fixed (NativeCategoryBreakdown* categoryPointer = nativeCategories)
        {
            ThrowIfFailed(NativeMethods.SceneIsovist3d(handle, viewpointPointer, (nuint)nativeViewpoints.Length,
                materialPointer, (nuint)nativeMaterials.Length, assignmentPointer, (nuint)nativeAssignments.Length,
                &nativeOptions, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                rayPointer, (nuint)nativeRays.Length, attributionPointer, (nuint)nativeAttribution.Length,
                categoryPointer, (nuint)nativeCategories.Length));
        }
        ValidateMetadata(metadata.StructureSize, 88, nameof(Isovist3dResult));
        return new()
        {
            Viewpoints = viewpoints.ToArray(),
            Summaries = nativeSummaries.Select(value => new Isovist3dSummary(value.ViewpointId,
                CubicFromMeters(value.VolumeCubicMeters), SquareFromMeters(value.RadialSurfaceSquareMeters),
                FromMeters(value.MeanRadialMeters), FromMeters(value.MinimumRadialMeters),
                FromMeters(value.MaximumRadialMeters), value.VisibleSolidAngleSteradians,
                value.OpennessRatio, CubicFromMeters(value.VolumeConvergenceDeltaCubicMeters),
                value.DominantOccluderObjectId, value.DominantOccluderFraction)).ToArray(),
            Rays = nativeRays.Select(value => new Isovist3dRay(value.DirectionX, value.DirectionY, value.DirectionZ,
                FromMeters(value.EndpointX), FromMeters(value.EndpointY), FromMeters(value.EndpointZ),
                FromMeters(value.DistanceMeters), value.Transmission, value.ObjectId, value.InstanceId,
                value.MeshId, value.TriangleId, value.CategoryMask)).ToArray(),
            Attribution = ConvertAttribution(nativeAttribution, checked((int)metadata.AttributionCount)),
            Categories = ConvertCategories(nativeCategories, checked((int)metadata.CategoryCount)),
            Options = options,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Computes material-aware partial landmark visibility and solid-angle scores.</summary>
    public unsafe LandmarkVisibilityResult AnalyzeLandmarkVisibility(
        IReadOnlyList<LandmarkObserver> observers,
        IReadOnlyList<Landmark> landmarks,
        AnalysisMaterialLibrary materials,
        LandmarkVisibilityOptions options)
    {
        ArgumentNullException.ThrowIfNull(observers);
        ArgumentNullException.ThrowIfNull(landmarks);
        ArgumentNullException.ThrowIfNull(materials);
        if (observers.Count == 0 || landmarks.Count == 0) throw new ArgumentException("Observers and landmarks are required.");
        ThrowIfDisposed();
        int entryCount = checked(observers.Count * landmarks.Count);
        int attributionCapacity = checked(entryCount * (int)options.TopK);
        int categoryCapacity = checked(entryCount * Math.Max(1, checked((int)Statistics.InstanceCount)));
        NativeLandmarkObserver[] nativeObservers = observers.Select(value => new NativeLandmarkObserver
        {
            ObserverId = value.ObserverId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
            ForwardX = value.ForwardX,
            ForwardY = value.ForwardY,
            ForwardZ = value.ForwardZ,
            UpX = value.UpX,
            UpY = value.UpY,
            UpZ = value.UpZ,
        }).ToArray();
        NativeLandmark[] nativeLandmarks = landmarks.Select(value => new NativeLandmark
        {
            LandmarkId = value.LandmarkId,
            PositionX = value.PositionX * unitScaleToMeters,
            PositionY = value.PositionY * unitScaleToMeters,
            PositionZ = value.PositionZ * unitScaleToMeters,
            RadiusMeters = value.Radius * unitScaleToMeters,
            Weight = value.Weight,
        }).ToArray();
        NativeAnalysisMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeLandmarkOptions nativeOptions = new()
        {
            StructureSize = 72,
            SampleCount = options.SampleCount,
            HorizontalFieldOfViewRadians = options.HorizontalFieldOfViewDegrees * Math.PI / 180.0,
            VerticalFieldOfViewRadians = options.VerticalFieldOfViewDegrees * Math.PI / 180.0,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            EndpointClearanceMeters = options.EndpointClearance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
            TopK = options.TopK,
            MaximumMaterialLayers = options.MaximumMaterialLayers,
            Reserved = 0,
            MinimumTransmission = options.MinimumTransmission,
        };
        NativeLandmarkVisibilityEntry[] nativeEntries = new NativeLandmarkVisibilityEntry[entryCount];
        NativeAttributionEntry[] nativeAttribution = new NativeAttributionEntry[attributionCapacity];
        NativeCategoryBreakdown[] nativeCategories = new NativeCategoryBreakdown[categoryCapacity];
        NativeLandmarkMetadata metadata = default;
        fixed (NativeLandmarkObserver* observerPointer = nativeObservers)
        fixed (NativeLandmark* landmarkPointer = nativeLandmarks)
        fixed (NativeAnalysisMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (NativeLandmarkVisibilityEntry* entryPointer = nativeEntries)
        fixed (NativeAttributionEntry* attributionPointer = nativeAttribution)
        fixed (NativeCategoryBreakdown* categoryPointer = nativeCategories)
        {
            ThrowIfFailed(NativeMethods.SceneLandmarkVisibility(handle, observerPointer, (nuint)nativeObservers.Length,
                landmarkPointer, (nuint)nativeLandmarks.Length, materialPointer, (nuint)nativeMaterials.Length,
                assignmentPointer, (nuint)nativeAssignments.Length, &nativeOptions, &metadata,
                entryPointer, (nuint)nativeEntries.Length, attributionPointer, (nuint)nativeAttribution.Length,
                categoryPointer, (nuint)nativeCategories.Length));
        }
        ValidateMetadata(metadata.StructureSize, 88, nameof(LandmarkVisibilityResult));
        return new()
        {
            Entries = nativeEntries.Select(value => new LandmarkVisibilityEntry(value.ObserverId, value.LandmarkId,
                FromMeters(value.DistanceMeters), value.ApparentSolidAngleSteradians, value.VisibleFraction,
                value.VisibleSolidAngleSteradians, value.WeightedVisibilityScore, (value.Flags & 1) != 0,
                value.DominantBlockerObjectId, value.DominantBlockerFraction)).ToArray(),
            Attribution = ConvertAttribution(nativeAttribution, checked((int)metadata.AttributionCount)),
            Categories = ConvertCategories(nativeCategories, checked((int)metadata.CategoryCount)),
            LandmarkCount = landmarks.Count,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    private ObstructionAttribution[] ConvertAttribution(NativeAttributionEntry[] values, int count) =>
        values.Take(count).Select(value => new ObstructionAttribution(value.ObserverId, value.TargetId,
            value.Rank, value.ObjectId, value.CategoryMask, value.BlockedWeight, value.Fraction,
            FromMeters(value.MeanDistanceMeters), value.CounterfactualRecoveredWeight,
            CubicFromMeters(value.CounterfactualVolumeDeltaCubicMeters))).ToArray();

    private static CategoryBreakdown[] ConvertCategories(NativeCategoryBreakdown[] values, int count) =>
        values.Take(count).Select(value => new CategoryBreakdown(value.ObserverId, value.TargetId,
            value.CategoryMask, value.BlockedWeight, value.Fraction)).ToArray();

    private double FromMeters(double value) => value / unitScaleToMeters;
    private double SquareFromMeters(double value) => value / (unitScaleToMeters * unitScaleToMeters);
    private double CubicFromMeters(double value) => value / (unitScaleToMeters * unitScaleToMeters * unitScaleToMeters);

    private static void ValidateMetadata(uint actual, uint expected, string operation)
    {
        if (actual != expected) throw new InvalidOperationException($"Native {operation} ABI layout mismatch: {actual} != {expected}.");
    }
}
