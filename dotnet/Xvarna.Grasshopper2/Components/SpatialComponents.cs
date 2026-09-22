using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Rhino.Geometry;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

internal static class Gh2Json
{
    internal static JsonSerializerOptions Options { get; } = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true,
        NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    };

    internal static T Read<T>(string json) => JsonSerializer.Deserialize<T>(json, Options)
        ?? throw new ArgumentException("Request JSON is empty.");
    internal static string Write<T>(T value) => JsonSerializer.Serialize(value, Options);
}

public abstract class SceneJsonComponent<TRequest, TResult> : Xvarna.Grasshopper2.Interop.XvarnaComponent where TResult : class
{
    protected SceneJsonComponent(Nomen nomen) : base(nomen) { }
    protected SceneJsonComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "Scene from XVARNA Scene."); inputs.AddText("Request JSON", "JSON", "Typed, versionable request object.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Typed immutable result."); outputs.AddText("Result JSON", "JSON", "Portable complete result."); outputs.AddText("Hash", "H", "Deterministic result identity."); outputs.AddText("Report", "Info", "Method and result summary."); }
    protected abstract TResult Execute(XvarnaScene scene, TRequest request);
    protected abstract string Identity(TResult result);
    protected virtual string Report(TResult result) => $"{typeof(TResult).Name}; deterministic native analysis; hash {Identity(result)}";
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); access.GetItem(1, out string json); if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try { TRequest request = Gh2Json.Read<TRequest>(json); TResult result = Execute(scene, request); access.SetItem(0, result); access.SetItem(1, Gh2Json.Write(result)); access.SetItem(2, Identity(result)); access.SetItem(3, Report(result)); }
        catch (Exception exception) when (exception is JsonException or ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException or ObjectDisposedException or IOException) { access.AddError("Analysis failed", exception.Message); }
    }
}

public sealed record IsovistRequest
{
    public IReadOnlyList<PlanarViewpoint> Viewpoints { get; init; } = [];
    public IsovistOptions Options { get; init; } = new(720, 360, 100, 1e-4, ulong.MaxValue);
}

[IoId("7551860d-e9fc-4b1e-b2a8-f26e5a95f044")]
public sealed class IsovistComponent : SceneJsonComponent<IsovistRequest, IsovistResult>
{
    public IsovistComponent() : base(new Nomen("XVARNA Isovist", "Planar isovists with convergence and attribution.", "XVARNA", "04 Visibility")) { }
    public IsovistComponent(IReader reader) : base(reader) { }
    protected override IsovistResult Execute(XvarnaScene scene, IsovistRequest request) => scene.AnalyzeIsovists(request.Viewpoints, request.Options);
    protected override string Identity(IsovistResult result) => result.ContentHash;
    protected override string Report(IsovistResult result) => $"{result.Viewpoints.Count:N0} viewpoints × {result.Options.SampleCount:N0} rays; {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; ordered boundaries and first blockers";
}

public sealed record IntervisibilityRequest
{
    public IReadOnlyList<VisibilityObserverPoint> Observers { get; init; } = [];
    public IReadOnlyList<VisibilityTargetPoint> Targets { get; init; } = [];
    public IntervisibilityOptions Options { get; init; } = new(1e-4, double.PositiveInfinity, 10, 1, ulong.MaxValue);
}

