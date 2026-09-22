using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>One oriented model-space point for Sky View and Shadow Mask analysis.</summary>
public readonly record struct SkySensor(
    ulong SensorId,
    double PositionX,
    double PositionY,
    double PositionZ,
    double NormalX,
    double NormalY,
    double NormalZ);

/// <summary>Deterministic Fibonacci hemisphere and obstruction-query policy.</summary>
public readonly record struct SkyViewOptions(
    uint SampleCount = 2048,
    ulong Seed = 0,
    double SensorOffset = 1.0e-4,
    double MaximumDistance = double.PositiveInfinity,
    ulong CategoryMask = ulong.MaxValue)
{
    /// <summary>Balanced deterministic default suitable for interactive design studies.</summary>
    public static SkyViewOptions Default { get; } = new(
        2048,
        0,
        1e-4,
        double.PositiveInfinity,
        ulong.MaxValue);
}

/// <summary>Binary state of one hemispherical direction.</summary>
public enum SkyRayState : uint
{
    /// <summary>Direction reaches open sky.</summary>
    Visible = 0,
    /// <summary>Direction is blocked by scene geometry.</summary>
    Blocked = 1,
}

/// <summary>World direction and complete first-hit attribution for one Shadow Mask sample.</summary>
public readonly record struct SkyRayEntry(
    SkyRayState State,
    double DirectionX,
    double DirectionY,
    double DirectionZ,
    ulong ObjectId,
    ulong InstanceId,
    ulong MeshId,
    uint TriangleId,
    double Distance);

/// <summary>Sky-view metrics, convergence diagnostics, and dominant obstruction.</summary>
public readonly record struct SensorSkySummary(
    ulong SensorId,
    double VisibleHemisphereFraction,
    double CosineWeightedSkyViewFactor,
    double VisibleSolidAngleSteradians,
    double CosineConvergenceDelta,
    double UnweightedConvergenceDelta,
    ulong VisibleCount,
    ulong BlockedCount,
    ulong DominantOccluderObjectId,
    double DominantOccluderSolidAngleSteradians,
    double DominantOccluderProjectedFraction);

/// <summary>Complete sensor-major Sky View and Shadow Mask result.</summary>
public sealed record SkyViewResult
{
    /// <summary>Source sensors retained for visualization and extraction.</summary>
    public required IReadOnlyList<SkySensor> Sensors { get; init; }
    /// <summary>One aggregate per source sensor.</summary>
    public required IReadOnlyList<SensorSkySummary> Summaries { get; init; }
    /// <summary>Sensor-major Shadow Mask entries.</summary>
    public required IReadOnlyList<SkyRayEntry> Timeline { get; init; }
    /// <summary>Sampling and ray-query policy.</summary>
    public required SkyViewOptions Options { get; init; }
    /// <summary>Native analysis duration in microseconds.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic BLAKE3 identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Thread-safe native progress snapshot for a running sky analysis.</summary>
public readonly record struct SkyViewProgress(
    ulong TotalRays,
    ulong CompletedRays,
    bool CancellationRequested)
{
    /// <summary>Completed fraction from zero through one; zero before native dispatch.</summary>
    public double Fraction => TotalRays == 0
        ? 0.0
        : Math.Clamp((double)CompletedRays / TotalRays, 0.0, 1.0);
}

public sealed partial class XvarnaScene
{
    private const uint ExpectedSkyOptionsSize = 40;
    private const uint ExpectedSkySummarySize = 96;
    private const uint ExpectedSkyEntrySize = 72;
    private const uint ExpectedSkyMetadataSize = 72;

    /// <summary>Computes Sky View and Shadow Mask synchronously.</summary>
    public SkyViewResult AnalyzeSkyView(
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options) =>
        AnalyzeSkyView(sensors, options, CancellationToken.None);

    /// <summary>Computes Sky View synchronously while observing cooperative cancellation.</summary>
    public unsafe SkyViewResult AnalyzeSkyView(
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options,
        CancellationToken cancellationToken)
    {
        using SafeJobHandle job = SafeJobHandle.Create();
        using CancellationTokenRegistration registration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(),
            job);
        return AnalyzeSkyViewCore(sensors, options, cancellationToken, job);
    }

