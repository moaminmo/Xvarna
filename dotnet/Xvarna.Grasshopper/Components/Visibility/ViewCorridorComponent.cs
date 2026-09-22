using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Tests protected cone-like view corridors through sampled circular apertures.</summary>
public sealed class ViewCorridorComponent : ScheduledAnalysisComponent<ViewCorridorWork, ViewCorridorResult>
{
    /// <summary>Initializes the view-corridor component.</summary>
    public ViewCorridorComponent() : base(
        "XVARNA View Corridor", "XV Corridor",
        "Tests protected view corridors with deterministic uniform-area aperture samples, convergence, and exact first-conflict attribution.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("9fb75738-dd69-4c47-bb97-963a46adab15");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Corridor";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene / Backend", "S", "Immutable XV Scene or cached session produced by XV Backend.", GH_ParamAccess.item);
        input.AddPointParameter("Origins", "O", "Corridor observer/apex points.", GH_ParamAccess.list);
        input.AddPointParameter("Targets", "T", "Target aperture centres; one or one per origin.", GH_ParamAccess.list);
        input.AddNumberParameter("Radii", "R", "Circular target-aperture radii; one or one per origin.", GH_ParamAccess.list);
        input.AddVectorParameter("Up", "U", "Zero, one, or one up vector per corridor; empty uses world Z.", GH_ParamAccess.list);
        input.AddIntegerParameter("Samples", "N", "Uniform-area aperture samples per corridor.", GH_ParamAccess.item, 1024);
        input.AddNumberParameter("Clearance", "Eps", "Endpoint clearance in model units; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        input.AddTextParameter("Category Mask", "C", "Included scene categories; -1 selects all.", GH_ParamAccess.item, "-1");
        input.AddBooleanParameter("Run", "Run", "Compute the native corridor study.", GH_ParamAccess.item, true);
        input[4].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Corridor Result", "R", "Immutable result for downstream Study and automation.", GH_ParamAccess.item);
        output.AddNumberParameter("Open Fraction", "Open", "Unblocked fraction of the target aperture.", GH_ParamAccess.list);
        output.AddNumberParameter("Open Solid Angle", "Ω", "Estimated unblocked aperture solid angle in steradians.", GH_ParamAccess.list);
        output.AddNumberParameter("Convergence", "Δ", "Full-versus-interleaved-half open-fraction delta.", GH_ParamAccess.list);
        output.AddTextParameter("Dominant Blocker", "OID", "Object responsible for the greatest aperture conflict.", GH_ParamAccess.list);
        output.AddNumberParameter("Blocker Share", "BS", "Dominant blocker share of all aperture samples.", GH_ParamAccess.list);
        output.AddNumberParameter("Nearest Blocker", "Near", "Nearest conflict distance in model units.", GH_ParamAccess.list);
        output.AddPointParameter("Aperture Samples", "P", "Target aperture samples, one branch per corridor.", GH_ParamAccess.tree);
        output.AddIntegerParameter("States", "St", "0 open, 1 blocked, one branch per corridor.", GH_ParamAccess.tree);
        output.AddTextParameter("Blocker IDs", "B", "First-blocker object IDs per sample.", GH_ParamAccess.tree);
        output.AddTextParameter("Hash", "H", "Deterministic scene/input/policy/result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Method, resolution, convergence, conflicts, and runtime.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out ViewCorridorWork work)
    {
        work = default;
        object? wrapped = null; List<Point3d> origins = []; List<Point3d> targets = [];
        List<double> radii = []; List<Vector3d> ups = []; int samples = 1024; double clearance = 0;
        string category = "-1"; bool run = true;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, origins) || origins.Count == 0
            || !data.GetDataList(2, targets) || targets.Count == 0 || !data.GetDataList(3, radii) || radii.Count == 0) return false;
        data.GetDataList(4, ups); data.GetData(5, ref samples); data.GetData(6, ref clearance);
        data.GetData(7, ref category); data.GetData(8, ref run);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = AdvancedVisibilityHelpers.UnwrapScene(wrapped);
        XvarnaComputeSession? compute = AdvancedVisibilityHelpers.UnwrapCompute(wrapped);
        if (scene is null && compute is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Input must come from XV Scene or XV Backend."); return false; }
        if (!AdvancedVisibilityHelpers.Compatible(origins.Count, targets.Count)
            || !AdvancedVisibilityHelpers.Compatible(origins.Count, radii.Count)
            || !AdvancedVisibilityHelpers.Compatible(origins.Count, ups.Count))
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Targets, Radii, and Up must contain one or one item per origin (Up may be empty)."); return false; }
        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 4, 262_144);
            ViewCorridorDefinition[] corridors = origins.Select((origin, index) => new ViewCorridorDefinition(
                checked((ulong)index + 1), AdvancedVisibilityHelpers.Point(origin),
                AdvancedVisibilityHelpers.Point(AdvancedVisibilityHelpers.Select(targets, index, Point3d.Unset)),
                AdvancedVisibilityHelpers.Vector(AdvancedVisibilityHelpers.Select(ups, index, Vector3d.ZAxis)),
                AdvancedVisibilityHelpers.Select(radii, index, 0.0))).ToArray();
            double resolved = clearance > 0 && double.IsFinite(clearance)
                ? clearance : ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            ViewCorridorOptions options = new(checked((uint)samples), resolved,
                AdvancedVisibilityHelpers.ParseUnsigned(category, "Category mask", true));
            work = new(scene, compute, corridors, options);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(ViewCorridorWork work) =>
        checked((ulong)work.Corridors.Length * work.Options.SampleCount);

    /// <inheritdoc />
    protected override ViewCorridorResult Execute(
        ViewCorridorWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Tracing aperture samples"));
        cancellationToken.ThrowIfCancellationRequested();
        ViewCorridorResult result = work.Compute?.AnalyzeViewCorridors(work.Corridors, work.Options)
            ?? work.Scene!.AnalyzeViewCorridors(work.Corridors, work.Options);
        progress.Report((total, total, "Attribution and convergence"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, ViewCorridorWork work, ViewCorridorResult result)
    {
        DataTree<Point3d> points = new(); DataTree<int> states = new(); DataTree<string> blockers = new();
        int count = checked((int)result.Options.SampleCount);
        for (int corridor = 0; corridor < result.Summaries.Count; corridor++)
        {
            GH_Path path = new(corridor);
            foreach (ViewCorridorSample sample in result.Samples.Skip(corridor * count).Take(count))
            {
                points.Add(AdvancedVisibilityHelpers.RhinoPoint(sample.AperturePoint), path);
                states.Add((int)sample.State, path);
                blockers.Add(sample.BlockerObjectId == 0 ? string.Empty : sample.BlockerObjectId.ToString(CultureInfo.InvariantCulture), path);
            }
        }
        double maximumDelta = result.Summaries.Max(value => value.ConvergenceDelta);
        AnalysisQualityProfile quality = ProductContextRegistry.GetQuality(OnPingDocument());
        XvarnaEvidenceLabel evidence = XvarnaEvidenceLabel.Convergence(
            "interleaved aperture openness", maximumDelta, 0.0,
            result.Options.SampleCount, quality.ConvergenceTolerance);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA protected View Corridor{Environment.NewLine}" +
            $"Study: {result.Corridors.Count:N0} corridor(s) × {result.Options.SampleCount:N0} uniform-area aperture samples = {result.Samples.Count:N0} rays{Environment.NewLine}" +
            $"Conflicts: {result.Summaries.Sum(value => (decimal)value.BlockedSampleCount):N0} blocked; max Δopen {maximumDelta:G6}{Environment.NewLine}" +
            $"Evidence: {evidence.Level}; {evidence.Statement}{Environment.NewLine}" +
            $"{AdvancedVisibilityHelpers.ExecutionLine(result.Execution)}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, result.Summaries.Select(value => value.OpenFraction));
        data.SetDataList(2, result.Summaries.Select(value => value.OpenSolidAngleSteradians));
        data.SetDataList(3, result.Summaries.Select(value => value.ConvergenceDelta));
        data.SetDataList(4, result.Summaries.Select(value => value.DominantBlockerObjectId == 0 ? string.Empty : value.DominantBlockerObjectId.ToString(CultureInfo.InvariantCulture)));
        data.SetDataList(5, result.Summaries.Select(value => value.DominantBlockerFraction));
        data.SetDataList(6, result.Summaries.Select(value => value.NearestBlockerDistance));
        data.SetDataTree(7, points); data.SetDataTree(8, states); data.SetDataTree(9, blockers);
        data.SetData(10, result.ContentHash); data.SetData(11, report);
        Message = string.Create(CultureInfo.InvariantCulture,
            $"DAENA {AdvancedVisibilityHelpers.BackendMessage(result.Execution)}\n{result.Corridors.Count:N0} corridor(s)");
    }
}

/// <summary>Immutable captured input for scheduled corridor work.</summary>
public readonly record struct ViewCorridorWork(
    XvarnaScene? Scene,
    XvarnaComputeSession? Compute,
    ViewCorridorDefinition[] Corridors,
    ViewCorridorOptions Options);
