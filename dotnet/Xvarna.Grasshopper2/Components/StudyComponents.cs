using System.Text.Json;
using System.Globalization;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

public sealed record StudyRankRequest
{
    public StudyProblem Problem { get; init; } = new() { Variables = [], ObjectiveDirections = [] };
    public IReadOnlyList<StudyCandidate> Candidates { get; init; } = [];
    public IReadOnlyList<StudyEvaluation> Evaluations { get; init; } = [];
}

[IoId("fb9a0879-81ea-4738-8945-1be9ba5f9ca8")]
public sealed class StudyRankComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public StudyRankComponent() : base(new Nomen("XVARNA Study", "Feasibility-first Pareto rank, hypervolume and sensitivity.", "XVARNA", "05 Study")) { }
    public StudyRankComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Study JSON", "JSON", "StudyProblem, candidates and evaluations.").Set("{}"); inputs.AddNumber("Reference Objective 1", "R1", "2D hypervolume reference.").Set(1); inputs.AddNumber("Reference Objective 2", "R2", "2D hypervolume reference.").Set(1); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Ranked", "R", "Typed ranked result."); outputs.AddText("Result JSON", "JSON", "Portable result."); outputs.AddText("Pareto IDs", "P", "Non-dominated candidate IDs.", Access.Twig); outputs.AddNumber("Hypervolume", "HV", "Exact 2D hypervolume when two objectives exist."); outputs.AddText("Sensitivity JSON", "S", "Spearman association summary."); outputs.AddText("Hash", "H", "Result identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string json); access.GetItem(1, out double r1); access.GetItem(2, out double r2); try { StudyRankRequest q = Gh2Json.Read<StudyRankRequest>(json); RankedStudyResult r = StudyAnalyzer.Rank(q.Problem, q.Candidates, q.Evaluations); double hv = q.Problem.ObjectiveDirections.Count == 2 ? StudyAnalyzer.Hypervolume2d(q.Problem, q.Candidates, q.Evaluations, r1, r2) : double.NaN; IReadOnlyList<StudySensitivity> sensitivity = StudyAnalyzer.SpearmanSensitivity(q.Problem, q.Candidates, q.Evaluations); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); Gh2Data.SetTwig(access, 2, r.ParetoCandidateIds.Select(v => v.ToString()).ToArray()); access.SetItem(3, hv); access.SetItem(4, Gh2Json.Write(sensitivity)); access.SetItem(5, r.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Study ranking failed", e.Message); } }
}

public sealed record OptimizerStartRequest
{
    public StudyProblem Problem { get; init; } = new() { Variables = [], ObjectiveDirections = [] };
    public MultiObjectiveOptimizerOptions Options { get; init; } = MultiObjectiveOptimizerOptions.Default;
}

[IoId("59ef17be-41b6-4d5c-a598-c33a7ecdfdb9")]
public sealed class OptimizerStartComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private XvarnaOptimizer? optimizer;
    public OptimizerStartComponent() : base(new Nomen("XVARNA Optimizer", "Starts a deterministic mixed-variable NSGA-II ask/tell run.", "XVARNA", "05 Study")) { }
    public OptimizerStartComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Optimizer JSON", "JSON", "StudyProblem and optimizer options.").Set("{}"); inputs.AddBoolean("Restart", "R", "True creates a fresh deterministic optimizer.").Set(true); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Optimizer", "O", "Stateful optimizer handle."); outputs.AddGeneric("Batch", "B", "Candidates to evaluate."); outputs.AddText("Batch JSON", "JSON", "Portable candidate batch."); outputs.AddNumber("Generation", "G", "Batch generation."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string json); access.GetItem(1, out bool restart); try { OptimizerStartRequest q = Gh2Json.Read<OptimizerStartRequest>(json); if (optimizer is null || restart) { optimizer?.Dispose(); optimizer = new(q.Problem, q.Options); } OptimizerBatch batch = optimizer.Ask(); access.SetItem(0, optimizer); access.SetItem(1, batch); access.SetItem(2, Gh2Json.Write(batch)); access.SetItem(3, (double)batch.Generation); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Optimizer start failed", e.Message); } }
}

