using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Xvarna.Native;

/// <summary>Named parameter registry entry in a versioned Study Manifest.</summary>
public sealed record StudyParameterDefinition
{
    /// <summary>Stable non-zero numeric ID.</summary>
    public required ulong ParameterId { get; init; }
    /// <summary>Stable machine/display name.</summary>
    public required string Name { get; init; }
    /// <summary>Continuous, integer, or categorical.</summary>
    public required DesignVariableKind Kind { get; init; }
    /// <summary>Inclusive lower bound.</summary>
    public required double LowerBound { get; init; }
    /// <summary>Inclusive upper bound.</summary>
    public required double UpperBound { get; init; }
    /// <summary>Display/engineering unit.</summary>
    public string Unit { get; init; } = string.Empty;
    /// <summary>Labels for categorical levels; empty for numeric parameters.</summary>
    public IReadOnlyList<string> Categories { get; init; } = Array.Empty<string>();
}

/// <summary>Named scalar or componentwise-vector objective.</summary>
public sealed record StudyObjectiveDefinition
{
    /// <summary>Stable non-zero objective ID.</summary>
    public required ulong ObjectiveId { get; init; }
    /// <summary>Stable machine/display name.</summary>
    public required string Name { get; init; }
    /// <summary>Minimize or maximize all scalar components.</summary>
    public required ObjectiveDirection Direction { get; init; }
    /// <summary>Display/engineering unit.</summary>
    public string Unit { get; init; } = string.Empty;
    /// <summary>Empty means scalar; names define a componentwise vector.</summary>
    public IReadOnlyList<string> Components { get; init; } = Array.Empty<string>();
}

/// <summary>Named residual constraint; values are feasible at or below zero.</summary>
public sealed record StudyConstraintDefinition
{
    /// <summary>Stable non-zero constraint ID.</summary>
    public required ulong ConstraintId { get; init; }
    /// <summary>Stable machine/display name.</summary>
    public required string Name { get; init; }
    /// <summary>Display/engineering unit.</summary>
    public string Unit { get; init; } = string.Empty;
}

/// <summary>Versioned registry and optimizer policy for a persistent VAHMAN workspace.</summary>
public sealed record StudyPlatformManifest
{
    /// <summary>Current manifest schema.</summary>
    public string SchemaVersion { get; init; } = "0.16.0";
    /// <summary>Stable project/study identity.</summary>
    public required string StudyId { get; init; }
    /// <summary>Human-readable title.</summary>
    public required string Title { get; init; }
    /// <summary>Scope and design intent.</summary>
    public string Description { get; init; } = string.Empty;
    /// <summary>UTC creation time.</summary>
    public DateTimeOffset CreatedUtc { get; init; } = DateTimeOffset.UtcNow;
    /// <summary>Creating XVARNA version.</summary>
    public string EngineVersion { get; init; } = "0.16.0";
    /// <summary>Ordered parameter registry.</summary>
    public required IReadOnlyList<StudyParameterDefinition> Parameters { get; init; }
    /// <summary>Ordered scalar/vector objective registry.</summary>
    public required IReadOnlyList<StudyObjectiveDefinition> Objectives { get; init; }
    /// <summary>Ordered residual constraints.</summary>
    public IReadOnlyList<StudyConstraintDefinition> Constraints { get; init; } = Array.Empty<StudyConstraintDefinition>();
    /// <summary>Optional baseline variant for delta comparison.</summary>
    public ulong? BaselineVariantId { get; init; }
    /// <summary>Deterministic ask/tell policy.</summary>
    public MultiObjectiveOptimizerOptions Optimizer { get; init; } = new();
    /// <summary>Preserved project metadata.</summary>
    public IReadOnlyDictionary<string, string> Metadata { get; init; } = new Dictionary<string, string>();
}

/// <summary>Exact candidate stored in a persistent external-evaluation batch.</summary>
public sealed record StudyPlatformCandidate
{
    /// <summary>Stable variant ID.</summary>
    public required ulong CandidateId { get; init; }
    /// <summary>Optimizer generation.</summary>
    public required ulong Generation { get; init; }
    /// <summary>Ordered manifest parameter values.</summary>
    public required IReadOnlyList<double> Values { get; init; }
}

