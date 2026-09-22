using System.Runtime.InteropServices;
using System.Text;

namespace Xvarna.Native;

/// <summary>Validated immutable EnergyPlus Weather data retained as UTF-8 for native analysis.</summary>
public sealed class EpwWeatherFile
{
    private const uint ExpectedMetadataSize = 104;
    private readonly byte[] utf8;

    private EpwWeatherFile(byte[] utf8, NativeEpwMetadata metadata, string? sourcePath)
    {
        this.utf8 = utf8;
        WeatherCount = metadata.WeatherCount;
        RecordsPerHour = metadata.RecordsPerHour;
        MissingGlobalHorizontalCount = metadata.MissingGlobalHorizontalCount;
        MissingDirectNormalCount = metadata.MissingDirectNormalCount;
        MissingDiffuseHorizontalCount = metadata.MissingDiffuseHorizontalCount;
        Location = new(metadata.LatitudeDegrees, metadata.LongitudeDegrees, metadata.ElevationMeters);
        TimeZoneHours = metadata.TimeZoneHours;
        ContentHash = SolarConversions.FormatHash(
            metadata.ContentHash0,
            metadata.ContentHash1,
            metadata.ContentHash2,
            metadata.ContentHash3);
        SourcePath = sourcePath;
    }

    /// <summary>Number of parsed radiation intervals.</summary>
    public ulong WeatherCount { get; }
    /// <summary>Declared records per hour.</summary>
    public uint RecordsPerHour { get; }
    /// <summary>Station coordinates from the LOCATION header.</summary>
    public SolarLocation Location { get; }
    /// <summary>Local standard-time offset from UTC, in hours.</summary>
    public double TimeZoneHours { get; }
    /// <summary>GHI values sanitized to zero.</summary>
    public ulong MissingGlobalHorizontalCount { get; }
    /// <summary>DNI values sanitized to zero.</summary>
    public ulong MissingDirectNormalCount { get; }
    /// <summary>DHI values sanitized to zero.</summary>
    public ulong MissingDiffuseHorizontalCount { get; }
    /// <summary>Semantic BLAKE3 identity of parsed weather content.</summary>
    public string ContentHash { get; }
    /// <summary>Original path when created with <see cref="Load"/>.</summary>
    public string? SourcePath { get; }

    internal byte[] Utf8 => utf8;

    /// <summary>Loads, parses, and validates an EPW file.</summary>
    public static EpwWeatherFile Load(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        byte[] bytes = File.ReadAllBytes(path);
        return Inspect(bytes, Path.GetFullPath(path));
    }

    /// <summary>Parses and validates EPW text already held in memory.</summary>
    public static EpwWeatherFile Parse(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        return Inspect(Encoding.UTF8.GetBytes(text), null);
    }

    private static unsafe EpwWeatherFile Inspect(byte[] bytes, string? sourcePath)
    {
        NativeLibraryBootstrap.Initialize();
        NativeEpwMetadata metadata = default;
        fixed (byte* bytesPointer = bytes)
        {
            NativeStatus status = NativeMethods.EpwInspect(bytesPointer, (nuint)bytes.Length, &metadata);
            if (status != NativeStatus.Success)
            {
                throw new XvarnaNativeException(status);
            }
        }
        if (metadata.StructureSize != ExpectedMetadataSize
            || metadata.StructureSize != Marshal.SizeOf<NativeEpwMetadata>())
        {
            throw new InvalidOperationException("Native EPW ABI layout mismatch.");
        }
        return new(bytes, metadata, sourcePath);
    }
}

/// <summary>Perez annual irradiance, NREL SPA, obstruction, and ground-reflection policy.</summary>
public readonly record struct AnnualIrradianceOptions(
    double SensorOffset,
    double MaximumDistance,
    ulong CategoryMask,
    double GroundAlbedo,
    bool UseWeatherAlbedo,
    SolarPositionOptions Solar)
{
    /// <summary>Interactive-quality defaults with EPW albedo preference and a 0.2 fallback.</summary>
    public static AnnualIrradianceOptions Default { get; } = new(
        1.0e-4,
        double.PositiveInfinity,
        ulong.MaxValue,
        0.2,
        true,
        SolarPositionOptions.Default);
}