[IoId("5f88d25b-34de-4946-b315-c7288f101e80")]
public sealed class IntervisibilityComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public IntervisibilityComponent() : base(new Nomen("XVARNA Intervisibility", "Directed visibility and transparent privacy screening.", "XVARNA", "04 Visibility")) { }
    public IntervisibilityComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "XVARNA Scene.");
        inputs.AddPoint("Observers", "O", "Observer points.", Access.Twig);
        inputs.AddPoint("Targets", "T", "View/privacy target points.", Access.Twig);
        inputs.AddVector("Target Facings", "F", "Empty, one, or one facing per target.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Observer Weights", "OW", "Empty, one, or one weight per observer.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Target Sensitivities", "TS", "Empty, one, or one sensitivity per target.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Endpoint Clearance", "Eps", "Endpoint clearance in model units.").Set(1e-4);
        inputs.AddNumber("Maximum Distance", "Max", "Zero means unbounded.").Set(0);
        inputs.AddNumber("Privacy Reference", "Ref", "Distance at risk factor 0.5.").Set(10);
        inputs.AddNumber("Facing Exponent", "Exp", "Directional sensitivity exponent.").Set(1);
        inputs.AddText("Category Mask", "C", "Unsigned mask; -1 selects all.").Set("-1");
        inputs.AddText("Advanced JSON", "JSON", "Optional full request override for automation/import.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Result", "R", "Complete directed matrix.");
        outputs.AddInteger("States", "S", "Row-major visibility state.", Access.Twig);
        outputs.AddNumber("Distances", "D", "Row-major distances.", Access.Twig);
        outputs.AddNumber("Privacy Risk", "PR", "Row-major privacy risk.", Access.Twig);
        outputs.AddText("Blocker IDs", "OID", "Row-major first blocker IDs.", Access.Twig);
        outputs.AddNumber("Observer Visibility", "OV", "Visible fraction per observer.", Access.Twig);
        outputs.AddNumber("Target Exposure", "TE", "Exposure per target.", Access.Twig);
        outputs.AddText("Hash", "H", "Deterministic identity.");
        outputs.AddText("Report", "Info", "Method and matrix summary.");
    }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); Point3d[] observers = [], targets = []; Vector3d[] facings = [];
        double[] weights = [], sensitivities = []; access.GetItemArray(1, out observers); access.GetItemArray(2, out targets);
        if (!access.GetNull(3)) access.GetItemArray(3, out facings); if (!access.GetNull(4)) access.GetItemArray(4, out weights);
        if (!access.GetNull(5)) access.GetItemArray(5, out sensitivities); access.GetItem(6, out double clearance);
        access.GetItem(7, out double maximum); access.GetItem(8, out double reference); access.GetItem(9, out double exponent);
        access.GetItem(10, out string category); string? json = null; if (!access.GetNull(11)) access.GetItem(11, out json);
        if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try
        {
            IntervisibilityRequest q;
            if (!string.IsNullOrWhiteSpace(json)) q = Gh2Json.Read<IntervisibilityRequest>(json);
            else
            {
                if (observers.Length == 0 || targets.Length == 0) throw new ArgumentException("Observers and targets are required.");
                if (!Gh2AnalysisAdapters.Compatible(facings.Length, targets.Length) || !Gh2AnalysisAdapters.Compatible(weights.Length, observers.Length)
                    || !Gh2AnalysisAdapters.Compatible(sensitivities.Length, targets.Length)) throw new ArgumentException("Facings, weights, and sensitivities must broadcast or align.");
                q = new()
                {
                    Observers = observers.Select((p, i) => new VisibilityObserverPoint(checked((ulong)i + 1), p.X, p.Y, p.Z, Gh2AnalysisAdapters.Select(weights, i, 1.0))).ToArray(),
                    Targets = targets.Select((p, i) => { Vector3d f = Gh2AnalysisAdapters.Select(facings, i, Vector3d.Zero); return new VisibilityTargetPoint(checked((ulong)i + 1), p.X, p.Y, p.Z, Gh2AnalysisAdapters.Select(sensitivities, i, 1.0), facings.Length > 0, f.X, f.Y, f.Z); }).ToArray(),
                    Options = new(clearance, maximum == 0 ? double.PositiveInfinity : maximum, reference, exponent, Gh2AnalysisAdapters.ParseUnsigned(category, "Category mask", true)),
                };
            }
            SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(5);
            IntervisibilityResult r = scene.AnalyzeIntervisibility(q.Observers, q.Targets, q.Options);
            SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(100); access.SetItem(0, r);
            Gh2Data.SetTwig(access, 1, r.Entries.Select(v => (int)v.State).ToArray()); Gh2Data.SetTwig(access, 2, r.Entries.Select(v => v.Distance).ToArray());
            Gh2Data.SetTwig(access, 3, r.Entries.Select(v => v.PrivacyRisk).ToArray()); Gh2Data.SetTwig(access, 4, r.Entries.Select(v => v.BlockerObjectId == 0 ? string.Empty : v.BlockerObjectId.ToString(CultureInfo.InvariantCulture)).ToArray());
            Gh2Data.SetTwig(access, 5, r.ObserverSummaries.Select(v => v.VisibleFraction).ToArray()); Gh2Data.SetTwig(access, 6, r.TargetSummaries.Select(v => v.ExposureFraction).ToArray());
            access.SetItem(7, r.ContentHash); access.SetItem(8, $"{r.Observers.Count:N0} observers × {r.Targets.Count:N0} targets; directed visibility/privacy; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms");
        }
        catch (Exception e) when (e is JsonException or ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException or OperationCanceledException) { access.AddError("Intervisibility failed", e.Message); }
    }
}

