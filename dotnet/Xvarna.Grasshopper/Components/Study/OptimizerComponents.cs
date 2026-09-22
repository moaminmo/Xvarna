using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Study;

/// <summary>A native optimizer session passed between explicit Grasshopper generation steps.</summary>
public sealed class XvarnaOptimizationSession : IDisposable
{
    private readonly object gate = new();
    private readonly XvarnaOptimizer optimizer;

    internal XvarnaOptimizationSession(StudyProblem problem, MultiObjectiveOptimizerOptions options)
    {
        Problem = problem; Options = options; optimizer = new(problem, options); CurrentBatch = optimizer.Ask();
    }

    /// <summary>Optimization dimension contract.</summary>
    public StudyProblem Problem { get; }
    /// <summary>Evolution policy.</summary>
    public MultiObjectiveOptimizerOptions Options { get; }
    /// <summary>Candidate batch awaiting external evaluation.</summary>
    public OptimizerBatch CurrentBatch { get; private set; }
    /// <summary>Last completed generation, if any.</summary>
    public OptimizerSnapshot? LastSnapshot { get; private set; }

    internal void Advance(double[][] objectives, double[][] constraints)
    {
        lock (gate)
        {
            if (objectives.Length != CurrentBatch.Candidates.Count || constraints.Length != objectives.Length)
                throw new ArgumentException("Objective and constraint branches must match the pending candidate count.");
            StudyEvaluation[] evaluations = objectives.Select((row, index) => new StudyEvaluation
            {
                CandidateId = CurrentBatch.Candidates[index].CandidateId,
                Objectives = row,
                ConstraintResiduals = constraints[index],
            }).ToArray();
            LastSnapshot = optimizer.Tell(evaluations);
            CurrentBatch = optimizer.Ask();
        }
    }

    /// <inheritdoc />
    public void Dispose() { optimizer.Dispose(); GC.SuppressFinalize(this); }

    /// <inheritdoc />
    public override string ToString() => string.Create(CultureInfo.InvariantCulture,
        $"VAHMAN optimizer: generation {CurrentBatch.Generation}, {CurrentBatch.Candidates.Count} pending");
}

/// <summary>Creates deterministic mixed-variable optimizer sessions and initial candidate batches.</summary>
public sealed class OptimizerStartComponent : GH_Component
{
    private XvarnaOptimizationSession? session;
    private string signature = string.Empty;