[IoId("0f59c78c-36f6-4fa0-9cbd-1621438f27cc")]
public sealed class OptimizerStepComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public OptimizerStepComponent() : base(new Nomen("XVARNA Optimizer Step", "Commits evaluations and asks the next candidate batch.", "XVARNA", "05 Study")) { }
    public OptimizerStepComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Optimizer", "O", "XVARNA Optimizer handle."); inputs.AddText("Evaluations JSON", "JSON", "Aligned StudyEvaluation array.").Set("[]"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Snapshot", "S", "Population and archive snapshot."); outputs.AddText("Snapshot JSON", "JSON", "Portable snapshot."); outputs.AddGeneric("Next Batch", "B", "Next candidates to evaluate."); outputs.AddText("Next JSON", "N", "Portable next batch."); outputs.AddText("Hash", "H", "Snapshot identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object wrapped); access.GetItem(1, out string json); if (wrapped is not XvarnaOptimizer optimizer) { access.AddError("Invalid optimizer", "Connect XVARNA Optimizer."); return; } try { StudyEvaluation[] evaluations = Gh2Json.Read<StudyEvaluation[]>(json); OptimizerSnapshot snapshot = optimizer.Tell(evaluations); OptimizerBatch next = optimizer.Ask(); access.SetItem(0, snapshot); access.SetItem(1, Gh2Json.Write(snapshot)); access.SetItem(2, next); access.SetItem(3, Gh2Json.Write(next)); access.SetItem(4, snapshot.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Optimizer step failed", e.Message); } }
}

[IoId("b0ea0f05-bce9-44dc-af16-3310454e6251")]
public sealed class StudyManifestComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public StudyManifestComponent() : base(new Nomen("XVARNA Manifest", "Validates and canonicalizes a versioned Study Manifest.", "XVARNA", "05 Study")) { }
    public StudyManifestComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Study ID", "ID", "Stable machine/project identity.").Set("study-001"); inputs.AddText("Title", "T", "Human-readable report title.").Set("XVARNA Study");
        inputs.AddText("Description", "D", "Scientific scope.").Set(string.Empty); inputs.AddText("Parameter Names", "PN", "Ordered parameter names.", Access.Twig);
        inputs.AddNumber("Lower Bounds", "L", "One lower bound per parameter.", Access.Twig); inputs.AddNumber("Upper Bounds", "U", "One upper bound per parameter.", Access.Twig);
        inputs.AddInteger("Parameter Kinds", "PK", "0 continuous, 1 integer; empty/one/aligned.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Parameter Units", "PU", "Empty, one, or aligned units.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Objective Names", "ON", "Ordered scalar objective names.", Access.Twig); inputs.AddInteger("Directions", "Dir", "0 minimize, 1 maximize; empty/one/aligned.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Objective Units", "OU", "Empty, one, or aligned units.", Access.Twig, Requirement.MayBeMissing); inputs.AddInteger("Population", "Pop", "NSGA-II parent population.").Set(64);
        inputs.AddInteger("Offspring", "Off", "Candidates per generation.").Set(64); inputs.AddInteger("Archive", "Arc", "Pareto archive capacity.").Set(256);
        inputs.AddText("Seed", "Seed", "Unsigned deterministic seed.").Set("42"); inputs.AddText("Advanced JSON", "JSON", "Optional full vector/categorical/constraint manifest override.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Manifest", "M", "Typed manifest."); outputs.AddText("Canonical JSON", "JSON", "Validated canonical manifest."); outputs.AddText("JSON Schema", "Schema", "Draft 2020-12 schema."); outputs.AddText("Report", "R", "Registry summary."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string studyId); access.GetItem(1, out string title); access.GetItem(2, out string description); string[] names = [], parameterUnits = [], objectives = [], objectiveUnits = [];
        double[] lower = [], upper = []; int[] kinds = [], directions = []; access.GetItemArray(3, out names); access.GetItemArray(4, out lower); access.GetItemArray(5, out upper);
        if (!access.GetNull(6)) access.GetItemArray(6, out kinds); if (!access.GetNull(7)) access.GetItemArray(7, out parameterUnits); access.GetItemArray(8, out objectives);
        if (!access.GetNull(9)) access.GetItemArray(9, out directions); if (!access.GetNull(10)) access.GetItemArray(10, out objectiveUnits); access.GetItem(11, out int population);
        access.GetItem(12, out int offspring); access.GetItem(13, out int archive); access.GetItem(14, out string seedText); string? json = null; if (!access.GetNull(15)) access.GetItem(15, out json);
        try
        {
            StudyPlatformManifest manifest;
            if (!string.IsNullOrWhiteSpace(json)) manifest = Gh2Json.Read<StudyPlatformManifest>(json);
            else
            {
                if (names.Length == 0 || names.Length != lower.Length || names.Length != upper.Length) throw new ArgumentException("Parameter names and lower/upper bounds must have the same non-zero count.");
                if (objectives.Length == 0) throw new ArgumentException("At least one objective name is required.");
                if (!Gh2AnalysisAdapters.Compatible(kinds.Length, names.Length) || !Gh2AnalysisAdapters.Compatible(parameterUnits.Length, names.Length)
                    || !Gh2AnalysisAdapters.Compatible(directions.Length, objectives.Length) || !Gh2AnalysisAdapters.Compatible(objectiveUnits.Length, objectives.Length)) throw new ArgumentException("Kinds, directions, and units must be empty, one item, or aligned.");
                manifest = new()
                {
                    StudyId = studyId,
                    Title = title,
                    Description = description,
                    CreatedUtc = DateTimeOffset.UtcNow,
                    Parameters = names.Select((name, i) => new StudyParameterDefinition { ParameterId = checked((ulong)i + 1), Name = name, Kind = (DesignVariableKind)Gh2AnalysisAdapters.Select(kinds, i, 0), LowerBound = lower[i], UpperBound = upper[i], Unit = Gh2AnalysisAdapters.Select(parameterUnits, i, string.Empty) }).ToArray(),
                    Objectives = objectives.Select((name, i) => new StudyObjectiveDefinition { ObjectiveId = checked((ulong)i + 1), Name = name, Direction = (ObjectiveDirection)Gh2AnalysisAdapters.Select(directions, i, 0), Unit = Gh2AnalysisAdapters.Select(objectiveUnits, i, string.Empty) }).ToArray(),
                    Optimizer = new(population, offspring, archive, Gh2AnalysisAdapters.ParseUnsigned(seedText, "Seed")),
                };
            }
            string canonical = StudyPlatformWorkspace.SerializeManifest(manifest); StudyPlatformManifest normalized = Gh2Json.Read<StudyPlatformManifest>(canonical); access.SetItem(0, normalized); access.SetItem(1, canonical); access.SetItem(2, StudyPlatformWorkspace.ManifestSchemaJson); access.SetItem(3, $"study {normalized.StudyId}; {normalized.Parameters.Count:N0} parameters, {normalized.Objectives.Sum(v => Math.Max(1, v.Components.Count)):N0} objective values, {normalized.Constraints.Count:N0} constraints; schema {normalized.SchemaVersion}");
        }
        catch (Exception e) when (e is JsonException or ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException) { access.AddError("Manifest validation failed", e.Message); }
    }
}