/// <summary>Annual energy totals, peak irradiance, visibility, and dominant obstruction.</summary>
public readonly record struct SensorIrradianceSummary(
    ulong SensorId,
    double DirectWhM2,
    double DiffuseSkyWhM2,
    double GroundReflectedWhM2,
    double GlobalWhM2,
    double PeakGlobalWM2,
    DateTimeOffset PeakTimestampUtc,
    double DomeVisibilityRatio,
    double HorizonVisibilityRatio,
    ulong VisibleCount,
    ulong BlockedCount,
    ulong BackFacingCount,
    ulong DominantSolarOccluderObjectId,
    double DominantSolarOccluderLossWhM2,
    ulong DominantSkyOccluderObjectId,
    double DominantSkyOccluderProjectedFraction);

/// <summary>One EPW interval of received component energy and first-hit attribution.</summary>
public readonly record struct IrradianceTimelineEntry(
    SunState State,
    DateTimeOffset TimestampUtc,
    double DirectWhM2,
    double DiffuseDomeWhM2,
    double DiffuseCircumsolarWhM2,
    double DiffuseHorizonWhM2,
    double DiffuseSkyWhM2,
    double GroundReflectedWhM2,
    double GlobalWhM2,
    double GlobalWM2,
    double AttributedLossWhM2,
    ulong ObjectId,
    ulong InstanceId,
    ulong MeshId,
    uint TriangleId,
    double Distance);

/// <summary>Complete sensor-major Perez annual irradiance result.</summary>
public sealed record AnnualIrradianceResult
{
    /// <summary>Source model-space sensors.</summary>
    public required IReadOnlyList<SolarSensor> Sensors { get; init; }
    /// <summary>Validated source weather.</summary>
    public required EpwWeatherFile Weather { get; init; }
    /// <summary>One annual aggregate per sensor.</summary>
    public required IReadOnlyList<SensorIrradianceSummary> Summaries { get; init; }
    /// <summary>Sensor-major interval component matrix.</summary>
    public required IReadOnlyList<IrradianceTimelineEntry> Timeline { get; init; }
    /// <summary>Analysis policy.</summary>
    public required AnnualIrradianceOptions Options { get; init; }
    /// <summary>Native analysis time in microseconds.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Thread-safe progress snapshot for static sky rays plus EPW intervals.</summary>
public readonly record struct AnnualIrradianceProgress(
    ulong TotalWorkUnits,
    ulong CompletedWorkUnits,
    bool CancellationRequested)
{
    /// <summary>Completed fraction from zero through one.</summary>
    public double Fraction => TotalWorkUnits == 0
        ? 0.0
        : Math.Clamp((double)CompletedWorkUnits / TotalWorkUnits, 0.0, 1.0);
}

public sealed partial class XvarnaScene
{
    private const uint ExpectedAnnualOptionsSize = 80;
    private const uint ExpectedAnnualSummarySize = 136;
    private const uint ExpectedAnnualEntrySize = 128;
    private const uint ExpectedAnnualMetadataSize = 96;

    /// <summary>Computes annual irradiance synchronously.</summary>
    public AnnualIrradianceResult AnalyzeAnnualIrradiance(
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options) =>
        AnalyzeAnnualIrradiance(sensors, weather, options, CancellationToken.None);

    /// <summary>Computes annual irradiance synchronously with cooperative cancellation.</summary>
    public unsafe AnnualIrradianceResult AnalyzeAnnualIrradiance(
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options,
        CancellationToken cancellationToken)
    {
        using SafeJobHandle job = SafeJobHandle.Create();
        using CancellationTokenRegistration registration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(),
            job);
        return AnalyzeAnnualIrradianceCore(sensors, weather, options, cancellationToken, job);
    }

