using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Mixed-variable representation used by XVARNA Study.</summary>
public enum DesignVariableKind
{
    /// <summary>Real-valued bounded parameter.</summary>
    Continuous = 0,
    /// <summary>Exactly rounded bounded integer parameter.</summary>
    Integer = 1,
    /// <summary>Zero-based discrete category level.</summary>
    Categorical = 2,
}

/// <summary>Objective preference direction.</summary>
public enum ObjectiveDirection
{
    /// <summary>Smaller objective values are preferred.</summary>
    Minimize = 0,
    /// <summary>Larger objective values are preferred.</summary>
    Maximize = 1,
}

/// <summary>One bounded continuous, integer, or categorical design variable.</summary>
public readonly record struct DesignVariable(
    ulong VariableId, DesignVariableKind Kind, double LowerBound, double UpperBound);

/// <summary>Immutable dimensional contract for ranking and ask/tell optimization.</summary>
public sealed record StudyProblem
{
    /// <summary>Ordered design variables.</summary>
    public required IReadOnlyList<DesignVariable> Variables { get; init; }
    /// <summary>Ordered objective directions.</summary>
    public required IReadOnlyList<ObjectiveDirection> ObjectiveDirections { get; init; }
    /// <summary>Residual constraint count; every residual must be ≤ 0 for feasibility.</summary>
    public int ConstraintCount { get; init; }
}

/// <summary>One design variant.</summary>
public sealed record StudyCandidate
{
    /// <summary>Stable candidate identifier.</summary>
    public required ulong CandidateId { get; init; }
    /// <summary>Generation that produced the variant.</summary>
    public ulong Generation { get; init; }
    /// <summary>Ordered parameter values.</summary>
    public required IReadOnlyList<double> Parameters { get; init; }
}

/// <summary>External objective and residual-constraint evaluation.</summary>
public sealed record StudyEvaluation
{
    /// <summary>Evaluated candidate identifier.</summary>
    public required ulong CandidateId { get; init; }
    /// <summary>Ordered objective vector.</summary>
    public required IReadOnlyList<double> Objectives { get; init; }
    /// <summary>Ordered residual constraints; values ≤ 0 are feasible.</summary>
    public IReadOnlyList<double> ConstraintResiduals { get; init; } = Array.Empty<double>();
}

/// <summary>Constraint-aware non-dominated rank and diversity metadata.</summary>
public readonly record struct RankedStudySolution(
    ulong CandidateId, ulong ParetoRank, bool Feasible, double TotalConstraintViolation,
    double CrowdingDistance, ulong DominatedSolutionCount);

/// <summary>Immutable ranked multi-objective study.</summary>
public sealed record RankedStudyResult
{
    /// <summary>Ranked rows in candidate input order.</summary>
    public required IReadOnlyList<RankedStudySolution> Solutions { get; init; }
    /// <summary>Feasible rank-zero candidate IDs.</summary>
    public required IReadOnlyList<ulong> ParetoCandidateIds { get; init; }
    /// <summary>Number of constraint-dominated fronts.</summary>
    public required ulong FrontCount { get; init; }
    /// <summary>Native analysis runtime.</summary>
    public required ulong AnalysisTimeMicroseconds { get; init; }
    /// <summary>Deterministic ranking identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Descriptive Spearman parameter/objective sensitivity.</summary>
public readonly record struct StudySensitivity(
    ulong VariableIndex, ulong ObjectiveIndex, double SpearmanRho);

/// <summary>Native multi-objective ranking, sensitivity, and hypervolume operations.</summary>
public static class StudyAnalyzer
{
    private const uint VariableSize = 32, RankedSize = 48, MetadataSize = 88, SensitivitySize = 32;

