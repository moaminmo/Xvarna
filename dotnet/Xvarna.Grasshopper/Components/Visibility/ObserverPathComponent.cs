using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Evaluates Target/Weighted/Green View along tangent-facing observer paths.</summary>
public sealed class ObserverPathComponent : ScheduledAnalysisComponent<ObserverPathWork, ObserverPathResult>
{
    /// <summary>Initializes the dynamic observer-path component.</summary>
    public ObserverPathComponent() : base(
        "XVARNA Dynamic Observer Path", "XV View Path",
        "Samples tangent-facing paths and computes distance-weighted Target, Weighted, and Green View time series with best/worst hotspots.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("55795ea2-4e18-4979-91c3-a1eddb748701");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA View Path";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene / Backend", "S", "Immutable XV Scene or cached session produced by XV Backend.", GH_ParamAccess.item);
        input.AddCurveParameter("Polyline Paths", "P", "Polyline observer trajectories; direction follows curve order.", GH_ParamAccess.list);
        input.AddVectorParameter("Up", "U", "Zero, one, or one camera-up vector per path; empty uses world Z.", GH_ParamAccess.list);
        input.AddMeshParameter("Target Meshes", "T", "Target surfaces evaluated from every path sample.", GH_ParamAccess.list);
        input.AddTextParameter("Target IDs", "ID", "Zero, one, or one unsigned 64-bit ID per target mesh.", GH_ParamAccess.list);
        input.AddTextParameter("Target Categories", "TC", "Zero, one, or one category mask per target mesh.", GH_ParamAccess.list);
        input.AddNumberParameter("Target Weights", "W", "Zero, one, or one explicit desirability weight [0,1] per target mesh.", GH_ParamAccess.list);
        input.AddTextParameter("Green Mask", "G", "Target categories counted as green; -1 counts all.", GH_ParamAccess.item, "1");
        input.AddNumberParameter("Spacing", "D", "Maximum path sample spacing in model units.", GH_ParamAccess.item, 1.0);
        input.AddIntegerParameter("Samples per Patch", "N", "Target triangle samples at every path position.", GH_ParamAccess.item, 16);
        input.AddNumberParameter("Horizontal FOV", "HF", "Camera horizontal field of view in degrees.", GH_ParamAccess.item, 90.0);
        input.AddNumberParameter("Vertical FOV", "VF", "Camera vertical field of view in degrees.", GH_ParamAccess.item, 60.0);
        input.AddNumberParameter("Maximum Distance", "Max", "Maximum camera-to-target distance in model units.", GH_ParamAccess.item, 100.0);
        input.AddNumberParameter("Distance Reference", "D50", "Distance where weighting equals one half.", GH_ParamAccess.item, 25.0);
        input.AddBooleanParameter("Two Sided", "2S", "Treat target triangles as visible from both sides.", GH_ParamAccess.item, false);
        input.AddBooleanParameter("Run", "Run", "Compute the native dynamic-view study.", GH_ParamAccess.item, true);
        input[2].Optional = true; input[4].Optional = true; input[5].Optional = true; input[6].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Observer Path Result", "R", "Immutable result for downstream Study and automation.", GH_ParamAccess.item);
        output.AddNumberParameter("Mean Target View", "TV", "Distance-weighted mean raw Target View per path.", GH_ParamAccess.list);
        output.AddNumberParameter("Mean Weighted View", "WV", "Distance-weighted mean quality score per path.", GH_ParamAccess.list);
        output.AddNumberParameter("Mean Green View", "GVI", "Distance-weighted mean Green View Index per path.", GH_ParamAccess.list);
        output.AddPointParameter("Sample Points", "P", "Ordered observer samples, one branch per path.", GH_ParamAccess.tree);
        output.AddVectorParameter("Forward", "F", "Tangent camera direction at every sample.", GH_ParamAccess.tree);
        output.AddNumberParameter("Target View Series", "TVs", "Raw Target View along each path.", GH_ParamAccess.tree);
        output.AddNumberParameter("Weighted View Series", "WVs", "Weighted View along each path.", GH_ParamAccess.tree);
        output.AddNumberParameter("Green View Series", "GVs", "Green View Index along each path.", GH_ParamAccess.tree);
        output.AddPointParameter("Worst Points", "Worst", "Lowest weighted-view hotspot on each path.", GH_ParamAccess.list);
        output.AddPointParameter("Best Points", "Best", "Highest weighted-view hotspot on each path.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "Deterministic scene/input/policy/result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Sampling, metric, hotspot, runtime, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out ObserverPathWork work)
    {
        work = default;
        object? wrapped = null; List<Curve> curves = []; List<Vector3d> ups = []; List<Mesh> targets = [];
        List<string> ids = []; List<string> categories = []; List<double> weights = [];
        string green = "1"; double spacing = 1, horizontal = 90, vertical = 60, maximum = 100, distanceReference = 25;
        int samples = 16; bool twoSided = false, run = true;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, curves) || curves.Count == 0
            || !data.GetDataList(3, targets) || targets.Count == 0) return false;
        data.GetDataList(2, ups); data.GetDataList(4, ids); data.GetDataList(5, categories); data.GetDataList(6, weights);
        data.GetData(7, ref green); data.GetData(8, ref spacing); data.GetData(9, ref samples);
        data.GetData(10, ref horizontal); data.GetData(11, ref vertical); data.GetData(12, ref maximum);
        data.GetData(13, ref distanceReference); data.GetData(14, ref twoSided); data.GetData(15, ref run);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = AdvancedVisibilityHelpers.UnwrapScene(wrapped);
        XvarnaComputeSession? compute = AdvancedVisibilityHelpers.UnwrapCompute(wrapped);
        if (scene is null && compute is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Input must come from XV Scene or XV Backend."); return false; }
        if (!AdvancedVisibilityHelpers.Compatible(curves.Count, ups.Count))
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Up must be empty, one item, or one item per path."); return false; }
        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 1, 4096);
            ObserverPathDefinition[] paths = curves.Select((curve, index) =>
            {
                if (!curve.TryGetPolyline(out Polyline polyline) || polyline.Count < 2)
                    throw new ArgumentException($"Path {index + 1} must be a polyline curve.");
                return new ObserverPathDefinition
                {
                    PathId = checked((ulong)index + 1),
                    Vertices = polyline.Select(AdvancedVisibilityHelpers.Point).ToArray(),
                    Up = AdvancedVisibilityHelpers.Vector(AdvancedVisibilityHelpers.Select(ups, index, Vector3d.ZAxis)),
                };
            }).ToArray();
            IReadOnlyList<ViewTargetTriangle> patches = AdvancedVisibilityHelpers.TriangulateTargets(targets, ids, categories, weights);
            double clearance = ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            TargetViewOptions view = new(checked((uint)samples), horizontal, vertical, maximum, clearance,
                ulong.MaxValue, ulong.MaxValue, AdvancedVisibilityHelpers.ParseUnsigned(green, "Green mask", true),
                distanceReference, 2.0, 1.0, twoSided);
            ObserverPathOptions options = new(spacing, view);
            work = new(scene, compute, paths, patches.ToArray(), options);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(ObserverPathWork work)
    {
        ulong vertices = checked((ulong)work.Paths.Sum(path => path.Vertices.Count));
        return checked(vertices * (ulong)work.Patches.Length * work.Options.View.SamplesPerPatch);
    }