/// <summary>Optional indexed/spatial field used to compute baseline delta maps.</summary>
public sealed record StudyMetricField
{
    /// <summary>Stable metric identity.</summary>
    public required string FieldId { get; init; }
    /// <summary>Display unit.</summary>
    public string Unit { get; init; } = string.Empty;
    /// <summary>Optional aligned world positions, each containing x/y/z.</summary>
    public IReadOnlyList<double[]> Positions { get; init; } = Array.Empty<double[]>();
    /// <summary>Aligned scalar values.</summary>
    public required IReadOnlyList<double> Values { get; init; }
}

/// <summary>Rich result for one candidate in a pending platform batch.</summary>
public sealed record StudyPlatformEvaluation
{
    /// <summary>Pending variant ID.</summary>
    public required ulong VariantId { get; init; }
    /// <summary>One scalar/vector array per manifest objective.</summary>
    public required IReadOnlyList<IReadOnlyList<double>> ObjectiveValues { get; init; }
    /// <summary>Residual constraint values.</summary>
    public IReadOnlyList<double> ConstraintResiduals { get; init; } = Array.Empty<double>();
    /// <summary>Optional result fields for scenario delta maps.</summary>
    public IReadOnlyList<StudyMetricField> MetricFields { get; init; } = Array.Empty<StudyMetricField>();
    /// <summary>External runtime in microseconds.</summary>
    public ulong ElapsedMicroseconds { get; init; }
    /// <summary>Upstream scene/result identity.</summary>
    public string ProvenanceHash { get; init; } = string.Empty;
}

/// <summary>Durable external-evaluation batch.</summary>
public sealed record StudyPlatformBatch
{
    /// <summary>Platform schema.</summary>
    public required string SchemaVersion { get; init; }
    /// <summary>Monotonic batch ID.</summary>
    public required ulong BatchId { get; init; }
    /// <summary>Candidate generation.</summary>
    public required ulong Generation { get; init; }
    /// <summary>Pending/completed state.</summary>
    public required string Status { get; init; }
    /// <summary>Exact candidates.</summary>
    public required IReadOnlyList<StudyPlatformCandidate> Candidates { get; init; }
    /// <summary>Envelope identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Observable persistent workspace/checkpoint state.</summary>
public sealed record StudyWorkspaceState
{
    /// <summary>Monotonic transaction revision.</summary>
    public required ulong Revision { get; init; }
    /// <summary>Completed generations.</summary>
    public required ulong Generation { get; init; }
    /// <summary>Accepted evaluations.</summary>
    public required ulong EvaluationCount { get; init; }
    /// <summary>Pending batch ID.</summary>
    public ulong? ActiveBatchId { get; init; }
    /// <summary>Exact pending candidates.</summary>
    public required IReadOnlyList<StudyPlatformCandidate> PendingCandidates { get; init; }
    /// <summary>Registered variant count.</summary>
    public required ulong VariantCount { get; init; }
    /// <summary>Completed evaluation count.</summary>
    public required ulong CompletedCount { get; init; }
    /// <summary>Ledger identity.</summary>
    public required string LedgerHash { get; init; }
    /// <summary>Checkpoint identity.</summary>
    public required string CheckpointHash { get; init; }
}

/// <summary>Bootstrap/permutation report policy.</summary>
public readonly record struct StudyReportOptions(
    int BootstrapSamples = 1000, int PermutationSamples = 1000,
    double ConfidenceLevel = 0.95, double Alpha = 0.05,
    ulong Seed = 0x585641524E4116UL)
{
    /// <summary>Production statistical policy used when no override is supplied.</summary>
    public static StudyReportOptions Default { get; } = new(1000, 1000, 0.95, 0.05, 0x585641524E4116UL);
}

/// <summary>Paths and identity of a generated report package.</summary>
public sealed record StudyReportArtifacts
{
    /// <summary>Platform schema.</summary>
    public required string SchemaVersion { get; init; }
    /// <summary>Derived analysis identity.</summary>
    public required string AnalysisHash { get; init; }
    /// <summary>Completed variant count.</summary>
    public required ulong VariantCount { get; init; }
    /// <summary>Feasible Pareto count.</summary>
    public required ulong ParetoCount { get; init; }
    /// <summary>Self-contained narrative HTML.</summary>
    public required string Report { get; init; }
    /// <summary>Self-contained interactive viewer.</summary>
    public required string Viewer { get; init; }
    /// <summary>Apache Parquet table.</summary>
    public required string Parquet { get; init; }
    /// <summary>glTF 2.0 objective-space asset.</summary>
    public required string Gltf { get; init; }
    /// <summary>Complete portable JSON.</summary>
    public required string Data { get; init; }
}

/// <summary>Transactional versioned Study workspace backed by the native Rust engine.</summary>
public sealed unsafe class StudyPlatformWorkspace
{
    private const uint ReportOptionsSize = 48;
    private static readonly JsonSerializerOptions Json = CreateJsonOptions();