    /// <summary>Ranks a completed study with feasibility-first Pareto dominance.</summary>
    public static unsafe RankedStudyResult Rank(
        StudyProblem problem, IReadOnlyList<StudyCandidate> candidates,
        IReadOnlyList<StudyEvaluation> evaluations)
    {
        StudyInterop.Validate(problem, candidates, evaluations);
        NativeVariableSpec[] variables = StudyInterop.Variables(problem, VariableSize);
        uint[] directions = problem.ObjectiveDirections.Select(value => (uint)value).ToArray();
        StudyInterop.Flatten(problem, candidates, evaluations, out ulong[] ids, out ulong[] generations,
            out double[] parameters, out double[] objectives, out double[] constraints);
        NativeRankedSolution[] ranked = new NativeRankedSolution[candidates.Count];
        ulong[] pareto = new ulong[candidates.Count];
        NativeStudyMetadata metadata = default;
        fixed (NativeVariableSpec* variablePointer = variables)
        fixed (uint* directionPointer = directions)
        fixed (ulong* idPointer = ids)
        fixed (ulong* generationPointer = generations)
        fixed (double* parameterPointer = parameters)
        fixed (double* objectivePointer = objectives)
        fixed (double* constraintPointer = constraints)
        fixed (NativeRankedSolution* rankedPointer = ranked)
        fixed (ulong* paretoPointer = pareto)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.StudyRank(variablePointer, (nuint)variables.Length,
                directionPointer, (nuint)directions.Length, (nuint)problem.ConstraintCount,
                idPointer, generationPointer, parameterPointer, objectivePointer, constraintPointer,
                (nuint)candidates.Count, &metadata, rankedPointer, (nuint)ranked.Length,
                paretoPointer, (nuint)pareto.Length));
        }
        if (metadata.StructureSize != MetadataSize || Marshal.SizeOf<NativeStudyMetadata>() != MetadataSize
            || ranked.Any(value => value.StructureSize != RankedSize))
            throw new InvalidOperationException("Native XVARNA Study ABI layout mismatch.");
        return new()
        {
            Solutions = ranked.Select(StudyInterop.Ranked).ToArray(),
            ParetoCandidateIds = pareto.Take(checked((int)metadata.ParetoCount)).ToArray(),
            FrontCount = metadata.FrontCount,
            AnalysisTimeMicroseconds = metadata.AnalysisTimeMicroseconds,
            ContentHash = StudyInterop.Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    /// <summary>Computes descriptive Spearman coefficients without causal/confidence claims.</summary>
    public static unsafe IReadOnlyList<StudySensitivity> SpearmanSensitivity(
        StudyProblem problem, IReadOnlyList<StudyCandidate> candidates,
        IReadOnlyList<StudyEvaluation> evaluations)
    {
        StudyInterop.Validate(problem, candidates, evaluations);
        NativeVariableSpec[] variables = StudyInterop.Variables(problem, VariableSize);
        uint[] directions = problem.ObjectiveDirections.Select(value => (uint)value).ToArray();
        StudyInterop.Flatten(problem, candidates, evaluations, out ulong[] ids, out _,
            out double[] parameters, out double[] objectives, out double[] constraints);
        NativeSensitivityCoefficient[] coefficients = new NativeSensitivityCoefficient[
            checked(problem.Variables.Count * problem.ObjectiveDirections.Count)];
        fixed (NativeVariableSpec* variablePointer = variables)
        fixed (uint* directionPointer = directions)
        fixed (ulong* idPointer = ids)
        fixed (double* parameterPointer = parameters)
        fixed (double* objectivePointer = objectives)
        fixed (double* constraintPointer = constraints)
        fixed (NativeSensitivityCoefficient* outputPointer = coefficients)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.StudySpearman(variablePointer, (nuint)variables.Length,
                directionPointer, (nuint)directions.Length, (nuint)problem.ConstraintCount,
                idPointer, parameterPointer, objectivePointer, constraintPointer, (nuint)candidates.Count,
                outputPointer, (nuint)coefficients.Length));
        }
        if (coefficients.Any(value => value.StructureSize != SensitivitySize))
            throw new InvalidOperationException("Native XVARNA Study sensitivity ABI layout mismatch.");
        return coefficients.Select(value => new StudySensitivity(
            value.VariableIndex, value.ObjectiveIndex, value.SpearmanRho)).ToArray();
    }

    /// <summary>Computes exact feasible Pareto hypervolume for exactly two objectives.</summary>
    public static unsafe double Hypervolume2d(
        StudyProblem problem, IReadOnlyList<StudyCandidate> candidates,
        IReadOnlyList<StudyEvaluation> evaluations, double referenceFirst, double referenceSecond)
    {
        StudyInterop.Validate(problem, candidates, evaluations);
        if (problem.ObjectiveDirections.Count != 2)
            throw new ArgumentException("Hypervolume2d requires exactly two objectives.", nameof(problem));
        NativeVariableSpec[] variables = StudyInterop.Variables(problem, VariableSize);
        uint[] directions = problem.ObjectiveDirections.Select(value => (uint)value).ToArray();
        StudyInterop.Flatten(problem, candidates, evaluations, out ulong[] ids, out _,
            out double[] parameters, out double[] objectives, out double[] constraints);
        double value = 0.0;
        fixed (NativeVariableSpec* variablePointer = variables)
        fixed (uint* directionPointer = directions)
        fixed (ulong* idPointer = ids)
        fixed (double* parameterPointer = parameters)
        fixed (double* objectivePointer = objectives)
        fixed (double* constraintPointer = constraints)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.StudyHypervolume2d(variablePointer,
                (nuint)variables.Length, directionPointer, (nuint)problem.ConstraintCount,
                idPointer, parameterPointer, objectivePointer, constraintPointer,
                (nuint)candidates.Count, referenceFirst, referenceSecond, &value));
        }
        return value;
    }
}