public sealed record VisibilityGraphRequest
{
    public IReadOnlyList<VisibilityNodePoint> Nodes { get; init; } = [];
    public bool Sparse { get; init; }
    public uint MaximumNeighbors { get; init; } = 16;
    public double EndpointClearance { get; init; } = 1e-4;
    public double MaximumDistance { get; init; } = double.PositiveInfinity;
    public ulong CategoryMask { get; init; } = ulong.MaxValue;
    public bool ComputeCentrality { get; init; } = true;
}

[IoId("e9bcdb86-b0af-46fc-9c02-e3852c0fc6e6")]
public sealed class VisibilityGraphComponent : SceneJsonComponent<VisibilityGraphRequest, VisibilityGraphResult>
{
    public VisibilityGraphComponent() : base(new Nomen("XVARNA Visibility Graph", "Dense or bounded sparse visibility topology.", "XVARNA", "04 Visibility")) { }
    public VisibilityGraphComponent(IReader reader) : base(reader) { }
    protected override VisibilityGraphResult Execute(XvarnaScene scene, VisibilityGraphRequest r) => r.Sparse
        ? scene.AnalyzeSparseVisibilityGraph(r.Nodes, new(r.EndpointClearance, r.MaximumDistance, r.CategoryMask, r.MaximumNeighbors, r.ComputeCentrality))
        : scene.AnalyzeVisibilityGraph(r.Nodes, new(r.EndpointClearance, r.MaximumDistance, r.CategoryMask, r.ComputeCentrality));
    protected override string Identity(VisibilityGraphResult result) => result.ContentHash;
}

public sealed record TargetViewRequest
{
    public IReadOnlyList<ViewCamera> Cameras { get; init; } = [];
    public IReadOnlyList<ViewTargetTriangle> Targets { get; init; } = [];
    public TargetViewOptions Options { get; init; } = TargetViewOptions.Default;
}

