using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    internal const string LibraryName = "xvarna_core";

    [DllImport(LibraryName, EntryPoint = "xv_abi_version_major", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint AbiVersionMajor();

    [DllImport(LibraryName, EntryPoint = "xv_abi_version_minor", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint AbiVersionMinor();

    [DllImport(LibraryName, EntryPoint = "xv_abi_version_patch", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint AbiVersionPatch();

    [DllImport(LibraryName, EntryPoint = "xv_engine_version_major", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint EngineVersionMajor();

    [DllImport(LibraryName, EntryPoint = "xv_engine_version_minor", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint EngineVersionMinor();

    [DllImport(LibraryName, EntryPoint = "xv_engine_version_patch", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern uint EngineVersionPatch();

    [DllImport(LibraryName, EntryPoint = "xv_job_create", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus JobCreate(ulong* outputHandle);

    [DllImport(LibraryName, EntryPoint = "xv_job_cancel", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus JobCancel(SafeJobHandle handle);

    [DllImport(LibraryName, EntryPoint = "xv_job_progress", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus JobProgress(
        SafeJobHandle handle,
        NativeJobProgress* outputProgress);

    [DllImport(LibraryName, EntryPoint = "xv_job_release", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus JobRelease(ulong handle);

    [DllImport(LibraryName, EntryPoint = "xv_mesh_audit", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus MeshAudit(
        double* positions,
        nuint vertexCount,
        uint* triangles,
        nuint faceCount,
        double absoluteTolerance,
        double maximumF32ErrorRatio,
        NativeMeshAuditSummary* outputSummary,
        uint* outputFaceFlags,
        nuint faceFlagsCapacity,
        uint* outputVertexFlags,
        nuint vertexFlagsCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_surface_grid", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SurfaceGrid(
        double* positions,
        nuint vertexCount,
        uint* triangles,
        nuint faceCount,
        NativeSurfaceGridOptions* options,
        NativeSurfaceGridMetadata* outputMetadata,
        NativeSurfaceCell* outputCells,
        nuint cellCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_surface_pv_potential", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SurfacePvPotential(
        NativeSurfaceCell* cells,
        double* irradianceWhM2,
        nuint cellCount,
        NativePvPotentialOptions* options,
        NativePvPotentialMetadata* outputMetadata,
        NativePvPotentialCell* outputCells,
        nuint cellCapacity,
        NativePvPotentialRegion* outputRegions,
        nuint regionCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_create", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneCreate(
        NativeSceneOptions* options,
        ulong* outputHandle);

    [DllImport(LibraryName, EntryPoint = "xv_scene_add_mesh", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneAddMesh(
        SafeSceneHandle handle,
        double* positions,
        nuint vertexCount,
        uint* triangles,
        nuint faceCount,
        ulong* outputMeshId);

    [DllImport(LibraryName, EntryPoint = "xv_scene_add_instance", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneAddInstance(
        SafeSceneHandle handle,
        ulong meshId,
        double* rowMajorTransform,
        ulong objectId,
        ulong instanceId,
        ulong categoryMask);

    [DllImport(LibraryName, EntryPoint = "xv_scene_add_instance_layered", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneAddInstanceLayered(
        SafeSceneHandle handle,
        ulong meshId,
        double* rowMajorTransform,
        ulong objectId,
        ulong instanceId,
        ulong categoryMask,
        uint layer);

    [DllImport(LibraryName, EntryPoint = "xv_scene_build", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneBuild(
        SafeSceneHandle handle,
        NativeSceneStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_scene_get_stats", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneGetStats(
        SafeSceneHandle handle,
        NativeSceneStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_scene_apply_instance_deltas", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneApplyInstanceDeltas(
        SafeSceneHandle handle,
        NativeSceneInstanceDelta* deltas,
        nuint deltaCount,
        NativeSceneUpdateOptions* options,
        NativeSceneUpdateReport* outputReport,
        NativeSceneStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_scene_replace_mesh", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneReplaceMesh(
        SafeSceneHandle handle,
        ulong meshId,
        double* positions,
        nuint vertexCount,
        uint* triangles,
        nuint faceCount,
        NativeSceneUpdateOptions* options,
        NativeSceneUpdateReport* outputReport,
        NativeSceneStats* outputStats);

    [DllImport(LibraryName, EntryPoint = "xv_scene_trace_closest", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneTraceClosest(
        SafeSceneHandle handle,
        NativeRay* rays,
        nuint rayCount,
        NativeHit* outputHits,
        nuint hitCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_trace_any", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneTraceAny(
        SafeSceneHandle handle,
        NativeRay* rays,
        nuint rayCount,
        byte* outputHits,
        nuint hitCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_isovist", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneIsovist(
        SafeSceneHandle sceneHandle,
        NativeViewpoint* viewpoints,
        nuint viewpointCount,
        NativeIsovistOptions* options,
        NativeIsovistMetadata* outputMetadata,
        NativeIsovistSummary* outputSummaries,
        nuint summaryCapacity,
        NativeIsovistRay* outputRays,
        nuint rayCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_intervisibility", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneIntervisibility(
        SafeSceneHandle sceneHandle,
        NativeVisibilityObserver* observers,
        nuint observerCount,
        NativeVisibilityTarget* targets,
        nuint targetCount,
        NativeIntervisibilityOptions* options,
        NativeIntervisibilityMetadata* outputMetadata,
        NativeObserverVisibilitySummary* outputObserverSummaries,
        nuint observerSummaryCapacity,
        NativeTargetVisibilitySummary* outputTargetSummaries,
        nuint targetSummaryCapacity,
        NativeIntervisibilityEntry* outputEntries,
        nuint entryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_visibility_graph", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneVisibilityGraph(
        SafeSceneHandle sceneHandle,
        NativeVisibilityNode* nodes,
        nuint nodeCount,
        NativeVisibilityGraphOptions* options,
        NativeVisibilityGraphMetadata* outputMetadata,
        NativeVisibilityNodeMetrics* outputMetrics,
        nuint metricCapacity,
        NativeVisibilityGraphPair* outputPairs,
        nuint pairCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_sparse_visibility_graph", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneSparseVisibilityGraph(
        SafeSceneHandle sceneHandle,
        NativeVisibilityNode* nodes,
        nuint nodeCount,
        NativeSparseVisibilityGraphOptions* options,
        NativeVisibilityGraphMetadata* outputMetadata,
        NativeVisibilityNodeMetrics* outputMetrics,
        nuint metricCapacity,
        NativeVisibilityGraphPair* outputPairs,
        nuint pairCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_release", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneRelease(ulong handle);

    [DllImport(LibraryName, EntryPoint = "xv_sun_positions", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SunPositions(
        NativeSolarOptions* options,
        NativeTimeSample* times,
        nuint timeCount,
        NativeSunSetMetadata* outputMetadata,
        NativeSunSample* outputSamples,
        nuint sampleCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_epw_inspect", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus EpwInspect(
        byte* epwUtf8,
        nuint epwLength,
        NativeEpwMetadata* outputMetadata);

    [DllImport(LibraryName, EntryPoint = "xv_scene_direct_sun", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneDirectSun(
        SafeSceneHandle sceneHandle,
        NativeSolarSensor* sensors,
        nuint sensorCount,
        NativeSunSetMetadata* sunMetadata,
        NativeSunSample* sunSamples,
        nuint sunCount,
        NativeDirectSunOptions* options,
        NativeDirectSunMetadata* outputMetadata,
        NativeDirectSunSummary* outputSummaries,
        nuint summaryCapacity,
        NativeSunTimelineEntry* outputTimeline,
        nuint timelineCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_sky_view", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneSkyView(
        SafeSceneHandle sceneHandle,
        NativeSolarSensor* sensors,
        nuint sensorCount,
        NativeSkyViewOptions* options,
        SafeJobHandle jobHandle,
        NativeSkyViewMetadata* outputMetadata,
        NativeSkyViewSummary* outputSummaries,
        nuint summaryCapacity,
        NativeSkyRayEntry* outputTimeline,
        nuint timelineCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_annual_irradiance", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneAnnualIrradiance(
        SafeSceneHandle sceneHandle,
        NativeSolarSensor* sensors,
        nuint sensorCount,
        byte* epwUtf8,
        nuint epwLength,
        NativeAnnualIrradianceOptions* options,
        SafeJobHandle jobHandle,
        NativeAnnualIrradianceMetadata* outputMetadata,
        NativeIrradianceSummary* outputSummaries,
        nuint summaryCapacity,
        NativeIrradianceTimelineEntry* outputTimeline,
        nuint timelineCapacity);
}

internal enum NativeStatus
{
    Success = 0,
    NullPointer = 1,
    InvalidLength = 2,
    InvalidArgument = 3,
    InvalidHandle = 4,
    InvalidState = 5,
    Cancelled = 6,
    Panic = 255,
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeJobProgress
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong TotalUnits;
    internal ulong CompletedUnits;
    internal ulong Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSceneOptions
{
    internal uint StructureSize;
    internal uint MaximumLeafSize;
    internal uint ThreadCount;
    internal uint Reserved;
    internal double UnitScaleToMeters;
    internal double AbsoluteToleranceMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeRay
{
    internal double OriginX;
    internal double OriginY;
    internal double OriginZ;
    internal double DirectionX;
    internal double DirectionY;
    internal double DirectionZ;
    internal double TMin;
    internal double TMax;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeHit
{
    internal uint StructureSize;
    internal uint Flags;
    internal double Distance;
    internal double BarycentricU;
    internal double BarycentricV;
    internal ulong ObjectId;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal uint TriangleId;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSceneStats
{
    internal uint StructureSize;
    internal uint ThreadCount;
    internal ulong MeshResourceCount;
    internal ulong InstanceCount;
    internal ulong UniqueTriangleCount;
    internal ulong InstancedTriangleCount;
    internal ulong BlasNodeCount;
    internal ulong TlasNodeCount;
    internal ulong MaximumBvhDepth;
    internal ulong ApproximateMemoryBytes;
    internal ulong BuildTimeMicroseconds;
    internal double RebaseOriginX;
    internal double RebaseOriginY;
    internal double RebaseOriginZ;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeSceneInstanceDelta
{
    internal uint StructureSize;
    internal uint Operation;
    internal uint Flags;
    internal uint Layer;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal ulong ObjectId;
    internal ulong CategoryMask;
    internal fixed double Transform[16];
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSceneUpdateOptions
{
    internal uint StructureSize;
    internal uint MaximumConsecutiveRefits;
    internal double MaximumRefitQualityRatio;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeSceneUpdateReport
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong DeltaCount;
    internal ulong StaticChangeCount;
    internal ulong DynamicChangeCount;
    internal ulong ReusedBlasCount;
    internal ulong RebuiltBlasCount;
    internal ulong UpdateTimeMicroseconds;
    internal double MaximumRefitQualityRatio;
    internal fixed ulong PreviousHash[4];
    internal fixed ulong CurrentHash[4];
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarOptions
{
    internal uint StructureSize;
    internal uint Reserved;
    internal double LatitudeDegrees;
    internal double LongitudeDegrees;
    internal double ElevationMeters;
    internal double DeltaTSeconds;
    internal double PressureMillibars;
    internal double TemperatureCelsius;
    internal double NorthRotationDegrees;
    internal double MinimumAltitudeDegrees;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTimeSample
{
    internal long UnixSecondsUtc;
    internal double DurationHours;
    internal double Weight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSunSample
{
    internal long UnixSecondsUtc;
    internal double DirectionX;
    internal double DirectionY;
    internal double DirectionZ;
    internal double AltitudeDegrees;
    internal double AzimuthDegrees;
    internal double DurationHours;
    internal double Weight;
    internal uint Flags;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSunSetMetadata
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong SampleCount;
    internal ulong ActiveSampleCount;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSolarSensor
{
    internal ulong SensorId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
    internal double NormalX;
    internal double NormalY;
    internal double NormalZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDirectSunOptions
{
    internal uint StructureSize;
    internal uint Reserved;
    internal double SensorOffsetMeters;
    internal double MaximumDistanceMeters;
    internal double MinimumIncidenceCosine;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDirectSunSummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorId;
    internal double DirectSunHours;
    internal double ShadowHours;
    internal double EligibleHours;
    internal double SolarAccessRatio;
    internal ulong VisibleCount;
    internal ulong BlockedCount;
    internal ulong BackFacingCount;
    internal ulong DominantOccluderObjectId;
    internal double DominantOccluderHours;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSunTimelineEntry
{
    internal uint StructureSize;
    internal uint State;
    internal ulong ObjectId;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal uint TriangleId;
    internal uint Reserved;
    internal double DistanceMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDirectSunMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorCount;
    internal ulong SunCount;
    internal ulong TimelineCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSkyViewOptions
{
    internal uint StructureSize;
    internal uint SampleCount;
    internal ulong Seed;
    internal double SensorOffsetMeters;
    internal double MaximumDistanceMeters;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSkyViewSummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorId;
    internal double VisibleHemisphereFraction;
    internal double CosineWeightedSvf;
    internal double VisibleSolidAngleSteradians;
    internal double CosineConvergenceDelta;
    internal double UnweightedConvergenceDelta;
    internal ulong VisibleCount;
    internal ulong BlockedCount;
    internal ulong DominantOccluderObjectId;
    internal double DominantOccluderSolidAngleSteradians;
    internal double DominantOccluderProjectedFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSkyRayEntry
{
    internal uint StructureSize;
    internal uint State;
    internal double DirectionX;
    internal double DirectionY;
    internal double DirectionZ;
    internal ulong ObjectId;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal uint TriangleId;
    internal uint Reserved;
    internal double DistanceMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSkyViewMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorCount;
    internal ulong SampleCount;
    internal ulong TimelineCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeEpwMetadata
{
    internal uint StructureSize;
    internal uint RecordsPerHour;
    internal ulong WeatherCount;
    internal ulong MissingGlobalHorizontalCount;
    internal ulong MissingDirectNormalCount;
    internal ulong MissingDiffuseHorizontalCount;
    internal double LatitudeDegrees;
    internal double LongitudeDegrees;
    internal double TimeZoneHours;
    internal double ElevationMeters;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualIrradianceOptions
{
    internal uint StructureSize;
    internal uint Flags;
    internal double SensorOffsetMeters;
    internal double MaximumDistanceMeters;
    internal ulong CategoryMask;
    internal double GroundAlbedo;
    internal double DeltaTSeconds;
    internal double PressureMillibars;
    internal double TemperatureCelsius;
    internal double NorthRotationDegrees;
    internal double MinimumAltitudeDegrees;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIrradianceSummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorId;
    internal double DirectWhM2;
    internal double DiffuseSkyWhM2;
    internal double GroundReflectedWhM2;
    internal double GlobalWhM2;
    internal double PeakGlobalWM2;
    internal long PeakUnixSecondsUtc;
    internal double DomeVisibilityRatio;
    internal double HorizonVisibilityRatio;
    internal ulong VisibleCount;
    internal ulong BlockedCount;
    internal ulong BackFacingCount;
    internal ulong DominantSolarOccluderObjectId;
    internal double DominantSolarOccluderLossWhM2;
    internal ulong DominantSkyOccluderObjectId;
    internal double DominantSkyOccluderProjectedFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIrradianceTimelineEntry
{
    internal uint StructureSize;
    internal uint State;
    internal long UnixSecondsUtc;
    internal double DirectWhM2;
    internal double DiffuseDomeWhM2;
    internal double DiffuseCircumsolarWhM2;
    internal double DiffuseHorizonWhM2;
    internal double DiffuseSkyWhM2;
    internal double GroundReflectedWhM2;
    internal double GlobalWhM2;
    internal double GlobalWM2;
    internal double AttributedLossWhM2;
    internal ulong ObjectId;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal uint TriangleId;
    internal uint Reserved;
    internal double DistanceMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualIrradianceMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong SensorCount;
    internal ulong WeatherCount;
    internal ulong TimelineCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong MissingGlobalHorizontalCount;
    internal ulong MissingDirectNormalCount;
    internal ulong MissingDiffuseHorizontalCount;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeMeshAuditSummary
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong VertexCount;
    internal ulong FaceCount;
    internal ulong AcceptedFaceCount;
    internal ulong NonFiniteVertexCount;
    internal ulong InvalidIndexFaceCount;
    internal ulong NonFiniteFaceCount;
    internal ulong DegenerateFaceCount;
    internal ulong DuplicateFaceCount;
    internal ulong IsolatedVertexCount;
    internal ulong BoundaryEdgeCount;
    internal ulong NonManifoldEdgeCount;
    internal ulong InconsistentWindingEdgeCount;
    internal ulong ConnectedComponentCount;
    internal double SurfaceArea;
    internal double SignedVolume;
    internal double BoundsMinX;
    internal double BoundsMinY;
    internal double BoundsMinZ;
    internal double BoundsMaxX;
    internal double BoundsMaxY;
    internal double BoundsMaxZ;
    internal double MaximumCoordinateMagnitude;
    internal double EstimatedF32Error;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSurfaceGridOptions
{
    internal uint StructureSize;
    internal uint Reserved;
    internal double TargetEdgeLength;
    internal double SensorOffset;
    internal ulong FirstSensorId;
    internal ulong MaximumCellCount;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSurfaceCell
{
    internal uint StructureSize;
    internal uint SourceFaceIndex;
    internal ulong SensorId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
    internal double NormalX;
    internal double NormalY;
    internal double NormalZ;
    internal double Area;
    internal double AX;
    internal double AY;
    internal double AZ;
    internal double BX;
    internal double BY;
    internal double BZ;
    internal double CX;
    internal double CY;
    internal double CZ;
    internal uint SubdivisionDepth;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSurfaceGridMetadata
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong SourceFaceCount;
    internal ulong CellCount;
    internal ulong SkippedFaceCount;
    internal ulong MaximumSubdivisionDepth;
    internal double SourceArea;
    internal double SampledArea;
    internal double MaximumCellEdgeLength;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePvPotentialOptions
{
    internal uint StructureSize;
    internal uint Reserved;
    internal double UnitScaleToMeters;
    internal double MinimumIrradianceWhM2;
    internal double ModuleEfficiency;
    internal double CoverageRatio;
    internal double SystemLossFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePvPotentialCell
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong SensorId;
    internal ulong RegionId;
    internal double AreaM2;
    internal double IrradianceWhM2;
    internal double IncidentEnergyKWh;
    internal double ProxyYieldKWh;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePvPotentialRegion
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong RegionId;
    internal ulong CellCount;
    internal double AreaM2;
    internal double MeanIrradianceWhM2;
    internal double MinimumIrradianceWhM2;
    internal double MaximumIrradianceWhM2;
    internal double IncidentEnergyKWh;
    internal double ProxyYieldKWh;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePvPotentialMetadata
{
    internal uint StructureSize;
    internal uint Flags;
    internal ulong CellCount;
    internal ulong EligibleCellCount;
    internal ulong RegionCount;
    internal double TotalAreaM2;
    internal double EligibleAreaM2;
    internal double MeanIrradianceWhM2;
    internal double P10IrradianceWhM2;
    internal double P50IrradianceWhM2;
    internal double P90IrradianceWhM2;
    internal double TotalIncidentEnergyKWh;
    internal double EligibleIncidentEnergyKWh;
    internal double CapacityKwp;
    internal double ProxyYieldKWh;
    internal double SpecificYieldKWhKwp;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewpoint
{
    internal ulong ViewpointId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
    internal double PlaneNormalX;
    internal double PlaneNormalY;
    internal double PlaneNormalZ;
    internal double ForwardX;
    internal double ForwardY;
    internal double ForwardZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovistOptions
{
    internal uint StructureSize;
    internal uint SampleCount;
    internal double FieldOfViewRadians;
    internal double MaximumDistanceMeters;
    internal double EyeOffsetMeters;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovistSummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong ViewpointId;
    internal double AreaSquareMeters;
    internal double PerimeterMeters;
    internal double CentroidDistanceMeters;
    internal double MeanRadialMeters;
    internal double MinimumRadialMeters;
    internal double MaximumRadialMeters;
    internal double RadialStandardDeviationMeters;
    internal double RadialSkewness;
    internal double Compactness;
    internal double AreaConvergenceDeltaSquareMeters;
    internal ulong OccludedCount;
    internal ulong OpenCount;
    internal ulong DominantOccluderObjectId;
    internal double DominantOccluderFraction;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovistRay
{
    internal uint StructureSize;
    internal uint State;
    internal double DirectionX;
    internal double DirectionY;
    internal double DirectionZ;
    internal double EndpointX;
    internal double EndpointY;
    internal double EndpointZ;
    internal double DistanceMeters;
    internal ulong ObjectId;
    internal ulong InstanceId;
    internal ulong MeshId;
    internal uint TriangleId;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIsovistMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong ViewpointCount;
    internal ulong SampleCount;
    internal ulong RayCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityObserver
{
    internal ulong ObserverId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
    internal double Weight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityTarget
{
    internal ulong TargetId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
    internal double FacingX;
    internal double FacingY;
    internal double FacingZ;
    internal double Sensitivity;
    internal uint Flags;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIntervisibilityOptions
{
    internal uint StructureSize;
    internal uint Reserved;
    internal double EndpointClearanceMeters;
    internal double MaximumDistanceMeters;
    internal double PrivacyReferenceDistanceMeters;
    internal double FacingExponent;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIntervisibilityEntry
{
    internal uint StructureSize;
    internal uint State;
    internal double DistanceMeters;
    internal double DirectionX;
    internal double DirectionY;
    internal double DirectionZ;
    internal double PrivacyRisk;
    internal double FacingFactor;
    internal double DistanceFactor;
    internal ulong BlockerObjectId;
    internal ulong BlockerInstanceId;
    internal ulong BlockerMeshId;
    internal uint BlockerTriangleId;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverVisibilitySummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong ObserverId;
    internal ulong VisibleCount;
    internal double VisibleFraction;
    internal double TotalPrivacyRisk;
    internal double PeakPrivacyRisk;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTargetVisibilitySummary
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong TargetId;
    internal ulong VisibleObserverCount;
    internal double ExposureFraction;
    internal double CumulativePrivacyRisk;
    internal double CombinedPrivacyRisk;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeIntervisibilityMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong ObserverCount;
    internal ulong TargetCount;
    internal ulong EntryCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityNode
{
    internal ulong NodeId;
    internal double PositionX;
    internal double PositionY;
    internal double PositionZ;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityGraphOptions
{
    internal uint StructureSize;
    internal uint Flags;
    internal double EndpointClearanceMeters;
    internal double MaximumDistanceMeters;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSparseVisibilityGraphOptions
{
    internal uint StructureSize;
    internal uint Flags;
    internal double EndpointClearanceMeters;
    internal double MaximumDistanceMeters;
    internal ulong CategoryMask;
    internal uint MaximumNeighbors;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityGraphPair
{
    internal uint StructureSize;
    internal uint State;
    internal ulong FirstIndex;
    internal ulong SecondIndex;
    internal double DistanceMeters;
    internal ulong BlockerObjectId;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityNodeMetrics
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong NodeId;
    internal ulong Degree;
    internal double DegreeCentrality;
    internal ulong ComponentIndex;
    internal double HarmonicCloseness;
    internal double BetweennessCentrality;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVisibilityGraphMetadata
{
    internal uint StructureSize;
    internal uint Reserved;
    internal ulong NodeCount;
    internal ulong PairCount;
    internal ulong VisibleEdgeCount;
    internal ulong ConnectedComponentCount;
    internal ulong AnalysisTimeMicroseconds;
    internal ulong ContentHash0;
    internal ulong ContentHash1;
    internal ulong ContentHash2;
    internal ulong ContentHash3;
}
