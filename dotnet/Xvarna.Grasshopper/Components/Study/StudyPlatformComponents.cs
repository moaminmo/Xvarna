using System.Drawing;
using System.Globalization;
using System.Text.Json;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Study;

/// <summary>Builds the versioned 0.16 Study Manifest and exposes its JSON Schema.</summary>
public sealed class StudyManifestComponent : GH_Component
{
    /// <summary>Initializes the manifest component.</summary>
    public StudyManifestComponent() : base("XVARNA Study Manifest", "XV Manifest",
        "Builds a versioned parameter/objective/constraint registry with scalar/vector objectives and deterministic optimizer policy.",
        "XVARNA", "05 Study")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("b0ea0f05-bce9-44dc-af16-3310454e6251");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Study ID", "ID", "Stable machine/project identity.", GH_ParamAccess.item);
        input.AddTextParameter("Title", "T", "Human-readable report title.", GH_ParamAccess.item);
        input.AddTextParameter("Description", "D", "Study intent and scientific scope.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Created UTC", "UTC", "RFC3339 creation time; the default is captured when this component is created.", GH_ParamAccess.item,
            DateTimeOffset.UtcNow.ToString("O", CultureInfo.InvariantCulture));
        input.AddTextParameter("Parameter Names", "PN", "Ordered unique parameter names.", GH_ParamAccess.list);
        input.AddIntegerParameter("Parameter Kinds", "PK", "0 continuous, 1 integer, 2 categorical; one or one per parameter.", GH_ParamAccess.list, 0);
        input.AddNumberParameter("Lower Bounds", "L", "One inclusive lower bound per parameter.", GH_ParamAccess.list);
        input.AddNumberParameter("Upper Bounds", "U", "One inclusive upper bound per parameter.", GH_ParamAccess.list);
        input.AddTextParameter("Parameter Units", "PU", "Empty, one, or one display unit per parameter.", GH_ParamAccess.list);
        input.AddTextParameter("Categories", "Cat", "One branch per parameter; required labels only for categorical parameters.", GH_ParamAccess.tree);
        input.AddTextParameter("Objective Names", "ON", "Ordered unique objective names.", GH_ParamAccess.list);
        input.AddIntegerParameter("Directions", "Dir", "0 minimize, 1 maximize; one or one per objective.", GH_ParamAccess.list, 0);
        input.AddTextParameter("Objective Units", "OU", "Empty, one, or one unit per objective.", GH_ParamAccess.list);
        input.AddTextParameter("Vector Components", "Vec", "One branch per objective; empty branch means scalar.", GH_ParamAccess.tree);
        input.AddTextParameter("Constraint Names", "CN", "Optional residual constraints; feasible at ≤ 0.", GH_ParamAccess.list);
        input.AddTextParameter("Constraint Units", "CU", "Empty, one, or one unit per constraint.", GH_ParamAccess.list);
        input.AddTextParameter("Baseline ID", "Base", "Optional baseline variant ID for scenario delta maps.", GH_ParamAccess.item, string.Empty);
        input.AddIntegerParameter("Population", "Pop", "NSGA-II parent population size.", GH_ParamAccess.item, 64);
        input.AddIntegerParameter("Offspring", "Off", "Candidates per later batch.", GH_ParamAccess.item, 64);
        input.AddIntegerParameter("Archive", "Arc", "Bounded historical Pareto archive capacity.", GH_ParamAccess.item, 256);
        input.AddTextParameter("Seed", "Seed", "Unsigned deterministic seed.", GH_ParamAccess.item, "42");
        foreach (int optional in new[] { 8, 9, 12, 13, 14, 15, 16 }) input[optional].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddTextParameter("Manifest JSON", "JSON", "Schema-valid versioned manifest JSON.", GH_ParamAccess.item);
        output.AddTextParameter("JSON Schema", "Schema", "Normative Draft 2020-12 schema.", GH_ParamAccess.item);
        output.AddGenericParameter("Manifest", "M", "Strongly typed reusable manifest.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Registry dimensions, vector flattening, optimizer policy, and version.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        string studyId = string.Empty, title = string.Empty, description = string.Empty, created = string.Empty;
        List<string> parameterNames = [], parameterUnits = [], objectiveNames = [], objectiveUnits = [];
        List<string> constraintNames = [], constraintUnits = []; List<int> kinds = [], directions = [];
        List<double> lower = [], upper = []; string baseline = string.Empty, seedText = "42";
        int population = 64, offspring = 64, archive = 256;
        GH_Structure<GH_String> categories = [], components = [];
        if (!data.GetData(0, ref studyId) || !data.GetData(1, ref title)
            || !data.GetData(3, ref created) || !data.GetDataList(4, parameterNames)
            || !data.GetDataList(6, lower) || !data.GetDataList(7, upper)
            || !data.GetDataList(10, objectiveNames)) return;
        data.GetData(2, ref description); data.GetDataList(5, kinds); data.GetDataList(8, parameterUnits);
        data.GetDataTree(9, out categories); data.GetDataList(11, directions); data.GetDataList(12, objectiveUnits);
        data.GetDataTree(13, out components); data.GetDataList(14, constraintNames); data.GetDataList(15, constraintUnits);
        data.GetData(16, ref baseline); data.GetData(17, ref population); data.GetData(18, ref offspring);
        data.GetData(19, ref archive); data.GetData(20, ref seedText);
        try
        {
            if (parameterNames.Count == 0 || lower.Count != parameterNames.Count || upper.Count != parameterNames.Count)
                throw new ArgumentException("Parameter names and lower/upper bounds must have the same non-zero count.");
            if (objectiveNames.Count == 0) throw new ArgumentException("At least one objective is required.");
            DateTimeOffset createdUtc = DateTimeOffset.Parse(created, CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal);
            ulong seed = ParseId(seedText, "Seed", allowEmpty: false)!.Value;
            ulong? baselineId = ParseId(baseline, "Baseline ID", allowEmpty: true);
            StudyParameterDefinition[] parameters = parameterNames.Select((name, index) => new StudyParameterDefinition
            {
                ParameterId = checked((ulong)index + 1),
                Name = name,
                Kind = (DesignVariableKind)At(kinds, index, 0),
                LowerBound = lower[index],
                UpperBound = upper[index],
                Unit = At(parameterUnits, index, string.Empty),
                Categories = Branch(categories, index),
            }).ToArray();
            StudyObjectiveDefinition[] objectives = objectiveNames.Select((name, index) => new StudyObjectiveDefinition
            {
                ObjectiveId = checked((ulong)index + 1),
                Name = name,
                Direction = (ObjectiveDirection)At(directions, index, 0),
                Unit = At(objectiveUnits, index, string.Empty),
                Components = Branch(components, index),
            }).ToArray();
            StudyConstraintDefinition[] constraints = constraintNames.Select((name, index) => new StudyConstraintDefinition
            {
                ConstraintId = checked((ulong)index + 1),
                Name = name,
                Unit = At(constraintUnits, index, string.Empty),
            }).ToArray();
            StudyPlatformManifest manifest = new()
            {
                StudyId = studyId,
                Title = title,
                Description = description,
                CreatedUtc = createdUtc,
                Parameters = parameters,
                Objectives = objectives,
                Constraints = constraints,
                BaselineVariantId = baselineId,
                Optimizer = new(population, offspring, archive, seed),
            };
            string json = StudyPlatformWorkspace.SerializeManifest(manifest);
            int scalarColumns = objectives.Sum(value => Math.Max(1, value.Components.Count));
            string report = string.Create(CultureInfo.InvariantCulture,
                $"XVARNA Study Manifest 0.16.0{Environment.NewLine}" +
                $"Registry: {parameters.Length:N0} parameters; {objectives.Length:N0} objective groups → {scalarColumns:N0} Pareto columns; {constraints.Length:N0} constraints{Environment.NewLine}" +
                $"Optimizer: population {population:N0}; offspring {offspring:N0}; archive {archive:N0}; seed {seed}{Environment.NewLine}" +
                $"Baseline: {(baselineId?.ToString(CultureInfo.InvariantCulture) ?? "none")}; created {createdUtc:O}");
            data.SetData(0, json); data.SetData(1, StudyPlatformWorkspace.ManifestSchemaJson);
            data.SetData(2, manifest); data.SetData(3, report);
            Message = $"MANIFEST\n{parameters.Length}P · {scalarColumns}F";
        }
        catch (Exception exception) when (exception is ArgumentException or FormatException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }

    private static int At(IReadOnlyList<int> values, int index, int fallback) => values.Count switch
    { 0 => fallback, 1 => values[0], _ when values.Count > index => values[index], _ => throw new ArgumentException("Integer registry inputs must contain zero, one, or one value per entry.") };
    private static string At(IReadOnlyList<string> values, int index, string fallback) => values.Count switch
    { 0 => fallback, 1 => values[0], _ when values.Count > index => values[index], _ => throw new ArgumentException("Unit inputs must contain zero, one, or one value per entry.") };
    private static string[] Branch(GH_Structure<GH_String> tree, int index) => tree.Branches.Count > index
        ? tree.Branches[index].Select(value => value.Value).Where(value => !string.IsNullOrWhiteSpace(value)).ToArray() : [];
    private static ulong? ParseId(string value, string name, bool allowEmpty)
    {
        if (allowEmpty && string.IsNullOrWhiteSpace(value)) return null;
        return ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong id)
            ? id : throw new ArgumentException($"{name} must be an unsigned integer.");
    }
}