[IoId("19ad71f3-ccf4-47b7-878b-12bf8414b923")]
public sealed class TargetViewComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public TargetViewComponent() : base(new Nomen("XVARNA Target View", "Solid-angle Target, Weighted and Green View quality.", "XVARNA", "04 Visibility")) { }
    public TargetViewComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddPoint("Eyes", "E", "Camera positions.", Access.Twig);
        inputs.AddVector("Forward", "F", "One or one per eye.", Access.Twig); inputs.AddVector("Up", "U", "Empty, one, or one per eye.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddMesh("Target Meshes", "T", "Meshes whose visible solid angle is measured.", Access.Twig);
        inputs.AddText("Target IDs", "ID", "Empty, one, or one ID per mesh.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Target Categories", "C", "Empty, one, or one category mask per mesh.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Target Weights", "W", "Empty, one, or one weight per mesh.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Green Mask", "G", "Category bits counted as green.").Set("1"); inputs.AddInteger("Samples/Patch", "N", "Deterministic samples per triangle.").Set(32);
        inputs.AddNumber("Horizontal FOV", "HF", "Degrees.").Set(90); inputs.AddNumber("Vertical FOV", "VF", "Degrees.").Set(60);
        inputs.AddNumber("Maximum Distance", "Max", "Model units.").Set(100); inputs.AddNumber("Distance Reference", "D50", "Half-weight distance.").Set(25);
        inputs.AddNumber("Distance Exponent", "DE", "Falloff exponent.").Set(2); inputs.AddNumber("Direction Exponent", "FE", "Camera-centre exponent.").Set(1);
        inputs.AddBoolean("Two Sided", "2S", "Evaluate both target sides.").Set(false);
        inputs.AddText("Advanced JSON", "JSON", "Optional full request override.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Result", "R", "Complete result."); outputs.AddNumber("Target View", "TV", "Visible target/FOV solid angle.", Access.Twig);
        outputs.AddNumber("Weighted View", "WV", "Weighted view score.", Access.Twig); outputs.AddNumber("Green View", "GVI", "Green view index.", Access.Twig);
        outputs.AddNumber("Green Share", "GS", "Green share of visible targets.", Access.Twig); outputs.AddText("Dominant Target", "TID", "Dominant target ID.", Access.Twig);
        outputs.AddText("Hash", "H", "Deterministic identity."); outputs.AddText("Report", "Info", "Method/backend summary.");
    }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); Point3d[] eyes = []; Vector3d[] forward = [], up = []; Mesh[] meshes = [];
        string[] ids = [], categories = []; double[] weights = []; access.GetItemArray(1, out eyes); access.GetItemArray(2, out forward);
        if (!access.GetNull(3)) access.GetItemArray(3, out up); access.GetItemArray(4, out meshes); if (!access.GetNull(5)) access.GetItemArray(5, out ids);
        if (!access.GetNull(6)) access.GetItemArray(6, out categories); if (!access.GetNull(7)) access.GetItemArray(7, out weights);
        access.GetItem(8, out string green); access.GetItem(9, out int samples); access.GetItem(10, out double hfov); access.GetItem(11, out double vfov);
        access.GetItem(12, out double maximum); access.GetItem(13, out double d50); access.GetItem(14, out double de); access.GetItem(15, out double fe);
        access.GetItem(16, out bool twoSided); string? json = null; if (!access.GetNull(17)) access.GetItem(17, out json);
        if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try
        {
            TargetViewRequest q;
            if (!string.IsNullOrWhiteSpace(json)) q = Gh2Json.Read<TargetViewRequest>(json);
            else
            {
                if (eyes.Length == 0 || forward.Length == 0 || meshes.Length == 0) throw new ArgumentException("Eyes, forward vectors, and target meshes are required.");
                if (!Gh2AnalysisAdapters.Compatible(forward.Length, eyes.Length) || !Gh2AnalysisAdapters.Compatible(up.Length, eyes.Length)) throw new ArgumentException("Forward and Up must broadcast or align with eyes.");
                q = new()
                {
                    Cameras = eyes.Select((p, i) => new ViewCamera(checked((ulong)i + 1), Gh2AnalysisAdapters.Point(p), Gh2AnalysisAdapters.Vector(Gh2AnalysisAdapters.Select(forward, i, Vector3d.XAxis)), Gh2AnalysisAdapters.Vector(Gh2AnalysisAdapters.Select(up, i, Vector3d.ZAxis)))).ToArray(),
                    Targets = Gh2AnalysisAdapters.TriangulateTargets(meshes, ids, categories, weights),
                    Options = new(checked((uint)samples), hfov, vfov, maximum, 1e-4, ulong.MaxValue, ulong.MaxValue, Gh2AnalysisAdapters.ParseUnsigned(green, "Green mask", true), d50, de, fe, twoSided),
                };
            }
            SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(5); TargetViewResult r = scene.AnalyzeTargetView(q.Cameras, q.Targets, q.Options);
            SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(100); access.SetItem(0, r);
            Gh2Data.SetTwig(access, 1, r.Summaries.Select(v => v.TargetViewFraction).ToArray()); Gh2Data.SetTwig(access, 2, r.Summaries.Select(v => v.WeightedViewScore).ToArray());
            Gh2Data.SetTwig(access, 3, r.Summaries.Select(v => v.GreenViewIndex).ToArray()); Gh2Data.SetTwig(access, 4, r.Summaries.Select(v => v.GreenShareOfVisibleTargets).ToArray());
            Gh2Data.SetTwig(access, 5, r.Summaries.Select(v => v.DominantTargetId.ToString(CultureInfo.InvariantCulture)).ToArray()); access.SetItem(6, r.ContentHash);
            access.SetItem(7, $"{r.Summaries.Count:N0} observers; solid-angle Target/Weighted/Green View; backend {r.Execution?.Backend.ToString() ?? "CPU"}; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms");
        }
        catch (Exception e) when (e is JsonException or ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException or OperationCanceledException) { access.AddError("Target View failed", e.Message); }
    }
}

public sealed record CorridorRequest
{
    public IReadOnlyList<ViewCorridorDefinition> Corridors { get; init; } = [];
    public ViewCorridorOptions Options { get; init; } = new(256, 1e-4, ulong.MaxValue);
}

[IoId("9fb75738-dd69-4c47-bb97-963a46adab15")]
public sealed class ViewCorridorComponent : SceneJsonComponent<CorridorRequest, ViewCorridorResult>
{
    public ViewCorridorComponent() : base(new Nomen("XVARNA View Corridor", "Protected-aperture corridor visibility and conflicts.", "XVARNA", "04 Visibility")) { }
    public ViewCorridorComponent(IReader reader) : base(reader) { }
    protected override ViewCorridorResult Execute(XvarnaScene scene, CorridorRequest request) => scene.AnalyzeViewCorridors(request.Corridors, request.Options);
    protected override string Identity(ViewCorridorResult result) => result.ContentHash;
}

