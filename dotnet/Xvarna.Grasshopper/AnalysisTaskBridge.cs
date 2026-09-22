using Xvarna.Native;

namespace Xvarna.Grasshopper;

/// <summary>Routes Grasshopper task-capable components through the shared bounded scheduler.</summary>
internal static class AnalysisTaskBridge
{
    internal static async Task<T> Schedule<T>(
        string name,
        string scope,
        ulong totalUnits,
        Func<CancellationToken, IProgress<(ulong Completed, ulong Total, string Phase)>, T> operation,
        CancellationToken cancellationToken,
        int debounceMilliseconds = 0)
    {
        if (debounceMilliseconds > 0)
        {
            await Task.Delay(debounceMilliseconds, cancellationToken).ConfigureAwait(false);
        }
        using ScheduledAnalysis<T> scheduled = XvarnaAnalysisScheduler.Schedule(
            name,
            scope,
            totalUnits,
            operation,
            cancellationToken);
        return await scheduled.Completion.ConfigureAwait(false);
    }
}
