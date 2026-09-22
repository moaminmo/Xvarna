using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Computes DAENA planar isovists with complete radial and blocker attribution.</summary>
public sealed class IsovistComponent : ScheduledAnalysisComponent<IsovistWork, IsovistResult>
{
    /// <summary>Initializes the planar isovist component.</summary>
    public IsovistComponent()
        : base("XVARNA Isovist", "XV Isovist",
            "Computes deterministic DAENA planar isovists with area, perimeter, compactness, depth distribution, convergence, and first-blocker attribution.",
            "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("7551860d-e9fc-4b1e-b2a8-f26e5a95f044");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Isovist";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        input.AddPointParameter("Eyes", "E", "Eye/viewpoint positions in active Rhino units.", GH_ParamAccess.list);
        input.AddVectorParameter("Plane Normals", "N", "One plane normal or one per eye. Empty uses world Z.", GH_ParamAccess.list);
        input.AddVectorParameter("Forward", "F", "One forward vector or one per eye. Empty uses world X.", GH_ParamAccess.list);
        input.AddNumberParameter("Field of View", "FOV", "Horizontal field of view in degrees: >0 and <=360.", GH_ParamAccess.item, 360.0);
        input.AddIntegerParameter("Samples", "R", "Angular rays per eye, from 32 through 65536.", GH_ParamAccess.item, 720);
        input.AddNumberParameter("Maximum Distance", "Max", "Finite radial clipping distance in model units.", GH_ParamAccess.item, 100.0);
        input.AddNumberParameter("Eye Offset", "Eps", "Offset along the plane normal; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        input.AddTextParameter("Category Mask", "C", "Unsigned 64-bit category filter; -1 selects all.", GH_ParamAccess.item, "-1");
        input.AddBooleanParameter("Run", "Run", "Compute the native visibility study.", GH_ParamAccess.item, true);
        input[2].Optional = true;
        input[3].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Isovist Result", "I", "Immutable DAENA result for automation and downstream use.", GH_ParamAccess.item);
        output.AddCurveParameter("Isovists", "P", "Ordered sampled isovist boundary per eye.", GH_ParamAccess.list);
        output.AddLineParameter("Radials", "L", "All sampled eye-to-boundary lines.", GH_ParamAccess.tree);
        output.AddNumberParameter("Distances", "D", "Radial distance tree, one branch per eye.", GH_ParamAccess.tree);
        output.AddNumberParameter("Area", "A", "Sampled isovist area in squared model units.", GH_ParamAccess.list);
        output.AddNumberParameter("Perimeter", "P", "Sampled isovist perimeter in model units.", GH_ParamAccess.list);
        output.AddNumberParameter("Compactness", "K", "Circular compactness from zero through one.", GH_ParamAccess.list);
        output.AddNumberParameter("Mean Depth", "MD", "Mean radial depth.", GH_ParamAccess.list);
        output.AddNumberParameter("Convergence", "ΔA", "Full-versus-half-sample area delta.", GH_ParamAccess.list);
        output.AddTextParameter("Dominant Occluder", "OID", "Object with the largest angular blocked share.", GH_ParamAccess.list);
        output.AddNumberParameter("Open Fraction", "Open", "Directions reaching the radial limit.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "Deterministic scene/input/policy/result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "R", "DAENA method, resolution, convergence, state, runtime, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out IsovistWork work)
    {
        work = default;
        object? wrapped = null;
        List<Point3d> eyes = [];
        List<Vector3d> normals = [];
        List<Vector3d> forwards = [];
        double fov = 360.0;
        int samples = 720;
        double maximum = 100.0;
        double offset = 0.0;
        string category = "-1";
        bool run = true;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, eyes) || eyes.Count == 0) return false;
        data.GetDataList(2, normals);
        data.GetDataList(3, forwards);
        data.GetData(4, ref fov);
        data.GetData(5, ref samples);
        data.GetData(6, ref maximum);
        data.GetData(7, ref offset);
        data.GetData(8, ref category);
        data.GetData(9, ref run);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = Unwrap(wrapped);
        if (scene is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene must come directly from XV Scene."); return false; }
        if (!Compatible(normals.Count, eyes.Count) || !Compatible(forwards.Count, eyes.Count))
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Normals and Forward must be empty, one item, or one item per eye."); return false; }

        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 32, 65_536);
            ulong mask = ParseMask(category);
            double resolvedOffset = offset > 0.0 && double.IsFinite(offset)
                ? offset : ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            PlanarViewpoint[] viewpoints = eyes.Select((eye, index) =>
            {
                Vector3d normal = Select(normals, index, Vector3d.ZAxis);
                Vector3d forward = Select(forwards, index, Vector3d.XAxis);
                return new PlanarViewpoint(checked((ulong)index + 1), eye.X, eye.Y, eye.Z,
                    normal.X, normal.Y, normal.Z, forward.X, forward.Y, forward.Z);
            }).ToArray();
            work = new(scene, viewpoints,
                new IsovistOptions(checked((uint)samples), fov, maximum, resolvedOffset, mask),
                eyes.ToArray());
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(IsovistWork work) =>
        checked((ulong)work.Viewpoints.Length * work.Options.SampleCount);

    /// <inheritdoc />
    protected override IsovistResult Execute(
        IsovistWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Tracing radial visibility"));
        cancellationToken.ThrowIfCancellationRequested();
        IsovistResult result = work.Scene.AnalyzeIsovists(work.Viewpoints, work.Options);
        progress.Report((total, total, "Computing spatial metrics"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, IsovistWork work, IsovistResult result)
    {
        IReadOnlyList<Point3d> eyes = work.Eyes;
        DataTree<Line> radialTree = new();
        DataTree<double> distanceTree = new();
        List<Curve> polygons = [];
        int samples = checked((int)result.Options.SampleCount);
        bool closed = Math.Abs(result.Options.FieldOfViewDegrees - 360.0) <= 1.0e-9;
        for (int eyeIndex = 0; eyeIndex < eyes.Count; eyeIndex++)
        {
            GH_Path path = new(eyeIndex);
            IReadOnlyList<IsovistRay> rays = result.Rays.Skip(eyeIndex * samples).Take(samples).ToArray();
            List<Point3d> points = rays.Select(ray => new Point3d(ray.EndpointX, ray.EndpointY, ray.EndpointZ)).ToList();
            if (closed) points.Add(points[0]); else { points.Insert(0, eyes[eyeIndex]); points.Add(eyes[eyeIndex]); }
            polygons.Add(new PolylineCurve(points));
            radialTree.AddRange(rays.Select(ray => new Line(eyes[eyeIndex], new Point3d(ray.EndpointX, ray.EndpointY, ray.EndpointZ))), path);
            distanceTree.AddRange(rays.Select(ray => ray.Distance), path);
        }
        double maximumDelta = result.Summaries.Max(value => value.AreaConvergenceDelta);
        ulong open = result.Summaries.Aggregate(0UL, (sum, value) => checked(sum + value.OpenCount));
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA deterministic sampled planar isovist over ZAMYAD{Environment.NewLine}" +
            $"Study: {eyes.Count:N0} eye(s) × {samples:N0} rays = {result.Rays.Count:N0}; FOV {result.Options.FieldOfViewDegrees:G6}°{Environment.NewLine}" +
            $"States: {result.Rays.Count - checked((int)open):N0} occluded; {open:N0} open at {result.Options.MaximumDistance:G10}; max Δarea {maximumDelta:G6}{Environment.NewLine}" +
            $"Metrics: area, perimeter, centroid offset, radial min/mean/max/σ/skew, compactness, dominant first blocker{Environment.NewLine}" +
            $"Policy: offset {result.Options.EyeOffset:G10}; category {result.Options.CategoryMask}; sampled approximation with explicit convergence{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, polygons); data.SetDataTree(2, radialTree); data.SetDataTree(3, distanceTree);
        data.SetDataList(4, result.Summaries.Select(value => value.Area));
        data.SetDataList(5, result.Summaries.Select(value => value.Perimeter));
        data.SetDataList(6, result.Summaries.Select(value => value.Compactness));
        data.SetDataList(7, result.Summaries.Select(value => value.MeanRadial));
        data.SetDataList(8, result.Summaries.Select(value => value.AreaConvergenceDelta));
        data.SetDataList(9, result.Summaries.Select(value => value.DominantOccluderObjectId == 0 ? string.Empty : value.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture)));
        data.SetDataList(10, result.Summaries.Select(value => (double)value.OpenCount / result.Options.SampleCount));
        data.SetData(11, result.ContentHash); data.SetData(12, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"DAENA\n{eyes.Count:N0} × {samples:N0}");
    }

    private static bool Compatible(int count, int sourceCount) => count is 0 or 1 || count == sourceCount;
    private static T Select<T>(IReadOnlyList<T> values, int index, T fallback) => values.Count == 0 ? fallback : values[values.Count == 1 ? 0 : index];
    private static ulong ParseMask(string value) => value == "-1" ? ulong.MaxValue : ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);
    private static XvarnaScene? Unwrap(object? value) => value switch { XvarnaScene scene => scene, GH_ObjectWrapper { Value: XvarnaScene scene } => scene, _ => null };
}

/// <summary>Immutable captured input for scheduled planar-isovist work.</summary>
public readonly record struct IsovistWork(
    XvarnaScene Scene,
    PlanarViewpoint[] Viewpoints,
    IsovistOptions Options,
    Point3d[] Eyes);
