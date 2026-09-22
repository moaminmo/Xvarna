using System.Collections.Concurrent;
using System.Diagnostics;

namespace Xvarna.Native;

/// <summary>Lifecycle state of one scheduled analysis.</summary>
public enum AnalysisJobState
{
    /// <summary>Waiting for a bounded execution slot.</summary>
    Queued,
    /// <summary>Executing on a scheduler worker.</summary>
    Running,
    /// <summary>Completed successfully and remains current for its scope.</summary>
    Completed,
    /// <summary>Cancelled before publication.</summary>
    Cancelled,
    /// <summary>Failed with a captured exception.</summary>
    Failed,
    /// <summary>Completed after a newer generation superseded it.</summary>
    Stale,
}

/// <summary>Immutable progress event shared by every managed analysis.</summary>
public readonly record struct AnalysisProgress(
    Guid JobId,
    string Name,
    AnalysisJobState State,
    ulong CompletedUnits,
    ulong TotalUnits,
    double Fraction,
    string Phase,
    bool CancellationRequested,
    bool IsStale);

/// <summary>Process-wide scheduler policy.</summary>
public readonly record struct AnalysisSchedulerOptions(
    int MaximumConcurrency,
    int MaximumQueuedJobs = 256)
{
    /// <summary>Balanced default that leaves one logical core available to Rhino.</summary>
    public static AnalysisSchedulerOptions Default { get; } = new(
        Math.Max(1, Environment.ProcessorCount - 1),
        256);
}

/// <summary>Cumulative scheduler load, latency, completion, and cancellation telemetry.</summary>
public sealed record AnalysisSchedulerStatistics
{
    /// <summary>Configured concurrent worker slots.</summary>
    public required int MaximumConcurrency { get; init; }
    /// <summary>Configured queue capacity.</summary>
    public required int MaximumQueuedJobs { get; init; }
    /// <summary>Jobs currently waiting for a slot.</summary>
    public required int QueuedJobs { get; init; }
    /// <summary>Jobs currently executing.</summary>
    public required int RunningJobs { get; init; }
    /// <summary>Successfully published jobs.</summary>
    public required ulong CompletedJobs { get; init; }
    /// <summary>Cooperatively cancelled jobs.</summary>
    public required ulong CancelledJobs { get; init; }
    /// <summary>Jobs superseded by a newer scope generation.</summary>
    public required ulong StaleJobs { get; init; }
    /// <summary>Jobs that raised an exception.</summary>
    public required ulong FailedJobs { get; init; }
    /// <summary>Average queue wait in milliseconds.</summary>
    public required double AverageQueueMilliseconds { get; init; }
}

/// <summary>Typed handle for cancellation, progress inspection, and asynchronous completion.</summary>
public sealed class ScheduledAnalysis<T> : IDisposable
{
    private readonly CancellationTokenSource cancellation;
    private int disposed;

    internal ScheduledAnalysis(
        Guid id,
        string name,
        CancellationTokenSource cancellation,
        Task<T> completion,
        Func<AnalysisProgress> progress)
    {
        Id = id;
        Name = name;
        this.cancellation = cancellation;
        Completion = completion;
        GetProgress = progress;
    }

    /// <summary>Stable unique job identity.</summary>
    public Guid Id { get; }
    /// <summary>Human-readable analysis name.</summary>
    public string Name { get; }
    /// <summary>Task completed with the analysis result or terminal exception.</summary>
    public Task<T> Completion { get; }
    /// <summary>Thread-safe live progress snapshot.</summary>
    public Func<AnalysisProgress> GetProgress { get; }

    /// <summary>Requests cooperative cancellation at the next safe boundary.</summary>
    public void Cancel() => cancellation.Cancel();

    /// <inheritdoc />
    public void Dispose()
    {
        if (Interlocked.Exchange(ref disposed, 1) == 0)
        {
            cancellation.Dispose();
        }
    }
}

/// <summary>Bounded process-wide scheduler used by every XVARNA analysis connector.</summary>
public static class XvarnaAnalysisScheduler
{
    private sealed class JobProgress
    {
        internal readonly Guid Id;
        internal readonly string Name;
        internal readonly string Scope;
        internal readonly long Generation;
        internal long Completed;
        internal long Total;
        internal int State;
        internal string Phase = "Queued";
        internal int CancellationRequested;

