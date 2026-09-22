using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class SchedulerTests
{
    [Fact]
    public async Task SchedulerPublishesProgressAndCompletion()
    {
        string scope = $"test:{Guid.NewGuid():N}";
        using ScheduledAnalysis<int> job = XvarnaAnalysisScheduler.Schedule(
            "test analysis",
            scope,
            10,
            (_, progress) =>
            {
                progress.Report((5, 10, "half"));
                return 42;
            });

        Assert.Equal(42, await job.Completion);
        AnalysisProgress progress = job.GetProgress();
        Assert.Equal(AnalysisJobState.Completed, progress.State);
        Assert.Equal(1.0, progress.Fraction);
        Assert.Equal(10ul, progress.CompletedUnits);
    }

    [Fact]
    public async Task SchedulerCancellationIsCooperative()
    {
        using ScheduledAnalysis<int> job = XvarnaAnalysisScheduler.Schedule(
            "cancel analysis",
            $"test:{Guid.NewGuid():N}",
            100,
            (token, progress) =>
            {
                for (ulong index = 0; index < 100; index++)
                {
                    token.ThrowIfCancellationRequested();
                    progress.Report((index, 100, "loop"));
                    Thread.Sleep(1);
                }
                return 1;
            });
        job.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(async () => await job.Completion);
        Assert.Equal(AnalysisJobState.Cancelled, job.GetProgress().State);
        Assert.True(job.GetProgress().CancellationRequested);
    }

    [Fact]
    public async Task NewerScopeGenerationSuppressesStalePublication()
    {
        string scope = $"test:{Guid.NewGuid():N}";
        using ManualResetEventSlim release = new(false);
        using ScheduledAnalysis<int> first = XvarnaAnalysisScheduler.Schedule(
            "old generation",
            scope,
            1,
            (_, _) =>
            {
                release.Wait(TimeSpan.FromSeconds(10));
                return 1;
            });
        using ScheduledAnalysis<int> second = XvarnaAnalysisScheduler.Schedule(
            "new generation",
            scope,
            1,
            (_, _) => 2);
        Assert.Equal(2, await second.Completion);
        release.Set();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(async () => await first.Completion);
        Assert.Equal(AnalysisJobState.Stale, first.GetProgress().State);
    }
}