    internal unsafe AnnualIrradianceResult AnalyzeAnnualIrradianceCore(
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options,
        CancellationToken cancellationToken,
        SafeJobHandle job)
    {
        ArgumentNullException.ThrowIfNull(sensors);
        ArgumentNullException.ThrowIfNull(weather);
        if (sensors.Count == 0)
        {
            throw new ArgumentException("At least one irradiance sensor is required.", nameof(sensors));
        }
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();
        int weatherCount = checked((int)weather.WeatherCount);
        int timelineCount = checked(sensors.Count * weatherCount);
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
        NativeAnnualIrradianceOptions nativeOptions = new()
        {
            StructureSize = ExpectedAnnualOptionsSize,
            Flags = options.UseWeatherAlbedo ? 1u : 0u,
            SensorOffsetMeters = options.SensorOffset * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
            GroundAlbedo = options.GroundAlbedo,
            DeltaTSeconds = options.Solar.DeltaTSeconds,
            PressureMillibars = options.Solar.PressureMillibars,
            TemperatureCelsius = options.Solar.TemperatureCelsius,
            NorthRotationDegrees = options.Solar.NorthRotationDegrees,
            MinimumAltitudeDegrees = options.Solar.MinimumAltitudeDegrees,
        };
        NativeIrradianceSummary[] nativeSummaries = new NativeIrradianceSummary[sensors.Count];
        NativeIrradianceTimelineEntry[] nativeTimeline = new NativeIrradianceTimelineEntry[timelineCount];
        NativeAnnualIrradianceMetadata metadata = default;
        byte[] epw = weather.Utf8;
        fixed (NativeSolarSensor* sensorsPointer = nativeSensors)
        fixed (byte* epwPointer = epw)
        fixed (NativeIrradianceSummary* summariesPointer = nativeSummaries)
        fixed (NativeIrradianceTimelineEntry* timelinePointer = nativeTimeline)
        {
            NativeStatus status = NativeMethods.SceneAnnualIrradiance(
                handle,
                sensorsPointer,
                (nuint)nativeSensors.Length,
                epwPointer,
                (nuint)epw.Length,
                &nativeOptions,
                job,
                &metadata,
                summariesPointer,
                (nuint)nativeSummaries.Length,
                timelinePointer,
                (nuint)nativeTimeline.Length);
            if (status == NativeStatus.Cancelled)
            {
                throw new OperationCanceledException("XVARNA annual irradiance was cancelled.", cancellationToken);
            }
            ThrowIfFailed(status);
        }
        ValidateAnnualLayouts(metadata, nativeSummaries, nativeTimeline);
        SensorIrradianceSummary[] summaries = Array.ConvertAll(nativeSummaries, value =>
            new SensorIrradianceSummary(
                value.SensorId,
                value.DirectWhM2,
                value.DiffuseSkyWhM2,
                value.GroundReflectedWhM2,
                value.GlobalWhM2,
                value.PeakGlobalWM2,
                DateTimeOffset.FromUnixTimeSeconds(value.PeakUnixSecondsUtc),
                value.DomeVisibilityRatio,
                value.HorizonVisibilityRatio,
                value.VisibleCount,
                value.BlockedCount,
                value.BackFacingCount,
                value.DominantSolarOccluderObjectId,
                value.DominantSolarOccluderLossWhM2,
                value.DominantSkyOccluderObjectId,
                value.DominantSkyOccluderProjectedFraction));
        IrradianceTimelineEntry[] timeline = Array.ConvertAll(nativeTimeline, value =>
            new IrradianceTimelineEntry(
                (SunState)value.State,
                DateTimeOffset.FromUnixTimeSeconds(value.UnixSecondsUtc),
                value.DirectWhM2,
                value.DiffuseDomeWhM2,
                value.DiffuseCircumsolarWhM2,
                value.DiffuseHorizonWhM2,
                value.DiffuseSkyWhM2,
                value.GroundReflectedWhM2,
                value.GlobalWhM2,
                value.GlobalWM2,
                value.AttributedLossWhM2,
                value.ObjectId,
                value.InstanceId,
                value.MeshId,
                value.TriangleId,
                value.DistanceMeters / unitScaleToMeters));
        return new()
        {
            Sensors = sensors.ToArray(),
            Weather = weather,
            Summaries = summaries,
            Timeline = timeline,
            Options = options,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = SolarConversions.FormatHash(
                metadata.ContentHash0,
                metadata.ContentHash1,
                metadata.ContentHash2,
                metadata.ContentHash3),
        };
    }

    /// <summary>Runs annual irradiance without blocking the caller thread.</summary>
    public Task<AnnualIrradianceResult> AnalyzeAnnualIrradianceAsync(
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options,
        CancellationToken cancellationToken = default) =>
        BeginAnnualIrradiance(sensors, weather, options, cancellationToken).Completion;

