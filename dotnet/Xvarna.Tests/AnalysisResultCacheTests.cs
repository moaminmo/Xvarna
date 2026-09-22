using Xunit;
using Xvarna.Grasshopper;
namespace Xvarna.Tests;
public sealed class AnalysisResultCacheTests
{
    [Fact]
    public async Task ConcurrentIterationsDoNotSupersedeEachOther()
    {
        var component = Guid.NewGuid();
        using var release = new ManualResetEventSlim(false);
        using var first = Xvarna.Native.XvarnaAnalysisScheduler.Schedule("branch 0",
            AnalysisResultCache<double, string>.SchedulingScope(component, 0), 1,
            (_, _) => { release.Wait(TimeSpan.FromSeconds(10)); return 10; });
        using var second = Xvarna.Native.XvarnaAnalysisScheduler.Schedule("branch 1",
            AnalysisResultCache<double, string>.SchedulingScope(component, 1), 1,
            (_, _) => 20);
        release.Set();
        Assert.Equal(10, await first.Completion);
        Assert.Equal(20, await second.Completion);
    }
    [Fact]
    public void RetainedResultKeepsOriginalInputsAndBranches()
    {
        var cache = new AnalysisResultCache<double, string>();
        cache.Store(0, 0.25, "coarse grid", 3);
        cache.Store(1, 0.01, "fine grid", 3);
        Assert.True(cache.TryGet(0, out var first));
        Assert.Equal(0.25, first!.Work);
        Assert.Equal("coarse grid", first.Result);
        Assert.True(cache.TryGet(1, out var second));
        Assert.Equal(0.01, second!.Work);
        Assert.Equal(3, second.RunGeneration);
        Assert.False(cache.TryGet(2, out _));
    }
}