/// <summary>Creates/opens a workspace and starts or resumes durable external batches.</summary>
public sealed class StudyWorkspaceComponent : GH_Component
{
    /// <summary>Initializes the workspace component.</summary>
    public StudyWorkspaceComponent() : base("XVARNA Study Workspace", "XV Workspace",
        "Creates, validates, opens, and resumes a transactional Study directory with exact optimizer checkpoint state.",
        "XVARNA", "05 Study")
    { }
    /// <inheritdoc />
    public override Guid ComponentGuid => new("eb7df94d-8891-44cf-b3f1-a5ee4b36c1ae");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Workspace", "Dir", "Persistent workspace directory.", GH_ParamAccess.item);
        input.AddTextParameter("Manifest JSON", "M", "Required for Create; ignored for Open/Begin.", GH_ParamAccess.item);
        input.AddIntegerParameter("Action", "A", "0 status/open, 1 create, 2 begin or resume batch.", GH_ParamAccess.item, 0);
        input.AddBooleanParameter("Run", "Run", "Execute the selected idempotent action.", GH_ParamAccess.item, true);
        input[1].Optional = true;
    }
    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Workspace", "W", "Reusable managed workspace object.", GH_ParamAccess.item);
        output.AddGenericParameter("State", "S", "Validated checkpoint/resume state.", GH_ParamAccess.item);
        output.AddGenericParameter("Batch", "B", "Pending batch for action 2.", GH_ParamAccess.item);
        output.AddTextParameter("Batch ID", "BID", "Pending batch ID, if any.", GH_ParamAccess.item);
        output.AddTextParameter("Candidate IDs", "ID", "Pending candidate IDs.", GH_ParamAccess.list);
        output.AddNumberParameter("Parameters", "X", "Pending parameter rows, one branch per candidate.", GH_ParamAccess.tree);
        output.AddTextParameter("Ledger Hash", "LH", "Current ledger identity.", GH_ParamAccess.item);
        output.AddTextParameter("Checkpoint Hash", "CH", "Exact optimizer/checkpoint identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Revision, generation, completed/pending dimensions and recovery state.", GH_ParamAccess.item);
    }
    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        string directory = string.Empty, manifest = string.Empty; int action = 0; bool run = true;
        if (!data.GetData(0, ref directory)) return;
        data.GetData(1, ref manifest); data.GetData(2, ref action); data.GetData(3, ref run);
        if (!run) { Message = "WORKSPACE\nPaused"; return; }
        try
        {
            StudyPlatformWorkspace workspace = action switch
            {
                0 => StudyPlatformWorkspace.Open(directory),
                1 when !string.IsNullOrWhiteSpace(manifest) => StudyPlatformWorkspace.CreateJson(directory, manifest),
                1 => throw new ArgumentException("Manifest JSON is required for Create."),
                2 => StudyPlatformWorkspace.Open(directory),
                _ => throw new ArgumentException("Action must be 0 status, 1 create, or 2 begin/resume."),
            };
            StudyPlatformBatch? batch = action == 2 ? workspace.BeginBatch() : null;
            StudyWorkspaceState state = workspace.State;
            DataTree<double> parameters = new();
            IReadOnlyList<StudyPlatformCandidate> candidates = batch?.Candidates ?? state.PendingCandidates;
            foreach ((StudyPlatformCandidate candidate, int index) in candidates.Select((value, index) => (value, index)))
                parameters.AddRange(candidate.Values, new GH_Path(index));
            string report = string.Create(CultureInfo.InvariantCulture,
                $"VAHMAN Study Workspace 0.16.0{Environment.NewLine}" +
                $"Revision {state.Revision:N0}; generation {state.Generation:N0}; {state.CompletedCount:N0}/{state.VariantCount:N0} completed variants{Environment.NewLine}" +
                $"Active batch: {(state.ActiveBatchId?.ToString(CultureInfo.InvariantCulture) ?? "none")}; pending {state.PendingCandidates.Count:N0}{Environment.NewLine}" +
                $"Ledger {state.LedgerHash}{Environment.NewLine}Checkpoint {state.CheckpointHash}");
            data.SetData(0, workspace); data.SetData(1, state); data.SetData(2, batch);
            data.SetData(3, (batch?.BatchId ?? state.ActiveBatchId)?.ToString(CultureInfo.InvariantCulture) ?? string.Empty);
            data.SetDataList(4, candidates.Select(value => value.CandidateId.ToString(CultureInfo.InvariantCulture)));
            data.SetDataTree(5, parameters); data.SetData(6, state.LedgerHash); data.SetData(7, state.CheckpointHash); data.SetData(8, report);
            Message = $"WORKSPACE\nG{state.Generation} · {state.PendingCandidates.Count} pending";
        }
        catch (Exception exception) when (exception is ArgumentException or InvalidOperationException or IOException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

/// <summary>Atomically commits Grasshopper objective/constraint trees to a pending batch.</summary>
public sealed class StudyCommitComponent : GH_Component
{
    /// <summary>Initializes the commit component.</summary>
    public StudyCommitComponent() : base("XVARNA Study Commit", "XV Commit",
        "Commits one complete scalar/vector objective and constraint row per pending candidate, exactly once.",
        "XVARNA", "05 Study")
    { }
    /// <inheritdoc />
    public override Guid ComponentGuid => new("33df6840-624c-48a5-a313-44ac1d5c39b7");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Workspace", "W", "XV Workspace object or workspace directory text.", GH_ParamAccess.item);
        input.AddTextParameter("Batch ID", "BID", "Pending batch ID.", GH_ParamAccess.item);
        input.AddTextParameter("Candidate IDs", "ID", "One ID per objective branch.", GH_ParamAccess.list);
        input.AddNumberParameter("Objectives", "F", "Flattened objective rows, one branch per candidate.", GH_ParamAccess.tree);
        input.AddIntegerParameter("Objective Widths", "FW", "One group width per manifest scalar/vector objective; sum equals row length.", GH_ParamAccess.list);
        input.AddNumberParameter("Constraints", "G", "Residual rows aligned with objectives; values ≤ 0 feasible.", GH_ParamAccess.tree);
        input.AddTextParameter("Metric Fields JSON", "Map", "Optional JSON array of metric fields per candidate branch/list item.", GH_ParamAccess.list);
        input.AddBooleanParameter("Commit", "Go", "Pulse true to atomically commit.", GH_ParamAccess.item, false);
        input[5].Optional = true; input[6].Optional = true;
    }
    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("State", "S", "Completed generation/checkpoint state.", GH_ParamAccess.item);
        output.AddIntegerParameter("Generation", "Gen", "Completed optimizer generation.", GH_ParamAccess.item);
        output.AddIntegerParameter("Evaluations", "N", "Accepted evaluation count.", GH_ParamAccess.item);
        output.AddTextParameter("Ledger Hash", "LH", "Committed ledger identity.", GH_ParamAccess.item);
        output.AddTextParameter("Checkpoint Hash", "CH", "Exact resume identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Atomic commit and resume summary.", GH_ParamAccess.item);
    }
    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        object workspaceInput = null!; string batchText = string.Empty; List<string> ids = [], fieldJson = [];
        List<int> widths = []; GH_Structure<GH_Number> objectives = [], constraints = []; bool commit = false;
        if (!data.GetData(0, ref workspaceInput) || !data.GetData(1, ref batchText)
            || !data.GetDataList(2, ids) || !data.GetDataTree(3, out objectives)
            || !data.GetDataList(4, widths)) return;
        data.GetDataTree(5, out constraints); data.GetDataList(6, fieldJson); data.GetData(7, ref commit);
        if (!commit) { Message = "COMMIT\nReady"; return; }
        try
        {
            StudyPlatformWorkspace workspace = workspaceInput switch
            {
                StudyPlatformWorkspace value => value,
                GH_ObjectWrapper wrapper when wrapper.Value is StudyPlatformWorkspace value => value,
                string path => StudyPlatformWorkspace.Open(path),
                GH_String text => StudyPlatformWorkspace.Open(text.Value),
                _ => throw new ArgumentException("Workspace must be an XV Workspace object or directory text."),
            };
            ulong batchId = ParseUnsigned(batchText, "Batch ID");
            double[][] objectiveRows = Rows(objectives, "Objectives", allowEmpty: false);
            double[][] constraintRows = constraints.Branches.Count == 0
                ? Enumerable.Range(0, objectiveRows.Length).Select(_ => Array.Empty<double>()).ToArray()
                : Rows(constraints, "Constraints", allowEmpty: true);
            if (ids.Count != objectiveRows.Length || constraintRows.Length != objectiveRows.Length
                || widths.Count == 0 || widths.Any(value => value <= 0)
                || widths.Sum() != objectiveRows[0].Length
                || fieldJson.Count != 0 && fieldJson.Count != ids.Count)
                throw new ArgumentException("IDs, rectangular rows, objective widths, constraints, and field JSON must align exactly.");
            StudyPlatformEvaluation[] evaluations = objectiveRows.Select((row, index) =>
            {
                int offset = 0;
                IReadOnlyList<double>[] grouped = widths.Select(width =>
                {
                    double[] values = row.Skip(offset).Take(width).ToArray(); offset += width; return values;
                }).ToArray();
                IReadOnlyList<StudyMetricField> fields = fieldJson.Count == 0 || string.IsNullOrWhiteSpace(fieldJson[index])
                    ? Array.Empty<StudyMetricField>()
                    : JsonSerializer.Deserialize<StudyMetricField[]>(fieldJson[index], new JsonSerializerOptions(JsonSerializerDefaults.Web))
                        ?? throw new ArgumentException($"Metric Fields JSON row {index} is empty.");
                return new StudyPlatformEvaluation
                {
                    VariantId = ParseUnsigned(ids[index], "Candidate ID"),
                    ObjectiveValues = grouped,
                    ConstraintResiduals = constraintRows[index],
                    MetricFields = fields,
                };
            }).ToArray();
            StudyWorkspaceState state = workspace.CommitBatch(batchId, evaluations);
            string report = string.Create(CultureInfo.InvariantCulture,
                $"VAHMAN atomic batch commit 0.16.0{Environment.NewLine}" +
                $"Batch {batchId}; generation {state.Generation:N0}; accepted {evaluations.Length:N0}; total {state.EvaluationCount:N0}{Environment.NewLine}" +
                $"Ledger {state.LedgerHash}{Environment.NewLine}Checkpoint {state.CheckpointHash}");
            data.SetData(0, state); data.SetData(1, checked((int)state.Generation));
            data.SetData(2, checked((int)state.EvaluationCount)); data.SetData(3, state.LedgerHash);
            data.SetData(4, state.CheckpointHash); data.SetData(5, report); Message = $"COMMIT\nG{state.Generation}";
        }
        catch (Exception exception) when (exception is ArgumentException or InvalidOperationException or OverflowException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
    private static ulong ParseUnsigned(string value, string name) =>
        ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed)
            ? parsed : throw new ArgumentException($"{name} '{value}' is invalid.");
    private static double[][] Rows(GH_Structure<GH_Number> tree, string name, bool allowEmpty)
    {
        double[][] rows = tree.Branches.Select(branch => branch.Select(value => value.Value).ToArray()).ToArray();
        if (rows.Length == 0 || !allowEmpty && rows[0].Length == 0 || rows.Any(row => row.Length != rows[0].Length))
            throw new ArgumentException($"{name} must contain rectangular aligned branches.");
        return rows;
    }
}

