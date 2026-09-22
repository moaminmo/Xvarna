using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_scene_isovist_3d", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneIsovist3d(
        SafeSceneHandle sceneHandle, NativeSpatialViewpoint* viewpoints, nuint viewpointCount,
        NativeAnalysisMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeIsovist3dOptions* options, NativeIsovist3dMetadata* outputMetadata,
        NativeIsovist3dSummary* outputSummaries, nuint summaryCapacity,
        NativeIsovist3dRay* outputRays, nuint rayCapacity,
        NativeAttributionEntry* outputAttribution, nuint attributionCapacity,
        NativeCategoryBreakdown* outputCategories, nuint categoryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_landmark_visibility", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneLandmarkVisibility(
        SafeSceneHandle sceneHandle, NativeLandmarkObserver* observers, nuint observerCount,
        NativeLandmark* landmarks, nuint landmarkCount,
        NativeAnalysisMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeLandmarkOptions* options, NativeLandmarkMetadata* outputMetadata,
        NativeLandmarkVisibilityEntry* outputEntries, nuint entryCapacity,
        NativeAttributionEntry* outputAttribution, nuint attributionCapacity,
        NativeCategoryBreakdown* outputCategories, nuint categoryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_compare_solar_scenarios", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneCompareSolarScenarios(
        SafeSceneHandle sceneHandle, NativeSolarSensor* sensors, nuint sensorCount,
        NativeSunSetMetadata* sunMetadata, NativeSunSample* sunSamples, nuint sunCount,
        NativeSolarScenario* scenarios, nuint scenarioCount,
        NativeAnalysisMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeSolarScenarioOptions* options, NativeSolarScenarioMetadata* outputMetadata,
        NativeSolarScenarioSummary* outputSummaries, nuint summaryCapacity,
        NativeSolarScenarioTimelineEntry* outputTimeline, nuint timelineCapacity,
        NativeSolarAttributionEntry* outputAttribution, nuint attributionCapacity,
        NativeSolarCategoryBreakdown* outputCategories, nuint categoryCapacity,
        NativeSolarScenarioDelta* outputDeltas, nuint deltaCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_solar_envelope", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneSolarEnvelope(
        SafeSceneHandle sceneHandle, NativeSolarSensor* sensors, nuint sensorCount,
        NativeEnvelopeCandidate* candidates, nuint candidateCount,
        NativeSunSetMetadata* sunMetadata, NativeSunSample* sunSamples, nuint sunCount,
        NativeAnalysisMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeSolarEnvelopeOptions* options, NativeSolarEnvelopeMetadata* outputMetadata,
        NativeSolarEnvelopeCell* outputCells, nuint cellCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_solar_envelope_oriented", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneSolarEnvelopeOriented(
        SafeSceneHandle sceneHandle, NativeSolarSensor* sensors, nuint sensorCount,
        NativeOrientedEnvelopeCandidate* candidates, nuint candidateCount,
        NativeSunSetMetadata* sunMetadata, NativeSunSample* sunSamples, nuint sunCount,
        NativeAnalysisMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeSolarEnvelopeOptions* options, NativeSolarEnvelopeMetadata* outputMetadata,
        NativeSolarEnvelopeCell* outputCells, nuint cellCapacity);
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnalysisMaterial
{
    internal ulong MaterialId;
    internal double VisibleTransmittance, SolarTransmittance, Reflectance;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeMaterialAssignment
{
    internal ulong ObjectId, MaterialId;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSpatialViewpoint
{
    internal ulong ViewpointId;
    internal double PositionX, PositionY, PositionZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovist3dOptions
{
    internal uint StructureSize, SampleCount;
    internal double MaximumDistanceMeters, EyeOffsetMeters;
    internal ulong CategoryMask;
    internal uint TopK, CounterfactualCount, MaximumMaterialLayers, Reserved;
    internal double MinimumTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovist3dSummary
{
    internal uint StructureSize, Reserved;
    internal ulong ViewpointId;
    internal double VolumeCubicMeters, RadialSurfaceSquareMeters, MeanRadialMeters;
    internal double MinimumRadialMeters, MaximumRadialMeters, VisibleSolidAngleSteradians;
    internal double OpennessRatio, VolumeConvergenceDeltaCubicMeters;
    internal ulong DominantOccluderObjectId;
    internal double DominantOccluderFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovist3dRay
{
    internal uint StructureSize, TriangleId;
    internal double DirectionX, DirectionY, DirectionZ;
    internal double EndpointX, EndpointY, EndpointZ, DistanceMeters, Transmission;
    internal ulong ObjectId, InstanceId, MeshId, CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAttributionEntry
{
    internal uint StructureSize, Rank;
    internal ulong ObserverId, TargetId, ObjectId, CategoryMask;
    internal double BlockedWeight, Fraction, MeanDistanceMeters;
    internal double CounterfactualRecoveredWeight, CounterfactualVolumeDeltaCubicMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeCategoryBreakdown
{
    internal uint StructureSize, Reserved;
    internal ulong ObserverId, TargetId, CategoryMask;
    internal double BlockedWeight, Fraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovist3dMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong ViewpointCount, SampleCount, RayCount, AttributionCount, CategoryCount;
    internal ulong AnalysisTimeMicroseconds, ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeLandmarkObserver
{
    internal ulong ObserverId;
    internal double PositionX, PositionY, PositionZ;
    internal double ForwardX, ForwardY, ForwardZ;
    internal double UpX, UpY, UpZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeLandmark
{
    internal ulong LandmarkId;
    internal double PositionX, PositionY, PositionZ, RadiusMeters, Weight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeLandmarkOptions
{
    internal uint StructureSize, SampleCount;
    internal double HorizontalFieldOfViewRadians, VerticalFieldOfViewRadians;
    internal double MaximumDistanceMeters, EndpointClearanceMeters;
    internal ulong CategoryMask;
    internal uint TopK, MaximumMaterialLayers, Reserved;
    internal double MinimumTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeLandmarkVisibilityEntry
{
    internal uint StructureSize, Flags;
    internal ulong ObserverId, LandmarkId;
    internal double DistanceMeters, ApparentSolidAngleSteradians, VisibleFraction;
    internal double VisibleSolidAngleSteradians, WeightedVisibilityScore;
    internal ulong DominantBlockerObjectId;
    internal double DominantBlockerFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeLandmarkMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong ObserverCount, LandmarkCount, EntryCount, AttributionCount, CategoryCount;
    internal ulong AnalysisTimeMicroseconds, ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenario
{
    internal ulong ScenarioId, MaterialOffset, MaterialCount, AssignmentOffset, AssignmentCount;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenarioOptions
{
    internal uint StructureSize, TopK;
    internal double SensorOffsetMeters, MaximumDistanceMeters, MinimumIncidenceCosine;
    internal ulong CategoryMask;
    internal uint MaximumMaterialLayers, Reserved;
    internal double MinimumTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenarioSummary
{
    internal uint StructureSize, Reserved;
    internal ulong ScenarioId, SensorId;
    internal double ReceivedSunHours, LostSunHours, EligibleSunHours, SolarAccessRatio;
    internal ulong DominantOccluderObjectId;
    internal double DominantOccluderHours;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenarioTimelineEntry
{
    internal uint StructureSize, State;
    internal double Transmission;
    internal ulong FirstObjectId;
    internal uint LayerCount, Flags;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarAttributionEntry
{
    internal uint StructureSize, Rank;
    internal ulong ScenarioId, SensorId, ObjectId, CategoryMask;
    internal double LostSunHours, Fraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarCategoryBreakdown
{
    internal uint StructureSize, Reserved;
    internal ulong ScenarioId, SensorId, CategoryMask;
    internal double LostSunHours, Fraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenarioDelta
{
    internal uint StructureSize, Reserved;
    internal ulong ScenarioId, SensorId;
    internal double ReceivedSunHoursDelta, LostSunHoursDelta, SolarAccessRatioDelta;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarScenarioMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong ScenarioCount, SensorCount, SunCount, SummaryCount, TimelineCount;
    internal ulong AttributionCount, CategoryCount, DeltaCount, AnalysisTimeMicroseconds;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeEnvelopeCandidate
{
    internal ulong CandidateId;
    internal double PositionX, PositionY, PositionZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeOrientedEnvelopeCandidate
{
    internal ulong CandidateId;
    internal double PositionX, PositionY, PositionZ;
    internal double AxisX, AxisY, AxisZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarEnvelopeOptions
{
    internal uint StructureSize, MaximumMaterialLayers;
    internal double CandidateRadiusMeters, RequiredPreservedFraction, TargetShadedFraction;
    internal double VerticalClearanceMeters, SensorOffsetMeters, MaximumDistanceMeters;
    internal ulong CategoryMask;
    internal double MinimumTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeEnvelopeControl
{
    internal ulong SensorId;
    internal long UnixSecondsUtc;
    internal double RayElevationMeters, WeightedHours;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarEnvelopeCell
{
    internal uint StructureSize, Reserved;
    internal ulong CandidateId;
    internal double PositionX, PositionY, PositionZ;
    internal double MaximumSolarAccessElevationMeters, MaximumSolarAccessHeightMeters;
    internal double MinimumShadingElevationMeters, MinimumShadingHeightMeters;
    internal double ConsideredBaselineSunHours;
    internal ulong ConstraintCount;
    internal NativeEnvelopeControl AccessControl, ShadingControl;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarEnvelopeMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong SensorCount, CandidateCount, SunCount, AnalysisTimeMicroseconds;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}
