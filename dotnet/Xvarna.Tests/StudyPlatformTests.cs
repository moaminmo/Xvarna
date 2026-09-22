using System.Text.Json;
using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class StudyPlatformTests
{
    [Fact]
    public void ManifestSchemaIsDraft202012AndNativeJsonIsWellFormed()
    {
        using JsonDocument schema = JsonDocument.Parse(StudyPlatformWorkspace.ManifestSchemaJson);
        Assert.Equal("https://json-schema.org/draft/2020-12/schema",
            schema.RootElement.GetProperty("$schema").GetString());
        Assert.Equal("0.16.0", schema.RootElement.GetProperty("properties")
            .GetProperty("schemaVersion").GetProperty("const").GetString());
    }

    [Fact]
    public void WorkspaceResumesBatchAndExportsShareableReportPackage()
    {
        string root = Path.Combine(Path.GetTempPath(), $"xvarna-managed-study-{Guid.NewGuid():N}");
        try
        {
            StudyPlatformManifest manifest = new()
            {
                StudyId = "managed-courtyard",
                Title = "Managed Courtyard Study",
                Description = "Vector objective, checkpoint, delta-map, and report integration test.",
                CreatedUtc = new DateTimeOffset(2026, 9, 2, 12, 0, 0, TimeSpan.Zero),
                Parameters =
                [
                    new() { ParameterId = 1, Name = "depth", Kind = DesignVariableKind.Continuous,
                        LowerBound = 0, UpperBound = 1, Unit = "m" },
                    new() { ParameterId = 2, Name = "facade", Kind = DesignVariableKind.Categorical,
                        LowerBound = 0, UpperBound = 1, Categories = ["clear", "frit"] },
                ],
                Objectives =
                [
                    new() { ObjectiveId = 1, Name = "daylight", Direction = ObjectiveDirection.Maximize,
                        Unit = "%", Components = ["sDA", "UDI"] },
                    new() { ObjectiveId = 2, Name = "energy", Direction = ObjectiveDirection.Minimize,
                        Unit = "kWh" },
                ],
                Constraints = [new() { ConstraintId = 1, Name = "ase-limit", Unit = "%" }],
                BaselineVariantId = 1,
                Optimizer = new(PopulationSize: 4, OffspringSize: 4, ArchiveCapacity: 16, Seed: 17),
            };
            StudyPlatformWorkspace workspace = StudyPlatformWorkspace.Create(root, manifest);
            StudyPlatformBatch batch = workspace.BeginBatch();
            Assert.Equal(4, batch.Candidates.Count);
            Assert.Equal("pending", batch.Status);
            StudyPlatformWorkspace reopened = StudyPlatformWorkspace.Open(root);
            StudyPlatformBatch resumed = reopened.BeginBatch();
            Assert.Equal(batch.BatchId, resumed.BatchId);
            Assert.Equal(batch.ContentHash, resumed.ContentHash);
            StudyPlatformEvaluation[] evaluations = batch.Candidates.Select(candidate =>
            {
                double depth = candidate.Values[0], facade = candidate.Values[1];
                return new StudyPlatformEvaluation
                {
                    VariantId = candidate.CandidateId,
                    ObjectiveValues =
                    [
                        new[] { 50 + depth * 40, 60 + facade * 10 },
                        new[] { 100 - depth * 30 + facade * 5 },
                    ],
                    ConstraintResiduals = [depth - 0.85],
                    MetricFields =
                    [
                        new()
                        {
                            FieldId = "daylight.totalLux", Unit = "lux",
                            Positions = [new[] { 0.0, 0.0, 0.8 }, new[] { 1.0, 0.0, 0.8 }],
                            Values = [depth * 1000, 500 + facade * 100],
                        },
                    ],
                    ElapsedMicroseconds = 20,
                    ProvenanceHash = $"managed-{candidate.CandidateId}",
                };
            }).ToArray();
            StudyWorkspaceState state = reopened.CommitBatch(batch.BatchId, evaluations);
            Assert.Equal<ulong>(1, state.Generation);
            Assert.Equal<ulong>(4, state.CompletedCount);
            Assert.Null(state.ActiveBatchId);
            StudyReportArtifacts report = reopened.GenerateReport(Path.Combine(root, "published"),
                new(BootstrapSamples: 64, PermutationSamples: 64, Seed: 91));
            Assert.Equal<ulong>(4, report.VariantCount);
            Assert.True(File.Exists(report.Report));
            Assert.True(File.Exists(report.Viewer));
            Assert.True(File.Exists(report.Parquet));
            Assert.True(File.Exists(report.Gltf));
            Assert.Equal("PAR1", System.Text.Encoding.ASCII.GetString(File.ReadAllBytes(report.Parquet), 0, 4));
            using JsonDocument gltf = JsonDocument.Parse(File.ReadAllText(report.Gltf));
            Assert.Equal("2.0", gltf.RootElement.GetProperty("asset").GetProperty("version").GetString());
            Assert.Contains("managed-courtyard", File.ReadAllText(report.Viewer), StringComparison.Ordinal);
        }
        finally
        {
            if (Directory.Exists(root)) Directory.Delete(root, recursive: true);
        }
    }
}