    private StudyPlatformWorkspace(string root, StudyWorkspaceState state)
    {
        Root = root;
        State = state;
    }

    /// <summary>Absolute workspace directory.</summary>
    public string Root { get; }
    /// <summary>Most recently observed checkpoint state.</summary>
    public StudyWorkspaceState State { get; private set; }

    /// <summary>Returns the normative Draft 2020-12 Study Manifest schema JSON.</summary>
    public static string ManifestSchemaJson => NativeText((output, capacity, required) =>
        NativeMethods.StudyManifestSchemaJson(output, capacity, required));

    /// <summary>Creates a workspace, or idempotently opens it when the exact manifest already exists.</summary>
    public static StudyPlatformWorkspace Create(string root, StudyPlatformManifest manifest)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        ArgumentNullException.ThrowIfNull(manifest);
        return CreateJson(root, SerializeManifest(manifest));
    }

    /// <summary>Serializes a strongly typed manifest with the normative camel-case/string-enum contract.</summary>
    public static string SerializeManifest(StudyPlatformManifest manifest)
    {
        ArgumentNullException.ThrowIfNull(manifest);
        return JsonSerializer.Serialize(manifest, Json);
    }

    /// <summary>Creates/idempotently opens a workspace from a schema-valid manifest JSON document.</summary>
    public static StudyPlatformWorkspace CreateJson(string root, string manifestJson)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        ArgumentException.ThrowIfNullOrWhiteSpace(manifestJson);
        byte[] rootBytes = Encoding.UTF8.GetBytes(Path.GetFullPath(root));
        byte[] manifestBytes = Encoding.UTF8.GetBytes(manifestJson);
        string result = NativeText((output, capacity, required) =>
        {
            unsafe
            {
                fixed (byte* rootPointer = rootBytes)
                fixed (byte* manifestPointer = manifestBytes)
                    return NativeMethods.StudyWorkspaceCreate(rootPointer, (nuint)rootBytes.Length,
                        manifestPointer, (nuint)manifestBytes.Length, output, capacity, required);
            }
        });
        return new(Path.GetFullPath(root), Deserialize<StudyWorkspaceState>(result));
    }

    /// <summary>Opens and validates a workspace, recovering a journaled interrupted transaction.</summary>
    public static StudyPlatformWorkspace Open(string root)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        string full = Path.GetFullPath(root);
        byte[] rootBytes = Encoding.UTF8.GetBytes(full);
        string result = NativeText((output, capacity, required) =>
        {
            unsafe
            {
                fixed (byte* rootPointer = rootBytes)
                    return NativeMethods.StudyWorkspaceStatus(rootPointer, (nuint)rootBytes.Length,
                        output, capacity, required);
            }
        });
        return new(full, Deserialize<StudyWorkspaceState>(result));
    }

    /// <summary>Refreshes validated checkpoint/ledger state.</summary>
    public StudyWorkspaceState Refresh()
    {
        StudyPlatformWorkspace refreshed = Open(Root);
        State = refreshed.State;
        return State;
    }

    /// <summary>Resumes the active batch or transactionally starts and checkpoints the next batch.</summary>
    public StudyPlatformBatch BeginBatch()
    {
        byte[] rootBytes = Encoding.UTF8.GetBytes(Root);
        string result = NativeText((output, capacity, required) =>
        {
            unsafe
            {
                fixed (byte* rootPointer = rootBytes)
                    return NativeMethods.StudyWorkspaceBeginBatch(rootPointer, (nuint)rootBytes.Length,
                        output, capacity, required);
            }
        });
        StudyPlatformBatch batch = Deserialize<StudyPlatformBatch>(result);
        Refresh();
        return batch;
    }

    /// <summary>Atomically and idempotently commits exactly one result per pending candidate.</summary>
    public StudyWorkspaceState CommitBatch(
        ulong batchId, IReadOnlyList<StudyPlatformEvaluation> evaluations)
    {
        ArgumentNullException.ThrowIfNull(evaluations);
        byte[] rootBytes = Encoding.UTF8.GetBytes(Root);
        byte[] input = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new { batchId, evaluations }, Json));
        string result = NativeText((output, capacity, required) =>
        {
            unsafe
            {
                fixed (byte* rootPointer = rootBytes)
                fixed (byte* inputPointer = input)
                    return NativeMethods.StudyWorkspaceCommitBatch(rootPointer, (nuint)rootBytes.Length,
                        inputPointer, (nuint)input.Length, output, capacity, required);
            }
        });
        State = Deserialize<StudyWorkspaceState>(result);
        return State;
    }

    /// <summary>Builds JSON, Parquet, glTF, self-contained HTML report, and standalone viewer.</summary>
    public StudyReportArtifacts GenerateReport(string outputDirectory, StudyReportOptions options = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(outputDirectory);
        if (options == default) options = StudyReportOptions.Default;
        byte[] rootBytes = Encoding.UTF8.GetBytes(Root);
        byte[] outputBytes = Encoding.UTF8.GetBytes(Path.GetFullPath(outputDirectory));
        string result = NativeText((output, capacity, required) =>
        {
            unsafe
            {
                NativeStudyReportOptions native = new()
                {
                    StructureSize = ReportOptionsSize,
                    BootstrapSamples = checked((ulong)options.BootstrapSamples),
                    PermutationSamples = checked((ulong)options.PermutationSamples),
                    ConfidenceLevel = options.ConfidenceLevel,
                    Alpha = options.Alpha,
                    Seed = options.Seed,
                };
                if (Marshal.SizeOf<NativeStudyReportOptions>() != ReportOptionsSize)
                    throw new InvalidOperationException("Native Study report option layout mismatch.");
                fixed (byte* rootPointer = rootBytes)
                fixed (byte* outputDirectoryPointer = outputBytes)
                    return NativeMethods.StudyWorkspaceReport(rootPointer, (nuint)rootBytes.Length,
                        outputDirectoryPointer, (nuint)outputBytes.Length, &native,
                        output, capacity, required);
            }
        });
        return Deserialize<StudyReportArtifacts>(result);
    }

    private unsafe delegate NativeStatus TextOperation(byte* output, nuint capacity, nuint* required);

    private static unsafe string NativeText(TextOperation operation)
    {
        nuint required = 0;
        StudyInterop.ThrowIfFailed(operation(null, 0, &required));
        if (required is 0 or > 512 * 1024 * 1024)
            throw new InvalidOperationException("Native Study text sizing is invalid.");
        byte[] output = new byte[checked((int)required)];
        fixed (byte* outputPointer = output)
            StudyInterop.ThrowIfFailed(operation(outputPointer, required, &required));
        if (required != (nuint)output.Length || output[^1] != 0)
            throw new InvalidOperationException("Native Study UTF-8 contract mismatch.");
        return Encoding.UTF8.GetString(output, 0, output.Length - 1);
    }

    private static T Deserialize<T>(string json) =>
        JsonSerializer.Deserialize<T>(json, Json)
        ?? throw new InvalidOperationException("Native Study JSON result was empty.");

    private static JsonSerializerOptions CreateJsonOptions()
    {
        JsonSerializerOptions options = new(JsonSerializerDefaults.Web)
        {
            PropertyNameCaseInsensitive = false,
            WriteIndented = true,
            UnmappedMemberHandling = JsonUnmappedMemberHandling.Skip,
        };
        options.Converters.Add(new JsonStringEnumConverter(JsonNamingPolicy.CamelCase));
        return options;
    }
}
