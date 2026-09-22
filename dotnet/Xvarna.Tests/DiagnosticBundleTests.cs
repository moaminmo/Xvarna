using System.IO.Compression;
using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class DiagnosticBundleTests
{
    [Fact]
    public void BundleIsRedactedAndContainsNoModelData()
    {
        string root = Path.Combine(Path.GetTempPath(), $"xvarna-diagnostics-test-{Guid.NewGuid():N}");
        try
        {
            string secretPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "private-model.3dm");
            DiagnosticBundleResult result = XvarnaDiagnosticBundle.Create(root, [$"failed at {secretPath}"], $"user={Environment.UserName}");
            Assert.True(File.Exists(result.ArchivePath)); Assert.Equal(64, result.ContentHash.Length);
            using ZipArchive archive = ZipFile.OpenRead(result.ArchivePath);
            Assert.Equal(2, archive.Entries.Count); ZipArchiveEntry manifest = Assert.Single(archive.Entries, value => value.FullName == "diagnostics.json");
            using StreamReader reader = new(manifest.Open()); string json = reader.ReadToEnd();
            Assert.Contains("\\u003CPATH\\u003E", json); Assert.DoesNotContain(secretPath, json, StringComparison.OrdinalIgnoreCase);
            Assert.DoesNotContain(Environment.UserName, json, StringComparison.OrdinalIgnoreCase);
            Assert.DoesNotContain(".3dm\"", json, StringComparison.OrdinalIgnoreCase);
        }
        finally
        {
            if (Directory.Exists(root)) Directory.Delete(root, true);
        }
    }
}
