using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper.Components.Visibility;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Computes freeform oriented solar-access and target-shading envelopes.</summary>
public sealed class SolarEnvelopeComponent : ScheduledAnalysisComponent<SolarEnvelopeWork, SolarEnvelopeResult>
{
    private Point3d[] previewAccess = [];
    private Point3d[] previewShade = [];

    /// <summary>Initializes the envelope component.</summary>
    public SolarEnvelopeComponent() : base(
        "XVARNA Solar Envelope", "XV Solar Envelope",
        "Computes material-aware access-preserving and target-shading limits along freeform candidate axes.",
        "XVARNA", "03 Solar")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("b4215c17-eb11-44ac-bc8f-2013ea1b93f6");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override string AnalysisName => "HVARE Solar Envelope";
    /// <inheritdoc />
    public override BoundingBox ClippingBox => previewAccess.Concat(previewShade).Any()
        ? new BoundingBox(previewAccess.Concat(previewShade)) : base.ClippingBox;
    /// <inheritdoc />
    public override void DrawViewportWires(IGH_PreviewArgs args)
    {
        base.DrawViewportWires(args);
        if (previewAccess.Length > 1) args.Display.DrawPointCloud(new PointCloud(previewAccess), 3, Color.OrangeRed);
        if (previewShade.Length > 1) args.Display.DrawPointCloud(new PointCloud(previewShade), 3, Color.DeepSkyBlue);
    }

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene containing existing context.", GH_ParamAccess.item);
        input.AddGenericParameter("Sun Set", "Sun", "XV Sun Vectors result.", GH_ParamAccess.item);
        input.AddPointParameter("Protected Sensors", "P", "Existing points whose baseline solar access is protected.", GH_ParamAccess.list);
        input.AddVectorParameter("Normals", "N", "One normal or one per protected sensor.", GH_ParamAccess.list);
        input.AddPointParameter("Candidates", "C", "Candidate column base points.", GH_ParamAccess.list);
        input.AddGenericParameter("Materials", "M", "Optional context material catalog.", GH_ParamAccess.item);
        input.AddNumberParameter("Candidate Radius", "R", "Radius around each oriented candidate axis in model units.", GH_ParamAccess.item, 0.5);
        input.AddNumberParameter("Preserve", "Keep", "Required preserved fraction of projected baseline hours.", GH_ParamAccess.item, 0.8);
        input.AddNumberParameter("Shade", "Shade", "Target fraction intercepted by shading threshold.", GH_ParamAccess.item, 0.5);
        input.AddNumberParameter("Axis Clearance", "Eps", "Safety distance below/above limiting constraints along each candidate axis.", GH_ParamAccess.item, 0.05);
        input.AddTextParameter("Category Mask", "Cat", "-1 for all or unsigned mask.", GH_ParamAccess.item, "-1");
        input.AddVectorParameter("Candidate Axes", "Axes", "Optional one build direction or one per candidate; defaults to World Z.", GH_ParamAccess.list);
        input[5].Optional = true;
        input[11].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Envelope Result", "R", "Complete immutable material-aware threshold result.", GH_ParamAccess.item);
        output.AddPointParameter("Access Envelope", "Access", "Endpoint at the maximum access-preserving axis distance.", GH_ParamAccess.list);
        output.AddNumberParameter("Maximum Access Height", "Hmax", "Distance along each candidate axis.", GH_ParamAccess.list);
        output.AddPointParameter("Shading Envelope", "Shade", "Endpoint at the minimum target-shading axis distance.", GH_ParamAccess.list);
        output.AddNumberParameter("Minimum Shading Height", "Hmin", "Distance along each candidate axis.", GH_ParamAccess.list);
        output.AddNumberParameter("Considered Hours", "Hours", "Transmission-weighted projected baseline hours.", GH_ParamAccess.list);
        output.AddTextParameter("Access Control", "AC", "Controlling sensor and UTC interval.", GH_ParamAccess.list);
        output.AddTextParameter("Shading Control", "SC", "Controlling sensor and UTC interval.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "BLAKE3 result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Proxy scope, thresholds, constraints, and runtime report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out SolarEnvelopeWork work)
    {
        work = default; object? sceneValue = null, sunValue = null, materialValue = null;
        List<Point3d> sensors = [], candidates = []; List<Vector3d> normals = [], axes = [];
        double radius = 0.5, preserve = 0.8, shade = 0.5, clearance = 0.05; string mask = "-1";
        if (!data.GetData(0, ref sceneValue) || !data.GetData(1, ref sunValue)
            || !data.GetDataList(2, sensors) || sensors.Count == 0 || !data.GetDataList(3, normals)
            || !data.GetDataList(4, candidates) || candidates.Count == 0) return false;
        data.GetData(5, ref materialValue); data.GetData(6, ref radius); data.GetData(7, ref preserve);
        data.GetData(8, ref shade); data.GetData(9, ref clearance); data.GetData(10, ref mask);
        data.GetDataList(11, axes);
        try
        {
            if (normals.Count is not 1 && normals.Count != sensors.Count) throw new ArgumentException("Normals require one value or one per protected sensor.");
            if (axes.Count is not 0 and not 1 && axes.Count != candidates.Count) throw new ArgumentException("Candidate Axes require zero, one value, or one per candidate.");
            XvarnaScene scene = IntelligenceHelpers.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            XvarnaSunSet sun = IntelligenceHelpers.Unwrap<XvarnaSunSet>(sunValue) ?? throw new ArgumentException("Sun Set must come from XV Sun Vectors.");
            AnalysisMaterialLibrary materials = IntelligenceHelpers.Unwrap<AnalysisMaterialLibrary>(materialValue) ?? AnalysisMaterialLibrary.Opaque;
            SolarSensor[] protectedSensors = sensors.Select((point, index) =>
            {
                Vector3d normal = normals[normals.Count == 1 ? 0 : index];
                return new SolarSensor(checked((ulong)index + 1), point.X, point.Y, point.Z, normal.X, normal.Y, normal.Z);
            }).ToArray();
            Vector3d[] candidateAxes = candidates.Select((_, index) =>
            {
                Vector3d axis = axes.Count == 0 ? Vector3d.ZAxis : axes[axes.Count == 1 ? 0 : index];
                if (!axis.Unitize()) throw new ArgumentException($"Candidate axis {index} must be finite and non-zero.");
                return axis;
            }).ToArray();
            OrientedEnvelopeCandidate[] cells = candidates.Select((point, index) =>
                new OrientedEnvelopeCandidate(checked((ulong)index + 1), point.X, point.Y, point.Z,
                    candidateAxes[index].X, candidateAxes[index].Y, candidateAxes[index].Z)).ToArray();
            work = new(scene, sun, protectedSensors, cells, candidateAxes, materials,
                new SolarEnvelopeOptions(radius, preserve, shade, clearance,
                    ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits, double.PositiveInfinity,
                    IntelligenceHelpers.Parse(mask, "Category mask", true)));
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(SolarEnvelopeWork work) => checked(
        (ulong)work.Sensors.Length * (ulong)work.Candidates.Length * (ulong)work.SunSet.Samples.Count);

    /// <inheritdoc />
    protected override SolarEnvelopeResult Execute(SolarEnvelopeWork work, CancellationToken token,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Projecting protected solar rays"));
        token.ThrowIfCancellationRequested();
        SolarEnvelopeResult result = work.Scene.AnalyzeOrientedSolarEnvelope(work.Sensors, work.Candidates, work.SunSet, work.Materials, work.Options);
        progress.Report((total, total, "Weighted envelope thresholds")); return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, SolarEnvelopeWork work, SolarEnvelopeResult result)
    {
        previewAccess = result.Cells.Select((value, index) => (value, index))
            .Where(entry => double.IsFinite(entry.value.MaximumSolarAccessHeight))
            .Select(entry => new Point3d(entry.value.X, entry.value.Y, entry.value.Z)
                + work.CandidateAxes[entry.index] * entry.value.MaximumSolarAccessHeight).ToArray();
        previewShade = result.Cells.Select((value, index) => (value, index))
            .Where(entry => double.IsFinite(entry.value.MinimumShadingHeight))
            .Select(entry => new Point3d(entry.value.X, entry.value.Y, entry.value.Z)
                + work.CandidateAxes[entry.index] * entry.value.MinimumShadingHeight).ToArray();
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: HVARE freeform oriented-column solar/shading envelope screening{Environment.NewLine}" +
            $"Study: {work.Candidates.Length:N0} candidates × {work.Sensors.Length:N0} protected sensors × {work.SunSet.Samples.Count:N0} intervals{Environment.NewLine}" +
            $"Policy: radius {work.Options.CandidateRadius:G6}; preserve {work.Options.RequiredPreservedFraction:P1}; target shade {work.Options.TargetShadedFraction:P1}{Environment.NewLine}" +
            $"Method: closest ray/axis constraints + transmission-weighted quantiles; per-candidate normalized axes{Environment.NewLine}" +
            $"Evidence: deterministic analytic geometry; early-massing proxy, not final code-compliance or radiosity; analysis {result.AnalysisTimeMicroseconds / 1000.0:N3} ms{Environment.NewLine}" +
            $"Hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, previewAccess);
        data.SetDataList(2, result.Cells.Select(value => value.MaximumSolarAccessHeight));
        data.SetDataList(3, previewShade); data.SetDataList(4, result.Cells.Select(value => value.MinimumShadingHeight));
        data.SetDataList(5, result.Cells.Select(value => value.ConsideredBaselineSunHours));
        data.SetDataList(6, result.Cells.Select(value => Control(value.AccessControl)));
        data.SetDataList(7, result.Cells.Select(value => Control(value.ShadingControl)));
        data.SetData(8, result.ContentHash); data.SetData(9, report);
        Message = $"Solar Envelope\n{result.Cells.Count:N0} cells";
    }

    /// <inheritdoc />
    protected override void ClearScheduledPreview() { previewAccess = []; previewShade = []; }
    private static string Control(EnvelopeControl value) => value.SensorId == 0 ? string.Empty
        : string.Create(CultureInfo.InvariantCulture, $"sensor {value.SensorId}; {value.TimestampUtc:O}; ray Z {value.RayElevation:G8}");
}

/// <summary>Immutable captured solar-envelope work.</summary>
public readonly record struct SolarEnvelopeWork(
    XvarnaScene Scene, XvarnaSunSet SunSet, SolarSensor[] Sensors,
    OrientedEnvelopeCandidate[] Candidates, Vector3d[] CandidateAxes, AnalysisMaterialLibrary Materials,
    SolarEnvelopeOptions Options);
