using System.IO.Compression;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Xvarna.Native;

/// <summary>Paths and identity of a redacted support bundle.</summary>
public sealed record DiagnosticBundleResult(string ArchivePath, string ManifestPath, string ContentHash);

/// <summary>Creates a deterministic-schema, privacy-redacted support archive.</summary>
public static class XvarnaDiagnosticBundle
{
    /// <summary>Captures host, engine, adapter, cache, scheduler and supplied error/log context.</summary>
    public static DiagnosticBundleResult Create(
        string directory,
        IReadOnlyList<string>? recentEvents = null,
        string? errorChain = null)
    {
        if (string.IsNullOrWhiteSpace(directory)) throw new ArgumentException("Diagnostic output directory is required.", nameof(directory));
        string root = Path.GetFullPath(directory);
        Directory.CreateDirectory(root);
        string staging = Path.Combine(root, $".xvarna-diagnostic-{Guid.NewGuid():N}");
        Directory.CreateDirectory(staging);
        try
        {
            EngineProbeResult engine = EngineProbe.TryLoad();
            IReadOnlyList<ComputeAdapterInfo> adapters;
            try { adapters = XvarnaComputeSession.EnumerateAdapters(); }
            catch (Exception exception) when (exception is XvarnaNativeException or InvalidOperationException) { adapters = []; errorChain = Join(errorChain, exception.ToString()); }
            CacheStatistics cache = XvarnaCache.GetStatistics();
            AnalysisSchedulerStatistics scheduler = XvarnaAnalysisScheduler.GetStatistics();
            string[] events = (recentEvents ?? []).Select(Redact).ToArray();
            var manifest = new
            {
                schemaVersion = "1.0",
                generatedUtc = DateTimeOffset.UtcNow,
                privacy = new { pathsRedacted = true, userNameRedacted = true, environmentVariablesExcluded = true },
                host = new
                {
                    os = RuntimeInformation.OSDescription,
                    architecture = RuntimeInformation.OSArchitecture.ToString(),
                    processArchitecture = RuntimeInformation.ProcessArchitecture.ToString(),
                    framework = RuntimeInformation.FrameworkDescription,
                    processorCount = Environment.ProcessorCount,
                },
                engine = new { engine.IsAvailable, version = engine.EngineVersion?.ToString(), abi = engine.NativeAbi?.ToString(), engine.Runtime, status = Redact(engine.Status) },
                adapters,
                cache = new
                {
                    directory = Redact(cache.Directory),
                    cache.MemoryHitCount,
                    cache.DiskHitCount,
                    cache.MissCount,
                    cache.WriteCount,
                    cache.EvictionCount,
                    cache.CorruptionCount,
                    cache.MemoryBytes,
                    cache.DiskBytes,
                    cache.MemoryEntryCount,
                    cache.DiskEntryCount,
                    lastEvent = Redact(cache.LastEvent),
                },
                scheduler,
                recentEvents = events,
                errorChain = Redact(errorChain ?? string.Empty),
            };
            JsonSerializerOptions options = new() { WriteIndented = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
            string json = JsonSerializer.Serialize(manifest, options);
            string manifestPath = Path.Combine(staging, "diagnostics.json");
            File.WriteAllText(manifestPath, json);
            File.WriteAllText(Path.Combine(staging, "README.txt"), "XVARNA redacted diagnostic bundle. No model geometry, weather data, environment variables, or user documents are included.\n");
            string hash = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(json))).ToLowerInvariant();
            string archive = Path.Combine(root, $"xvarna-diagnostics-{DateTimeOffset.UtcNow:yyyyMMdd-HHmmss}-{hash[..12]}.zip");
            if (File.Exists(archive)) File.Delete(archive);
            ZipFile.CreateFromDirectory(staging, archive, CompressionLevel.Optimal, false);
            return new(archive, "diagnostics.json", hash);
        }
        finally
        {
            if (Directory.Exists(staging)) Directory.Delete(staging, true);
        }
    }

    private static string Join(string? first, string second) => string.IsNullOrWhiteSpace(first) ? second : $"{first}{Environment.NewLine}{second}";

    private static string Redact(string value)
    {
        string result = Regex.Replace(value, @"(?i)(?:[a-z]:\\|\\\\)[^\s\""'|]+", "<PATH>");
        string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        if (!string.IsNullOrWhiteSpace(profile)) result = result.Replace(profile, "<USER_PROFILE>", StringComparison.OrdinalIgnoreCase);
        string user = Environment.UserName;
        if (!string.IsNullOrWhiteSpace(user)) result = result.Replace(user, "<USER>", StringComparison.OrdinalIgnoreCase);
        return result;
    }
}