/// <summary>Constraint-aware NSGA-II ask/tell engine configuration.</summary>
public readonly record struct MultiObjectiveOptimizerOptions(
    int PopulationSize = 64, int OffspringSize = 64, int ArchiveCapacity = 256,
    ulong Seed = 42, double CrossoverProbability = 0.9, double MutationProbability = 0.0,
    double CrossoverDistributionIndex = 15.0, double MutationDistributionIndex = 20.0)
{
    /// <summary>Production defaults.</summary>
    public static MultiObjectiveOptimizerOptions Default { get; } =
        new(64, 64, 256, 42, 0.9, 0.0, 15.0, 20.0);
}

/// <summary>Candidate batch produced for external Grasshopper/CLI evaluation.</summary>
public sealed record OptimizerBatch
{
    /// <summary>Generation label on generated candidates.</summary>
    public required ulong Generation { get; init; }
    /// <summary>Candidate parameter vectors.</summary>
    public required IReadOnlyList<StudyCandidate> Candidates { get; init; }
}

/// <summary>Completed optimizer generation with population and historical Pareto archive.</summary>
public sealed record OptimizerSnapshot
{
    /// <summary>Completed generation count.</summary>
    public required ulong Generation { get; init; }
    /// <summary>Total accepted external evaluations.</summary>
    public required ulong EvaluationCount { get; init; }
    /// <summary>Current parent population ranking.</summary>
    public required RankedStudyResult Population { get; init; }
    /// <summary>Bounded historical Pareto archive IDs.</summary>
    public required IReadOnlyList<ulong> ArchiveCandidateIds { get; init; }
    /// <summary>Finite diversity diagnostic.</summary>
    public required double MeanFiniteCrowdingDistance { get; init; }
    /// <summary>Deterministic optimizer-state identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Safe stateful native mixed-variable constrained multi-objective optimizer.</summary>
public sealed class XvarnaOptimizer : IDisposable
{
    private const uint VariableSize = 32, RankedSize = 48, ConfigSize = 72, MetadataSize = 88;
    private readonly SafeOptimizerHandle handle;
    private readonly StudyProblem problem;
    private readonly MultiObjectiveOptimizerOptions options;
    private StudyCandidate[] pending = [];