public sealed record ObserverPathRequest
{
    public IReadOnlyList<ObserverPathDefinition> Paths { get; init; } = [];
    public IReadOnlyList<ViewTargetTriangle> Targets { get; init; } = [];
    public ObserverPathOptions Options { get; init; } = ObserverPathOptions.Default;
}

[IoId("55795ea2-4e18-4979-91c3-a1eddb748701")]
public sealed class ObserverPathComponent : SceneJsonComponent<ObserverPathRequest, ObserverPathResult>
{
    public ObserverPathComponent() : base(new Nomen("XVARNA View Path", "Dynamic observer-path view quality and hotspots.", "XVARNA", "04 Visibility")) { }
    public ObserverPathComponent(IReader reader) : base(reader) { }
    protected override ObserverPathResult Execute(XvarnaScene scene, ObserverPathRequest request) => scene.AnalyzeObserverPaths(request.Paths, request.Targets, request.Options);
    protected override string Identity(ObserverPathResult result) => result.ContentHash;
}

internal sealed record Isovist3dRequest
{
    public IReadOnlyList<SpatialViewpoint> Viewpoints { get; init; } = [];
    public Isovist3dOptions Options { get; init; } = new(4096, 100, 1e-4, ulong.MaxValue, 10, 3, 8, 1e-4);
}

[IoId("47c40f7d-0c50-41b2-8559-a14fa2e6f3b9")]
public sealed class Isovist3dComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public Isovist3dComponent() : base(new Nomen("XVARNA Isovist 3D", "Material-aware 3D isovist, top-K and counterfactuals.", "XVARNA", "04 Visibility")) { }
    public Isovist3dComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Materials", "M", "XVARNA Materials.", Access.Item, Requirement.MayBeMissing); inputs.AddText("Request JSON", "JSON", "Viewpoints and Isovist3dOptions.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete result."); outputs.AddText("Result JSON", "JSON", "Portable result."); outputs.AddText("Top-K", "K", "Ranked obstruction attribution as JSON."); outputs.AddText("Categories", "C", "Category breakdown as JSON."); outputs.AddText("Hash", "H", "Result identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object s); object? m = null; if (!access.GetNull(1)) access.GetItem(1, out m); access.GetItem(2, out string json); if (s is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; } try { Isovist3dRequest q = Gh2Json.Read<Isovist3dRequest>(json); Isovist3dResult r = scene.AnalyzeIsovists3d(q.Viewpoints, m as AnalysisMaterialLibrary ?? AnalysisMaterialLibrary.Opaque, q.Options); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); access.SetItem(2, Gh2Json.Write(r.Attribution)); access.SetItem(3, Gh2Json.Write(r.Categories)); access.SetItem(4, r.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("3D Isovist failed", e.Message); } }
}

internal sealed record LandmarkRequest
{
    public IReadOnlyList<LandmarkObserver> Observers { get; init; } = [];
    public IReadOnlyList<Landmark> Landmarks { get; init; } = [];
    public LandmarkVisibilityOptions Options { get; init; } = new(256, 90, 60, 1000, 1e-4, ulong.MaxValue, 10, 8, 1e-4);
}

[IoId("1c937283-62f0-42a3-84ac-3d8ccb70f9fc")]
public sealed class LandmarkVisibilityComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public LandmarkVisibilityComponent() : base(new Nomen("XVARNA Landmark", "Material-aware landmark solid-angle visibility.", "XVARNA", "04 Visibility")) { }
    public LandmarkVisibilityComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Materials", "M", "XVARNA Materials.", Access.Item, Requirement.MayBeMissing); inputs.AddText("Request JSON", "JSON", "Observers, landmarks and options.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete result."); outputs.AddText("Result JSON", "JSON", "Portable result."); outputs.AddText("Attribution", "A", "Top-K attribution JSON."); outputs.AddText("Categories", "C", "Category breakdown JSON."); outputs.AddText("Hash", "H", "Result identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object s); object? m = null; if (!access.GetNull(1)) access.GetItem(1, out m); access.GetItem(2, out string json); if (s is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; } try { LandmarkRequest q = Gh2Json.Read<LandmarkRequest>(json); LandmarkVisibilityResult r = scene.AnalyzeLandmarkVisibility(q.Observers, q.Landmarks, m as AnalysisMaterialLibrary ?? AnalysisMaterialLibrary.Opaque, q.Options); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); access.SetItem(2, Gh2Json.Write(r.Attribution)); access.SetItem(3, Gh2Json.Write(r.Categories)); access.SetItem(4, r.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Landmark analysis failed", e.Message); } }
}

