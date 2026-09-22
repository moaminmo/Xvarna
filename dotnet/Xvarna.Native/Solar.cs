using System.Buffers.Binary;
using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>WGS84 observer location used for solar positioning.</summary>
public readonly record struct SolarLocation(
    double LatitudeDegrees,
    double LongitudeDegrees,
    double ElevationMeters = 0.0);

/// <summary>Explicit NREL SPA atmosphere, time, north, and horizon options.</summary>
public readonly record struct SolarPositionOptions(
    double DeltaTSeconds = 69.0,
    double PressureMillibars = 1013.25,
    double TemperatureCelsius = 15.0,
    double NorthRotationDegrees = 0.0,
    double MinimumAltitudeDegrees = 0.0)
{
    /// <summary>Standard atmosphere, 69-second Delta T, model north aligned to true north, and zero-degree horizon.</summary>
    public static SolarPositionOptions Default { get; } = new(69.0, 1013.25, 15.0, 0.0, 0.0);
}

/// <summary>One weighted UTC interval centred on a timestamp.</summary>
public readonly record struct SolarTimeSample(
    DateTimeOffset Timestamp,
    double DurationHours,
    double Weight = 1.0);

/// <summary>One source-aligned apparent sun position and model-space direction.</summary>
public readonly record struct SunSample(
    DateTimeOffset TimestampUtc,
    double DirectionX,
    double DirectionY,
    double DirectionZ,
    double AltitudeDegrees,
    double AzimuthDegrees,
    double DurationHours,
    double Weight,
    bool IsActive);

/// <summary>Immutable ordered solar-position result with deterministic identity.</summary>
public sealed class XvarnaSunSet
{
    internal XvarnaSunSet(
        NativeSunSetMetadata nativeMetadata,
        NativeSunSample[] nativeSamples,
        IReadOnlyList<SunSample> samples,
        string contentHash)
    {
        NativeMetadata = nativeMetadata;
        NativeSamples = nativeSamples;
        Samples = samples;
        ContentHash = contentHash;
    }

    /// <summary>Ordered source-aligned samples.</summary>
    public IReadOnlyList<SunSample> Samples { get; }
    /// <summary>Number active after altitude and schedule-weight filtering.</summary>
    public ulong ActiveSampleCount => NativeMetadata.ActiveSampleCount;
    /// <summary>True when atmospheric refraction correction was enabled.</summary>
    public bool IncludesAtmosphericRefraction => (NativeMetadata.Flags & 1) != 0;
    /// <summary>BLAKE3 identity of location, options, schedule, and calculated directions.</summary>
    public string ContentHash { get; }

    internal NativeSunSetMetadata NativeMetadata { get; }
    internal NativeSunSample[] NativeSamples { get; }
}

/// <summary>High-accuracy Reda–Andreas NREL SPA solar positioning.</summary>
public static class SolarEngine
{
    private const uint ExpectedSolarOptionsSize = 72;
    private const uint ExpectedSunMetadataSize = 56;

    /// <summary>Calculates an ordered sun set. Timestamps are converted to UTC before native execution.</summary>
    public static unsafe XvarnaSunSet Calculate(
        SolarLocation location,
        IReadOnlyList<SolarTimeSample> samples,
        SolarPositionOptions options)
    {
        ArgumentNullException.ThrowIfNull(samples);
        if (samples.Count == 0)
        {
            throw new ArgumentException("At least one time sample is required.", nameof(samples));
        }
        NativeLibraryBootstrap.Initialize();
        NativeSolarOptions nativeOptions = new()
        {
            StructureSize = ExpectedSolarOptionsSize,
            Reserved = 0,
            LatitudeDegrees = location.LatitudeDegrees,
            LongitudeDegrees = location.LongitudeDegrees,
            ElevationMeters = location.ElevationMeters,
            DeltaTSeconds = options.DeltaTSeconds,
            PressureMillibars = options.PressureMillibars,
            TemperatureCelsius = options.TemperatureCelsius,
            NorthRotationDegrees = options.NorthRotationDegrees,
            MinimumAltitudeDegrees = options.MinimumAltitudeDegrees,
        };
        NativeTimeSample[] nativeTimes = new NativeTimeSample[samples.Count];
        for (int index = 0; index < samples.Count; index++)
        {
            SolarTimeSample sample = samples[index];
            nativeTimes[index] = new()
            {
                UnixSecondsUtc = sample.Timestamp.ToUniversalTime().ToUnixTimeSeconds(),
                DurationHours = sample.DurationHours,
                Weight = sample.Weight,
            };
        }
        NativeSunSample[] nativeSamples = new NativeSunSample[samples.Count];
        NativeSunSetMetadata metadata = default;
        fixed (NativeTimeSample* timesPointer = nativeTimes)
        fixed (NativeSunSample* samplesPointer = nativeSamples)
        {
            NativeStatus status = NativeMethods.SunPositions(
                &nativeOptions,
                timesPointer,
                (nuint)nativeTimes.Length,
                &metadata,
                samplesPointer,
                (nuint)nativeSamples.Length);
            ThrowIfFailed(status);
        }
        if (metadata.StructureSize != ExpectedSunMetadataSize
            || metadata.StructureSize != Marshal.SizeOf<NativeSunSetMetadata>())
        {
            throw new InvalidOperationException($"Native sun-set layout mismatch: {metadata.StructureSize}.");
        }
        SunSample[] managed = Array.ConvertAll(nativeSamples, sample => new SunSample(
            DateTimeOffset.FromUnixTimeSeconds(sample.UnixSecondsUtc),
            sample.DirectionX,
            sample.DirectionY,
            sample.DirectionZ,
            sample.AltitudeDegrees,
            sample.AzimuthDegrees,
            sample.DurationHours,
            sample.Weight,
            (sample.Flags & 1) != 0));
        return new(metadata, nativeSamples, managed, SolarConversions.FormatHash(
            metadata.ContentHash0,
            metadata.ContentHash1,
            metadata.ContentHash2,
            metadata.ContentHash3));
    }