    /// <summary>Creates an empty deterministic ask/tell optimizer.</summary>
    public unsafe XvarnaOptimizer(StudyProblem problem, MultiObjectiveOptimizerOptions options)
    {
        ArgumentNullException.ThrowIfNull(problem);
        StudyInterop.ValidateProblem(problem);
        this.problem = problem;
        this.options = options;
        NativeVariableSpec[] variables = StudyInterop.Variables(problem, VariableSize);
        uint[] directions = problem.ObjectiveDirections.Select(value => (uint)value).ToArray();
        NativeOptimizerConfig config = new()
        {
            StructureSize = ConfigSize,
            PopulationSize = checked((ulong)options.PopulationSize),
            OffspringSize = checked((ulong)options.OffspringSize),
            ArchiveCapacity = checked((ulong)options.ArchiveCapacity),
            Seed = options.Seed,
            CrossoverProbability = options.CrossoverProbability,
            MutationProbability = options.MutationProbability,
            CrossoverDistributionIndex = options.CrossoverDistributionIndex,
            MutationDistributionIndex = options.MutationDistributionIndex,
        };
        ulong raw = 0;
        fixed (NativeVariableSpec* variablePointer = variables)
        fixed (uint* directionPointer = directions)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.OptimizerCreate(variablePointer, (nuint)variables.Length,
                directionPointer, (nuint)directions.Length, (nuint)problem.ConstraintCount, &config, &raw));
        }
        handle = new(raw);
    }

    /// <summary>Creates a production-default optimizer.</summary>
    public XvarnaOptimizer(StudyProblem problem) : this(problem, MultiObjectiveOptimizerOptions.Default) { }

    /// <summary>Produces a stratified initial population or evolutionary offspring.</summary>
    public unsafe OptimizerBatch Ask()
    {
        ThrowIfDisposed();
        int capacity = Math.Max(options.PopulationSize, options.OffspringSize);
        ulong[] ids = new ulong[capacity], generations = new ulong[capacity];
        double[] parameters = new double[checked(capacity * problem.Variables.Count)];
        NativeOptimizerMetadata metadata = default;
        fixed (ulong* idPointer = ids)
        fixed (ulong* generationPointer = generations)
        fixed (double* parameterPointer = parameters)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.OptimizerAsk(handle, (nuint)problem.Variables.Count,
                &metadata, idPointer, generationPointer, parameterPointer, (nuint)capacity,
                (nuint)parameters.Length));
        }
        ValidateMetadata(metadata);
        int count = checked((int)metadata.PendingCandidateCount);
        pending = Enumerable.Range(0, count).Select(index => new StudyCandidate
        {
            CandidateId = ids[index],
            Generation = generations[index],
            Parameters = parameters.Skip(index * problem.Variables.Count).Take(problem.Variables.Count).ToArray(),
        }).ToArray();
        return new() { Generation = pending.FirstOrDefault()?.Generation ?? 0, Candidates = pending };
    }

    /// <summary>Completes every pending candidate evaluation and advances one generation.</summary>
    public unsafe OptimizerSnapshot Tell(IReadOnlyList<StudyEvaluation> evaluations)
    {
        ArgumentNullException.ThrowIfNull(evaluations);
        ThrowIfDisposed();
        if (pending.Length == 0 || evaluations.Count != pending.Length)
            throw new InvalidOperationException("Tell requires exactly one evaluation for every pending candidate.");
        Dictionary<ulong, StudyEvaluation> map = evaluations.ToDictionary(value => value.CandidateId);
        if (map.Count != evaluations.Count || pending.Any(candidate => !map.ContainsKey(candidate.CandidateId)))
            throw new ArgumentException("Evaluation IDs must match the current optimizer batch exactly.", nameof(evaluations));
        StudyEvaluation[] ordered = pending.Select(candidate => map[candidate.CandidateId]).ToArray();
        ulong[] ids = pending.Select(candidate => candidate.CandidateId).ToArray();
        double[] objectives = ordered.SelectMany(value => value.Objectives).ToArray();
        double[] constraints = ordered.SelectMany(value => value.ConstraintResiduals).ToArray();
        if (ordered.Any(value => value.Objectives.Count != problem.ObjectiveDirections.Count
            || value.ConstraintResiduals.Count != problem.ConstraintCount))
            throw new ArgumentException("Objective or constraint vector dimensions do not match the problem.", nameof(evaluations));
        NativeRankedSolution[] population = new NativeRankedSolution[options.PopulationSize];
        ulong[] archive = new ulong[options.ArchiveCapacity];
        NativeOptimizerMetadata metadata = default;
        fixed (ulong* idPointer = ids)
        fixed (double* objectivePointer = objectives)
        fixed (double* constraintPointer = constraints)
        fixed (NativeRankedSolution* populationPointer = population)
        fixed (ulong* archivePointer = archive)
        {
            StudyInterop.ThrowIfFailed(NativeMethods.OptimizerTell(handle,
                (nuint)problem.ObjectiveDirections.Count, (nuint)problem.ConstraintCount,
                idPointer, objectivePointer, constraintPointer, (nuint)ids.Length, &metadata,
                populationPointer, (nuint)population.Length, archivePointer, (nuint)archive.Length));
        }
        ValidateMetadata(metadata);
        int populationCount = checked((int)metadata.PopulationCount);
        RankedStudySolution[] ranked = population.Take(populationCount).Select(value =>
        {
            if (value.StructureSize != RankedSize) throw new InvalidOperationException("Native optimizer rank layout mismatch.");
            return StudyInterop.Ranked(value);
        }).ToArray();
        string hash = StudyInterop.Hash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3);
        pending = [];
        return new()
        {
            Generation = metadata.Generation,
            EvaluationCount = metadata.EvaluationCount,
            ArchiveCandidateIds = archive.Take(checked((int)metadata.ArchiveCount)).ToArray(),
            MeanFiniteCrowdingDistance = metadata.MeanFiniteCrowdingDistance,
            ContentHash = hash,
            Population = new()
            {
                Solutions = ranked,
                ParetoCandidateIds = ranked.Where(value => value.Feasible && value.ParetoRank == 0).Select(value => value.CandidateId).Order().ToArray(),
                FrontCount = ranked.Length == 0 ? 0 : ranked.Max(value => value.ParetoRank) + 1,
                AnalysisTimeMicroseconds = 0,
                ContentHash = hash,
            },
        };
    }

    /// <inheritdoc />
    public void Dispose() { handle.Dispose(); GC.SuppressFinalize(this); }

    private void ThrowIfDisposed()
    {
        if (handle.IsClosed || handle.IsInvalid) throw new ObjectDisposedException(nameof(XvarnaOptimizer));
    }

    private static void ValidateMetadata(NativeOptimizerMetadata metadata)
    {
        if (metadata.StructureSize != MetadataSize || Marshal.SizeOf<NativeOptimizerMetadata>() != MetadataSize)
            throw new InvalidOperationException("Native optimizer metadata ABI layout mismatch.");
    }
}

