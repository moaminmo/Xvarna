using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper;

/// <summary>Terminal output transported by Grasshopper's task-capable solve pipeline.</summary>
public sealed record ScheduledComponentOutput<TResult>(TResult? Result, string? Error, bool Cancelled)
    where TResult : class;

/// <summary>
/// Shared async/cancel/stale-result bridge for every compute-heavy Grasshopper analysis.
/// Inputs are captured on the solution thread; native work runs on XVARNA's bounded scheduler.
/// </summary>
public abstract class ScheduledAnalysisComponent<TWork, TResult>
    : GH_TaskCapableComponent<ScheduledComponentOutput<TResult>>
    where TResult : class
{
    private readonly AnalysisResultCache<TWork, TResult> results = new();

    /// <summary>Initializes one scheduled analysis component.</summary>
    protected ScheduledAnalysisComponent(
        string name,
        string nickname,
        string description,
        string category,
        string subcategory)
        : base(name, nickname, description, category, subcategory)
    {
        UseTasks = true;
    }

    /// <summary>Short scheduler/viewport phase name.</summary>
    protected abstract string AnalysisName { get; }

    /// <summary>Captures and validates immutable work without using Rhino objects off-thread.</summary>
    protected abstract bool TryReadWork(IGH_DataAccess dataAccess, out TWork work);

    /// <summary>Estimates deterministic progress units for scheduling telemetry.</summary>
    protected abstract ulong EstimateWorkUnits(TWork work);

    /// <summary>Executes native work. Implementations report safe phase boundaries.</summary>
    protected abstract TResult Execute(
        TWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress);

    /// <summary>Publishes a completed current-generation result on the solution thread.</summary>
    protected abstract void Emit(IGH_DataAccess dataAccess, TWork work, TResult result);

    /// <summary>Handles a paused/invalid/cancelled solve before outputs are published.</summary>
    protected virtual void ClearScheduledPreview()
    {
    }

    /// <inheritdoc />
    protected sealed override void SolveInstance(IGH_DataAccess dataAccess)
    {
        if (!TryReadWork(dataAccess, out TWork work))
        {
            if (!InPreSolve) ClearScheduledPreview();
            return;
        }
        results.TryGet(dataAccess.Iteration, out var previous);
        AnalysisQualityProfile quality = ProductContextRegistry.GetQuality(OnPingDocument());
        bool waitingForManualRun = quality.Execution == XvarnaExecutionMode.Manual
            && previous is not null && quality.RunGeneration == previous.RunGeneration;
        if (waitingForManualRun)
        {
            if (InPreSolve) return;
            if (previous is not null)
            {
                Emit(dataAccess, previous.Work, previous.Result);
                Message = $"{AnalysisName}\nMANUAL · CACHED";
                AddRuntimeMessage(GH_RuntimeMessageLevel.Remark,
                    "Manual mode is active. Showing the last successful result; pulse Run on XV Quality to recompute.");
            }
            else
            {
                Message = $"{AnalysisName}\nWAITING";
                AddRuntimeMessage(GH_RuntimeMessageLevel.Remark,
                    "Manual mode is active. Pulse Run on XV Quality to launch this analysis.");
            }
            return;
        }
        ulong units = EstimateWorkUnits(work);
        if (InPreSolve)
        {
            Message = $"{AnalysisName}\n{quality.Preset} · Running";
            TaskList.Add(AnalysisTaskBridge.Schedule(
                AnalysisName,
                AnalysisResultCache<TWork, TResult>.SchedulingScope(InstanceGuid, dataAccess.Iteration),
                units,
                (token, progress) => ExecuteSafely(work, token, progress),
                CancelToken,
                quality.Execution == XvarnaExecutionMode.Live ? quality.DebounceMilliseconds : 0));
            return;
        }

        ScheduledComponentOutput<TResult> output;
        if (!GetSolveResults(dataAccess, out output))
        {
            output = ExecuteSafely(
                work,
                CancelToken,
                new InlineProgress<(ulong Completed, ulong Total, string Phase)>());
        }
        if (output.Cancelled)
        {
            Message = $"{AnalysisName}\nCancelled";
            if (quality.RetainStaleResult && previous is not null)
            {
                Emit(dataAccess, previous.Work, previous.Result);
                Message = $"{AnalysisName}\nSTALE · Cancelled";
                AddRuntimeMessage(GH_RuntimeMessageLevel.Warning,
                    "The current solve was cancelled or superseded. A labelled stale result remains visible.");
                ProductContextRegistry.Record(OnPingDocument(), AnalysisName, "retained stale result after cancellation");
                return;
            }
            ClearScheduledPreview();
            AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, $"{AnalysisName} was cancelled or superseded.");
            return;
        }
        if (output.Error is not null || output.Result is null)
        {
            Message = $"{AnalysisName}\nError";
            if (quality.RetainStaleResult && previous is not null)
            {
                Emit(dataAccess, previous.Work, previous.Result);
                Message = $"{AnalysisName}\nSTALE · Error";
                AddRuntimeMessage(GH_RuntimeMessageLevel.Warning,
                    $"Current analysis failed: {output.Error}. A labelled stale result remains visible.");
                ProductContextRegistry.Record(OnPingDocument(), AnalysisName, $"retained stale result after error: {output.Error}");
                return;
            }
            ClearScheduledPreview();
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, output.Error ?? $"{AnalysisName} returned no result.");
            return;
        }
        results.Store(dataAccess.Iteration, work, output.Result, quality.RunGeneration);
        Emit(dataAccess, work, output.Result);
        ProductContextRegistry.Record(OnPingDocument(), AnalysisName, $"completed with {quality.Preset} quality");
    }

    private ScheduledComponentOutput<TResult> ExecuteSafely(
        TWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            TResult result = Execute(work, cancellationToken, progress);
            cancellationToken.ThrowIfCancellationRequested();
            return new(result, null, false);
        }
        catch (OperationCanceledException)
        {
            return new(null, null, true);
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ObjectDisposedException
            or ArgumentException
            or OverflowException
            or InvalidOperationException
            or IOException)
        {
            return new(null, exception.Message, false);
        }
    }

    private sealed class InlineProgress<T> : IProgress<T>
    {
        public void Report(T value)
        {
        }
    }
}