[IoId("e41c6334-13ca-4ca5-bd65-cf95e6d51398")]
public sealed class ScenarioCompareComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public ScenarioCompareComponent() : base(new Nomen("XVARNA Compare", "Aligned scenario deltas with attribution changes.", "XVARNA", "04 Visibility")) { }
    public ScenarioCompareComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Baseline Name", "A", "Baseline label.").Set("Baseline"); inputs.AddGeneric("Baseline", "AR", "Baseline 3D Isovist result."); inputs.AddText("Candidate Name", "B", "Candidate label.").Set("Candidate"); inputs.AddGeneric("Candidate", "BR", "Candidate 3D Isovist result."); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Comparison", "D", "Metric and attribution deltas."); outputs.AddText("JSON", "JSON", "Portable comparison."); outputs.AddText("Report", "R", "Aligned comparison summary."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string a); access.GetItem(1, out object ar); access.GetItem(2, out string b); access.GetItem(3, out object br); if (ar is not Isovist3dResult left || br is not Isovist3dResult right) { access.AddError("Invalid results", "Connect two XVARNA Isovist 3D results."); return; } try { SpatialScenarioComparison r = DaenaScenarios.Compare(a, left, b, right); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); access.SetItem(2, $"{r.Metrics.Count:N0} aligned viewpoints; {r.AttributionDeltas.Count:N0} object-attribution deltas"); } catch (ArgumentException e) { access.AddError("Comparison failed", e.Message); } }
}

internal sealed record SolarScenariosRequest
{
    public IReadOnlyList<SolarSensor> Sensors { get; init; } = [];
    public IReadOnlyList<ulong> ScenarioIds { get; init; } = [];
    public IReadOnlyList<string> Names { get; init; } = [];
    public SolarScenarioOptions Options { get; init; } = new(1e-4, double.PositiveInfinity, 0, ulong.MaxValue, 8, 1e-4, 10);
}

[IoId("fae40110-d73a-4949-bd4a-ac2256dde02b")]
public sealed class SolarScenariosComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SolarScenariosComponent() : base(new Nomen("XVARNA Solar Scenarios", "Material solar scenarios and delta attribution.", "XVARNA", "03 Environment")) { }
    public SolarScenariosComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Sun Set", "Sun", "XVARNA Sun Vectors."); inputs.AddGeneric("Material Libraries", "M", "One XVARNA Materials library per scenario.", Access.Twig); inputs.AddText("Request JSON", "JSON", "Sensors, IDs, names and options.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Comparison", "R", "Complete comparison."); outputs.AddText("JSON", "JSON", "Portable result."); outputs.AddText("Attribution", "A", "Ranked loss attribution JSON."); outputs.AddText("Deltas", "D", "Baseline deltas JSON."); outputs.AddText("Hash", "H", "Result identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object so); access.GetItem(1, out object su); object[] libraries = []; access.GetItemArray(2, out libraries); access.GetItem(3, out string json); if (so is not XvarnaScene scene || su is not XvarnaSunSet sun) { access.AddError("Invalid input", "Connect Scene and Sun Vectors."); return; } try { SolarScenariosRequest q = Gh2Json.Read<SolarScenariosRequest>(json); if (q.Names.Count != libraries.Length || q.ScenarioIds.Count != libraries.Length || libraries.Any(v => v is not AnalysisMaterialLibrary)) throw new ArgumentException("Names, ScenarioIds and Material Libraries must align."); SolarMaterialScenario[] scenarios = libraries.Select((v, i) => new SolarMaterialScenario { ScenarioId = q.ScenarioIds[i], Name = q.Names[i], Materials = (AnalysisMaterialLibrary)v }).ToArray(); SolarScenarioComparison r = scene.CompareSolarScenarios(q.Sensors, sun, scenarios, q.Options); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); access.SetItem(2, Gh2Json.Write(r.Attribution)); access.SetItem(3, Gh2Json.Write(r.Deltas)); access.SetItem(4, r.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Solar comparison failed", e.Message); } }
}