    /// <inheritdoc />
    protected override ObserverPathResult Execute(
        ObserverPathWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Sampling observer paths"));
        cancellationToken.ThrowIfCancellationRequested();
        ObserverPathResult result = work.Compute?.AnalyzeObserverPaths(work.Paths, work.Patches, work.Options)
            ?? work.Scene!.AnalyzeObserverPaths(work.Paths, work.Patches, work.Options);
        progress.Report((total, total, "Aggregating hotspots"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, ObserverPathWork work, ObserverPathResult result)
    {
        DataTree<Point3d> points = new(); DataTree<Vector3d> forwards = new(); DataTree<double> target = new();
        DataTree<double> weighted = new(); DataTree<double> green = new(); List<Point3d> worst = []; List<Point3d> best = [];
        int offset = 0;
        for (int path = 0; path < result.Summaries.Count; path++)
        {
            ObserverPathSummary summary = result.Summaries[path]; GH_Path branch = new(path);
            IReadOnlyList<ObserverPathSample> samples = result.Samples.Skip(offset).Take(checked((int)summary.SampleCount)).ToArray();
            foreach (ObserverPathSample sample in samples)
            {
                points.Add(AdvancedVisibilityHelpers.RhinoPoint(sample.Position), branch);
                forwards.Add(new(sample.Forward.X, sample.Forward.Y, sample.Forward.Z), branch);
                target.Add(sample.TargetViewFraction, branch); weighted.Add(sample.WeightedViewScore, branch); green.Add(sample.GreenViewIndex, branch);
            }
            worst.Add(AdvancedVisibilityHelpers.RhinoPoint(samples[checked((int)summary.WorstSampleIndex)].Position));
            best.Add(AdvancedVisibilityHelpers.RhinoPoint(samples[checked((int)summary.BestSampleIndex)].Position));
            offset += samples.Count;
        }
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA Dynamic Observer Path{Environment.NewLine}" +
            $"Study: {result.Paths.Count:N0} path(s); {result.Samples.Count:N0} tangent-facing sample(s); spacing ≤ {result.Options.Spacing:G8}{Environment.NewLine}" +
            $"View model: solid-angle Target / Weighted / Green View; {result.Options.View.SamplesPerPatch:N0} samples/target triangle/sample position{Environment.NewLine}" +
            $"{AdvancedVisibilityHelpers.ExecutionLine(result.Execution)}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, result.Summaries.Select(value => value.MeanTargetViewFraction));
        data.SetDataList(2, result.Summaries.Select(value => value.MeanWeightedViewScore));
        data.SetDataList(3, result.Summaries.Select(value => value.MeanGreenViewIndex));
        data.SetDataTree(4, points); data.SetDataTree(5, forwards); data.SetDataTree(6, target);
        data.SetDataTree(7, weighted); data.SetDataTree(8, green); data.SetDataList(9, worst); data.SetDataList(10, best);
        data.SetData(11, result.ContentHash); data.SetData(12, report);
        Message = string.Create(CultureInfo.InvariantCulture,
            $"DAENA {AdvancedVisibilityHelpers.BackendMessage(result.Execution)}\n{result.Samples.Count:N0} samples");
    }
}

/// <summary>Immutable captured input for scheduled dynamic observer-path work.</summary>
public readonly record struct ObserverPathWork(
    XvarnaScene? Scene,
    XvarnaComputeSession? Compute,
    ObserverPathDefinition[] Paths,
    ViewTargetTriangle[] Patches,
    ObserverPathOptions Options);