[IoId("eb7df94d-8891-44cf-b3f1-a5ee4b36c1ae")]
public sealed class StudyWorkspaceComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private StudyPlatformWorkspace? workspace;
    private string? currentRoot;
    public StudyWorkspaceComponent() : base(new Nomen("XVARNA Workspace", "Creates or resumes a journaled Study workspace.", "XVARNA", "05 Study")) { }
    public StudyWorkspaceComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Directory", "Dir", "Workspace directory."); inputs.AddGeneric("Manifest", "M", "Manifest from XVARNA Manifest.", Access.Item, Requirement.MayBeMissing); inputs.AddBoolean("Create", "New", "Create when absent; otherwise open.").Set(true); inputs.AddBoolean("Begin Batch", "Ask", "Begin/resume the active optimizer batch.").Set(true); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Workspace", "W", "Durable workspace handle."); outputs.AddGeneric("State", "S", "Current workspace state."); outputs.AddText("State JSON", "JSON", "Portable state."); outputs.AddGeneric("Batch", "B", "Active/new batch."); outputs.AddText("Report", "R", "Revision and persistence hashes."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string directory); object? mo = null; if (!access.GetNull(1)) access.GetItem(1, out mo); access.GetItem(2, out bool create); access.GetItem(3, out bool begin); try { string root = System.IO.Path.GetFullPath(directory); if (workspace is null || !string.Equals(currentRoot, root, StringComparison.OrdinalIgnoreCase)) { workspace = create && mo is StudyPlatformManifest manifest && !Directory.Exists(root) ? StudyPlatformWorkspace.Create(root, manifest) : StudyPlatformWorkspace.Open(root); currentRoot = root; } StudyWorkspaceState state = workspace.Refresh(); StudyPlatformBatch? batch = begin ? workspace.BeginBatch() : null; access.SetItem(0, workspace); access.SetItem(1, state); access.SetItem(2, Gh2Json.Write(state)); access.SetItem(3, batch!); access.SetItem(4, $"revision {state.Revision:N0}; generation {state.Generation:N0}; evaluations {state.EvaluationCount:N0}; variants {state.CompletedCount:N0}/{state.VariantCount:N0}; ledger {state.LedgerHash}; checkpoint {state.CheckpointHash}"); } catch (Exception e) when (e is ArgumentException or IOException or XvarnaNativeException or InvalidOperationException) { access.AddError("Workspace failed", e.Message); } }
}

