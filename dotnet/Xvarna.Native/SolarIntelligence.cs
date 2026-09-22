namespace Xvarna.Native;

/// <summary>Named optical material alternative over one unchanged scene.</summary>
public sealed record SolarMaterialScenario
{
    /// <summary>Stable scenario identity.</summary>
    public required ulong ScenarioId { get; init; }
    /// <summary>Human-readable scenario name.</summary>
    public required string Name { get; init; }
    /// <summary>Object-to-material catalog for this alternative.</summary>
    public required AnalysisMaterialLibrary Materials { get; init; }
}

/// <summary>Material traversal and ranked-loss policy for solar scenarios.</summary>
public readonly record struct SolarScenarioOptions(
    double SensorOffset = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    double MinimumIncidenceCosine = 0.0,
    ulong CategoryMask = ulong.MaxValue,
    uint MaximumMaterialLayers = 8,
    double MinimumTransmission = 1.0e-4,
    uint TopK = 10);

/// <summary>One scenario/sensor material-aware direct-solar aggregate.</summary>
public readonly record struct SolarScenarioSummary(
    ulong ScenarioId, ulong SensorId, double ReceivedSunHours, double LostSunHours,
    double EligibleSunHours, double SolarAccessRatio,
    ulong DominantOccluderObjectId, double DominantOccluderHours);

/// <summary>Eligibility state of one scenario/sensor/time cell.</summary>
public enum SolarScenarioState : uint
{
    /// <summary>Sun interval is inactive.</summary>
    Inactive = 0,
    /// <summary>Sun lies behind the oriented sensor.</summary>
    BackFacing = 1,
    /// <summary>Optical transmission was evaluated.</summary>
    Evaluated = 2,
}

/// <summary>One scenario-major optical timeline cell.</summary>
public readonly record struct SolarScenarioTimelineEntry(
    SolarScenarioState State, double Transmission, ulong FirstObjectId,
    uint LayerCount, bool LayerLimitReached);

/// <summary>Ranked material/geometric solar loss source.</summary>
public readonly record struct SolarLossAttribution(
    ulong ScenarioId, ulong SensorId, uint Rank, ulong ObjectId,
    ulong CategoryMask, double LostSunHours, double Fraction);

/// <summary>Exact, non-overlapping solar-loss category-mask combination.</summary>
public readonly record struct SolarCategoryBreakdown(
    ulong ScenarioId, ulong SensorId, ulong CategoryMask, double LostSunHours, double Fraction);

/// <summary>Compared minus first-scenario baseline values for one sensor.</summary>
public readonly record struct SolarScenarioDelta(
    ulong ScenarioId, ulong SensorId, double ReceivedSunHoursDelta,
    double LostSunHoursDelta, double SolarAccessRatioDelta);

