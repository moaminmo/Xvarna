namespace Xvarna.Native;

/// <summary>Advanced DAENA domain methods executed through a cached VAYU session.</summary>
public sealed partial class XvarnaComputeSession
{
    private const uint ExpectedDaenaExecutionSize = 464;
    private const uint ExpectedDomainTargetOptionsSize = 96;
    private const uint ExpectedDomainTargetEntrySize = 112;
    private const uint ExpectedDomainTargetSummarySize = 96;
    private const uint ExpectedDomainTargetMetadataSize = 72;
    private const uint ExpectedDomainCorridorOptionsSize = 24;
    private const uint ExpectedDomainCorridorSummarySize = 88;
    private const uint ExpectedDomainCorridorSampleSize = 112;
    private const uint ExpectedDomainCorridorMetadataSize = 72;
    private const uint ExpectedDomainPathOptionsSize = 112;
    private const uint ExpectedDomainPathSampleSize = 120;
    private const uint ExpectedDomainPathSummarySize = 88;
    private const uint ExpectedDomainPathMetadataSize = 64;

    /// <summary>Computes Target/Weighted/Green View through this cached backend.</summary>
    public unsafe TargetViewResult AnalyzeTargetView(
        IReadOnlyList<ViewCamera> observers,
        IReadOnlyList<ViewTargetTriangle> targetPatches,
        TargetViewOptions options)
    {
        ArgumentNullException.ThrowIfNull(observers);
        ArgumentNullException.ThrowIfNull(targetPatches);
        if (observers.Count == 0 || targetPatches.Count == 0)
        {
            throw new ArgumentException("Observers and target patches must be non-empty.");
        }
        ThrowIfDisposed();
        NativeViewObserver[] nativeObservers = observers.Select(ToNativeCamera).ToArray();
        NativeViewTargetPatch[] nativePatches = ConvertTargetPatches(targetPatches);
        ulong[] targetIds = targetPatches
            .Where(patch => (patch.CategoryMask & options.TargetCategoryMask) != 0)
            .Select(patch => patch.TargetId).Distinct().Order().ToArray();
        if (targetIds.Length == 0)
        {
            throw new ArgumentException("Target category filter excludes every patch.", nameof(options));
        }
        int entryCount = checked(observers.Count * targetIds.Length);
        NativeTargetViewSummary[] nativeSummaries = new NativeTargetViewSummary[observers.Count];
        NativeTargetViewEntry[] nativeEntries = new NativeTargetViewEntry[entryCount];
        NativeTargetViewMetadata metadata = default;
        NativeDaenaExecutionInfo execution = default;
        NativeTargetViewOptions nativeOptions = ToNativeTargetOptions(options);
        fixed (NativeViewObserver* observerPointer = nativeObservers)
        fixed (NativeViewTargetPatch* patchPointer = nativePatches)
        fixed (NativeTargetViewSummary* summaryPointer = nativeSummaries)
        fixed (NativeTargetViewEntry* entryPointer = nativeEntries)
        {
            ThrowIfFailed(NativeMethods.ComputeTargetView(
                handle, observerPointer, (nuint)nativeObservers.Length,
                patchPointer, (nuint)nativePatches.Length, &nativeOptions, &execution, &metadata,
                summaryPointer, (nuint)nativeSummaries.Length,
                entryPointer, (nuint)nativeEntries.Length));
        }
        ValidateExecution(execution);
        if (metadata.StructureSize != ExpectedDomainTargetMetadataSize
            || metadata.TargetCount != (ulong)targetIds.Length
            || nativeSummaries.Any(value => value.StructureSize != ExpectedDomainTargetSummarySize)
            || nativeEntries.Any(value => value.StructureSize != ExpectedDomainTargetEntrySize))
        {
            throw new InvalidOperationException("Native VAYU Target View ABI layout mismatch.");
        }
        return new()
        {
            Observers = observers.ToArray(),
            TargetPatches = targetPatches.ToArray(),
            TargetIds = targetIds,
            Options = options,
            Summaries = nativeSummaries.Select(value => new TargetViewSummary(
                value.ObserverId, value.FovSolidAngleSteradians,
                value.PotentialTargetSolidAngleSteradians, value.VisibleTargetSolidAngleSteradians,
                value.TargetViewFraction, value.TargetUniverseVisibilityFraction,
                value.WeightedViewScore, value.GreenViewIndex, value.GreenShareOfVisibleTargets,
                value.DominantTargetId, value.ConvergenceDeltaSteradians)).ToArray(),
            Entries = nativeEntries.Select(value => new TargetViewEntry(
                value.ObserverId, value.TargetId, value.CategoryMask,
                value.PotentialSolidAngleSteradians, value.VisibleSolidAngleSteradians,
                value.VisibilityFraction, value.FovFraction, value.WeightedFovScore,
                value.ConvergenceDeltaSteradians, value.EligibleSampleCount,
                value.VisibleSampleCount, value.DominantBlockerObjectId,
                value.DominantBlockedSolidAngleSteradians)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(
                metadata.ContentHash0, metadata.ContentHash1,
                metadata.ContentHash2, metadata.ContentHash3),
            Execution = ConvertExecution(execution),
        };
    }

    /// <summary>Computes protected View Corridor through this cached backend.</summary>
    public unsafe ViewCorridorResult AnalyzeViewCorridors(
        IReadOnlyList<ViewCorridorDefinition> corridors,
        ViewCorridorOptions options)
    {
        ArgumentNullException.ThrowIfNull(corridors);
        if (corridors.Count == 0)
        {
            throw new ArgumentException("At least one corridor is required.", nameof(corridors));
        }
        ThrowIfDisposed();
        double scale = scene.UnitScaleToMeters;
        int sampleCount = checked(corridors.Count * checked((int)options.SampleCount));
        NativeViewCorridor[] nativeCorridors = corridors.Select(value => new NativeViewCorridor
        {
            CorridorId = value.CorridorId,
            OriginX = value.Origin.X * scale,
            OriginY = value.Origin.Y * scale,
            OriginZ = value.Origin.Z * scale,
            TargetX = value.Target.X * scale,
            TargetY = value.Target.Y * scale,
            TargetZ = value.Target.Z * scale,
            UpX = value.Up.X,
            UpY = value.Up.Y,
            UpZ = value.Up.Z,
            TargetRadiusMeters = value.TargetRadius * scale,
        }).ToArray();
        NativeViewCorridorOptions nativeOptions = new()
        {
            StructureSize = ExpectedDomainCorridorOptionsSize,
            SampleCount = options.SampleCount,
            EndpointClearanceMeters = options.EndpointClearance * scale,
            CategoryMask = options.CategoryMask,
        };
        NativeViewCorridorSummary[] nativeSummaries = new NativeViewCorridorSummary[corridors.Count];
        NativeViewCorridorSample[] nativeSamples = new NativeViewCorridorSample[sampleCount];
        NativeViewCorridorMetadata metadata = default;
        NativeDaenaExecutionInfo execution = default;
        fixed (NativeViewCorridor* corridorPointer = nativeCorridors)
        fixed (NativeViewCorridorSummary* summaryPointer = nativeSummaries)
        fixed (NativeViewCorridorSample* samplePointer = nativeSamples)
        {
            ThrowIfFailed(NativeMethods.ComputeViewCorridor(
                handle, corridorPointer, (nuint)nativeCorridors.Length, &nativeOptions,
                &execution, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                samplePointer, (nuint)nativeSamples.Length));
        }
        ValidateExecution(execution);
        if (metadata.StructureSize != ExpectedDomainCorridorMetadataSize
            || nativeSummaries.Any(value => value.StructureSize != ExpectedDomainCorridorSummarySize)
            || nativeSamples.Any(value => value.StructureSize != ExpectedDomainCorridorSampleSize))
        {
            throw new InvalidOperationException("Native VAYU View Corridor ABI layout mismatch.");
        }
        double inverse = 1.0 / scale;
        return new()
        {
            Corridors = corridors.ToArray(),
            Options = options,
            Summaries = nativeSummaries.Select(value => new ViewCorridorSummary(
                value.CorridorId, value.ApertureSolidAngleSteradians,
                value.OpenSampleCount, value.BlockedSampleCount, value.OpenFraction,
                value.OpenSolidAngleSteradians, value.ConvergenceDelta,
                value.DominantBlockerObjectId, value.DominantBlockerFraction,
                value.NearestBlockerDistanceMeters * inverse)).ToArray(),
            Samples = nativeSamples.Select(value => new ViewCorridorSample(
                value.CorridorId, (VisibilityState)value.State,
                new(value.ApertureX * inverse, value.ApertureY * inverse, value.ApertureZ * inverse),
                new(value.DirectionX, value.DirectionY, value.DirectionZ),
                value.ApertureDistanceMeters * inverse, value.FirstHitDistanceMeters * inverse,
                value.BlockerObjectId, value.BlockerInstanceId, value.BlockerMeshId,
                value.BlockerTriangleId)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(
                metadata.ContentHash0, metadata.ContentHash1,
                metadata.ContentHash2, metadata.ContentHash3),
            Execution = ConvertExecution(execution),
        };
    }

    /// <summary>Computes Dynamic Observer Path through this cached backend.</summary>
    public unsafe ObserverPathResult AnalyzeObserverPaths(
        IReadOnlyList<ObserverPathDefinition> paths,
        IReadOnlyList<ViewTargetTriangle> targetPatches,
        ObserverPathOptions options)
    {
        ArgumentNullException.ThrowIfNull(paths);
        ArgumentNullException.ThrowIfNull(targetPatches);
        if (paths.Count == 0 || targetPatches.Count == 0)
        {
            throw new ArgumentException("Paths and target patches must be non-empty.");
        }
        if (!double.IsFinite(options.Spacing) || options.Spacing <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Path spacing must be finite and positive.");
        }
        ThrowIfDisposed();
        double scale = scene.UnitScaleToMeters;
        List<NativePoint3> vertices = [];
        NativeObserverPath[] nativePaths = new NativeObserverPath[paths.Count];
        int expectedSamples = 0;
        for (int index = 0; index < paths.Count; index++)
        {
            ObserverPathDefinition path = paths[index];
            if (path.Vertices is null || path.Vertices.Count < 2)
            {
                throw new ArgumentException("Every path needs at least two vertices.", nameof(paths));
            }
            int offset = vertices.Count;
            vertices.AddRange(path.Vertices.Select(point => new NativePoint3
            {
                X = point.X * scale,
                Y = point.Y * scale,
                Z = point.Z * scale,
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
            double length = path.Vertices
                .Zip(path.Vertices.Skip(1), Distance)
                .Sum();
            expectedSamples = checked(expectedSamples
                + Math.Max(2, checked((int)Math.Ceiling(length / options.Spacing) + 1)));
        }
        NativeViewTargetPatch[] nativePatches = ConvertTargetPatches(targetPatches);
        NativeObserverPathOptions nativeOptions = new()
        {
            StructureSize = ExpectedDomainPathOptionsSize,
            SpacingMeters = options.Spacing * scale,
            View = ToNativeTargetOptions(options.View),
        };
        NativeObserverPathSummary[] nativeSummaries = new NativeObserverPathSummary[paths.Count];
        NativeObserverPathSample[] nativeSamples = new NativeObserverPathSample[expectedSamples];
        NativeObserverPathMetadata metadata = default;
        NativeDaenaExecutionInfo execution = default;
        NativePoint3[] nativeVertices = vertices.ToArray();
        fixed (NativeObserverPath* pathPointer = nativePaths)
        fixed (NativePoint3* vertexPointer = nativeVertices)
        fixed (NativeViewTargetPatch* patchPointer = nativePatches)
        fixed (NativeObserverPathSummary* summaryPointer = nativeSummaries)
        fixed (NativeObserverPathSample* samplePointer = nativeSamples)
        {
            ThrowIfFailed(NativeMethods.ComputeObserverPath(
                handle, pathPointer, (nuint)nativePaths.Length,
                vertexPointer, (nuint)nativeVertices.Length,
                patchPointer, (nuint)nativePatches.Length, &nativeOptions,
                &execution, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                samplePointer, (nuint)nativeSamples.Length));
        }
        ValidateExecution(execution);
        if (metadata.StructureSize != ExpectedDomainPathMetadataSize
            || metadata.SampleCount != (ulong)expectedSamples
            || nativeSummaries.Any(value => value.StructureSize != ExpectedDomainPathSummarySize)
            || nativeSamples.Any(value => value.StructureSize != ExpectedDomainPathSampleSize))
        {
            throw new InvalidOperationException("Native VAYU Observer Path ABI layout mismatch.");
        }
        double inverse = 1.0 / scale;
        return new()
        {
            Paths = paths.ToArray(),
            TargetPatches = targetPatches.ToArray(),
            Options = options,
            Summaries = nativeSummaries.Select(value => new ObserverPathSummary(
                value.PathId, value.PathLengthMeters * inverse, value.SampleCount,
                value.MeanTargetViewFraction, value.MeanWeightedViewScore,
                value.MeanGreenViewIndex, value.MinimumWeightedViewScore,
                value.MaximumWeightedViewScore, value.WorstSampleIndex,
                value.BestSampleIndex)).ToArray(),
            Samples = nativeSamples.Select(value => new ObserverPathSample(
                value.PathId, value.SampleIndex, value.DistanceAlongPathMeters * inverse,
                new(value.PositionX * inverse, value.PositionY * inverse, value.PositionZ * inverse),
                new(value.ForwardX, value.ForwardY, value.ForwardZ),
                value.TargetViewFraction, value.WeightedViewScore, value.GreenViewIndex,
                value.DominantTargetId, value.ConvergenceDeltaSteradians)).ToArray(),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(
                metadata.ContentHash0, metadata.ContentHash1,
                metadata.ContentHash2, metadata.ContentHash3),
            Execution = ConvertExecution(execution),
        };
    }

    private NativeViewObserver ToNativeCamera(ViewCamera value)
    {
        double scale = scene.UnitScaleToMeters;
        return new()
        {
            ObserverId = value.ObserverId,
            PositionX = value.Position.X * scale,
            PositionY = value.Position.Y * scale,
            PositionZ = value.Position.Z * scale,
            ForwardX = value.Forward.X,
            ForwardY = value.Forward.Y,
            ForwardZ = value.Forward.Z,
            UpX = value.Up.X,
            UpY = value.Up.Y,
            UpZ = value.Up.Z,
            Weight = value.Weight,
        };
    }

    private NativeViewTargetPatch[] ConvertTargetPatches(IReadOnlyList<ViewTargetTriangle> values)
    {
        double scale = scene.UnitScaleToMeters;
        return values.Select(value => new NativeViewTargetPatch
        {
            TargetId = value.TargetId,
            FirstX = value.First.X * scale,
            FirstY = value.First.Y * scale,
            FirstZ = value.First.Z * scale,
            SecondX = value.Second.X * scale,
            SecondY = value.Second.Y * scale,
            SecondZ = value.Second.Z * scale,
            ThirdX = value.Third.X * scale,
            ThirdY = value.Third.Y * scale,
            ThirdZ = value.Third.Z * scale,
            CategoryMask = value.CategoryMask,
            CategoryWeight = value.CategoryWeight,
        }).ToArray();
    }

    private NativeTargetViewOptions ToNativeTargetOptions(TargetViewOptions options)
    {
        double scale = scene.UnitScaleToMeters;
        return new()
        {
            StructureSize = ExpectedDomainTargetOptionsSize,
            Flags = options.TwoSidedTargets ? 1U : 0U,
            SamplesPerPatch = options.SamplesPerPatch,
            HorizontalFovRadians = options.HorizontalFieldOfViewDegrees * Math.PI / 180.0,
            VerticalFovRadians = options.VerticalFieldOfViewDegrees * Math.PI / 180.0,
            MaximumDistanceMeters = options.MaximumDistance * scale,
            EndpointClearanceMeters = options.EndpointClearance * scale,
            DistanceReferenceMeters = options.DistanceReference * scale,
            DistanceExponent = options.DistanceExponent,
            DirectionExponent = options.DirectionExponent,
            OccluderCategoryMask = options.OccluderCategoryMask,
            TargetCategoryMask = options.TargetCategoryMask,
            GreenCategoryMask = options.GreenCategoryMask,
        };
    }

    private static double Distance(SpatialPoint first, SpatialPoint second)
    {
        double x = second.X - first.X;
        double y = second.Y - first.Y;
        double z = second.Z - first.Z;
        return Math.Sqrt(x * x + y * y + z * z);
    }

    private static unsafe DaenaExecutionReport ConvertExecution(NativeDaenaExecutionInfo value)
    {
        ValidateExecution(value);
        return new()
        {
            Backend = (DaenaExecutionBackend)value.Backend,
            AdapterName = Decode(value.AdapterName, 128),
            UsedCreationFallback = (value.Flags & 1) != 0,
            UsedReusableBuffers = (value.Flags & 4) != 0,
            RuntimeFallbackBatchCount = value.RuntimeFallbackBatchCount,
            BatchCount = value.BatchCount,
            RayCount = value.RayCount,
            DispatchCount = value.DispatchCount,
            UploadMicroseconds = value.UploadMicroseconds,
            ExecutionMicroseconds = value.ExecutionMicroseconds,
            ReadbackMicroseconds = value.ReadbackMicroseconds,
            MaximumPrecisionErrorMeters = value.MaximumPrecisionErrorMeters,
            FallbackReason = Decode(value.FallbackReason, 256),
        };
    }

    private static void ValidateExecution(NativeDaenaExecutionInfo value)
    {
        if (value.StructureSize != ExpectedDaenaExecutionSize
            || !Enum.IsDefined((DaenaExecutionBackend)value.Backend))
        {
            throw new InvalidOperationException("Native VAYU DAENA execution layout mismatch.");
        }
    }
}