[IoId("33df6840-624c-48a5-a313-44ac1d5c39b7")]
public sealed class StudyCommitComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public StudyCommitComponent() : base(new Nomen("XVARNA Commit", "Atomically commits one evaluated Study batch.", "XVARNA", "05 Study")) { }
    public StudyCommitComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Workspace", "W", "XVARNA Workspace."); inputs.AddText("Batch ID", "B", "Unsigned active batch ID."); inputs.AddText("Candidate IDs", "ID", "One ID per evaluated candidate.", Access.Twig); inputs.AddNumber("Objectives", "F", "Flattened candidate-major scalar/vector objective values.", Access.Twig); inputs.AddInteger("Objective Widths", "FW", "One width per manifest objective; default is one scalar objective.", Access.Twig, Requirement.MayBeMissing); inputs.AddNumber("Constraints", "G", "Optional flattened candidate-major residuals.", Access.Twig, Requirement.MayBeMissing); inputs.AddText("Advanced JSON", "JSON", "Optional full evaluations override including metric fields.", Access.Item, Requirement.MayBeMissing); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("State", "S", "Committed workspace state."); outputs.AddText("State JSON", "JSON", "Portable state."); outputs.AddText("Ledger Hash", "H", "Hash-protected ledger identity."); outputs.AddText("Report", "R", "Commit summary."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object wrapped); access.GetItem(1, out string batchText); string[] ids = []; double[] objectives = [], constraints = []; int[] widths = []; access.GetItemArray(2, out ids); access.GetItemArray(3, out objectives); if (!access.GetNull(4)) access.GetItemArray(4, out widths); if (!access.GetNull(5)) access.GetItemArray(5, out constraints); string? json = null; if (!access.GetNull(6)) access.GetItem(6, out json); if (wrapped is not StudyPlatformWorkspace workspace) { access.AddError("Invalid workspace", "Connect XVARNA Workspace."); return; } try { StudyPlatformEvaluation[] evaluations; if (!string.IsNullOrWhiteSpace(json)) evaluations = Gh2Json.Read<StudyPlatformEvaluation[]>(json); else { if (ids.Length == 0) throw new ArgumentException("Candidate IDs are required."); if (widths.Length == 0) widths = [1]; if (widths.Any(v => v <= 0)) throw new ArgumentException("Objective widths must be positive."); int objectiveRowWidth = widths.Sum(); if (objectives.Length != ids.Length * objectiveRowWidth) throw new ArgumentException("Objectives must contain candidate-count × sum(Objective Widths) values."); if (constraints.Length != 0 && constraints.Length % ids.Length != 0) throw new ArgumentException("Constraints must be empty or rectangular candidate-major values."); int constraintWidth = constraints.Length == 0 ? 0 : constraints.Length / ids.Length; evaluations = ids.Select((id, row) => { int offset = row * objectiveRowWidth; int cursor = offset; IReadOnlyList<double>[] groups = widths.Select(width => { double[] values = objectives.Skip(cursor).Take(width).ToArray(); cursor += width; return (IReadOnlyList<double>)values; }).ToArray(); return new StudyPlatformEvaluation { VariantId = Gh2AnalysisAdapters.ParseUnsigned(id, "Candidate ID"), ObjectiveValues = groups, ConstraintResiduals = constraints.Skip(row * constraintWidth).Take(constraintWidth).ToArray() }; }).ToArray(); } SolutionCancellationToken.ThrowIfCancellationRequested(); StudyWorkspaceState state = workspace.CommitBatch(ulong.Parse(batchText, CultureInfo.InvariantCulture), evaluations); access.SetItem(0, state); access.SetItem(1, Gh2Json.Write(state)); access.SetItem(2, state.LedgerHash); access.SetItem(3, $"atomic idempotent commit; revision {state.Revision:N0}; {state.CompletedCount:N0}/{state.VariantCount:N0} variants complete"); } catch (Exception e) when (e is JsonException or ArgumentException or IOException or XvarnaNativeException or InvalidOperationException or OverflowException or OperationCanceledException) { access.AddError("Commit failed", e.Message); } }
}

