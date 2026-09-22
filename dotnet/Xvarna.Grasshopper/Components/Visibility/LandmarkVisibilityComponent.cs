using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Computes partial, material-aware landmark visibility.</summary>
public sealed class LandmarkVisibilityComponent : ScheduledAnalysisComponent<LandmarkWork, LandmarkVisibilityResult>
{
    /// <summary>Initializes the landmark component.</summary>
    public LandmarkVisibilityComponent() : base(
        "XVARNA Landmark Visibility", "XV Landmark",
        "Measures landmark apparent solid angle, partial material visibility, weighted score, and general source/category attribution.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("1c937283-62f0-42a3-84ac-3d8ccb70f9fc");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Landmark";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene.", GH_ParamAccess.item);
        input.AddPointParameter("Observers", "O", "Observer eye points.", GH_ParamAccess.list);
        input.AddVectorParameter("Forward", "F", "One camera forward vector or one per observer.", GH_ParamAccess.list);
        input.AddVectorParameter("Up", "U", "One camera up vector or one per observer.", GH_ParamAccess.list);
        input.AddPointParameter("Landmarks", "L", "Spherical landmark proxy centres.", GH_ParamAccess.list);
        input.AddNumberParameter("Radii", "R", "One radius or one per landmark.", GH_ParamAccess.list, 1.0);
        input.AddNumberParameter("Weights", "W", "One importance in [0,1] or one per landmark.", GH_ParamAccess.list, 1.0);
        input.AddGenericParameter("Materials", "M", "Optional XV Materials catalog.", GH_ParamAccess.item);
        input.AddIntegerParameter("Samples", "Q", "Disk samples per observer/landmark pair, 4..4096.", GH_ParamAccess.item, 64);
        input.AddNumberParameter("Horizontal FOV", "HF", "Camera horizontal field of view in degrees.", GH_ParamAccess.item, 90.0);
        input.AddNumberParameter("Vertical FOV", "VF", "Camera vertical field of view in degrees.", GH_ParamAccess.item, 90.0);
        input.AddNumberParameter("Maximum Distance", "Max", "Maximum landmark distance.", GH_ParamAccess.item, 1000.0);
        input.AddIntegerParameter("Top K", "K", "Ranked blockers retained per pair.", GH_ParamAccess.item, 10);
        input.AddTextParameter("Category Mask", "C", "-1 for all or unsigned mask.", GH_ParamAccess.item, "-1");
        input.AddNumberParameter("Clearance", "Eps", "0 uses Rhino document tolerance.", GH_ParamAccess.item, 0.0);
        input[7].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Result", "R", "Complete immutable landmark matrix.", GH_ParamAccess.item);
        output.AddNumberParameter("Visible Fraction", "V", "Observer branches containing one value per landmark.", GH_ParamAccess.tree);
        output.AddNumberParameter("Visible Solid Angle", "Ω", "Visible apparent solid angle in steradians.", GH_ParamAccess.tree);
        output.AddNumberParameter("Weighted Score", "W", "Importance-weighted camera-normalized score.", GH_ParamAccess.tree);
        output.AddBooleanParameter("Inside FOV", "In", "Landmark-centre camera inclusion.", GH_ParamAccess.tree);
        output.AddTextParameter("Dominant Blocker", "OID", "Dominant source object per pair.", GH_ParamAccess.tree);
        output.AddTextParameter("Top Objects", "Top", "Ranked blocker object IDs per observer/landmark path.", GH_ParamAccess.tree);
        output.AddNumberParameter("Top Fractions", "F", "Ranked blocker shares per observer/landmark path.", GH_ParamAccess.tree);
        output.AddTextParameter("Hash", "H", "BLAKE3 result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Sampling, material, matrix, and runtime report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out LandmarkWork work)
    {
        work = default; object? sceneValue = null, materialValue = null;
        List<Point3d> eyes = [], landmarks = []; List<Vector3d> forwards = [], ups = [];
        List<double> radii = [], weights = []; int samples = 64, topK = 10;
        double hfov = 90, vfov = 90, maximum = 1000, clearance = 0; string mask = "-1";
        if (!data.GetData(0, ref sceneValue) || !data.GetDataList(1, eyes) || eyes.Count == 0
            || !data.GetDataList(2, forwards) || !data.GetDataList(3, ups)
            || !data.GetDataList(4, landmarks) || landmarks.Count == 0) return false;
        data.GetDataList(5, radii); data.GetDataList(6, weights); data.GetData(7, ref materialValue);
        data.GetData(8, ref samples); data.GetData(9, ref hfov); data.GetData(10, ref vfov);
        data.GetData(11, ref maximum); data.GetData(12, ref topK); data.GetData(13, ref mask); data.GetData(14, ref clearance);
        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 4, 4096);
            if (!IntelligenceHelpers.Compatible(eyes.Count, forwards.Count) || !IntelligenceHelpers.Compatible(eyes.Count, ups.Count)
                || !IntelligenceHelpers.Compatible(landmarks.Count, radii.Count) || !IntelligenceHelpers.Compatible(landmarks.Count, weights.Count))
                throw new ArgumentException("Forward/Up and Radius/Weight lists require one value or one per corresponding point.");
            XvarnaScene scene = IntelligenceHelpers.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            AnalysisMaterialLibrary materials = IntelligenceHelpers.Unwrap<AnalysisMaterialLibrary>(materialValue) ?? AnalysisMaterialLibrary.Opaque;
            LandmarkObserver[] observers = eyes.Select((point, index) =>
            {
                Vector3d forward = IntelligenceHelpers.Select(forwards, index, Vector3d.XAxis);
                Vector3d up = IntelligenceHelpers.Select(ups, index, Vector3d.ZAxis);
                return new LandmarkObserver(checked((ulong)index + 1), point.X, point.Y, point.Z,
                    forward.X, forward.Y, forward.Z, up.X, up.Y, up.Z);
            }).ToArray();
            Landmark[] targets = landmarks.Select((point, index) => new Landmark(checked((ulong)index + 1),
                point.X, point.Y, point.Z, IntelligenceHelpers.Select(radii, index, 1.0),
                IntelligenceHelpers.Select(weights, index, 1.0))).ToArray();
            double resolvedClearance = clearance > 0 ? clearance : ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            work = new(scene, observers, targets, materials,
                new LandmarkVisibilityOptions(checked((uint)samples), hfov, vfov, maximum, resolvedClearance,
                    IntelligenceHelpers.Parse(mask, "Category mask", true), checked((uint)topK)));
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(LandmarkWork work) => checked(
        (ulong)work.Observers.Length * (ulong)work.Landmarks.Length * work.Options.SampleCount);

    /// <inheritdoc />
    protected override LandmarkVisibilityResult Execute(LandmarkWork work, CancellationToken token,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Landmark disk sampling"));
        token.ThrowIfCancellationRequested();
        LandmarkVisibilityResult result = work.Scene.AnalyzeLandmarkVisibility(work.Observers, work.Landmarks, work.Materials, work.Options);
        progress.Report((total, total, "Solid angle and attribution")); return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, LandmarkWork work, LandmarkVisibilityResult result)
    {
        DataTree<double> visible = new(), solid = new(), weighted = new(), topFractions = new();
        DataTree<bool> inside = new(); DataTree<string> dominant = new(), topObjects = new();
        for (int observer = 0; observer < work.Observers.Length; observer++)
        {
            GH_Path row = new(observer); IReadOnlyList<LandmarkVisibilityEntry> entries = result.Entries
                .Skip(observer * work.Landmarks.Length).Take(work.Landmarks.Length).ToArray();
            visible.AddRange(entries.Select(value => value.VisibleFraction), row);
            solid.AddRange(entries.Select(value => value.VisibleSolidAngle), row);
            weighted.AddRange(entries.Select(value => value.WeightedVisibilityScore), row);
            inside.AddRange(entries.Select(value => value.InsideFieldOfView), row);
            dominant.AddRange(entries.Select(value => value.DominantBlockerObjectId == 0 ? string.Empty : value.DominantBlockerObjectId.ToString(CultureInfo.InvariantCulture)), row);
            for (int landmark = 0; landmark < work.Landmarks.Length; landmark++)
            {
                GH_Path pair = new(observer, landmark); ulong observerId = work.Observers[observer].ObserverId;
                ulong landmarkId = work.Landmarks[landmark].LandmarkId;
                IEnumerable<ObstructionAttribution> ranked = result.Attribution.Where(value => value.ObserverId == observerId && value.TargetId == landmarkId);
                topObjects.AddRange(ranked.Select(value => value.ObjectId.ToString(CultureInfo.InvariantCulture)), pair);
                topFractions.AddRange(ranked.Select(value => value.Fraction), pair);
            }
        }
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA material-aware landmark visibility{Environment.NewLine}" +
            $"Matrix: {work.Observers.Length:N0} observers × {work.Landmarks.Length:N0} landmarks × {work.Options.SampleCount:N0} disk samples{Environment.NewLine}" +
            $"Attribution rows: {result.Attribution.Count:N0}; category rows: {result.Categories.Count:N0}; analysis {result.AnalysisTimeMicroseconds / 1000.0:N3} ms{Environment.NewLine}" +
            $"Hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataTree(1, visible); data.SetDataTree(2, solid);
        data.SetDataTree(3, weighted); data.SetDataTree(4, inside); data.SetDataTree(5, dominant);
        data.SetDataTree(6, topObjects); data.SetDataTree(7, topFractions); data.SetData(8, result.ContentHash); data.SetData(9, report);
        Message = $"Landmark\n{work.Observers.Length:N0} × {work.Landmarks.Length:N0}";
    }
}

/// <summary>Immutable captured landmark work.</summary>
public readonly record struct LandmarkWork(
    XvarnaScene Scene, LandmarkObserver[] Observers, Landmark[] Landmarks,
    AnalysisMaterialLibrary Materials, LandmarkVisibilityOptions Options);