        internal JobProgress(Guid id, string name, string scope, long generation, ulong total)
        {
            Id = id;
            Name = name;
            Scope = scope;
            Generation = generation;
            Total = checked((long)Math.Min(total, long.MaxValue));
            State = (int)AnalysisJobState.Queued;
        }
    }

    private sealed class DirectProgress(JobProgress job) : IProgress<(ulong Completed, ulong Total, string Phase)>
    {
        public void Report((ulong Completed, ulong Total, string Phase) value)
        {
            Interlocked.Exchange(ref job.Completed, checked((long)Math.Min(value.Completed, long.MaxValue)));
            Interlocked.Exchange(ref job.Total, checked((long)Math.Min(value.Total, long.MaxValue)));
            Volatile.Write(ref job.Phase, value.Phase ?? string.Empty);
        }
    }

    private static readonly object Gate = new();
    private static readonly ConcurrentDictionary<string, long> Generations = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<Guid, JobProgress> ActiveJobs = new();
    private static AnalysisSchedulerOptions options = AnalysisSchedulerOptions.Default;
    private static SemaphoreSlim slots = new(options.MaximumConcurrency, options.MaximumConcurrency);
    private static int queued;
    private static int running;
    private static long completed;
    private static long cancelled;
    private static long stale;
    private static long failed;
    private static long queueTicks;
    private static long started;

    /// <summary>Changes scheduler limits while no work is queued or running.</summary>
    public static void Configure(AnalysisSchedulerOptions value)
    {
        Validate(value);
        lock (Gate)
        {
            if (Volatile.Read(ref queued) != 0 || Volatile.Read(ref running) != 0)
            {
                throw new InvalidOperationException("Scheduler policy can change only while idle.");
            }
            SemaphoreSlim previous = slots;
            slots = new(value.MaximumConcurrency, value.MaximumConcurrency);
            options = value;
            previous.Dispose();
        }
    }

    /// <summary>Schedules bounded cancellable work and suppresses stale scope generations.</summary>
    public static ScheduledAnalysis<T> Schedule<T>(
        string name,
        string scope,
        ulong totalUnits,
        Func<CancellationToken, IProgress<(ulong Completed, ulong Total, string Phase)>, T> operation,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        ArgumentNullException.ThrowIfNull(operation);
        int queuedNow = Interlocked.Increment(ref queued);
        if (queuedNow > options.MaximumQueuedJobs)
        {
            Interlocked.Decrement(ref queued);
            throw new InvalidOperationException("XVARNA scheduler queue capacity was exceeded.");
        }
        long generation = Generations.AddOrUpdate(scope, 1, static (_, value) => checked(value + 1));
        Guid id = Guid.NewGuid();
        CancellationTokenSource linked = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        JobProgress progress = new(id, name, scope, generation, totalUnits);
        ActiveJobs[id] = progress;
        Stopwatch queuedTimer = Stopwatch.StartNew();
        Task<T> completion = Task.Run(async () =>
        {
            bool entered = false;
            try
            {
                await slots.WaitAsync(linked.Token).ConfigureAwait(false);
                entered = true;
                Interlocked.Decrement(ref queued);
                Interlocked.Increment(ref running);
                Interlocked.Increment(ref started);
                Interlocked.Add(ref queueTicks, queuedTimer.ElapsedTicks);
                Volatile.Write(ref progress.State, (int)AnalysisJobState.Running);
                Volatile.Write(ref progress.Phase, "Running");
                T result = operation(linked.Token, new DirectProgress(progress));
                linked.Token.ThrowIfCancellationRequested();
                bool isStale = Generations.TryGetValue(scope, out long current) && current != generation;
                if (isStale)
                {
                    Volatile.Write(ref progress.State, (int)AnalysisJobState.Stale);
                    Volatile.Write(ref progress.Phase, "Superseded");
                    Interlocked.Increment(ref stale);
                    throw new OperationCanceledException("Analysis result was superseded by a newer input generation.", linked.Token);
                }
                Interlocked.Exchange(ref progress.Completed, Volatile.Read(ref progress.Total));
                Volatile.Write(ref progress.State, (int)AnalysisJobState.Completed);
                Volatile.Write(ref progress.Phase, "Completed");
                Interlocked.Increment(ref completed);
                return result;
            }
            catch (OperationCanceledException)
            {
                Interlocked.Exchange(ref progress.CancellationRequested, 1);
                if ((AnalysisJobState)Volatile.Read(ref progress.State) != AnalysisJobState.Stale)
                {
                    Volatile.Write(ref progress.State, (int)AnalysisJobState.Cancelled);
                    Volatile.Write(ref progress.Phase, "Cancelled");
                    Interlocked.Increment(ref cancelled);
                }
                throw;
            }
            catch
            {
                Volatile.Write(ref progress.State, (int)AnalysisJobState.Failed);
                Volatile.Write(ref progress.Phase, "Failed");
                Interlocked.Increment(ref failed);
                throw;
            }
            finally
            {
                if (entered)
                {
                    Interlocked.Decrement(ref running);
                    slots.Release();
                }
                else
                {
                    Interlocked.Decrement(ref queued);
                }
                ActiveJobs.TryRemove(id, out _);
            }
        }, CancellationToken.None);
        return new(id, name, linked, completion, () => Snapshot(progress, linked.IsCancellationRequested));
    }

