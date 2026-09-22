using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

// Public record properties mirror the documented, versioned JSON field names below.
#pragma warning disable CS1591

namespace Xvarna.Native;

/// <summary>Fast interactive or costly reference evaluation fidelity.</summary>
public enum EvidenceFidelity { Fast, Reference }

/// <summary>Generated global-sensitivity design family.</summary>
public enum GlobalSensitivityMethod { SaltelliJansen, Morris }

/// <summary>Conservative constraint feasibility at a declared confidence interval.</summary>
public enum FeasibilityConfidence { Proven, Possible, Infeasible }

/// <summary>CPU/GPU and external-reference validation attached to a result.</summary>
public sealed record EvidenceValidation
{
    public bool ParityTested { get; init; }
    public double? MaximumParityDelta { get; init; }
    public bool ReferenceTested { get; init; }
    public string ReferenceMethod { get; init; } = string.Empty;
    public double? MaximumReferenceError { get; init; }
    public double? AcceptedFraction { get; init; }
}

/// <summary>Hash-protected scientific provenance for one computed result.</summary>
public sealed record EvidencePassport
{
    public string SchemaVersion { get; init; } = "xvarna.evidence-passport/1";
    public required string ResultHash { get; init; }
    public required string SceneHash { get; init; }
    public string StudyId { get; init; } = string.Empty;
    public ulong? VariantId { get; init; }
    public required string Method { get; init; }
    public required EvidenceFidelity Fidelity { get; init; }
    public string EngineVersion { get; init; } = "0.19.0";
    public required string Backend { get; init; }
    public string Device { get; init; } = string.Empty;
    public string Driver { get; init; } = string.Empty;
    public DateTimeOffset GeneratedUtc { get; init; } = DateTimeOffset.UtcNow;
    public ulong? Seed { get; init; }
    public ulong SampleCount { get; init; }
    public double? ConvergenceDelta { get; init; }
    public ulong ElapsedMicroseconds { get; init; }
    public IReadOnlyDictionary<string, string> InputHashes { get; init; } = new Dictionary<string, string>();
    public IReadOnlyList<string> Assumptions { get; init; } = Array.Empty<string>();
    public IReadOnlyList<string> Limitations { get; init; } = Array.Empty<string>();
    public IReadOnlyList<string> Citations { get; init; } = Array.Empty<string>();
    public EvidenceValidation Validation { get; init; } = new();
    public string ContentHash { get; init; } = string.Empty;
}

/// <summary>One row in a generated, externally evaluable sensitivity design.</summary>
public sealed record SensitivityDesignRow
{
    public required int RowIndex { get; init; }
    public required IReadOnlyList<double> Values { get; init; }
    public int? Trajectory { get; init; }
    public int? ChangedParameter { get; init; }
}

/// <summary>Versioned and hash-protected Saltelli/Jansen or Morris design.</summary>
public sealed record GlobalSensitivityDesign
{
    public required string SchemaVersion { get; init; }
    public required GlobalSensitivityMethod Method { get; init; }
    public required IReadOnlyList<DesignVariable> Variables { get; init; }
    public required int BaseSamples { get; init; }
    public required int Levels { get; init; }
    public required ulong Seed { get; init; }
    public required IReadOnlyList<SensitivityDesignRow> Rows { get; init; }
    public required string ContentHash { get; init; }
}

/// <summary>Saltelli first-order and Jansen total-order index.</summary>
public sealed record SobolIndex(int ParameterIndex, int OutputIndex, double FirstOrder,
    double TotalOrder, double Variance, int BaseSamples);

/// <summary>Morris elementary-effect μ, μ* and σ statistics.</summary>
public sealed record MorrisEffect(int ParameterIndex, int OutputIndex, double Mean,
    double MeanAbsolute, double StandardDeviation, int Trajectories);

/// <summary>Hash-linked global-sensitivity result.</summary>
public sealed record GlobalSensitivityResult
{
    public required string DesignHash { get; init; }
    public required GlobalSensitivityMethod Method { get; init; }
    public required int OutputCount { get; init; }
    public IReadOnlyList<SobolIndex> Sobol { get; init; } = Array.Empty<SobolIndex>();
    public IReadOnlyList<MorrisEffect> Morris { get; init; } = Array.Empty<MorrisEffect>();
    public required string ContentHash { get; init; }
}