internal sealed record SolarEnvelopeRequest
{
    public IReadOnlyList<SolarSensor> Sensors { get; init; } = [];
    public IReadOnlyList<OrientedEnvelopeCandidate> Candidates { get; init; } = [];
    public SolarEnvelopeOptions Options { get; init; } = new(0.1, 0.8, 0.5, 0, 1e-4, double.PositiveInfinity, ulong.MaxValue, 8, 1e-4);
}

[IoId("b4215c17-eb11-44ac-bc8f-2013ea1b93f6")]
public sealed class SolarEnvelopeComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SolarEnvelopeComponent() : base(new Nomen("XVARNA Solar Envelope", "Freeform solar-access and shading envelope.", "XVARNA", "03 Environment")) { }
    public SolarEnvelopeComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Sun Set", "Sun", "XVARNA Sun Vectors."); inputs.AddGeneric("Materials", "M", "XVARNA Materials.", Access.Item, Requirement.MayBeMissing); inputs.AddText("Request JSON", "JSON", "Sensors, oriented candidates and options.").Set("{}"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete envelope."); outputs.AddText("JSON", "JSON", "Portable result."); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Constraint provenance."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object so); access.GetItem(1, out object su); object? mo = null; if (!access.GetNull(2)) access.GetItem(2, out mo); access.GetItem(3, out string json); if (so is not XvarnaScene scene || su is not XvarnaSunSet sun) { access.AddError("Invalid input", "Connect Scene and Sun Vectors."); return; } try { SolarEnvelopeRequest q = Gh2Json.Read<SolarEnvelopeRequest>(json); SolarEnvelopeResult r = scene.AnalyzeOrientedSolarEnvelope(q.Sensors, q.Candidates, sun, mo as AnalysisMaterialLibrary ?? AnalysisMaterialLibrary.Opaque, q.Options); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(r)); access.SetItem(2, r.ContentHash); access.SetItem(3, $"{q.Sensors.Count:N0} protected sensors; {q.Candidates.Count:N0} freeform axes; controlling sensor/time provenance retained"); } catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Solar envelope failed", e.Message); } }
}

[IoId("a9dce77d-2ed9-430a-877a-33d159cb7e6f")]
public sealed class OccluderHighlightComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public OccluderHighlightComponent() : base(new Nomen("XVARNA Highlight Occluders", "Maps exact attribution IDs back to source meshes.", "XVARNA", "04 Visibility")) { }
    public OccluderHighlightComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddMesh("Source Meshes", "M", "Meshes used to build the scene.", Access.Twig); inputs.AddText("Object IDs", "OID", "One Object ID per mesh.", Access.Twig); inputs.AddText("Highlight IDs", "HID", "Attributed Object IDs.", Access.Twig); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddMesh("Highlighted", "H", "Matching meshes.", Access.Twig); outputs.AddMesh("Other", "O", "Non-matching meshes.", Access.Twig); outputs.AddBoolean("Selected", "S", "Selection mask.", Access.Twig); outputs.AddText("Report", "R", "Resolved and missing IDs."); }
    protected override void Process(IDataAccess access) { Mesh[] meshes = []; string[] ids = [], selected = []; access.GetItemArray(0, out meshes); access.GetItemArray(1, out ids); access.GetItemArray(2, out selected); try { if (meshes.Length != ids.Length) throw new ArgumentException("Meshes and Object IDs must align."); ulong[] source = ids.Select(v => ulong.Parse(v, CultureInfo.InvariantCulture)).ToArray(); HashSet<ulong> wanted = selected.Select(v => ulong.Parse(v, CultureInfo.InvariantCulture)).ToHashSet(); bool[] mask = source.Select(wanted.Contains).ToArray(); Gh2Data.SetTwig(access, 0, meshes.Where((_, i) => mask[i]).ToArray()); Gh2Data.SetTwig(access, 1, meshes.Where((_, i) => !mask[i]).ToArray()); Gh2Data.SetTwig(access, 2, mask); access.SetItem(3, $"{mask.Count(v => v):N0}/{meshes.Length:N0} highlighted; {wanted.Count(v => !source.Contains(v)):N0} requested IDs absent"); } catch (Exception e) when (e is ArgumentException or OverflowException) { access.AddError("Highlight mapping failed", e.Message); } }
}