[IoId("d99e587a-98ae-478d-b853-41868cedc715")]
public sealed class StudyReportComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public StudyReportComponent() : base(new Nomen("XVARNA Report", "Generates self-contained HTML, Parquet, glTF and viewer.", "XVARNA", "05 Study")) { }
    public StudyReportComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Workspace", "W", "XVARNA Workspace."); inputs.AddText("Output Directory", "Dir", "Report destination."); inputs.AddText("Options JSON", "JSON", "StudyReportOptions.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Artifacts", "A", "Typed artifact paths and counts."); outputs.AddText("HTML", "HTML", "Self-contained report path."); outputs.AddText("Viewer", "View", "Network-free viewer path."); outputs.AddText("Parquet", "PQ", "Columnar data path."); outputs.AddText("glTF", "GLTF", "Objective-space scene path."); outputs.AddText("Hash", "H", "Analysis identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object wrapped); access.GetItem(1, out string directory); access.GetItem(2, out string json); if (wrapped is not StudyPlatformWorkspace workspace) { access.AddError("Invalid workspace", "Connect XVARNA Workspace."); return; } try { StudyReportOptions options = string.IsNullOrWhiteSpace(json) || json.Trim() == "{}" ? StudyReportOptions.Default : Gh2Json.Read<StudyReportOptions>(json); StudyReportArtifacts r = workspace.GenerateReport(System.IO.Path.GetFullPath(directory), options); access.SetItem(0, r); access.SetItem(1, r.Report); access.SetItem(2, r.Viewer); access.SetItem(3, r.Parquet); access.SetItem(4, r.Gltf); access.SetItem(5, r.AnalysisHash); } catch (Exception e) when (e is JsonException or ArgumentException or IOException or XvarnaNativeException or InvalidOperationException) { access.AddError("Report generation failed", e.Message); } }
}

public sealed record MultiFidelityRequest
{
    public IReadOnlyList<ObjectiveDirection> Directions { get; init; } = [];
    public IReadOnlyList<ScientificCandidate> Candidates { get; init; } = [];
    public IReadOnlyList<FidelityObservation> Observations { get; init; } = [];
    public MultiFidelityPolicy Policy { get; init; } = new() { ReferencePoint = [] };
}

[IoId("45e96e48-72ec-43bb-a7d4-293b3be0416c")]
public sealed class MultiFidelityComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public MultiFidelityComponent() : base(new Nomen("XVARNA Fidelity", "Cost-aware transparent Fast-to-Reference selection.", "XVARNA", "05 Study")) { }
    public MultiFidelityComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) => inputs.AddText("Request JSON", "JSON", "Directions, candidates, observations and policy.").Set("{}");
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Selection", "S", "Calibrations and recommendations."); outputs.AddText("Result JSON", "JSON", "Portable complete result."); outputs.AddText("Recommended IDs", "ID", "Candidates selected for reference evaluation.", Access.Twig); outputs.AddText("Hash", "H", "Selection identity."); outputs.AddText("Report", "R", "Transparent model boundary."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string json); try { MultiFidelityRequest q = Gh2Json.Read<MultiFidelityRequest>(json); MultiFidelitySelection r = EvidenceEngine.RecommendReferenceEvaluations(q.Directions, q.Candidates, q.Observations, q.Policy); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); Gh2Data.SetTwig(access, 2, r.Recommendations.Select(v => v.CandidateId.ToString()).ToArray()); access.SetItem(3, r.ContentHash); access.SetItem(4, "linear discrepancy calibration with deterministic Monte-Carlo EHVI per reference cost; no Gaussian-process or qNEHVI claim"); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Multi-fidelity selection failed", e.Message); } }
}