/// <summary>Complete scenario-major optical comparison.</summary>
public sealed record SolarScenarioComparison
{
    /// <summary>Ordered source scenarios; the first is the delta baseline.</summary>
    public required IReadOnlyList<SolarMaterialScenario> Scenarios { get; init; }
    /// <summary>One aggregate per scenario/sensor.</summary>
    public required IReadOnlyList<SolarScenarioSummary> Summaries { get; init; }
    /// <summary>Scenario-major, sensor-major, time-major optical cells.</summary>
    public required IReadOnlyList<SolarScenarioTimelineEntry> Timeline { get; init; }
    /// <summary>Ranked source-object loss.</summary>
    public required IReadOnlyList<SolarLossAttribution> Attribution { get; init; }
    /// <summary>Exact category partitions.</summary>
    public required IReadOnlyList<SolarCategoryBreakdown> Categories { get; init; }
    /// <summary>Every non-baseline scenario/sensor delta.</summary>
    public required IReadOnlyList<SolarScenarioDelta> Deltas { get; init; }
    /// <summary>Number of source sensors.</summary>
    public required int SensorCount { get; init; }
    /// <summary>Number of temporal sun samples.</summary>
    public required int SunCount { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Backwards-compatible vertical-column candidate base for early massing screening.</summary>
public readonly record struct EnvelopeCandidate(ulong CandidateId, double X, double Y, double Z);

/// <summary>Freeform candidate base and build-axis direction for solar-envelope screening.</summary>
public readonly record struct OrientedEnvelopeCandidate(
    ulong CandidateId, double X, double Y, double Z,
    double AxisX, double AxisY, double AxisZ);

/// <summary>Solar-access and target-shading envelope policy in source model units.</summary>
public readonly record struct SolarEnvelopeOptions(
    double CandidateRadius = 0.5,
    double RequiredPreservedFraction = 0.8,
    double TargetShadedFraction = 0.5,
    double VerticalClearance = 0.05,
    double SensorOffset = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    ulong CategoryMask = ulong.MaxValue,
    uint MaximumMaterialLayers = 8,
    double MinimumTransmission = 1.0e-4);

/// <summary>Sensor and UTC interval controlling one envelope threshold.</summary>
public readonly record struct EnvelopeControl(
    ulong SensorId, DateTimeOffset TimestampUtc, double RayElevation, double WeightedHours);

/// <summary>Solar-access and target-shading thresholds at one candidate.</summary>
public readonly record struct SolarEnvelopeCell(
    ulong CandidateId, double X, double Y, double Z,
    double MaximumSolarAccessElevation, double MaximumSolarAccessHeight,
    double MinimumShadingElevation, double MinimumShadingHeight,
    double ConsideredBaselineSunHours, ulong ConstraintCount,
    EnvelopeControl AccessControl, EnvelopeControl ShadingControl);

/// <summary>Complete freeform material-aware solar/shading envelope.</summary>
public sealed record SolarEnvelopeResult
{
    /// <summary>One threshold cell per candidate.</summary>
    public required IReadOnlyList<SolarEnvelopeCell> Cells { get; init; }
    /// <summary>Protected sensor count.</summary>
    public required int SensorCount { get; init; }
    /// <summary>Temporal sun sample count.</summary>
    public required int SunCount { get; init; }
    /// <summary>Native analysis duration.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

public sealed partial class XvarnaScene
{
    /// <summary>Compares material alternatives over one geometry and sun schedule.</summary>
    public unsafe SolarScenarioComparison CompareSolarScenarios(
        IReadOnlyList<SolarSensor> sensors,
        XvarnaSunSet sunSet,
        IReadOnlyList<SolarMaterialScenario> scenarios,
        SolarScenarioOptions options)
    {
        ArgumentNullException.ThrowIfNull(sensors);
        ArgumentNullException.ThrowIfNull(sunSet);
        ArgumentNullException.ThrowIfNull(scenarios);
        if (sensors.Count == 0 || scenarios.Count == 0) throw new ArgumentException("Sensors and scenarios are required.");
        if (scenarios.Any(value => value.Materials is null || string.IsNullOrWhiteSpace(value.Name)))
            throw new ArgumentException("Every scenario requires a name and material catalog.", nameof(scenarios));
        ThrowIfDisposed();

        NativeSolarSensor[] nativeSensors = ConvertSolarSensors(sensors);
        List<NativeAnalysisMaterial> nativeMaterials = [];
        List<NativeMaterialAssignment> nativeAssignments = [];
        NativeSolarScenario[] nativeScenarios = new NativeSolarScenario[scenarios.Count];
        for (int index = 0; index < scenarios.Count; index++)
        {
            SolarMaterialScenario scenario = scenarios[index];
            NativeAnalysisMaterial[] scenarioMaterials = scenario.Materials.NativeMaterials();
            NativeMaterialAssignment[] scenarioAssignments = scenario.Materials.NativeAssignments();
            nativeScenarios[index] = new()
            {
                ScenarioId = scenario.ScenarioId,
                MaterialOffset = checked((ulong)nativeMaterials.Count),
                MaterialCount = checked((ulong)scenarioMaterials.Length),
                AssignmentOffset = checked((ulong)nativeAssignments.Count),
                AssignmentCount = checked((ulong)scenarioAssignments.Length),
            };
            nativeMaterials.AddRange(scenarioMaterials);
            nativeAssignments.AddRange(scenarioAssignments);
        }
        NativeAnalysisMaterial[] materialArray = nativeMaterials.ToArray();
        NativeMaterialAssignment[] assignmentArray = nativeAssignments.ToArray();
        int summaryCount = checked(scenarios.Count * sensors.Count);
        int timelineCount = checked(summaryCount * sunSet.NativeSamples.Length);
        int attributionCapacity = checked(summaryCount * (int)options.TopK);
        int categoryCapacity = checked(summaryCount * Math.Max(1, checked((int)Statistics.InstanceCount)));
        int deltaCount = checked(Math.Max(0, scenarios.Count - 1) * sensors.Count);
        NativeSolarScenarioSummary[] nativeSummaries = new NativeSolarScenarioSummary[summaryCount];
        NativeSolarScenarioTimelineEntry[] nativeTimeline = new NativeSolarScenarioTimelineEntry[timelineCount];
        NativeSolarAttributionEntry[] nativeAttribution = new NativeSolarAttributionEntry[attributionCapacity];
        NativeSolarCategoryBreakdown[] nativeCategories = new NativeSolarCategoryBreakdown[categoryCapacity];
        NativeSolarScenarioDelta[] nativeDeltas = new NativeSolarScenarioDelta[deltaCount];
        NativeSolarScenarioOptions nativeOptions = new()
        {
            StructureSize = 56,
            TopK = options.TopK,
            SensorOffsetMeters = options.SensorOffset * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            MinimumIncidenceCosine = options.MinimumIncidenceCosine,
            CategoryMask = options.CategoryMask,
            MaximumMaterialLayers = options.MaximumMaterialLayers,
            Reserved = 0,
            MinimumTransmission = options.MinimumTransmission,
        };
        NativeSolarScenarioMetadata metadata = default;
        NativeSunSetMetadata sunMetadata = sunSet.NativeMetadata;
        fixed (NativeSolarSensor* sensorPointer = nativeSensors)
        fixed (NativeSunSample* sunPointer = sunSet.NativeSamples)
        fixed (NativeSolarScenario* scenarioPointer = nativeScenarios)
        fixed (NativeAnalysisMaterial* materialPointer = materialArray)
        fixed (NativeMaterialAssignment* assignmentPointer = assignmentArray)
        fixed (NativeSolarScenarioSummary* summaryPointer = nativeSummaries)
        fixed (NativeSolarScenarioTimelineEntry* timelinePointer = nativeTimeline)
        fixed (NativeSolarAttributionEntry* attributionPointer = nativeAttribution)
        fixed (NativeSolarCategoryBreakdown* categoryPointer = nativeCategories)
        fixed (NativeSolarScenarioDelta* deltaPointer = nativeDeltas)
        {
            ThrowIfFailed(NativeMethods.SceneCompareSolarScenarios(handle,
                sensorPointer, (nuint)nativeSensors.Length, &sunMetadata,
                sunPointer, (nuint)sunSet.NativeSamples.Length,
                scenarioPointer, (nuint)nativeScenarios.Length,
                materialPointer, (nuint)materialArray.Length,
                assignmentPointer, (nuint)assignmentArray.Length,
                &nativeOptions, &metadata, summaryPointer, (nuint)nativeSummaries.Length,
                timelinePointer, (nuint)nativeTimeline.Length,
                attributionPointer, (nuint)nativeAttribution.Length,
                categoryPointer, (nuint)nativeCategories.Length,
                deltaPointer, (nuint)nativeDeltas.Length));
        }
        ValidateMetadata(metadata.StructureSize, 112, nameof(SolarScenarioComparison));
        return new()
        {
            Scenarios = scenarios.ToArray(),
            Summaries = nativeSummaries.Select(value => new SolarScenarioSummary(value.ScenarioId,
                value.SensorId, value.ReceivedSunHours, value.LostSunHours, value.EligibleSunHours,
                value.SolarAccessRatio, value.DominantOccluderObjectId, value.DominantOccluderHours)).ToArray(),
            Timeline = nativeTimeline.Select(value => new SolarScenarioTimelineEntry((SolarScenarioState)value.State,
                value.Transmission, value.FirstObjectId, value.LayerCount, (value.Flags & 1) != 0)).ToArray(),
            Attribution = nativeAttribution.Take(checked((int)metadata.AttributionCount)).Select(value =>
                new SolarLossAttribution(value.ScenarioId, value.SensorId, value.Rank, value.ObjectId,
                    value.CategoryMask, value.LostSunHours, value.Fraction)).ToArray(),
            Categories = nativeCategories.Take(checked((int)metadata.CategoryCount)).Select(value =>
                new SolarCategoryBreakdown(value.ScenarioId, value.SensorId, value.CategoryMask,
                    value.LostSunHours, value.Fraction)).ToArray(),
            Deltas = nativeDeltas.Select(value => new SolarScenarioDelta(value.ScenarioId, value.SensorId,
                value.ReceivedSunHoursDelta, value.LostSunHoursDelta, value.SolarAccessRatioDelta)).ToArray(),
            SensorCount = sensors.Count,
            SunCount = sunSet.NativeSamples.Length,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Computes backwards-compatible vertical solar-access and shading thresholds.</summary>
    public unsafe SolarEnvelopeResult AnalyzeSolarEnvelope(
        IReadOnlyList<SolarSensor> protectedSensors,
        IReadOnlyList<EnvelopeCandidate> candidates,
        XvarnaSunSet sunSet,
        AnalysisMaterialLibrary materials,
        SolarEnvelopeOptions options)
    {
        ArgumentNullException.ThrowIfNull(candidates);
        return AnalyzeOrientedSolarEnvelope(
            protectedSensors,
            candidates.Select(value => new OrientedEnvelopeCandidate(
                value.CandidateId, value.X, value.Y, value.Z, 0.0, 0.0, 1.0)).ToArray(),
            sunSet,
            materials,
            options);
    }

    /// <summary>Computes a freeform solar-access and target-shading envelope along candidate axes.</summary>
    public unsafe SolarEnvelopeResult AnalyzeOrientedSolarEnvelope(
        IReadOnlyList<SolarSensor> protectedSensors,
        IReadOnlyList<OrientedEnvelopeCandidate> candidates,
        XvarnaSunSet sunSet,
        AnalysisMaterialLibrary materials,
        SolarEnvelopeOptions options)
    {
        ArgumentNullException.ThrowIfNull(protectedSensors);
        ArgumentNullException.ThrowIfNull(candidates);
        ArgumentNullException.ThrowIfNull(sunSet);
        ArgumentNullException.ThrowIfNull(materials);
        if (protectedSensors.Count == 0 || candidates.Count == 0) throw new ArgumentException("Protected sensors and candidates are required.");
        ThrowIfDisposed();
        NativeSolarSensor[] nativeSensors = ConvertSolarSensors(protectedSensors);
        NativeOrientedEnvelopeCandidate[] nativeCandidates = candidates.Select(value => new NativeOrientedEnvelopeCandidate
        {
            CandidateId = value.CandidateId,
            PositionX = value.X * unitScaleToMeters,
            PositionY = value.Y * unitScaleToMeters,
            PositionZ = value.Z * unitScaleToMeters,
            AxisX = value.AxisX,
            AxisY = value.AxisY,
            AxisZ = value.AxisZ,
        }).ToArray();
        NativeAnalysisMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeSolarEnvelopeOptions nativeOptions = new()
        {
            StructureSize = 72,
            MaximumMaterialLayers = options.MaximumMaterialLayers,
            CandidateRadiusMeters = options.CandidateRadius * unitScaleToMeters,
            RequiredPreservedFraction = options.RequiredPreservedFraction,
            TargetShadedFraction = options.TargetShadedFraction,
            VerticalClearanceMeters = options.VerticalClearance * unitScaleToMeters,
            SensorOffsetMeters = options.SensorOffset * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
            MinimumTransmission = options.MinimumTransmission,
        };
        NativeSolarEnvelopeCell[] nativeCells = new NativeSolarEnvelopeCell[candidates.Count];
        NativeSolarEnvelopeMetadata metadata = default;
        NativeSunSetMetadata sunMetadata = sunSet.NativeMetadata;
        fixed (NativeSolarSensor* sensorPointer = nativeSensors)
        fixed (NativeOrientedEnvelopeCandidate* candidatePointer = nativeCandidates)
        fixed (NativeSunSample* sunPointer = sunSet.NativeSamples)
        fixed (NativeAnalysisMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (NativeSolarEnvelopeCell* cellPointer = nativeCells)
        {
            ThrowIfFailed(NativeMethods.SceneSolarEnvelopeOriented(handle, sensorPointer, (nuint)nativeSensors.Length,
                candidatePointer, (nuint)nativeCandidates.Length, &sunMetadata,
                sunPointer, (nuint)sunSet.NativeSamples.Length,
                materialPointer, (nuint)nativeMaterials.Length,
                assignmentPointer, (nuint)nativeAssignments.Length,
                &nativeOptions, &metadata, cellPointer, (nuint)nativeCells.Length));
        }
        ValidateMetadata(metadata.StructureSize, 72, nameof(SolarEnvelopeResult));
        return new()
        {
            Cells = nativeCells.Select(value => new SolarEnvelopeCell(value.CandidateId,
                FromMeters(value.PositionX), FromMeters(value.PositionY), FromMeters(value.PositionZ),
                FromMeters(value.MaximumSolarAccessElevationMeters), FromMeters(value.MaximumSolarAccessHeightMeters),
                FromMeters(value.MinimumShadingElevationMeters), FromMeters(value.MinimumShadingHeightMeters),
                value.ConsideredBaselineSunHours, value.ConstraintCount,
                ConvertControl(value.AccessControl), ConvertControl(value.ShadingControl))).ToArray(),
            SensorCount = protectedSensors.Count,
            SunCount = sunSet.NativeSamples.Length,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    private NativeSolarSensor[] ConvertSolarSensors(IReadOnlyList<SolarSensor> sensors) => sensors.Select(value => new NativeSolarSensor
    {
        SensorId = value.SensorId,
        PositionX = value.PositionX * unitScaleToMeters,
        PositionY = value.PositionY * unitScaleToMeters,
        PositionZ = value.PositionZ * unitScaleToMeters,
        NormalX = value.NormalX,
        NormalY = value.NormalY,
        NormalZ = value.NormalZ,
    }).ToArray();

    private EnvelopeControl ConvertControl(NativeEnvelopeControl value) => new(
        value.SensorId, DateTimeOffset.FromUnixTimeSeconds(value.UnixSecondsUtc),
        FromMeters(value.RayElevationMeters), value.WeightedHours);
}
