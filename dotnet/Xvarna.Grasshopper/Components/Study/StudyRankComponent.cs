using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Study;

/// <summary>Ranks completed variants with native constraint-aware Pareto analysis.</summary>
public sealed class StudyRankComponent : ScheduledAnalysisComponent<StudyRankWork, StudyRankWorkResult>
{
    /// <summary>Initializes the Study rank component.</summary>
    public StudyRankComponent() : base(
        "XVARNA Pareto Study", "XV Study",
        "Ranks completed variants with feasibility-first non-dominated sorting, NSGA-II crowding, exact 2D hypervolume, and descriptive Spearman sensitivity.",
        "XVARNA", "05 Study")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("fb9a0879-81ea-4738-8945-1be9ba5f9ca8");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "VAHMAN Study";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddNumberParameter("Parameters", "X", "Candidate parameter rows: one data-tree branch per candidate.", GH_ParamAccess.tree);
        input.AddNumberParameter("Objectives", "F", "Objective rows aligned with Parameters.", GH_ParamAccess.tree);
        input.AddNumberParameter("Constraints", "G", "Residual constraints aligned with Parameters; values ≤ 0 are feasible.", GH_ParamAccess.tree);
        input.AddIntegerParameter("Variable Kinds", "K", "0 continuous, 1 integer, 2 categorical; one or one per parameter column.", GH_ParamAccess.list, 0);
        input.AddNumberParameter("Lower Bounds", "L", "One lower bound per parameter column.", GH_ParamAccess.list);
        input.AddNumberParameter("Upper Bounds", "U", "One upper bound per parameter column.", GH_ParamAccess.list);
        input.AddIntegerParameter("Objective Directions", "D", "0 minimize, 1 maximize; one per objective column.", GH_ParamAccess.list);
        input.AddTextParameter("Candidate IDs", "ID", "Optional unsigned IDs; empty uses 1..N.", GH_ParamAccess.list);
        input.AddNumberParameter("Reference Point", "Ref", "Optional two-value worst reference point for exact 2D hypervolume.", GH_ParamAccess.list);
        input.AddBooleanParameter("Run", "Run", "Run native ranking and diagnostics.", GH_ParamAccess.item, true);
        input[2].Optional = true; input[7].Optional = true; input[8].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Study Result", "R", "Immutable ranked study result.", GH_ParamAccess.item);
        output.AddTextParameter("Candidate IDs", "ID", "Candidate IDs in input order.", GH_ParamAccess.list);
        output.AddIntegerParameter("Pareto Rank", "Rank", "Zero-based constraint-aware non-dominated front.", GH_ParamAccess.list);
        output.AddBooleanParameter("Feasible", "OK", "True when every residual constraint is non-positive.", GH_ParamAccess.list);
        output.AddNumberParameter("Violation", "V", "Sum of positive residual constraints.", GH_ParamAccess.list);
        output.AddNumberParameter("Crowding", "C", "NSGA-II crowding distance; boundary variants are +infinity.", GH_ParamAccess.list);
        output.AddTextParameter("Pareto IDs", "P", "Feasible rank-zero candidate IDs.", GH_ParamAccess.list);
        output.AddNumberParameter("Sensitivity", "ρ", "Spearman rho tree: one branch per variable, objective columns within.", GH_ParamAccess.tree);
        output.AddNumberParameter("Hypervolume 2D", "HV", "Exact feasible Pareto hypervolume; NaN unless two objectives and Ref has two values.", GH_ParamAccess.item);
        output.AddTextParameter("Hash", "H", "Deterministic study identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Dimensions, feasibility, fronts, Pareto set, diagnostics, and runtime.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out StudyRankWork work)
    {
        work = default;
        GH_Structure<GH_Number> parameterTree = []; GH_Structure<GH_Number> objectiveTree = [];
        GH_Structure<GH_Number> constraintTree = []; List<int> kinds = []; List<double> lower = [], upper = [];
        List<int> directions = []; List<string> candidateIds = []; List<double> reference = []; bool run = true;
        if (!data.GetDataTree(0, out parameterTree) || !data.GetDataTree(1, out objectiveTree)
            || !data.GetDataList(4, lower) || !data.GetDataList(5, upper)
            || !data.GetDataList(6, directions)) return false;
        data.GetDataTree(2, out constraintTree); data.GetDataList(3, kinds); data.GetDataList(7, candidateIds);
        data.GetDataList(8, reference); data.GetData(9, ref run);
        if (!run) { Message = "Study\nPaused"; return false; }
        try
        {
            double[][] parameters = Rows(parameterTree, "Parameters");
            double[][] objectives = Rows(objectiveTree, "Objectives");
            double[][] constraints = constraintTree.Branches.Count == 0
                ? Enumerable.Range(0, parameters.Length).Select(_ => Array.Empty<double>()).ToArray()
                : Rows(constraintTree, "Constraints");
            if (parameters.Length == 0 || objectives.Length != parameters.Length || constraints.Length != parameters.Length)
                throw new ArgumentException("Parameter, objective, and constraint trees must have the same non-zero branch count.");
            int variableCount = parameters[0].Length, objectiveCount = objectives[0].Length;
            if (lower.Count != variableCount || upper.Count != variableCount || directions.Count != objectiveCount)
                throw new ArgumentException("Bounds and objective directions must match the parameter/objective column counts.");
            if (kinds.Count is not (1) && kinds.Count != variableCount)
                throw new ArgumentException("Variable Kinds must contain one value or one per parameter column.");
            if (candidateIds.Count != 0 && candidateIds.Count != parameters.Length)
                throw new ArgumentException("Candidate IDs must be empty or contain one value per candidate branch.");
            DesignVariable[] variables = Enumerable.Range(0, variableCount).Select(index => new DesignVariable(
                checked((ulong)index + 1), (DesignVariableKind)Advanced(index, kinds), lower[index], upper[index])).ToArray();
            StudyProblem problem = new()
            {
                Variables = variables,
                ObjectiveDirections = directions.Select(value => (ObjectiveDirection)value).ToArray(),
                ConstraintCount = constraints[0].Length,
            };
            StudyCandidate[] candidates = parameters.Select((row, index) => new StudyCandidate
            {
                CandidateId = candidateIds.Count == 0 ? checked((ulong)index + 1) : ParseId(candidateIds[index]),
                Parameters = row,
            }).ToArray();
            StudyEvaluation[] evaluations = objectives.Select((row, index) => new StudyEvaluation
            {
                CandidateId = candidates[index].CandidateId,
                Objectives = row,
                ConstraintResiduals = constraints[index],
            }).ToArray();
            work = new(problem, candidates, evaluations, variableCount, objectiveCount, reference.ToArray());
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(StudyRankWork work) => checked((ulong)work.Candidates.Length);

    /// <inheritdoc />
    protected override StudyRankWorkResult Execute(
        StudyRankWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Pareto ranking"));
        cancellationToken.ThrowIfCancellationRequested();
        RankedStudyResult result = StudyAnalyzer.Rank(work.Problem, work.Candidates, work.Evaluations);
        progress.Report((total / 2, total, "Sensitivity"));
        IReadOnlyList<StudySensitivity> sensitivity = StudyAnalyzer.SpearmanSensitivity(work.Problem, work.Candidates, work.Evaluations);
        double hypervolume = work.ObjectiveCount == 2 && work.Reference.Length == 2
            ? StudyAnalyzer.Hypervolume2d(work.Problem, work.Candidates, work.Evaluations, work.Reference[0], work.Reference[1])
            : double.NaN;
        progress.Report((total, total, "Diagnostics complete"));
        return new(result, sensitivity.ToArray(), hypervolume);
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, StudyRankWork work, StudyRankWorkResult output)
    {
        RankedStudyResult result = output.Result;
        IReadOnlyList<StudyCandidate> candidates = work.Candidates;
        IReadOnlyList<StudySensitivity> sensitivity = output.Sensitivity;
        int variableCount = work.VariableCount;
        int objectiveCount = work.ObjectiveCount;
        double hypervolume = output.Hypervolume;
        DataTree<double> sensitivityTree = new();
        foreach (StudySensitivity value in sensitivity)
            sensitivityTree.Add(value.SpearmanRho, new GH_Path(checked((int)value.VariableIndex)));
        string hv = double.IsNaN(hypervolume) ? "not requested" : hypervolume.ToString("G10", CultureInfo.InvariantCulture);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: VAHMAN / XVARNA Study constraint-aware Pareto analysis{Environment.NewLine}" +
            $"Table: {candidates.Count:N0} variants × {variableCount:N0} parameters × {objectiveCount:N0} objectives{Environment.NewLine}" +
            $"Ranking: {result.FrontCount:N0} front(s); {result.ParetoCandidateIds.Count:N0} feasible Pareto variants; {result.Solutions.Count(value => value.Feasible):N0} feasible{Environment.NewLine}" +
            $"Diagnostics: NSGA-II crowding; descriptive Spearman ρ (not causal); exact 2D hypervolume {hv}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, candidates.Select(value => value.CandidateId.ToString(CultureInfo.InvariantCulture)));
        data.SetDataList(2, result.Solutions.Select(value => checked((int)value.ParetoRank)));
        data.SetDataList(3, result.Solutions.Select(value => value.Feasible));
        data.SetDataList(4, result.Solutions.Select(value => value.TotalConstraintViolation));
        data.SetDataList(5, result.Solutions.Select(value => value.CrowdingDistance));
        data.SetDataList(6, result.ParetoCandidateIds.Select(value => value.ToString(CultureInfo.InvariantCulture)));
        data.SetDataTree(7, sensitivityTree); data.SetData(8, hypervolume); data.SetData(9, result.ContentHash); data.SetData(10, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"Study\n{result.ParetoCandidateIds.Count:N0} Pareto");
    }

    private static double[][] Rows(GH_Structure<GH_Number> tree, string name)
    {
        double[][] rows = tree.Branches.Select(branch => branch.Select(value => value.Value).ToArray()).ToArray();
        if (rows.Length == 0 || rows[0].Length == 0 || rows.Any(row => row.Length != rows[0].Length))
            throw new ArgumentException($"{name} requires non-empty rectangular branches.");
        return rows;
    }

    private static int Advanced(int index, IReadOnlyList<int> values) => values.Count == 1 ? values[0] : values[index];
    private static ulong ParseId(string value) => ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong id)
        ? id : throw new ArgumentException($"Candidate ID '{value}' is invalid.");
}

/// <summary>Immutable captured table for scheduled Pareto diagnostics.</summary>
public readonly record struct StudyRankWork(
    StudyProblem Problem,
    StudyCandidate[] Candidates,
    StudyEvaluation[] Evaluations,
    int VariableCount,
    int ObjectiveCount,
    double[] Reference);

/// <summary>Rank, sensitivity, and optional hypervolume scheduled payload.</summary>
public sealed record StudyRankWorkResult(
    RankedStudyResult Result,
    StudySensitivity[] Sensitivity,
    double Hypervolume);