/// <summary>One weighted climate/material/operation scenario outcome.</summary>
public sealed record ScenarioOutcome
{
    public required ulong CandidateId { get; init; }
    public required ulong ScenarioId { get; init; }
    public required double Probability { get; init; }
    public required IReadOnlyList<double> Objectives { get; init; }
    public IReadOnlyList<double> ConstraintResiduals { get; init; } = Array.Empty<double>();
}

/// <summary>Direction-aware robust statistics for one candidate.</summary>
public sealed record RobustScenarioSummary
{
    public required ulong CandidateId { get; init; }
    public required int ScenarioCount { get; init; }
    public required IReadOnlyList<double> MeanObjectives { get; init; }
    public required IReadOnlyList<double> StandardDeviations { get; init; }
    public required IReadOnlyList<double> WorstObjectives { get; init; }
    public required IReadOnlyList<double> CvarObjectives { get; init; }
    public required double ViolationProbability { get; init; }
}

/// <summary>Noisy objective and constraint estimate for one candidate.</summary>
public sealed record UncertainEvaluation
{
    public required ulong CandidateId { get; init; }
    public required IReadOnlyList<double> ObjectiveMeans { get; init; }
    public required IReadOnlyList<double> ObjectiveStandardDeviations { get; init; }
    public IReadOnlyList<double> ConstraintMeans { get; init; } = Array.Empty<double>();
    public IReadOnlyList<double> ConstraintStandardDeviations { get; init; } = Array.Empty<double>();
}

/// <summary>Conservative interval-Pareto result for one uncertain evaluation.</summary>
public sealed record UncertaintyRank
{
    public required ulong CandidateId { get; init; }
    public required int RobustParetoRank { get; init; }
    public required FeasibilityConfidence Feasibility { get; init; }
    public required int DominatedCount { get; init; }
    public required IReadOnlyList<double> LowerObjectiveBounds { get; init; }
    public required IReadOnlyList<double> UpperObjectiveBounds { get; init; }
}

/// <summary>Candidate identity and parameter vector for multi-fidelity selection.</summary>
public sealed record ScientificCandidate
{
    public required ulong CandidateId { get; init; }
    public ulong Generation { get; init; }
    public required IReadOnlyList<double> Values { get; init; }
}

/// <summary>One Fast or Reference result used by fidelity calibration.</summary>
public sealed record FidelityObservation
{
    public required ulong CandidateId { get; init; }
    public required EvidenceFidelity Fidelity { get; init; }
    public required IReadOnlyList<double> Objectives { get; init; }
    public required double Cost { get; init; }
    public required IReadOnlyList<double> NoiseStandardDeviations { get; init; }
    public required string EvidenceHash { get; init; }
}

/// <summary>Cost-aware Monte-Carlo reference-selection settings.</summary>
public sealed record MultiFidelityPolicy
{
    public int BatchSize { get; init; } = 1;
    public int MonteCarloSamples { get; init; } = 2048;
    public ulong Seed { get; init; } = 42;
    public required IReadOnlyList<double> ReferencePoint { get; init; }
    public double MinimumStandardDeviation { get; init; } = 1e-6;
}

/// <summary>Per-objective linear Fast-to-Reference discrepancy model.</summary>
public sealed record FidelityCalibration(int ObjectiveIndex, int PairedSamples, double Intercept,
    double Slope, double ResidualStandardDeviation, double RSquared);

/// <summary>Transparent value/cost score for one recommended reference run.</summary>
public sealed record FidelityRecommendation
{
    public required ulong CandidateId { get; init; }
    public required IReadOnlyList<double> PredictedReferenceMean { get; init; }
    public required IReadOnlyList<double> PredictedReferenceStandardDeviation { get; init; }
    public required double ExpectedHypervolumeImprovement { get; init; }
    public required double ExpectedCost { get; init; }
    public required double AcquisitionPerCost { get; init; }
}

/// <summary>Calibrations and ordered expensive-reference recommendations.</summary>
public sealed record MultiFidelitySelection
{
    public required IReadOnlyList<FidelityCalibration> Calibrations { get; init; }
    public required IReadOnlyList<FidelityRecommendation> Recommendations { get; init; }
    public required IReadOnlyList<ulong> PairedCandidateIds { get; init; }
    public required string ContentHash { get; init; }
}

