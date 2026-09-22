using System.Runtime.InteropServices;
using System.Text;

#pragma warning disable CS1591 // The complete 0.15 API reference is maintained in docs/sdk/daylight.md.

namespace Xvarna.Native;

/// <summary>Radiance-compatible optical primitive family.</summary>
public enum OpticalMaterialKind
{
    Plastic = 0,
    Glass = 1,
    Metal = 2,
    Trans = 3,
    Mirror = 4,
}

/// <summary>Physically bounded linear-RGB optical material.</summary>
public readonly record struct OpticalMaterial(
    ulong MaterialId,
    OpticalMaterialKind Kind,
    double ReflectanceRed,
    double ReflectanceGreen,
    double ReflectanceBlue,
    double TransmittanceRed,
    double TransmittanceGreen,
    double TransmittanceBlue,
    double Specularity = 0.0,
    double Roughness = 0.0,
    double RefractiveIndex = 1.52);

/// <summary>Immutable object-exact optical catalog shared by Fast Path and Radiance export.</summary>
public sealed class OpticalMaterialLibrary
{
    public OpticalMaterialLibrary(
        IReadOnlyList<OpticalMaterial>? materials = null,
        IReadOnlyList<MaterialAssignment>? assignments = null)
    {
        Materials = materials?.ToArray() ?? [];
        Assignments = assignments?.ToArray() ?? [];
        HashSet<ulong> materialIds = [];
        foreach (OpticalMaterial material in Materials)
        {
            if (material.MaterialId == 0 || !materialIds.Add(material.MaterialId)
                || !Enum.IsDefined(material.Kind)
                || !Fraction(material.ReflectanceRed) || !Fraction(material.ReflectanceGreen)
                || !Fraction(material.ReflectanceBlue) || !Fraction(material.TransmittanceRed)
                || !Fraction(material.TransmittanceGreen) || !Fraction(material.TransmittanceBlue)
                || material.ReflectanceRed + material.TransmittanceRed > 1.0 + 1.0e-12
                || material.ReflectanceGreen + material.TransmittanceGreen > 1.0 + 1.0e-12
                || material.ReflectanceBlue + material.TransmittanceBlue > 1.0 + 1.0e-12
                || !Fraction(material.Specularity) || !Fraction(material.Roughness)
                || !double.IsFinite(material.RefractiveIndex)
                || material.RefractiveIndex is < 1.0 or > 3.0
                || (material.Kind is not OpticalMaterialKind.Glass and not OpticalMaterialKind.Trans
                    && (material.TransmittanceRed > 0 || material.TransmittanceGreen > 0 || material.TransmittanceBlue > 0)))
            {
                throw new ArgumentException("Optical materials require unique non-zero IDs, bounded energy-conserving RGB values, and kind-compatible transmission.", nameof(materials));
            }
        }
        HashSet<ulong> objectIds = [];
        foreach (MaterialAssignment assignment in Assignments)
        {
            if (!materialIds.Contains(assignment.MaterialId) || !objectIds.Add(assignment.ObjectId))
            {
                throw new ArgumentException("Assignments require unique object IDs and known material IDs.", nameof(assignments));
            }
        }
    }

    public IReadOnlyList<OpticalMaterial> Materials { get; }
    public IReadOnlyList<MaterialAssignment> Assignments { get; }
    public static OpticalMaterialLibrary Opaque { get; } = new();

    internal NativeOpticalMaterial[] NativeMaterials() => Materials.Select(value => new NativeOpticalMaterial
    {
        MaterialId = value.MaterialId,
        Kind = (uint)value.Kind,
        ReflectanceRed = value.ReflectanceRed,
        ReflectanceGreen = value.ReflectanceGreen,
        ReflectanceBlue = value.ReflectanceBlue,
        TransmittanceRed = value.TransmittanceRed,
        TransmittanceGreen = value.TransmittanceGreen,
        TransmittanceBlue = value.TransmittanceBlue,
        Specularity = value.Specularity,
        Roughness = value.Roughness,
        RefractiveIndex = value.RefractiveIndex,
    }).ToArray();