    /// <summary>Starts an observable annual native job.</summary>
    public AnnualIrradianceAnalysisJob BeginAnnualIrradiance(
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options,
        CancellationToken cancellationToken = default) =>
        new(this, sensors, weather, options, cancellationToken);

    private static void ValidateAnnualLayouts(
        NativeAnnualIrradianceMetadata metadata,
        IReadOnlyList<NativeIrradianceSummary> summaries,
        IReadOnlyList<NativeIrradianceTimelineEntry> timeline)
    {
        if (metadata.StructureSize != ExpectedAnnualMetadataSize
            || metadata.StructureSize != Marshal.SizeOf<NativeAnnualIrradianceMetadata>()
            || summaries.Any(value => value.StructureSize != ExpectedAnnualSummarySize)
            || timeline.Any(value => value.StructureSize != ExpectedAnnualEntrySize))
        {
            throw new InvalidOperationException("Native annual irradiance ABI layout mismatch.");
        }
    }
}

/// <summary>An in-flight annual irradiance analysis with progress and cancellation.</summary>
public sealed class AnnualIrradianceAnalysisJob : IDisposable
{
    private readonly object syncRoot = new();
    private readonly SafeJobHandle handle;
    private readonly CancellationTokenRegistration cancellationRegistration;
    private AnnualIrradianceProgress lastProgress;

    internal AnnualIrradianceAnalysisJob(
        XvarnaScene scene,
        IReadOnlyList<SolarSensor> sensors,
        EpwWeatherFile weather,
        AnnualIrradianceOptions options,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(scene);
        ArgumentNullException.ThrowIfNull(sensors);
        ArgumentNullException.ThrowIfNull(weather);
        if (sensors.Count == 0)
        {
            throw new ArgumentException("At least one irradiance sensor is required.", nameof(sensors));
        }
        cancellationToken.ThrowIfCancellationRequested();
        SolarSensor[] snapshot = sensors.ToArray();
        ulong totalUnits = checked((ulong)snapshot.Length * (weather.WeatherCount + 168));
        lastProgress = new(totalUnits, 0, false);
        handle = SafeJobHandle.Create();
        cancellationRegistration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(),
            handle);
        Completion = Task.Run(
            () => scene.AnalyzeAnnualIrradianceCore(snapshot, weather, options, cancellationToken, handle),
            CancellationToken.None);
        _ = Completion.ContinueWith(
            static (_, state) => ((AnnualIrradianceAnalysisJob)state!).CompleteNativeLifetime(),
            this,
            CancellationToken.None,
            TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }

    /// <summary>Task completing with the immutable annual result.</summary>
    public Task<AnnualIrradianceResult> Completion { get; }

    /// <summary>Gets the latest non-blocking progress snapshot.</summary>
    public AnnualIrradianceProgress Progress
    {
        get
        {
            lock (syncRoot)
            {
                if (!handle.IsClosed && !handle.IsInvalid)
                {
                    lastProgress = ConvertProgress(handle.ReadProgress());
                }
                return lastProgress;
            }
        }
    }

    /// <summary>Requests cooperative cancellation.</summary>
    public void Cancel()
    {
        lock (syncRoot)
        {
            if (!handle.IsClosed && !handle.IsInvalid)
            {
                handle.RequestCancellation();
                lastProgress = lastProgress with { CancellationRequested = true };
            }
        }
    }

    /// <summary>Requests cancellation; native ownership ends safely after completion.</summary>
    public void Dispose() => Cancel();

    private void CompleteNativeLifetime()
    {
        lock (syncRoot)
        {
            if (!handle.IsClosed && !handle.IsInvalid)
            {
                lastProgress = ConvertProgress(handle.ReadProgress());
                if (Completion.IsCompletedSuccessfully)
                {
                    lastProgress = lastProgress with { CompletedWorkUnits = lastProgress.TotalWorkUnits };
                }
                cancellationRegistration.Dispose();
                handle.Dispose();
            }
        }
    }

    private static AnnualIrradianceProgress ConvertProgress(SkyViewProgress progress) =>
        new(progress.TotalRays, progress.CompletedRays, progress.CancellationRequested);
}
