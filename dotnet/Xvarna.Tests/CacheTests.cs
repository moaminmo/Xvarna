using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class CacheTests
{
    [Fact]
    public void CachePolicyRoundTripsThroughStableAbiAndClearIsScoped()
    {
        string directory = Path.Combine(Path.GetTempPath(), $"xvarna-cache-sdk-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        string unrelated = Path.Combine(directory, "keep.txt");
        File.WriteAllText(unrelated, "not an XVARNA cache entry");
        try
        {
            XvarnaCache.Configure(new CacheConfiguration(directory, 4 * 1024 * 1024, 16 * 1024 * 1024));
            CacheStatistics configured = XvarnaCache.GetStatistics();
            Assert.Equal(Path.GetFullPath(directory), configured.Directory);
            Assert.Equal(4ul * 1024 * 1024, configured.MemoryBudgetBytes);
            Assert.Equal(16ul * 1024 * 1024, configured.DiskBudgetBytes);

            XvarnaCache.Clear();
            CacheStatistics cleared = XvarnaCache.GetStatistics();
            Assert.Equal(0ul, cleared.MemoryEntryCount);
            Assert.True(File.Exists(unrelated));
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, true);
            }
        }
    }
}