    internal NativeMaterialAssignment[] NativeAssignments() => Assignments.Select(value => new NativeMaterialAssignment
    {
        ObjectId = value.ObjectId,
        MaterialId = value.MaterialId,
    }).ToArray();

    private static bool Fraction(double value) => double.IsFinite(value) && value is >= 0.0 and <= 1.0;
}

/// <summary>Oriented model-space workplane sensor with a positive represented area.</summary>
public readonly record struct DaylightSensor(
    ulong SensorId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double NormalX,
    double NormalY,
    double NormalZ,
    double Area);

/// <summary>Diffuse luminance model used by the deterministic Fast Path.</summary>
public enum DaylightSkyModel
{
    Isotropic = 0,
    CieOvercast = 1,
}

/// <summary>Reusable daylight-coefficient ray and transmission policy.</summary>
public readonly record struct DaylightMatrixOptions(
    uint SkyPatchCount = 576,
    double SensorOffset = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    ulong CategoryMask = ulong.MaxValue,
    uint MaximumMaterialLayers = 8,
    double MinimumTransmission = 1.0e-4,
    DaylightSkyModel SkyModel = DaylightSkyModel.Isotropic);

/// <summary>One photometric sun and sky condition.</summary>
public readonly record struct DaylightMoment(
    DateTimeOffset TimestampUtc,
    double SunDirectionX,
    double SunDirectionY,
    double SunDirectionZ,
    double DirectNormalIlluminanceLux,
    double DiffuseHorizontalIlluminanceLux);

public readonly record struct PointIlluminanceEntry(
    ulong SensorId,
    double DirectLux,
    double DiffuseLux,
    double TotalLux,
    double DirectTransmission);

public sealed record PointIlluminanceResult
{
    public required IReadOnlyList<PointIlluminanceEntry> Entries { get; init; }
    public required bool MatrixReused { get; init; }
    public required string MatrixHash { get; init; }
    public required ulong AnalysisTimeMicroseconds { get; init; }
    public required string ContentHash { get; init; }
}

public readonly record struct DaylightFactorEntry(
    ulong SensorId,
    double InteriorIlluminanceLux,
    double DaylightFactorPercent);

public sealed record DaylightFactorResult
{
    public required IReadOnlyList<DaylightFactorEntry> Entries { get; init; }
    public required double ExteriorHorizontalIlluminanceLux { get; init; }
    public required bool MatrixReused { get; init; }
    public required string MatrixHash { get; init; }
    public required ulong AnalysisTimeMicroseconds { get; init; }
    public required string ContentHash { get; init; }
}

/// <summary>Climate, occupancy, luminous-efficacy, sDA, ASE, and UDI policy.</summary>
public readonly record struct AnnualDaylightOptions(
    DaylightMatrixOptions Matrix,
    SolarPositionOptions Solar,
    double DirectLuminousEfficacyLmPerW = 100.0,
    double DiffuseLuminousEfficacyLmPerW = 120.0,
    double OccupiedStartHour = 8.0,
    double OccupiedEndHour = 18.0,
    double SdaThresholdLux = 300.0,
    double SdaRequiredFraction = 0.5,
    double AseThresholdLux = 1000.0,
    double AseMaximumHours = 250.0,
    double UdiLowerLux = 100.0,
    double UdiPreferredLux = 300.0,
    double UdiUpperLux = 3000.0)
{
    public static AnnualDaylightOptions Default { get; } = new(
        new DaylightMatrixOptions(), SolarPositionOptions.Default);
}

public readonly record struct AnnualDaylightSensorSummary(
    ulong SensorId,
    double OccupiedHours,
    double SdaQualifiedHours,
    double SdaOccupiedFraction,
    double AseExceedanceHours,
    bool AseFails,
    double UdiBelowFraction,
    double UdiSupplementalFraction,
    double UdiUsefulFraction,
    double UdiExceededFraction,
    double MeanOccupiedLux,
    double MinimumOccupiedLux,
    double MaximumOccupiedLux);

