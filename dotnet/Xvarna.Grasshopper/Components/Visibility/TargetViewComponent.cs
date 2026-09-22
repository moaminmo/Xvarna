using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Computes solid-angle Target, Weighted, and Green View from oriented cameras.</summary>
public sealed class TargetViewComponent : ScheduledAnalysisComponent<TargetViewWork, TargetViewResult>
{
    /// <summary>Initializes the advanced target-view component.</summary>
    public TargetViewComponent() : base(
        "XVARNA Target + Green View", "XV Target View",
        "Computes solid-angle Target View, explicit desirability/distance/direction Weighted View, Green View Index, convergence, and first-blocker attribution.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("19ad71f3-ccf4-47b7-878b-12bf8414b923");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Target View";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene / Backend", "S", "Immutable XV Scene or cached session produced by XV Backend.", GH_ParamAccess.item);
        input.AddPointParameter("Eyes", "E", "Camera eye positions in active Rhino units.", GH_ParamAccess.list);
        input.AddVectorParameter("Forward", "F", "One camera-forward vector or one per eye.", GH_ParamAccess.list);
        input.AddVectorParameter("Up", "U", "Zero, one, or one camera-up vector per eye; empty uses world Z.", GH_ParamAccess.list);
        input.AddMeshParameter("Target Meshes", "T", "Triangular or quad target surfaces; repeated IDs aggregate their solid angle.", GH_ParamAccess.list);
        input.AddTextParameter("Target IDs", "ID", "Zero, one, or one unsigned 64-bit ID per target mesh.", GH_ParamAccess.list);
        input.AddTextParameter("Target Categories", "TC", "Zero, one, or one target category mask per mesh; -1 selects all bits.", GH_ParamAccess.list);
        input.AddNumberParameter("Target Weights", "W", "Explicit desirability weight [0,1], zero/one/per target mesh.", GH_ParamAccess.list);
        input.AddTextParameter("Green Mask", "G", "Target category bits counted as green; -1 counts every category.", GH_ParamAccess.item, "1");
        input.AddIntegerParameter("Samples per Patch", "N", "Deterministic Hammersley samples per target triangle, 1..4096.", GH_ParamAccess.item, 32);
        input.AddNumberParameter("Horizontal FOV", "HF", "Rectangular camera horizontal field of view in degrees.", GH_ParamAccess.item, 90.0);
        input.AddNumberParameter("Vertical FOV", "VF", "Rectangular camera vertical field of view in degrees.", GH_ParamAccess.item, 60.0);
        input.AddNumberParameter("Maximum Distance", "Max", "Maximum eye-to-target distance in model units.", GH_ParamAccess.item, 100.0);
        input.AddNumberParameter("Distance Reference", "D50", "Distance where the weighting term equals one half.", GH_ParamAccess.item, 25.0);
        input.AddNumberParameter("Distance Exponent", "DE", "Non-negative distance falloff exponent.", GH_ParamAccess.item, 2.0);
        input.AddNumberParameter("Direction Exponent", "FE", "Non-negative camera-centre cosine exponent.", GH_ParamAccess.item, 1.0);
        input.AddBooleanParameter("Two Sided", "2S", "Treat target triangles as visible from both sides.", GH_ParamAccess.item, false);
        input.AddBooleanParameter("Run", "Run", "Compute the native DAENA study.", GH_ParamAccess.item, true);
        input[3].Optional = true; input[5].Optional = true; input[6].Optional = true; input[7].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Target View Result", "R", "Immutable result for downstream Study and automation.", GH_ParamAccess.item);
        output.AddNumberParameter("Target View", "TV", "Raw visible target solid angle divided by camera FOV solid angle.", GH_ParamAccess.list);
        output.AddNumberParameter("Weighted View", "WV", "Category × distance × direction weighted view score.", GH_ParamAccess.list);
        output.AddNumberParameter("Green View Index", "GVI", "Visible green solid angle divided by camera FOV solid angle.", GH_ParamAccess.list);
        output.AddNumberParameter("Green Share", "GS", "Green share of all visible target solid angle.", GH_ParamAccess.list);
        output.AddNumberParameter("Target Visibility", "Vis", "Per-eye/per-target visible divided by potential solid angle.", GH_ParamAccess.tree);
        output.AddNumberParameter("Visible Solid Angle", "Ω", "Per-eye/per-target visible steradians.", GH_ParamAccess.tree);
        output.AddTextParameter("Dominant Target", "TID", "Highest weighted-contribution target for each eye.", GH_ParamAccess.list);
        output.AddTextParameter("Dominant Blocker", "OID", "First blocker with the largest blocked solid angle per eye/target.", GH_ParamAccess.tree);
        output.AddNumberParameter("Convergence", "ΔΩ", "Full-versus-interleaved-half solid-angle delta.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "Deterministic scene/input/policy/result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Method, resolution, policy, runtime, and limitations.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out TargetViewWork work)
    {
        work = default;
        object? wrapped = null; List<Point3d> eyes = []; List<Vector3d> forward = []; List<Vector3d> up = [];
        List<Mesh> targets = []; List<string> ids = []; List<string> categories = []; List<double> weights = [];
        string greenMask = "1"; int samples = 32; double horizontal = 90, vertical = 60, maximum = 100;
        double distanceReference = 25, distanceExponent = 2, directionExponent = 1; bool twoSided = false, run = true;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, eyes) || eyes.Count == 0
            || !data.GetDataList(2, forward) || forward.Count == 0 || !data.GetDataList(4, targets) || targets.Count == 0) return false;
        data.GetDataList(3, up); data.GetDataList(5, ids); data.GetDataList(6, categories); data.GetDataList(7, weights);
        data.GetData(8, ref greenMask); data.GetData(9, ref samples); data.GetData(10, ref horizontal);
        data.GetData(11, ref vertical); data.GetData(12, ref maximum); data.GetData(13, ref distanceReference);
        data.GetData(14, ref distanceExponent); data.GetData(15, ref directionExponent);
        data.GetData(16, ref twoSided); data.GetData(17, ref run);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = AdvancedVisibilityHelpers.UnwrapScene(wrapped);
        XvarnaComputeSession? compute = AdvancedVisibilityHelpers.UnwrapCompute(wrapped);
        if (scene is null && compute is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Input must come from XV Scene or XV Backend."); return false; }
        if (!AdvancedVisibilityHelpers.Compatible(eyes.Count, forward.Count)
            || !AdvancedVisibilityHelpers.Compatible(eyes.Count, up.Count))
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Forward and Up must contain one or one item per eye (Up may be empty)."); return false; }

        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 1, 4096);
            IReadOnlyList<ViewTargetTriangle> patches = AdvancedVisibilityHelpers.TriangulateTargets(targets, ids, categories, weights);
            ViewCamera[] cameras = eyes.Select((eye, index) => new ViewCamera(checked((ulong)index + 1),
                AdvancedVisibilityHelpers.Point(eye),
                AdvancedVisibilityHelpers.Vector(AdvancedVisibilityHelpers.Select(forward, index, Vector3d.XAxis)),
                AdvancedVisibilityHelpers.Vector(AdvancedVisibilityHelpers.Select(up, index, Vector3d.ZAxis)))).ToArray();
            double clearance = ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            TargetViewOptions options = new(checked((uint)samples), horizontal, vertical, maximum, clearance,
                ulong.MaxValue, ulong.MaxValue, AdvancedVisibilityHelpers.ParseUnsigned(greenMask, "Green mask", true),
                distanceReference, distanceExponent, directionExponent, twoSided);
            work = new(scene, compute, cameras, patches.ToArray(), options);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(TargetViewWork work) =>
        checked((ulong)work.Cameras.Length * (ulong)work.Patches.Length * work.Options.SamplesPerPatch);

