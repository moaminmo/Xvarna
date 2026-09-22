using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_scene_point_illuminance", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ScenePointIlluminance(
        SafeSceneHandle sceneHandle,
        NativeDaylightSensor* sensors, nuint sensorCount,
        NativeOpticalMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        NativeDaylightMoment* moment,
        NativeDaylightMatrixOptions* options,
        SafeJobHandle jobHandle,
        NativeDaylightMetadata* outputMetadata,
        NativePointIlluminanceEntry* outputEntries, nuint entryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_daylight_factor", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneDaylightFactor(
        SafeSceneHandle sceneHandle,
        NativeDaylightSensor* sensors, nuint sensorCount,
        NativeOpticalMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        double exteriorHorizontalIlluminanceLux,
        NativeDaylightMatrixOptions* options,
        NativeDaylightMetadata* outputMetadata,
        NativeDaylightFactorEntry* outputEntries, nuint entryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_annual_daylight", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneAnnualDaylight(
        SafeSceneHandle sceneHandle,
        NativeDaylightSensor* sensors, nuint sensorCount,
        NativeOpticalMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        byte* epwUtf8, nuint epwLength,
        NativeAnnualDaylightOptions* options,
        SafeJobHandle jobHandle,
        NativeAnnualDaylightMetadata* outputMetadata,
        NativeAnnualDaylightSummary* outputSummaries, nuint summaryCapacity,
        NativeAnnualDaylightTimelineEntry* outputTimeline, nuint timelineCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_daylight_compare", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus DaylightCompare(
        double* fastLux, double* radianceLux, nuint count,
        double absoluteToleranceLux, double relativeTolerance,
        NativeDaylightValidationReport* outputReport);

    [DllImport(LibraryName, EntryPoint = "xv_scene_radiance_export_section", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneRadianceExportSection(
        SafeSceneHandle sceneHandle,
        NativeDaylightSensor* sensors, nuint sensorCount,
        NativeOpticalMaterial* materials, nuint materialCount,
        NativeMaterialAssignment* assignments, nuint assignmentCount,
        uint section,
        byte* outputUtf8, nuint outputCapacity,
        NativeRadianceExportMetadata* outputMetadata);
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeOpticalMaterial
{
    internal ulong MaterialId;
    internal uint Kind, Reserved;
    internal double ReflectanceRed, ReflectanceGreen, ReflectanceBlue;
    internal double TransmittanceRed, TransmittanceGreen, TransmittanceBlue;
    internal double Specularity, Roughness, RefractiveIndex;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightSensor
{
    internal ulong SensorId;
    internal double PositionX, PositionY, PositionZ;
    internal double NormalX, NormalY, NormalZ;
    internal double AreaSquareMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightMatrixOptions
{
    internal uint StructureSize, SkyModel, SkyPatchCount, MaximumMaterialLayers;
    internal double SensorOffsetMeters, MaximumDistanceMeters;
    internal ulong CategoryMask;
    internal double MinimumTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightMoment
{
    internal long UnixSecondsUtc;
    internal double SunDirectionX, SunDirectionY, SunDirectionZ;
    internal double DirectNormalIlluminanceLux, DiffuseHorizontalIlluminanceLux;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePointIlluminanceEntry
{
    internal uint StructureSize, Reserved;
    internal ulong SensorId;
    internal double DirectLux, DiffuseLux, TotalLux, DirectTransmission;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightMetadata
{
    internal uint StructureSize, Flags;
    internal ulong SensorCount, AnalysisTimeMicroseconds;
    internal ulong MatrixHash0, MatrixHash1, MatrixHash2, MatrixHash3;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightFactorEntry
{
    internal uint StructureSize, Reserved;
    internal ulong SensorId;
    internal double InteriorIlluminanceLux, DaylightFactorPercent;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualDaylightOptions
{
    internal uint StructureSize, SkyModel, SkyPatchCount, MaximumMaterialLayers;
    internal double SensorOffsetMeters, MaximumDistanceMeters;
    internal ulong CategoryMask;
    internal double MinimumTransmission;
    internal double DeltaTSeconds, PressureMillibars, TemperatureCelsius;
    internal double NorthRotationDegrees, MinimumAltitudeDegrees;
    internal double DirectLuminousEfficacyLmPerW, DiffuseLuminousEfficacyLmPerW;
    internal double OccupiedStartHour, OccupiedEndHour;
    internal double SdaThresholdLux, SdaRequiredFraction;
    internal double AseThresholdLux, AseMaximumHours;
    internal double UdiLowerLux, UdiPreferredLux, UdiUpperLux;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualDaylightSummary
{
    internal uint StructureSize, Flags;
    internal ulong SensorId;
    internal double OccupiedHours, SdaQualifiedHours, SdaOccupiedFraction;
    internal double AseExceedanceHours;
    internal double UdiBelowFraction, UdiSupplementalFraction;
    internal double UdiUsefulFraction, UdiExceededFraction;
    internal double MeanOccupiedLux, MinimumOccupiedLux, MaximumOccupiedLux;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualDaylightTimelineEntry
{
    internal uint StructureSize, Flags;
    internal long UnixSecondsUtc;
    internal double DirectLux, DiffuseLux, TotalLux;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeAnnualDaylightMetadata
{
    internal uint StructureSize, Flags;
    internal ulong SensorCount, WeatherCount, TimelineCount, AnalysisTimeMicroseconds;
    internal double SdaAreaPercent, AseAreaPercent;
    internal double UdiBelowPercent, UdiSupplementalPercent;
    internal double UdiUsefulPercent, UdiExceededPercent;
    internal double TotalSensorAreaSquareMeters;
    internal ulong MatrixHash0, MatrixHash1, MatrixHash2, MatrixHash3;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeDaylightValidationReport
{
    internal uint StructureSize, Flags;
    internal ulong Count;
    internal double MeanBiasLux, MeanAbsoluteErrorLux, RootMeanSquareErrorLux;
    internal double MeanAbsolutePercentageError, MaximumAbsoluteErrorLux;
    internal double RSquared, AcceptedFraction;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeRadianceExportMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong RequiredBytes;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}