public readonly record struct AnnualDaylightTimelineEntry(
    DateTimeOffset TimestampUtc,
    bool Occupied,
    double DirectLux,
    double DiffuseLux,
    double TotalLux);

public readonly record struct AnnualDaylightProjectSummary(
    double SdaAreaPercent,
    double AseAreaPercent,
    double UdiBelowPercent,
    double UdiSupplementalPercent,
    double UdiUsefulPercent,
    double UdiExceededPercent,
    double TotalSensorArea);

public sealed record AnnualDaylightResult
{
    public required IReadOnlyList<AnnualDaylightSensorSummary> Summaries { get; init; }
    public required IReadOnlyList<AnnualDaylightTimelineEntry> Timeline { get; init; }
    public required AnnualDaylightProjectSummary Project { get; init; }
    public required ulong WeatherCount { get; init; }
    public required bool MatrixReused { get; init; }
    public required string MatrixHash { get; init; }
    public required ulong AnalysisTimeMicroseconds { get; init; }
    public required string ContentHash { get; init; }
}

/// <summary>Fast Path versus aligned Radiance reference statistics.</summary>
public readonly record struct DaylightValidationReport(
    ulong Count,
    double MeanBiasLux,
    double MeanAbsoluteErrorLux,
    double RootMeanSquareErrorLux,
    double MeanAbsolutePercentageError,
    double MaximumAbsoluteErrorLux,
    double RSquared,
    double AcceptedFraction,
    bool Accepted,
    string ContentHash);

/// <summary>Deterministic, exact world-space Radiance scene interchange bundle.</summary>
public sealed record RadianceExportBundle
{
    public required string MaterialsRad { get; init; }
    public required string GeometryRad { get; init; }
    public required string SensorsPts { get; init; }
    public required string ManifestJson { get; init; }
    public required string ContentHash { get; init; }

    public void Write(string directory)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(directory);
        Directory.CreateDirectory(directory);
        File.WriteAllText(Path.Combine(directory, "materials.rad"), MaterialsRad, new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(directory, "geometry.rad"), GeometryRad, new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(directory, "sensors.pts"), SensorsPts, new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(directory, "manifest.json"), ManifestJson, new UTF8Encoding(false));
    }
}

public sealed partial class XvarnaScene
{
    private const uint DaylightMatrixOptionsSize = 48;
    private const uint AnnualDaylightOptionsSize = 176;