internal sealed class SafeOptimizerHandle : SafeHandle
{
    internal SafeOptimizerHandle(ulong value) : base(nint.Zero, true) => SetHandle(checked((nint)value));
    public override bool IsInvalid => handle == nint.Zero;
    protected override bool ReleaseHandle() => NativeMethods.OptimizerRelease(unchecked((ulong)handle)) == NativeStatus.Success;
}

internal static class StudyInterop
{
    internal static NativeVariableSpec[] Variables(StudyProblem problem, uint size) => problem.Variables.Select(value => new NativeVariableSpec
    {
        StructureSize = size,
        Kind = (uint)value.Kind,
        VariableId = value.VariableId,
        LowerBound = value.LowerBound,
        UpperBound = value.UpperBound,
    }).ToArray();

    internal static void ValidateProblem(StudyProblem problem)
    {
        if (problem.Variables is null || problem.Variables.Count == 0
            || problem.ObjectiveDirections is null || problem.ObjectiveDirections.Count == 0
            || problem.ConstraintCount < 0)
            throw new ArgumentException("Study problem dimensions must be non-empty and non-negative.", nameof(problem));
    }

    internal static void Validate(StudyProblem problem, IReadOnlyList<StudyCandidate> candidates,
        IReadOnlyList<StudyEvaluation> evaluations)
    {
        ArgumentNullException.ThrowIfNull(problem); ArgumentNullException.ThrowIfNull(candidates); ArgumentNullException.ThrowIfNull(evaluations);
        ValidateProblem(problem);
        if (candidates.Count == 0 || candidates.Count != evaluations.Count)
            throw new ArgumentException("Candidate and evaluation tables must be non-empty and aligned.");
        if (candidates.Any(value => value.Parameters.Count != problem.Variables.Count)
            || evaluations.Any(value => value.Objectives.Count != problem.ObjectiveDirections.Count
                || value.ConstraintResiduals.Count != problem.ConstraintCount))
            throw new ArgumentException("Parameter, objective, or constraint vector dimensions do not match the problem.");
    }

    internal static void Flatten(StudyProblem problem, IReadOnlyList<StudyCandidate> candidates,
        IReadOnlyList<StudyEvaluation> evaluations, out ulong[] ids, out ulong[] generations,
        out double[] parameters, out double[] objectives, out double[] constraints)
    {
        Dictionary<ulong, StudyEvaluation> evaluationMap = evaluations.ToDictionary(value => value.CandidateId);
        ids = candidates.Select(value => value.CandidateId).ToArray();
        generations = candidates.Select(value => value.Generation).ToArray();
        parameters = candidates.SelectMany(value => value.Parameters).ToArray();
        objectives = candidates.SelectMany(value => evaluationMap[value.CandidateId].Objectives).ToArray();
        constraints = candidates.SelectMany(value => evaluationMap[value.CandidateId].ConstraintResiduals).ToArray();
    }

    internal static RankedStudySolution Ranked(NativeRankedSolution value) => new(
        value.CandidateId, value.ParetoRank, (value.Flags & 1U) != 0,
        value.TotalConstraintViolation, value.CrowdingDistance, value.DominatedSolutionCount);

    internal static string Hash(ulong first, ulong second, ulong third, ulong fourth) =>
        SolarConversions.FormatHash(first, second, third, fourth);

    internal static void ThrowIfFailed(NativeStatus status)
    {
        if (status != NativeStatus.Success) throw new XvarnaNativeException(status);
    }
}