    internal unsafe SkyViewResult AnalyzeSkyViewCore(
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options,
        CancellationToken cancellationToken,
        SafeJobHandle job)
    {
        ArgumentNullException.ThrowIfNull(sensors);
        if (sensors.Count == 0)
        {
            throw new ArgumentException("At least one sky sensor is required.", nameof(sensors));
        }
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();
        int sampleCount = checked((int)options.SampleCount);
        int timelineCount = checked(sensors.Count * sampleCount);
        NativeSolarSensor[] nativeSensors = new NativeSolarSensor[sensors.Count];
        for (int index = 0; index < sensors.Count; index++)
        {
            SkySensor sensor = sensors[index];
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
        NativeSkyViewOptions nativeOptions = new()
        {
            StructureSize = ExpectedSkyOptionsSize,
            SampleCount = options.SampleCount,
            Seed = options.Seed,
            SensorOffsetMeters = options.SensorOffset * unitScaleToMeters,
            MaximumDistanceMeters = options.MaximumDistance * unitScaleToMeters,
            CategoryMask = options.CategoryMask,
        };
        NativeSkyViewSummary[] nativeSummaries = new NativeSkyViewSummary[sensors.Count];
        NativeSkyRayEntry[] nativeTimeline = new NativeSkyRayEntry[timelineCount];
        NativeSkyViewMetadata metadata = default;
        fixed (NativeSolarSensor* sensorsPointer = nativeSensors)
        fixed (NativeSkyViewSummary* summariesPointer = nativeSummaries)
        fixed (NativeSkyRayEntry* timelinePointer = nativeTimeline)
        {
            NativeStatus status = NativeMethods.SceneSkyView(
                handle,
                sensorsPointer,
                (nuint)nativeSensors.Length,
                &nativeOptions,
                job,
                &metadata,
                summariesPointer,
                (nuint)nativeSummaries.Length,
                timelinePointer,
                (nuint)nativeTimeline.Length);
            if (status == NativeStatus.Cancelled)
            {
                throw new OperationCanceledException("XVARNA Sky View was cancelled.", cancellationToken);
            }
            ThrowIfFailed(status);
        }
        ValidateSkyLayouts(metadata, nativeSummaries, nativeTimeline);
        SensorSkySummary[] summaries = Array.ConvertAll(nativeSummaries, value => new SensorSkySummary(
            value.SensorId,
            value.VisibleHemisphereFraction,
            value.CosineWeightedSvf,
            value.VisibleSolidAngleSteradians,
            value.CosineConvergenceDelta,
            value.UnweightedConvergenceDelta,
            value.VisibleCount,
            value.BlockedCount,
            value.DominantOccluderObjectId,
            value.DominantOccluderSolidAngleSteradians,
            value.DominantOccluderProjectedFraction));
        SkyRayEntry[] timeline = Array.ConvertAll(nativeTimeline, value => new SkyRayEntry(
            (SkyRayState)value.State,
            value.DirectionX,
            value.DirectionY,
            value.DirectionZ,
            value.ObjectId,
            value.InstanceId,
            value.MeshId,
            value.TriangleId,
            value.DistanceMeters / unitScaleToMeters));
        return new()
        {
            Sensors = sensors.ToArray(),
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

    /// <summary>Runs cooperative Sky View analysis without blocking the caller thread.</summary>
    public Task<SkyViewResult> AnalyzeSkyViewAsync(
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options,
        CancellationToken cancellationToken = default)
    {
        return BeginSkyView(sensors, options, cancellationToken).Completion;
    }

    /// <summary>Starts an observable native job with progress, cancellation, and a completion task.</summary>
    public SkyViewAnalysisJob BeginSkyView(
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options,
        CancellationToken cancellationToken = default) =>
        new(this, sensors, options, cancellationToken);

    private static void ValidateSkyLayouts(
        NativeSkyViewMetadata metadata,
        IReadOnlyList<NativeSkyViewSummary> summaries,
        IReadOnlyList<NativeSkyRayEntry> timeline)
    {
        if (metadata.StructureSize != ExpectedSkyMetadataSize
            || metadata.StructureSize != Marshal.SizeOf<NativeSkyViewMetadata>()
            || summaries.Any(value => value.StructureSize != ExpectedSkySummarySize)
            || timeline.Any(value => value.StructureSize != ExpectedSkyEntrySize))
        {
            throw new InvalidOperationException("Native Sky View ABI layout mismatch.");
        }
    }
}

/// <summary>An in-flight ASMAN analysis with cooperative cancellation and ray progress.</summary>
public sealed class SkyViewAnalysisJob : IDisposable
{
    private readonly object syncRoot = new();
    private readonly SafeJobHandle handle;
    private readonly CancellationTokenRegistration cancellationRegistration;
    private SkyViewProgress lastProgress;

    internal SkyViewAnalysisJob(
        XvarnaScene scene,
        IReadOnlyList<SkySensor> sensors,
        SkyViewOptions options,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(scene);
        ArgumentNullException.ThrowIfNull(sensors);
        if (sensors.Count == 0)
        {
            throw new ArgumentException("At least one sky sensor is required.", nameof(sensors));
        }
        cancellationToken.ThrowIfCancellationRequested();
        SkySensor[] snapshot = sensors.ToArray();
        ulong totalRays = checked((ulong)snapshot.Length * options.SampleCount);
        lastProgress = new(totalRays, 0, false);
        handle = SafeJobHandle.Create();
        cancellationRegistration = cancellationToken.Register(
            static state => ((SafeJobHandle)state!).RequestCancellation(),
            handle);
        Completion = Task.Run(
            () => scene.AnalyzeSkyViewCore(snapshot, options, cancellationToken, handle),
            CancellationToken.None);
        _ = Completion.ContinueWith(
            static (_, state) => ((SkyViewAnalysisJob)state!).CompleteNativeLifetime(),
            this,
            CancellationToken.None,
            TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }

    /// <summary>Task that completes with the immutable result or the contained failure.</summary>
    public Task<SkyViewResult> Completion { get; }

    /// <summary>Gets the latest native progress without blocking for completion.</summary>
    public SkyViewProgress Progress
    {
        get
        {
            lock (syncRoot)
            {
                if (!handle.IsClosed && !handle.IsInvalid)
                {
                    lastProgress = handle.ReadProgress();
                }
                return lastProgress;
            }
        }
    }

    /// <summary>Requests cancellation; completion observes an <see cref="OperationCanceledException"/>.</summary>
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

    /// <summary>Requests cancellation. Native ownership is released safely after completion.</summary>
    public void Dispose() => Cancel();

    private void CompleteNativeLifetime()
    {
        lock (syncRoot)
        {
            if (!handle.IsClosed && !handle.IsInvalid)
            {
                lastProgress = handle.ReadProgress();
                if (Completion.IsCompletedSuccessfully)
                {
                    lastProgress = lastProgress with { CompletedRays = lastProgress.TotalRays };
                }
                cancellationRegistration.Dispose();
                handle.Dispose();
            }
        }
    }
}

internal sealed class SafeJobHandle : SafeHandle
{
    private SafeJobHandle(ulong handleValue)
        : base(nint.Zero, true)
    {
        SetHandle(checked((nint)handleValue));
    }

    public override bool IsInvalid => handle == nint.Zero;

    internal static unsafe SafeJobHandle Create()
    {
        ulong handleValue = 0;
        NativeStatus status = NativeMethods.JobCreate(&handleValue);
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
        return new(handleValue);
    }

    internal void RequestCancellation()
    {
        if (!IsClosed && !IsInvalid)
        {
            _ = NativeMethods.JobCancel(this);
        }
    }

    internal unsafe SkyViewProgress ReadProgress()
    {
        NativeJobProgress progress = default;
        NativeStatus status = NativeMethods.JobProgress(this, &progress);
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
        if (progress.StructureSize != 32)
        {
            throw new InvalidOperationException("Native job-progress ABI layout mismatch.");
        }
        return new(
            progress.TotalUnits,
            progress.CompletedUnits,
            (progress.Flags & 1u) != 0);
    }

    protected override bool ReleaseHandle() =>
        NativeMethods.JobRelease(unchecked((ulong)handle)) == NativeStatus.Success;
}