    public PointIlluminanceResult AnalyzePointIlluminance(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials,
        DaylightMoment moment,
        DaylightMatrixOptions options,
        CancellationToken cancellationToken = default)
    {
        using SafeJobHandle job = SafeJobHandle.Create();
        using CancellationTokenRegistration registration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(), job);
        return AnalyzePointIlluminanceCore(sensors, materials, moment, options, cancellationToken, job);
    }

    public Task<PointIlluminanceResult> AnalyzePointIlluminanceAsync(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials,
        DaylightMoment moment,
        DaylightMatrixOptions options,
        CancellationToken cancellationToken = default) =>
        BeginPointIlluminance(sensors, materials, moment, options, cancellationToken).Completion;

    public DaylightAnalysisJob<PointIlluminanceResult> BeginPointIlluminance(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials,
        DaylightMoment moment,
        DaylightMatrixOptions options,
        CancellationToken cancellationToken = default)
    {
        DaylightSensor[] snapshot = ValidateInputs(sensors, materials);
        return new((handle, token) => AnalyzePointIlluminanceCore(snapshot, materials, moment, options, token, handle), cancellationToken);
    }

    internal unsafe PointIlluminanceResult AnalyzePointIlluminanceCore(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials,
        DaylightMoment moment,
        DaylightMatrixOptions options,
        CancellationToken cancellationToken,
        SafeJobHandle job)
    {
        DaylightSensor[] source = ValidateInputs(sensors, materials);
        cancellationToken.ThrowIfCancellationRequested();
        NativeDaylightSensor[] nativeSensors = ConvertSensors(source);
        NativeOpticalMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeDaylightMoment nativeMoment = new()
        {
            UnixSecondsUtc = moment.TimestampUtc.ToUnixTimeSeconds(),
            SunDirectionX = moment.SunDirectionX,
            SunDirectionY = moment.SunDirectionY,
            SunDirectionZ = moment.SunDirectionZ,
            DirectNormalIlluminanceLux = moment.DirectNormalIlluminanceLux,
            DiffuseHorizontalIlluminanceLux = moment.DiffuseHorizontalIlluminanceLux,
        };
        NativeDaylightMatrixOptions nativeOptions = ConvertOptions(options);
        NativePointIlluminanceEntry[] entries = new NativePointIlluminanceEntry[source.Length];
        NativeDaylightMetadata metadata = default;
        fixed (NativeDaylightSensor* sensorPointer = nativeSensors)
        fixed (NativeOpticalMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (NativePointIlluminanceEntry* entryPointer = entries)
        {
            NativeStatus status = NativeMethods.ScenePointIlluminance(
                handle, sensorPointer, (nuint)source.Length,
                materialPointer, (nuint)nativeMaterials.Length,
                assignmentPointer, (nuint)nativeAssignments.Length,
                &nativeMoment, &nativeOptions, job, &metadata,
                entryPointer, (nuint)entries.Length);
            ThrowCancellationOrFailure(status, cancellationToken, "point-in-time daylight");
        }
        ValidateMetadata(metadata, entries.Select(value => value.StructureSize), 48);
        return new()
        {
            Entries = Array.ConvertAll(entries, value => new PointIlluminanceEntry(
                value.SensorId, value.DirectLux, value.DiffuseLux, value.TotalLux, value.DirectTransmission)),
            MatrixReused = (metadata.Flags & 1) != 0,
            MatrixHash = Hash(metadata.MatrixHash0, metadata.MatrixHash1, metadata.MatrixHash2, metadata.MatrixHash3),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    public unsafe DaylightFactorResult AnalyzeDaylightFactor(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials,
        double exteriorHorizontalIlluminanceLux,
        DaylightMatrixOptions options)
    {
        DaylightSensor[] source = ValidateInputs(sensors, materials);
        NativeDaylightSensor[] nativeSensors = ConvertSensors(source);
        NativeOpticalMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeDaylightMatrixOptions nativeOptions = ConvertOptions(options);
        NativeDaylightFactorEntry[] entries = new NativeDaylightFactorEntry[source.Length];
        NativeDaylightMetadata metadata = default;
        fixed (NativeDaylightSensor* sensorPointer = nativeSensors)
        fixed (NativeOpticalMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (NativeDaylightFactorEntry* entryPointer = entries)
        {
            ThrowIfFailed(NativeMethods.SceneDaylightFactor(
                handle, sensorPointer, (nuint)source.Length,
                materialPointer, (nuint)nativeMaterials.Length,
                assignmentPointer, (nuint)nativeAssignments.Length,
                exteriorHorizontalIlluminanceLux, &nativeOptions, &metadata,
                entryPointer, (nuint)entries.Length));
        }
        ValidateMetadata(metadata, entries.Select(value => value.StructureSize), 32);
        return new()
        {
            Entries = Array.ConvertAll(entries, value => new DaylightFactorEntry(
                value.SensorId, value.InteriorIlluminanceLux, value.DaylightFactorPercent)),
            ExteriorHorizontalIlluminanceLux = exteriorHorizontalIlluminanceLux,
            MatrixReused = (metadata.Flags & 1) != 0,
            MatrixHash = Hash(metadata.MatrixHash0, metadata.MatrixHash1, metadata.MatrixHash2, metadata.MatrixHash3),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    public AnnualDaylightResult AnalyzeAnnualDaylight(
        IReadOnlyList<DaylightSensor> sensors,
        EpwWeatherFile weather,
        OpticalMaterialLibrary materials,
        AnnualDaylightOptions options,
        CancellationToken cancellationToken = default)
    {
        using SafeJobHandle job = SafeJobHandle.Create();
        using CancellationTokenRegistration registration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(), job);
        return AnalyzeAnnualDaylightCore(sensors, weather, materials, options, cancellationToken, job);
    }

    public Task<AnnualDaylightResult> AnalyzeAnnualDaylightAsync(
        IReadOnlyList<DaylightSensor> sensors,
        EpwWeatherFile weather,
        OpticalMaterialLibrary materials,
        AnnualDaylightOptions options,
        CancellationToken cancellationToken = default) =>
        BeginAnnualDaylight(sensors, weather, materials, options, cancellationToken).Completion;

    public DaylightAnalysisJob<AnnualDaylightResult> BeginAnnualDaylight(
        IReadOnlyList<DaylightSensor> sensors,
        EpwWeatherFile weather,
        OpticalMaterialLibrary materials,
        AnnualDaylightOptions options,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(weather);
        DaylightSensor[] snapshot = ValidateInputs(sensors, materials);
        return new((handle, token) => AnalyzeAnnualDaylightCore(snapshot, weather, materials, options, token, handle), cancellationToken);
    }

    internal unsafe AnnualDaylightResult AnalyzeAnnualDaylightCore(
        IReadOnlyList<DaylightSensor> sensors,
        EpwWeatherFile weather,
        OpticalMaterialLibrary materials,
        AnnualDaylightOptions options,
        CancellationToken cancellationToken,
        SafeJobHandle job)
    {
        ArgumentNullException.ThrowIfNull(weather);
        DaylightSensor[] source = ValidateInputs(sensors, materials);
        cancellationToken.ThrowIfCancellationRequested();
        int weatherCount = checked((int)weather.WeatherCount);
        NativeDaylightSensor[] nativeSensors = ConvertSensors(source);
        NativeOpticalMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        NativeAnnualDaylightOptions nativeOptions = ConvertAnnualOptions(options);
        NativeAnnualDaylightSummary[] summaries = new NativeAnnualDaylightSummary[source.Length];
        NativeAnnualDaylightTimelineEntry[] timeline = new NativeAnnualDaylightTimelineEntry[checked(source.Length * weatherCount)];
        NativeAnnualDaylightMetadata metadata = default;
        byte[] epw = weather.Utf8;
        fixed (NativeDaylightSensor* sensorPointer = nativeSensors)
        fixed (NativeOpticalMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        fixed (byte* epwPointer = epw)
        fixed (NativeAnnualDaylightSummary* summaryPointer = summaries)
        fixed (NativeAnnualDaylightTimelineEntry* timelinePointer = timeline)
        {
            NativeStatus status = NativeMethods.SceneAnnualDaylight(
                handle, sensorPointer, (nuint)source.Length,
                materialPointer, (nuint)nativeMaterials.Length,
                assignmentPointer, (nuint)nativeAssignments.Length,
                epwPointer, (nuint)epw.Length, &nativeOptions, job, &metadata,
                summaryPointer, (nuint)summaries.Length,
                timelinePointer, (nuint)timeline.Length);
            ThrowCancellationOrFailure(status, cancellationToken, "annual daylight");
        }
        if (metadata.StructureSize != 160 || Marshal.SizeOf<NativeAnnualDaylightMetadata>() != 160
            || summaries.Any(value => value.StructureSize != 104)
            || timeline.Any(value => value.StructureSize != 40))
        {
            throw new InvalidOperationException("Native annual daylight ABI layout mismatch.");
        }
        return new()
        {
            Summaries = Array.ConvertAll(summaries, value => new AnnualDaylightSensorSummary(
                value.SensorId, value.OccupiedHours, value.SdaQualifiedHours, value.SdaOccupiedFraction,
                value.AseExceedanceHours, (value.Flags & 1) != 0, value.UdiBelowFraction,
                value.UdiSupplementalFraction, value.UdiUsefulFraction, value.UdiExceededFraction,
                value.MeanOccupiedLux, value.MinimumOccupiedLux, value.MaximumOccupiedLux)),
            Timeline = Array.ConvertAll(timeline, value => new AnnualDaylightTimelineEntry(
                DateTimeOffset.FromUnixTimeSeconds(value.UnixSecondsUtc), (value.Flags & 1) != 0,
                value.DirectLux, value.DiffuseLux, value.TotalLux)),
            Project = new(metadata.SdaAreaPercent, metadata.AseAreaPercent, metadata.UdiBelowPercent,
                metadata.UdiSupplementalPercent, metadata.UdiUsefulPercent, metadata.UdiExceededPercent,
                metadata.TotalSensorAreaSquareMeters / (unitScaleToMeters * unitScaleToMeters)),
            WeatherCount = metadata.WeatherCount,
            MatrixReused = (metadata.Flags & 1) != 0,
            MatrixHash = Hash(metadata.MatrixHash0, metadata.MatrixHash1, metadata.MatrixHash2, metadata.MatrixHash3),
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    public unsafe RadianceExportBundle ExportRadiance(
        IReadOnlyList<DaylightSensor> sensors,
        OpticalMaterialLibrary materials)
    {
        DaylightSensor[] source = ValidateInputs(sensors, materials);
        NativeDaylightSensor[] nativeSensors = ConvertSensors(source);
        NativeOpticalMaterial[] nativeMaterials = materials.NativeMaterials();
        NativeMaterialAssignment[] nativeAssignments = materials.NativeAssignments();
        string[] sections = new string[4];
        string? contentHash = null;
        fixed (NativeDaylightSensor* sensorPointer = nativeSensors)
        fixed (NativeOpticalMaterial* materialPointer = nativeMaterials)
        fixed (NativeMaterialAssignment* assignmentPointer = nativeAssignments)
        {
            for (uint section = 0; section < 4; section++)
            {
                NativeRadianceExportMetadata metadata = default;
                ThrowIfFailed(NativeMethods.SceneRadianceExportSection(
                    handle, sensorPointer, (nuint)source.Length,
                    materialPointer, (nuint)nativeMaterials.Length,
                    assignmentPointer, (nuint)nativeAssignments.Length,
                    section, null, 0, &metadata));
                if (metadata.StructureSize != 48 || metadata.RequiredBytes == 0 || metadata.RequiredBytes > int.MaxValue)
                {
                    throw new InvalidOperationException("Native Radiance export metadata is invalid.");
                }
                byte[] bytes = new byte[(int)metadata.RequiredBytes];
                fixed (byte* outputPointer = bytes)
                {
                    ThrowIfFailed(NativeMethods.SceneRadianceExportSection(
                        handle, sensorPointer, (nuint)source.Length,
                        materialPointer, (nuint)nativeMaterials.Length,
                        assignmentPointer, (nuint)nativeAssignments.Length,
                        section, outputPointer, (nuint)bytes.Length, &metadata));
                }
                sections[section] = Encoding.UTF8.GetString(bytes, 0, bytes.Length - 1);
                contentHash ??= Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3);
            }
        }
        return new()
        {
            MaterialsRad = sections[0],
            GeometryRad = sections[1],
            SensorsPts = sections[2],
            ManifestJson = sections[3],
            ContentHash = contentHash!,
        };
    }

    private DaylightSensor[] ValidateInputs(IReadOnlyList<DaylightSensor> sensors, OpticalMaterialLibrary materials)
    {
        ArgumentNullException.ThrowIfNull(sensors);
        ArgumentNullException.ThrowIfNull(materials);
        if (sensors.Count == 0) throw new ArgumentException("At least one daylight sensor is required.", nameof(sensors));
        ThrowIfDisposed();
        return sensors.ToArray();
    }

    private NativeDaylightSensor[] ConvertSensors(IReadOnlyList<DaylightSensor> sensors) => sensors.Select(value => new NativeDaylightSensor
    {
        SensorId = value.SensorId,
        PositionX = value.PositionX * unitScaleToMeters,
        PositionY = value.PositionY * unitScaleToMeters,
        PositionZ = value.PositionZ * unitScaleToMeters,
        NormalX = value.NormalX,
        NormalY = value.NormalY,
        NormalZ = value.NormalZ,
        AreaSquareMeters = value.Area * unitScaleToMeters * unitScaleToMeters,
    }).ToArray();

    private NativeDaylightMatrixOptions ConvertOptions(DaylightMatrixOptions value) => new()
    {
        StructureSize = DaylightMatrixOptionsSize,
        SkyModel = (uint)value.SkyModel,
        SkyPatchCount = value.SkyPatchCount,
        MaximumMaterialLayers = value.MaximumMaterialLayers,
        SensorOffsetMeters = value.SensorOffset * unitScaleToMeters,
        MaximumDistanceMeters = value.MaximumDistance * unitScaleToMeters,
        CategoryMask = value.CategoryMask,
        MinimumTransmission = value.MinimumTransmission,
    };

    private NativeAnnualDaylightOptions ConvertAnnualOptions(AnnualDaylightOptions value) => new()
    {
        StructureSize = AnnualDaylightOptionsSize,
        SkyModel = (uint)value.Matrix.SkyModel,
        SkyPatchCount = value.Matrix.SkyPatchCount,
        MaximumMaterialLayers = value.Matrix.MaximumMaterialLayers,
        SensorOffsetMeters = value.Matrix.SensorOffset * unitScaleToMeters,
        MaximumDistanceMeters = value.Matrix.MaximumDistance * unitScaleToMeters,
        CategoryMask = value.Matrix.CategoryMask,
        MinimumTransmission = value.Matrix.MinimumTransmission,
        DeltaTSeconds = value.Solar.DeltaTSeconds,
        PressureMillibars = value.Solar.PressureMillibars,
        TemperatureCelsius = value.Solar.TemperatureCelsius,
        NorthRotationDegrees = value.Solar.NorthRotationDegrees,
        MinimumAltitudeDegrees = value.Solar.MinimumAltitudeDegrees,
        DirectLuminousEfficacyLmPerW = value.DirectLuminousEfficacyLmPerW,
        DiffuseLuminousEfficacyLmPerW = value.DiffuseLuminousEfficacyLmPerW,
        OccupiedStartHour = value.OccupiedStartHour,
        OccupiedEndHour = value.OccupiedEndHour,
        SdaThresholdLux = value.SdaThresholdLux,
        SdaRequiredFraction = value.SdaRequiredFraction,
        AseThresholdLux = value.AseThresholdLux,
        AseMaximumHours = value.AseMaximumHours,
        UdiLowerLux = value.UdiLowerLux,
        UdiPreferredLux = value.UdiPreferredLux,
        UdiUpperLux = value.UdiUpperLux,
    };

    private static void ValidateMetadata(NativeDaylightMetadata metadata, IEnumerable<uint> rows, uint expectedRowSize)
    {
        if (metadata.StructureSize != 88 || Marshal.SizeOf<NativeDaylightMetadata>() != 88
            || rows.Any(value => value != expectedRowSize))
        {
            throw new InvalidOperationException("Native daylight ABI layout mismatch.");
        }
    }

    private static void ThrowCancellationOrFailure(NativeStatus status, CancellationToken token, string operation)
    {
        if (status == NativeStatus.Cancelled) throw new OperationCanceledException($"XVARNA {operation} was cancelled.", token);
        ThrowIfFailed(status);
    }

    private static string Hash(ulong a, ulong b, ulong c, ulong d) =>
        SolarConversions.FormatHash(a, b, c, d);
}

/// <summary>Observable in-flight native daylight operation with cooperative cancellation.</summary>
public sealed class DaylightAnalysisJob<T> : IDisposable
{
    private readonly object syncRoot = new();
    private readonly SafeJobHandle handle;
    private readonly CancellationTokenRegistration registration;
    private AnnualIrradianceProgress lastProgress;

    internal DaylightAnalysisJob(Func<SafeJobHandle, CancellationToken, T> operation, CancellationToken token)
    {
        token.ThrowIfCancellationRequested();
        handle = SafeJobHandle.Create();
        registration = token.Register(static state => ((SafeJobHandle)state!).RequestCancellation(), handle);
        Completion = Task.Run(() => operation(handle, token), CancellationToken.None);
        _ = Completion.ContinueWith(static (_, state) => ((DaylightAnalysisJob<T>)state!).Complete(), this,
            CancellationToken.None, TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default);
    }

    public Task<T> Completion { get; }

    public AnnualIrradianceProgress Progress
    {
        get
        {
            lock (syncRoot)
            {
                if (!handle.IsClosed && !handle.IsInvalid)
                {
                    SkyViewProgress value = handle.ReadProgress();
                    lastProgress = new(value.TotalRays, value.CompletedRays, value.CancellationRequested);
                }
                return lastProgress;
            }
        }
    }

    public void Cancel()
    {
        lock (syncRoot)
        {
            if (!handle.IsClosed && !handle.IsInvalid) handle.RequestCancellation();
            lastProgress = lastProgress with { CancellationRequested = true };
        }
    }

    public void Dispose() => Cancel();

    private void Complete()
    {
        lock (syncRoot)
        {
            if (!handle.IsClosed && !handle.IsInvalid)
            {
                SkyViewProgress value = handle.ReadProgress();
                lastProgress = new(value.TotalRays, value.CompletedRays, value.CancellationRequested);
                if (Completion.IsCompletedSuccessfully)
                    lastProgress = lastProgress with { CompletedWorkUnits = lastProgress.TotalWorkUnits };
                registration.Dispose();
                handle.Dispose();
            }
        }
    }
}

/// <summary>Scientific comparison utilities for aligned Fast Path and Radiance arrays.</summary>
public static class DaylightValidation
{
    public static unsafe DaylightValidationReport Compare(
        IReadOnlyList<double> fastLux,
        IReadOnlyList<double> radianceLux,
        double absoluteToleranceLux = 50.0,
        double relativeTolerance = 0.10)
    {
        ArgumentNullException.ThrowIfNull(fastLux);
        ArgumentNullException.ThrowIfNull(radianceLux);
        NativeLibraryBootstrap.Initialize();
        double[] fast = fastLux.ToArray();
        double[] reference = radianceLux.ToArray();
        NativeDaylightValidationReport report = default;
        fixed (double* fastPointer = fast)
        fixed (double* referencePointer = reference)
        {
            NativeStatus status = NativeMethods.DaylightCompare(
                fastPointer, referencePointer, (nuint)fast.Length,
                absoluteToleranceLux, relativeTolerance, &report);
            if (status != NativeStatus.Success) throw new XvarnaNativeException(status);
        }
        if (report.StructureSize != 104 || Marshal.SizeOf<NativeDaylightValidationReport>() != 104)
            throw new InvalidOperationException("Native daylight-validation ABI layout mismatch.");
        return new(report.Count, report.MeanBiasLux, report.MeanAbsoluteErrorLux,
            report.RootMeanSquareErrorLux, report.MeanAbsolutePercentageError,
            report.MaximumAbsoluteErrorLux, report.RSquared, report.AcceptedFraction,
            (report.Flags & 1) != 0,
            SolarConversions.FormatHash(report.ContentHash0, report.ContentHash1, report.ContentHash2, report.ContentHash3));
    }
}

#pragma warning restore CS1591