    /// <summary>Reads process-wide queue and outcome counters.</summary>
    public static AnalysisSchedulerStatistics GetStatistics()
    {
        long count = Volatile.Read(ref started);
        double average = count == 0
            ? 0.0
            : Volatile.Read(ref queueTicks) * 1000.0 / Stopwatch.Frequency / count;
        return new()
        {
            MaximumConcurrency = options.MaximumConcurrency,
            MaximumQueuedJobs = options.MaximumQueuedJobs,
            QueuedJobs = Volatile.Read(ref queued),
            RunningJobs = Volatile.Read(ref running),
            CompletedJobs = checked((ulong)Math.Max(0, Volatile.Read(ref completed))),
            CancelledJobs = checked((ulong)Math.Max(0, Volatile.Read(ref cancelled))),
            StaleJobs = checked((ulong)Math.Max(0, Volatile.Read(ref stale))),
            FailedJobs = checked((ulong)Math.Max(0, Volatile.Read(ref failed))),
            AverageQueueMilliseconds = average,
        };
    }

    /// <summary>Returns stable snapshots of all queued and running analyses.</summary>
    public static IReadOnlyList<AnalysisProgress> GetActiveJobs() =>
        ActiveJobs.Values
            .Select(value => Snapshot(value, Volatile.Read(ref value.CancellationRequested) != 0))
            .OrderBy(value => value.Name, StringComparer.Ordinal)
            .ThenBy(value => value.JobId)
            .ToArray();

    private static AnalysisProgress Snapshot(JobProgress value, bool cancellationRequested)
    {
        ulong total = checked((ulong)Math.Max(0, Volatile.Read(ref value.Total)));
        ulong completedUnits = checked((ulong)Math.Max(0, Volatile.Read(ref value.Completed)));
        AnalysisJobState state = (AnalysisJobState)Volatile.Read(ref value.State);
        double fraction = total == 0 ? (state == AnalysisJobState.Completed ? 1.0 : 0.0) : Math.Clamp((double)completedUnits / total, 0.0, 1.0);
        return new(
            value.Id,
            value.Name,
            state,
            completedUnits,
            total,
            fraction,
            Volatile.Read(ref value.Phase),
            cancellationRequested || Volatile.Read(ref value.CancellationRequested) != 0,
            state == AnalysisJobState.Stale);
    }

    private static void Validate(AnalysisSchedulerOptions value)
    {
        if (value.MaximumConcurrency is < 1 or > 1024)
        {
            throw new ArgumentOutOfRangeException(nameof(value), "Maximum concurrency must be between 1 and 1024.");
        }
        if (value.MaximumQueuedJobs is < 1 or > 1_000_000)
        {
            throw new ArgumentOutOfRangeException(nameof(value), "Maximum queued jobs must be between 1 and 1000000.");
        }
    }
}