    /// <summary>Initializes the optimizer-start component.</summary>
    public OptimizerStartComponent() : base(
        "XVARNA Optimizer Start", "XV Optimizer",
        "Creates a deterministic native mixed-variable constrained NSGA-II session and a stratified initial population for external Grasshopper evaluation.",
        "XVARNA", "05 Study")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("59ef17be-41b6-4d5c-a598-c33a7ecdfdb9");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddIntegerParameter("Variable Kinds", "K", "0 continuous, 1 integer, 2 categorical; one or one per variable.", GH_ParamAccess.list, 0);
        input.AddNumberParameter("Lower Bounds", "L", "One lower bound per variable.", GH_ParamAccess.list);
        input.AddNumberParameter("Upper Bounds", "U", "One upper bound per variable.", GH_ParamAccess.list);
        input.AddIntegerParameter("Objective Directions", "D", "0 minimize, 1 maximize; one per objective.", GH_ParamAccess.list);
        input.AddIntegerParameter("Constraint Count", "G", "Number of residual constraints; values ≤ 0 are feasible.", GH_ParamAccess.item, 0);
        input.AddIntegerParameter("Population", "P", "Parent population size.", GH_ParamAccess.item, 64);
        input.AddIntegerParameter("Offspring", "O", "Candidates produced per later generation.", GH_ParamAccess.item, 64);
        input.AddIntegerParameter("Archive", "A", "Bounded historical Pareto archive capacity.", GH_ParamAccess.item, 256);
        input.AddIntegerParameter("Seed", "S", "Deterministic non-negative random seed.", GH_ParamAccess.item, 42);
        input.AddBooleanParameter("Reset", "R", "Recreate the session and initial population.", GH_ParamAccess.item, false);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Optimizer Session", "S", "Stateful session; connect to XV Optimizer Step.", GH_ParamAccess.item);
        output.AddNumberParameter("Candidates", "X", "Parameter vectors, one branch per candidate.", GH_ParamAccess.tree);
        output.AddTextParameter("Candidate IDs", "ID", "Stable pending candidate IDs.", GH_ParamAccess.list);
        output.AddIntegerParameter("Generation", "Gen", "Pending generation label.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Optimizer dimensions, operators, seed, and pending state.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<int> kinds = []; List<double> lower = [], upper = []; List<int> directions = [];
        int constraints = 0, population = 64, offspring = 64, archive = 256, seed = 42; bool reset = false;
        if (!data.GetDataList(0, kinds) || !data.GetDataList(1, lower) || !data.GetDataList(2, upper)
            || !data.GetDataList(3, directions)) return;
        data.GetData(4, ref constraints); data.GetData(5, ref population); data.GetData(6, ref offspring);
        data.GetData(7, ref archive); data.GetData(8, ref seed); data.GetData(9, ref reset);
        try
        {
            if (lower.Count == 0 || upper.Count != lower.Count || kinds.Count is not (1) && kinds.Count != lower.Count)
                throw new ArgumentException("Bounds must have equal non-zero lengths; Variable Kinds needs one or one per bound.");
            StudyProblem problem = new()
            {
                Variables = Enumerable.Range(0, lower.Count).Select(index => new DesignVariable(
                    checked((ulong)index + 1), (DesignVariableKind)(kinds.Count == 1 ? kinds[0] : kinds[index]), lower[index], upper[index])).ToArray(),
                ObjectiveDirections = directions.Select(value => (ObjectiveDirection)value).ToArray(),
                ConstraintCount = constraints,
            };
            MultiObjectiveOptimizerOptions options = new(population, offspring, archive, checked((ulong)seed), 0.9, 0.0, 15.0, 20.0);
            string nextSignature = string.Join("|", kinds.Concat(directions)) + ":" + string.Join("|", lower.Concat(upper))
                + $":{constraints}:{population}:{offspring}:{archive}:{seed}";
            if (reset || session is null || !string.Equals(signature, nextSignature, StringComparison.Ordinal))
            {
                session?.Dispose(); session = new(problem, options); signature = nextSignature;
            }
            OptimizerGrasshopper.EmitBatch(data, 0, session, 1, 2, 3);
            data.SetData(4, OptimizerGrasshopper.Report(session));
            Message = string.Create(CultureInfo.InvariantCulture, $"Study\nGen {session.CurrentBatch.Generation}");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

/// <summary>Consumes external evaluations, advances one native generation, and returns the next batch.</summary>
public sealed class OptimizerStepComponent : GH_Component
{
    /// <summary>Initializes the explicit optimizer-step component.</summary>
    public OptimizerStepComponent() : base(
        "XVARNA Optimizer Step", "XV Opt Step",
        "Accepts one objective/constraint branch per pending candidate, performs constraint-aware environmental selection, updates the Pareto archive, and asks the next generation.",
        "XVARNA", "05 Study")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("0f59c78c-36f6-4fa0-9cbd-1621438f27cc");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Optimizer Session", "S", "Session produced by XV Optimizer or a previous XV Opt Step.", GH_ParamAccess.item);
        input.AddNumberParameter("Objectives", "F", "One branch per pending candidate; columns match objective directions.", GH_ParamAccess.tree);
        input.AddNumberParameter("Constraints", "G", "One residual-constraint branch per candidate; values ≤ 0 are feasible.", GH_ParamAccess.tree);
        input.AddBooleanParameter("Advance", "Go", "Use a Button pulse to consume this batch exactly once and generate the next.", GH_ParamAccess.item, false);
        input[2].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Optimizer Session", "S", "Advanced session for another explicit step.", GH_ParamAccess.item);
        output.AddNumberParameter("Next Candidates", "X", "Next parameter vectors, one branch per candidate.", GH_ParamAccess.tree);
        output.AddTextParameter("Candidate IDs", "ID", "Stable pending candidate IDs.", GH_ParamAccess.list);
        output.AddIntegerParameter("Generation", "Gen", "Pending generation label.", GH_ParamAccess.item);
        output.AddIntegerParameter("Population Rank", "Rank", "Rank of the selected parent population from the completed step.", GH_ParamAccess.list);
        output.AddTextParameter("Pareto IDs", "P", "Current feasible rank-zero population IDs.", GH_ParamAccess.list);
        output.AddTextParameter("Archive IDs", "A", "Bounded historical Pareto archive IDs.", GH_ParamAccess.list);
        output.AddTextParameter("State Hash", "H", "Deterministic optimizer state identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Operators, evaluation count, diversity, Pareto state, and pending batch.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        object? wrapped = null; GH_Structure<GH_Number> objectives = []; GH_Structure<GH_Number> constraints = [];
        bool advance = false;
        if (!data.GetData(0, ref wrapped)) return;
        data.GetDataTree(1, out objectives); data.GetDataTree(2, out constraints); data.GetData(3, ref advance);
        XvarnaOptimizationSession? session = wrapped switch
        {
            XvarnaOptimizationSession value => value,
            GH_ObjectWrapper { Value: XvarnaOptimizationSession value } => value,
            _ => null,
        };
        if (session is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Session must come directly from XV Optimizer or XV Opt Step."); return; }
        try
        {
            if (advance)
            {
                double[][] objectiveRows = OptimizerGrasshopper.Rows(objectives, "Objectives");
                double[][] constraintRows = session.Problem.ConstraintCount == 0
                    ? Enumerable.Range(0, objectiveRows.Length).Select(_ => Array.Empty<double>()).ToArray()
                    : OptimizerGrasshopper.Rows(constraints, "Constraints");
                if (objectiveRows.Any(row => row.Length != session.Problem.ObjectiveDirections.Count)
                    || constraintRows.Any(row => row.Length != session.Problem.ConstraintCount))
                    throw new ArgumentException("Objective or constraint branch width does not match the optimizer problem.");
                session.Advance(objectiveRows, constraintRows);
            }
            OptimizerGrasshopper.EmitBatch(data, 0, session, 1, 2, 3);
            OptimizerSnapshot? snapshot = session.LastSnapshot;
            data.SetDataList(4, snapshot?.Population.Solutions.Select(value => checked((int)value.ParetoRank)) ?? []);
            data.SetDataList(5, snapshot?.Population.ParetoCandidateIds.Select(value => value.ToString(CultureInfo.InvariantCulture)) ?? []);
            data.SetDataList(6, snapshot?.ArchiveCandidateIds.Select(value => value.ToString(CultureInfo.InvariantCulture)) ?? []);
            data.SetData(7, snapshot?.ContentHash ?? string.Empty); data.SetData(8, OptimizerGrasshopper.Report(session));
            Message = string.Create(CultureInfo.InvariantCulture, $"Study\nGen {session.CurrentBatch.Generation}");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or ObjectDisposedException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

internal static class OptimizerGrasshopper
{
    internal static void EmitBatch(IGH_DataAccess data, int sessionIndex, XvarnaOptimizationSession session,
        int parametersIndex, int idsIndex, int generationIndex)
    {
        DataTree<double> tree = new();
        for (int index = 0; index < session.CurrentBatch.Candidates.Count; index++)
            tree.AddRange(session.CurrentBatch.Candidates[index].Parameters, new GH_Path(index));
        data.SetData(sessionIndex, session); data.SetDataTree(parametersIndex, tree);
        data.SetDataList(idsIndex, session.CurrentBatch.Candidates.Select(value => value.CandidateId.ToString(CultureInfo.InvariantCulture)));
        data.SetData(generationIndex, checked((int)session.CurrentBatch.Generation));
    }

    internal static double[][] Rows(GH_Structure<GH_Number> tree, string name)
    {
        double[][] rows = tree.Branches.Select(branch => branch.Select(value => value.Value).ToArray()).ToArray();
        if (rows.Length == 0 || rows[0].Length == 0 || rows.Any(row => row.Length != rows[0].Length))
            throw new ArgumentException($"{name} requires non-empty rectangular branches.");
        return rows;
    }

    internal static string Report(XvarnaOptimizationSession session)
    {
        MultiObjectiveOptimizerOptions options = session.Options; OptimizerSnapshot? snapshot = session.LastSnapshot;
        return string.Create(CultureInfo.InvariantCulture,
            $"Engine: VAHMAN / XVARNA Study mixed-variable constrained NSGA-II ask/tell{Environment.NewLine}" +
            $"Problem: {session.Problem.Variables.Count:N0} variable(s); {session.Problem.ObjectiveDirections.Count:N0} objective(s); {session.Problem.ConstraintCount:N0} residual constraint(s){Environment.NewLine}" +
            $"Evolution: population {options.PopulationSize:N0}; offspring {options.OffspringSize:N0}; archive {options.ArchiveCapacity:N0}; seed {options.Seed}{Environment.NewLine}" +
            $"Operators: stratified initialization; tournament; SBX η={options.CrossoverDistributionIndex:G6}; polynomial mutation η={options.MutationDistributionIndex:G6}; discrete mutation{Environment.NewLine}" +
            $"State: generation {session.CurrentBatch.Generation:N0}; {session.CurrentBatch.Candidates.Count:N0} pending; {snapshot?.EvaluationCount ?? 0:N0} evaluated; archive {snapshot?.ArchiveCandidateIds.Count ?? 0:N0}{Environment.NewLine}" +
            $"State hash: {snapshot?.ContentHash ?? "not available before first Tell"}");
    }
}