    /// <summary>Calculates an ordered sun set using <see cref="SolarPositionOptions.Default"/>.</summary>
    public static XvarnaSunSet Calculate(
        SolarLocation location,
        IReadOnlyList<SolarTimeSample> samples) =>
        Calculate(location, samples, SolarPositionOptions.Default);

    private static void ThrowIfFailed(NativeStatus status)
    {
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
    }
}

/// <summary>One oriented model-space point for direct-sun analysis.</summary>
public readonly record struct SolarSensor(
    ulong SensorId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double NormalX,
    double NormalY,
    double NormalZ);

/// <summary>Direct-sun surface and ray policy in source model units.</summary>
public readonly record struct DirectSunOptions(
    double SensorOffset,
    double MaximumDistance,
    double MinimumIncidenceCosine = 0.0,
    ulong CategoryMask = ulong.MaxValue)
{
    /// <summary>Default model-unit policy. Callers should normally replace SensorOffset with document tolerance.</summary>
    public static DirectSunOptions Default { get; } = new(1.0e-4, double.PositiveInfinity);
}

/// <summary>Classification of one sensor/time pair.</summary>
public enum SunState : uint
{
    /// <summary>Below the altitude threshold or zero-weight.</summary>
    Inactive = 0,
    /// <summary>Behind the oriented sensor.</summary>
    BackFacing = 1,
    /// <summary>Eligible and unobstructed.</summary>
    Visible = 2,
    /// <summary>Eligible and obstructed.</summary>
    Blocked = 3,
}

/// <summary>First-hit attribution for one sensor/time pair.</summary>
public readonly record struct SunTimelineEntry(
    SunState State,
    ulong ObjectId,
    ulong InstanceId,
    ulong MeshId,
    uint TriangleId,
    double Distance);

/// <summary>Weighted direct-sun and dominant-obstruction summary for one sensor.</summary>
public readonly record struct SensorSunSummary(
    ulong SensorId,
    double DirectSunHours,
    double ShadowHours,
    double EligibleHours,
    double SolarAccessRatio,
    ulong VisibleCount,
    ulong BlockedCount,
    ulong BackFacingCount,
    ulong DominantOccluderObjectId,
    double DominantOccluderHours);

/// <summary>Complete sensor-major direct-sun result with full temporal attribution.</summary>
public sealed record DirectSunResult
{
    /// <summary>One aggregate per source sensor.</summary>
    public required IReadOnlyList<SensorSunSummary> Summaries { get; init; }
    /// <summary>Sensor-major timeline: `sensorIndex * SunCount + sunIndex`.</summary>
    public required IReadOnlyList<SunTimelineEntry> Timeline { get; init; }
    /// <summary>Number of sun samples in each sensor row.</summary>
    public required ulong SunCount { get; init; }
    /// <summary>Native analysis duration in microseconds.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 identity of every analysis input and timeline classification.</summary>
    public required string ContentHash { get; init; }
}

public sealed partial class XvarnaScene
{
    private const uint ExpectedDirectSunOptionsSize = 40;
    private const uint ExpectedDirectSunMetadataSize = 72;
    private const uint ExpectedDirectSunSummarySize = 88;
    private const uint ExpectedTimelineEntrySize = 48;