/// <summary>Builds the complete shareable Study report package off the UI thread.</summary>
public sealed class StudyReportComponent : ScheduledAnalysisComponent<StudyReportWork, StudyReportArtifacts>
{
    /// <summary>Initializes the report component.</summary>
    public StudyReportComponent() : base("XVARNA Study Report", "XV Report",
        "Generates statistically qualified JSON, Apache Parquet, glTF 2.0, self-contained report HTML, and independent viewer.",
        "XVARNA", "05 Study")
    { }
    /// <inheritdoc />
    public override Guid ComponentGuid => new("d99e587a-98ae-478d-b853-41868cedc715");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override string AnalysisName => "VAHMAN Study Report";
    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Workspace", "W", "XV Workspace object or directory text.", GH_ParamAccess.item);
        input.AddTextParameter("Output Directory", "Out", "Report package directory.", GH_ParamAccess.item);
        input.AddIntegerParameter("Bootstrap", "Boot", "Bootstrap resamples, 32–10000.", GH_ParamAccess.item, 1000);
        input.AddIntegerParameter("Permutations", "Perm", "Permutation samples, 32–10000.", GH_ParamAccess.item, 1000);
        input.AddNumberParameter("Confidence", "CI", "Bootstrap confidence level.", GH_ParamAccess.item, 0.95);
        input.AddNumberParameter("Alpha", "α", "Family-wise alpha before Holm correction.", GH_ParamAccess.item, 0.05);
        input.AddTextParameter("Seed", "Seed", "Unsigned statistical seed.", GH_ParamAccess.item, "24859869943030038");
        input.AddBooleanParameter("Run", "Run", "Generate/update the complete package.", GH_ParamAccess.item, true);
    }
    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Artifacts", "A", "Typed report artifact paths and identity.", GH_ParamAccess.item);
        output.AddTextParameter("HTML Report", "HTML", "Self-contained narrative report.", GH_ParamAccess.item);
        output.AddTextParameter("Viewer", "View", "Independent self-contained interactive viewer.", GH_ParamAccess.item);
        output.AddTextParameter("Parquet", "PQ", "Typed Apache Parquet variant table.", GH_ParamAccess.item);
        output.AddTextParameter("glTF", "glTF", "Embedded-buffer glTF 2.0 objective-space asset.", GH_ParamAccess.item);
        output.AddTextParameter("JSON", "JSON", "Complete portable study data.", GH_ParamAccess.item);
        output.AddTextParameter("Hash", "H", "Derived analysis identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Counts, statistics policy, formats, and shareability scope.", GH_ParamAccess.item);
    }
    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out StudyReportWork work)
    {
        work = default; object workspace = null!; string output = string.Empty, seedText = string.Empty;
        int bootstrap = 1000, permutations = 1000; double confidence = 0.95, alpha = 0.05; bool run = true;
        if (!data.GetData(0, ref workspace) || !data.GetData(1, ref output)) return false;
        data.GetData(2, ref bootstrap); data.GetData(3, ref permutations); data.GetData(4, ref confidence);
        data.GetData(5, ref alpha); data.GetData(6, ref seedText); data.GetData(7, ref run);
        if (!run) { Message = "REPORT\nPaused"; return false; }
        try
        {
            bootstrap = ProductContextRegistry.ScaleSamples(OnPingDocument(), bootstrap, 32, 10_000);
            permutations = ProductContextRegistry.ScaleSamples(OnPingDocument(), permutations, 32, 10_000);
            string root = workspace switch
            {
                StudyPlatformWorkspace value => value.Root,
                GH_ObjectWrapper wrapper when wrapper.Value is StudyPlatformWorkspace value => value.Root,
                string path => path,
                GH_String text => text.Value,
                _ => throw new ArgumentException("Workspace must be an XV Workspace object or directory text."),
            };
            if (!ulong.TryParse(seedText, NumberStyles.None, CultureInfo.InvariantCulture, out ulong seed))
                throw new ArgumentException("Seed must be an unsigned integer.");
            work = new(root, output, new(bootstrap, permutations, confidence, alpha, seed)); return true;
        }
        catch (ArgumentException exception) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }
    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(StudyReportWork work) =>
        checked((ulong)Math.Max(1, work.Options.BootstrapSamples + work.Options.PermutationSamples));
    /// <inheritdoc />
    protected override StudyReportArtifacts Execute(StudyReportWork work, CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Ranking and statistical sensitivity"));
        cancellationToken.ThrowIfCancellationRequested();
        StudyReportArtifacts result = StudyPlatformWorkspace.Open(work.Root).GenerateReport(work.Output, work.Options);
        cancellationToken.ThrowIfCancellationRequested(); progress.Report((total, total, "Report package complete")); return result;
    }
    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, StudyReportWork work, StudyReportArtifacts result)
    {
        string report = string.Create(CultureInfo.InvariantCulture,
            $"VAHMAN Study/Optimization/Report 0.16.0{Environment.NewLine}" +
            $"Table: {result.VariantCount:N0} evaluated; {result.ParetoCount:N0} feasible Pareto variants{Environment.NewLine}" +
            $"Sensitivity: Spearman ρ; {work.Options.BootstrapSamples:N0} bootstrap; {work.Options.PermutationSamples:N0} permutations; CI {work.Options.ConfidenceLevel:P1}; Holm α {work.Options.Alpha:G4}{Environment.NewLine}" +
            $"Artifacts: embedded JSON + Apache Parquet + glTF 2.0 + two network-free HTML files{Environment.NewLine}" +
            $"Analysis hash: {result.AnalysisHash}");
        data.SetData(0, result); data.SetData(1, result.Report); data.SetData(2, result.Viewer);
        data.SetData(3, result.Parquet); data.SetData(4, result.Gltf); data.SetData(5, result.Data);
        data.SetData(6, result.AnalysisHash); data.SetData(7, report); Message = $"REPORT\n{result.ParetoCount} Pareto";
    }
}

/// <summary>Captured report request.</summary>
public readonly record struct StudyReportWork(string Root, string Output, StudyReportOptions Options);