    /// <inheritdoc />
    protected override TargetViewResult Execute(
        TargetViewWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Solid-angle visibility"));
        cancellationToken.ThrowIfCancellationRequested();
        TargetViewResult result = work.Compute?.AnalyzeTargetView(work.Cameras, work.Patches, work.Options)
            ?? work.Scene!.AnalyzeTargetView(work.Cameras, work.Patches, work.Options);
        progress.Report((total, total, "Weighting and attribution"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, TargetViewWork work, TargetViewResult result)
    {
        DataTree<double> visibility = new(); DataTree<double> solidAngles = new(); DataTree<string> blockers = new();
        int targetCount = result.TargetIds.Count;
        for (int eye = 0; eye < result.Summaries.Count; eye++)
        {
            GH_Path path = new(eye);
            foreach (TargetViewEntry entry in result.Entries.Skip(eye * targetCount).Take(targetCount))
            {
                visibility.Add(entry.VisibilityFraction, path); solidAngles.Add(entry.VisibleSolidAngleSteradians, path);
                blockers.Add(entry.DominantBlockerObjectId == 0 ? string.Empty : entry.DominantBlockerObjectId.ToString(CultureInfo.InvariantCulture), path);
            }
        }
        TargetViewOptions options = result.Options;
        double maximumDelta = result.Summaries.Max(value => value.ConvergenceDeltaSteradians);
        AnalysisQualityProfile quality = ProductContextRegistry.GetQuality(OnPingDocument());
        XvarnaEvidenceLabel evidence = XvarnaEvidenceLabel.Convergence(
            "interleaved target solid angle", maximumDelta, 0.0,
            options.SamplesPerPatch, quality.ConvergenceTolerance);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA solid-angle Target / Weighted / Green View{Environment.NewLine}" +
            $"Study: {result.Observers.Count:N0} camera(s); {result.TargetIds.Count:N0} logical target(s); {result.TargetPatches.Count:N0} triangle patch(es){Environment.NewLine}" +
            $"Sampling: {options.SamplesPerPatch:N0}/patch; exact rectangular FOV Ω; max ΔΩ {maximumDelta:G6} sr{Environment.NewLine}" +
            $"Evidence: {evidence.Level}; {evidence.Statement}{Environment.NewLine}" +
            $"Weighting: category × 1/(1+(d/{options.DistanceReference:G6})^{options.DistanceExponent:G6}) × cos^{options.DirectionExponent:G6}; raw Target View remains separate{Environment.NewLine}" +
            $"FOV: {options.HorizontalFieldOfViewDegrees:G6}° × {options.VerticalFieldOfViewDegrees:G6}°; max distance {options.MaximumDistance:G8}; two-sided {options.TwoSidedTargets}{Environment.NewLine}" +
            $"{AdvancedVisibilityHelpers.ExecutionLine(result.Execution)}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, result.Summaries.Select(value => value.TargetViewFraction));
        data.SetDataList(2, result.Summaries.Select(value => value.WeightedViewScore));
        data.SetDataList(3, result.Summaries.Select(value => value.GreenViewIndex));
        data.SetDataList(4, result.Summaries.Select(value => value.GreenShareOfVisibleTargets));
        data.SetDataTree(5, visibility); data.SetDataTree(6, solidAngles);
        data.SetDataList(7, result.Summaries.Select(value => value.DominantTargetId.ToString(CultureInfo.InvariantCulture)));
        data.SetDataTree(8, blockers); data.SetDataList(9, result.Summaries.Select(value => value.ConvergenceDeltaSteradians));
        data.SetData(10, result.ContentHash); data.SetData(11, report);
        Message = string.Create(CultureInfo.InvariantCulture,
            $"DAENA {AdvancedVisibilityHelpers.BackendMessage(result.Execution)}\n{result.Observers.Count:N0} × {result.TargetIds.Count:N0}");
    }
}

/// <summary>Immutable captured input for scheduled Target/Weighted/Green View work.</summary>
public readonly record struct TargetViewWork(
    XvarnaScene? Scene,
    XvarnaComputeSession? Compute,
    ViewCamera[] Cameras,
    ViewTargetTriangle[] Patches,
    TargetViewOptions Options);