    /// <summary>Computes Direct Sun Hours, Shadow Hours, Solar Access, and full first-hit attribution.</summary>
    public unsafe DirectSunResult AnalyzeDirectSun(
        IReadOnlyList<SolarSensor> sensors,
        XvarnaSunSet sunSet,
        DirectSunOptions options)
    {
        ArgumentNullException.ThrowIfNull(sensors);
        ArgumentNullException.ThrowIfNull(sunSet);
        if (sensors.Count == 0)
        {
            throw new ArgumentException("At least one sensor is required.", nameof(sensors));
        }
        ThrowIfDisposed();
        int timelineCount = checked(sensors.Count * sunSet.NativeSamples.Length);
        NativeSolarSensor[] nativeSensors = new NativeSolarSensor[sensors.Count];
        for (int index = 0; index < sensors.Count; index++)
        {
            SolarSensor sensor = sensors[index];
            nativeSensors[index] = new()
            {
                SensorId = sensor.SensorId,
                PositionX = sensor.PositionX * unitScaleToMeters,
                PositionY = sensor.PositionY * unitScaleToMeters,
                PositionZ = sensor.PositionZ * unitScaleToMeters,
                NormalX = sensor.NormalX,
                NormalY = sensor.NormalY,
                NormalZ = sensor.NormalZ,
            };
        }
        NativeDirectSunOptions nativeOptions = new()
        {
            StructureSize = ExpectedDirectSunOptionsSize,
            Reserved = 0,
            SensorOffsetMeters = options.SensorOffset * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            MinimumIncidenceCosine = options.MinimumIncidenceCosine,
            CategoryMask = options.CategoryMask,
        };
        NativeDirectSunSummary[] nativeSummaries = new NativeDirectSunSummary[sensors.Count];
        NativeSunTimelineEntry[] nativeTimeline = new NativeSunTimelineEntry[timelineCount];
        NativeDirectSunMetadata metadata = default;
        NativeSunSetMetadata sunMetadata = sunSet.NativeMetadata;
        fixed (NativeSolarSensor* sensorsPointer = nativeSensors)
        fixed (NativeSunSample* sunPointer = sunSet.NativeSamples)
        fixed (NativeDirectSunSummary* summariesPointer = nativeSummaries)
        fixed (NativeSunTimelineEntry* timelinePointer = nativeTimeline)
        {
            NativeStatus status = NativeMethods.SceneDirectSun(
                handle,
                sensorsPointer,
                (nuint)nativeSensors.Length,
                &sunMetadata,
                sunPointer,
                (nuint)sunSet.NativeSamples.Length,
                &nativeOptions,
                &metadata,
                summariesPointer,
                (nuint)nativeSummaries.Length,
                timelinePointer,
                (nuint)nativeTimeline.Length);
            ThrowIfFailed(status);
        }
        ValidateSolarLayouts(metadata, nativeSummaries, nativeTimeline);
        SensorSunSummary[] summaries = Array.ConvertAll(nativeSummaries, value => new SensorSunSummary(
            value.SensorId,
            value.DirectSunHours,
            value.ShadowHours,
            value.EligibleHours,
            value.SolarAccessRatio,
            value.VisibleCount,
            value.BlockedCount,
            value.BackFacingCount,
            value.DominantOccluderObjectId,
            value.DominantOccluderHours));
        SunTimelineEntry[] timeline = Array.ConvertAll(nativeTimeline, value => new SunTimelineEntry(
            (SunState)value.State,
            value.ObjectId,
            value.InstanceId,
            value.MeshId,
            value.TriangleId,
            value.DistanceMeters / unitScaleToMeters));
        return new()
        {
            Summaries = summaries,
            Timeline = timeline,
            SunCount = metadata.SunCount,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(
                metadata.ContentHash0,
                metadata.ContentHash1,
                metadata.ContentHash2,
                metadata.ContentHash3),
        };
    }

    private static void ValidateSolarLayouts(
        NativeDirectSunMetadata metadata,
        IReadOnlyList<NativeDirectSunSummary> summaries,
        IReadOnlyList<NativeSunTimelineEntry> timeline)
    {
        if (metadata.StructureSize != ExpectedDirectSunMetadataSize
            || metadata.StructureSize != Marshal.SizeOf<NativeDirectSunMetadata>()
            || summaries.Any(value => value.StructureSize != ExpectedDirectSunSummarySize)
            || timeline.Any(value => value.StructureSize != ExpectedTimelineEntrySize))
        {
            throw new InvalidOperationException("Native direct-sun ABI layout mismatch.");
        }
    }
}

internal static class SolarConversions
{
    internal static string FormatHash(ulong first, ulong second, ulong third, ulong fourth)
    {
        Span<byte> hash = stackalloc byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(hash[0..8], first);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[8..16], second);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[16..24], third);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[24..32], fourth);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }
}