/// <summary>Native RASHNU evidence, global sensitivity, robustness, and fidelity engine.</summary>
public static unsafe class EvidenceEngine
{
    private static readonly JsonSerializerOptions Json = CreateJsonOptions();

    public static EvidencePassport Seal(EvidencePassport passport) =>
        Invoke<EvidencePassport>(passport, NativeMethods.EvidencePassportSealJson);

    public static GlobalSensitivityDesign CreateSensitivityDesign(
        IReadOnlyList<DesignVariable> variables, GlobalSensitivityMethod method,
        int baseSamples, int levels = 6, ulong seed = 42) =>
        Invoke<GlobalSensitivityDesign>(new { variables, method, baseSamples, levels, seed },
            NativeMethods.SensitivityDesignJson);

    public static GlobalSensitivityResult AnalyzeSensitivity(
        GlobalSensitivityDesign design, IReadOnlyList<IReadOnlyList<double>> outputs) =>
        Invoke<GlobalSensitivityResult>(new { design, outputs }, NativeMethods.SensitivityAnalyzeJson);

    public static IReadOnlyList<RobustScenarioSummary> AggregateScenarios(
        IReadOnlyList<ObjectiveDirection> objectiveDirections, int constraintCount,
        IReadOnlyList<ScenarioOutcome> outcomes, double cvarAlpha = 0.95) =>
        Invoke<RobustScenarioSummary[]>(new { objectiveDirections, constraintCount, cvarAlpha, outcomes },
            NativeMethods.RobustScenariosJson);

    public static IReadOnlyList<UncertaintyRank> RankUncertainty(
        IReadOnlyList<ObjectiveDirection> objectiveDirections, int constraintCount,
        IReadOnlyList<UncertainEvaluation> evaluations, double intervalMultiplier = 1.96) =>
        Invoke<UncertaintyRank[]>(new { objectiveDirections, constraintCount, intervalMultiplier, evaluations },
            NativeMethods.UncertaintyRankJson);

    public static MultiFidelitySelection RecommendReferenceEvaluations(
        IReadOnlyList<ObjectiveDirection> objectiveDirections,
        IReadOnlyList<ScientificCandidate> candidates,
        IReadOnlyList<FidelityObservation> observations, MultiFidelityPolicy policy) =>
        Invoke<MultiFidelitySelection>(new { objectiveDirections, candidates, observations, policy },
            NativeMethods.MultiFidelityRecommendJson);

    private unsafe delegate NativeStatus JsonOperation(byte* input, nuint inputLength,
        byte* output, nuint outputCapacity, nuint* requiredBytes);

    private static T Invoke<T>(object input, JsonOperation operation)
    {
        ArgumentNullException.ThrowIfNull(input);
        byte[] encoded = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(input, Json));
        nuint required = 0;
        fixed (byte* inputPointer = encoded)
            StudyInterop.ThrowIfFailed(operation(inputPointer, (nuint)encoded.Length, null, 0, &required));
        if (required is 0 or > 512 * 1024 * 1024)
            throw new InvalidOperationException("Native evidence JSON sizing is invalid.");
        byte[] output = new byte[checked((int)required)];
        fixed (byte* inputPointer = encoded)
        fixed (byte* outputPointer = output)
            StudyInterop.ThrowIfFailed(operation(inputPointer, (nuint)encoded.Length,
                outputPointer, required, &required));
        if (required != (nuint)output.Length || output[^1] != 0)
            throw new InvalidOperationException("Native evidence UTF-8 contract mismatch.");
        return JsonSerializer.Deserialize<T>(output.AsSpan(0, output.Length - 1), Json)
            ?? throw new InvalidOperationException("Native evidence JSON result was empty.");
    }

    private static JsonSerializerOptions CreateJsonOptions()
    {
        JsonSerializerOptions options = new(JsonSerializerDefaults.Web)
        {
            PropertyNameCaseInsensitive = false,
            UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        };
        options.Converters.Add(new JsonStringEnumConverter(JsonNamingPolicy.CamelCase));
        return options;
    }
}
