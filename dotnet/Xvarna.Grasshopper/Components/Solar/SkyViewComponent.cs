using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Display;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Contained result used by the Grasshopper task scheduler.</summary>
public sealed record SkyViewWorkOutput(SkyViewResult? Result, string? Error, bool Cancelled);

/// <summary>Runs deterministic ASMAN sky-view analysis without blocking the Grasshopper solution thread.</summary>
public sealed class SkyViewComponent : GH_TaskCapableComponent<SkyViewWorkOutput>
{
    private Point3d[] previewPoints = [];
    private Color[] previewColors = [];

    /// <summary>Initializes the ASMAN Sky View component.</summary>
    public SkyViewComponent()
        : base(
            "XVARNA Sky View",
            "XV Sky View",
            "Computes cosine-weighted Sky View Factor, visible hemisphere, convergence diagnostics, and first-hit obstruction attribution.",
            "XVARNA",
            "03 Solar")
    {
        UseTasks = true;
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("fe598553-6b7f-4820-b0af-5b4402d426f5");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    public override BoundingBox ClippingBox
    {
        get
        {
            BoundingBox box = base.ClippingBox;
            foreach (Point3d point in previewPoints)
            {
                box.Union(point);
            }
            return box;
        }
    }

    /// <inheritdoc />
    public override void DrawViewportWires(IGH_PreviewArgs args)
    {
        base.DrawViewportWires(args);
        for (int index = 0; index < previewPoints.Length; index++)
        {
            args.Display.DrawPoint(previewPoints[index], PointStyle.RoundControlPoint, 7, previewColors[index]);
        }
    }

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        inputManager.AddPointParameter("Sensors", "P", "Sky-analysis points in active Rhino model units.", GH_ParamAccess.list);
        inputManager.AddVectorParameter("Normals", "N", "One oriented normal for all sensors or one per sensor.", GH_ParamAccess.list);
        inputManager.AddTextParameter("Sensor IDs", "ID", "Optional unsigned 64-bit IDs: none for 1..N, one start value, or one per sensor.", GH_ParamAccess.list);
        inputManager.AddIntegerParameter("Samples", "Q", "Fibonacci hemisphere samples per sensor, from 16 to 262144.", GH_ParamAccess.item, 2048);
        inputManager.AddTextParameter("Seed", "Seed", "Exact unsigned 64-bit azimuth-phase seed.", GH_ParamAccess.item, "0");
        inputManager.AddNumberParameter("Offset", "Eps", "Positive ray-origin offset in model units; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Maximum Distance", "Max", "Maximum obstruction distance in model units; 0 means unbounded.", GH_ParamAccess.item, 0.0);
        inputManager.AddTextParameter("Category Mask", "C", "Unsigned 64-bit category filter. -1 selects all categories.", GH_ParamAccess.item, "-1");
        inputManager.AddBooleanParameter("Run", "Run", "Run or recompute the analysis.", GH_ParamAccess.item, true);
        inputManager[3].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Result", "R", "Complete ASMAN result for XV Shadow Mask and downstream automation.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Sky View Factor", "SVF", "Lambert cosine-weighted visible-sky fraction from 0 to 1.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Visible Hemisphere", "VH", "Unweighted visible solid-angle fraction from 0 to 1.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Visible Solid Angle", "Ω", "Visible hemispherical solid angle in steradians.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Convergence Delta", "Δ", "Absolute cosine-weighted difference between interleaved half-sample estimates; not a confidence interval.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Dominant Occluder", "OID", "Object contributing the largest cosine-weighted blocked fraction.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Dominant Blocked Fraction", "OB", "Cosine-weighted blocked fraction attributed to the dominant object.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Visible Count", "VC", "Open-sky directions per sensor.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Blocked Count", "BC", "Blocked directions per sensor.", GH_ParamAccess.list);
        outputManager.AddColourParameter("Heatmap", "Col", "Viridis Sky View Factor colours used by viewport preview.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Hash", "H", "Deterministic BLAKE3 analysis identity.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "Rep", "Sampling, convergence, timing, attribution, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        if (!TryReadWork(dataAccess, out SkyViewWork work))
        {
            return;
        }

        if (InPreSolve)
        {
            Message = "ASMAN\nRunning";
            ulong total = checked((ulong)work.Sensors.Length * work.Options.SampleCount);
            TaskList.Add(AnalysisTaskBridge.Schedule(
                "ASMAN Sky View",
                $"grasshopper:{InstanceGuid:D}",
                total,
                (token, progress) =>
                {
                    progress.Report((0, total, "Sky visibility"));
                    SkyViewWorkOutput result = Compute(work, token);
                    progress.Report((total, total, "Assembling result"));
                    return result;
                },
                CancelToken));
            return;
        }

        SkyViewWorkOutput output;
        if (!GetSolveResults(dataAccess, out output))
        {
            output = Compute(work, CancellationToken.None);
        }
        if (output.Cancelled)
        {
            Message = "ASMAN\nCancelled";
            AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, "ASMAN Sky View was cancelled before completion.");
            return;
        }
        if (output.Error is not null || output.Result is null)
        {
            Message = "ASMAN\nError";
            ClearPreview();
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, output.Error ?? "ASMAN returned no result.");
            return;
        }

        Emit(dataAccess, output.Result);
    }

    private bool TryReadWork(IGH_DataAccess dataAccess, out SkyViewWork work)
    {
        work = default;
        object? wrappedScene = null;
        List<Point3d> points = [];
        List<Vector3d> normals = [];
        List<string> sensorIds = [];
        int samples = 2048;
        string seedText = "0";
        double offset = 0.0;
        double maximumDistance = 0.0;
        string categoryMaskText = "-1";
        bool run = true;
        if (!dataAccess.GetData(0, ref wrappedScene)
            || !dataAccess.GetDataList(1, points)
            || points.Count == 0
            || !dataAccess.GetDataList(2, normals)
            || normals.Count == 0)
        {
            ClearPreview();
            return false;
        }
        dataAccess.GetDataList(3, sensorIds);
        dataAccess.GetData(4, ref samples);
        dataAccess.GetData(5, ref seedText);
        dataAccess.GetData(6, ref offset);
        dataAccess.GetData(7, ref maximumDistance);
        dataAccess.GetData(8, ref categoryMaskText);
        dataAccess.GetData(9, ref run);
        if (!run)
        {
            Message = "ASMAN\nPaused";
            ClearPreview();
            return false;
        }

        XvarnaScene? scene = UnwrapScene(wrappedScene);
        if (scene is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene must come directly from XV Scene.");
            return false;
        }
        if (normals.Count != 1 && normals.Count != points.Count)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Normals must contain one value or exactly one value per sensor.");
            return false;
        }
        if (sensorIds.Count is not 0 and not 1 && sensorIds.Count != points.Count)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Sensor IDs must be empty, one start value, or exactly one value per sensor.");
            return false;
        }

        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 16, 262_144);
            if (samples is < 16 or > 262144)
            {
                throw new ArgumentOutOfRangeException(nameof(samples), "Samples must be between 16 and 262144.");
            }
            ulong seed = ParseUnsigned(seedText, "Seed");
            ulong categoryMask = string.Equals(categoryMaskText, "-1", StringComparison.Ordinal)
                ? ulong.MaxValue
                : ParseUnsigned(categoryMaskText, "Category Mask");
            double resolvedOffset = offset > 0.0 && double.IsFinite(offset)
                ? offset
                : ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            double resolvedMaximum = maximumDistance == 0.0
                ? double.PositiveInfinity
                : maximumDistance;
            SkySensor[] sensors = points.Select((point, index) =>
            {
                Vector3d normal = normals.Count == 1 ? normals[0] : normals[index];
                return new SkySensor(
                    ResolveSensorId(sensorIds, index),
                    point.X,
                    point.Y,
                    point.Z,
                    normal.X,
                    normal.Y,
                    normal.Z);
            }).ToArray();
            work = new(
                scene,
                sensors,
                new(checked((uint)samples), seed, resolvedOffset, resolvedMaximum, categoryMask));
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    private static SkyViewWorkOutput Compute(SkyViewWork work, CancellationToken cancellationToken)
    {
        try
        {
            return new(work.Scene.AnalyzeSkyView(work.Sensors, work.Options, cancellationToken), null, false);
        }
        catch (OperationCanceledException)
        {
            return new(null, null, true);
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ObjectDisposedException
            or ArgumentException
            or OverflowException
            or InvalidOperationException)
        {
            return new(null, exception.Message, false);
        }
    }

    private void Emit(IGH_DataAccess dataAccess, SkyViewResult result)
    {
        Color[] colors = result.Summaries
            .Select(summary => SkyPalette.Viridis(summary.CosineWeightedSkyViewFactor))
            .ToArray();
        previewPoints = result.Sensors
            .Select(sensor => new Point3d(sensor.PositionX, sensor.PositionY, sensor.PositionZ))
            .ToArray();
        previewColors = colors;
        ulong visible = result.Summaries.Aggregate(0UL, (total, item) => checked(total + item.VisibleCount));
        ulong blocked = result.Summaries.Aggregate(0UL, (total, item) => checked(total + item.BlockedCount));
        double maximumDelta = result.Summaries.Max(item => item.CosineConvergenceDelta);
        AnalysisQualityProfile quality = ProductContextRegistry.GetQuality(OnPingDocument());
        XvarnaEvidenceLabel evidence = XvarnaEvidenceLabel.Convergence(
            "interleaved Fibonacci SVF", maximumDelta, 0.0,
            result.Options.SampleCount, quality.ConvergenceTolerance);
        string maximumDistance = double.IsPositiveInfinity(result.Options.MaximumDistance)
            ? "unbounded"
            : result.Options.MaximumDistance.ToString("G12", CultureInfo.InvariantCulture);
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: ASMAN deterministic sky intelligence over ZAMYAD{Environment.NewLine}" +
            $"Sensors: {result.Sensors.Count:N0}; samples/sensor: {result.Options.SampleCount:N0}; rays: {result.Timeline.Count:N0}{Environment.NewLine}" +
            $"States: {visible:N0} visible; {blocked:N0} blocked; max convergence delta: {maximumDelta:G6}{Environment.NewLine}" +
            $"Evidence: {evidence.Level}; {evidence.Statement}{Environment.NewLine}" +
            $"Estimator: equal-solid-angle Fibonacci hemisphere; cosine SVF uses Lambert projected weighting{Environment.NewLine}" +
            $"Policy: seed {result.Options.Seed}; offset {result.Options.SensorOffset:G12}; max distance {maximumDistance}; category mask {result.Options.CategoryMask}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");

        dataAccess.SetData(0, result);
        dataAccess.SetDataList(1, result.Summaries.Select(item => item.CosineWeightedSkyViewFactor));
        dataAccess.SetDataList(2, result.Summaries.Select(item => item.VisibleHemisphereFraction));
        dataAccess.SetDataList(3, result.Summaries.Select(item => item.VisibleSolidAngleSteradians));
        dataAccess.SetDataList(4, result.Summaries.Select(item => item.CosineConvergenceDelta));
        dataAccess.SetDataList(5, result.Summaries.Select(item => item.DominantOccluderObjectId == 0
            ? string.Empty
            : item.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(6, result.Summaries.Select(item => item.DominantOccluderProjectedFraction));
        dataAccess.SetDataList(7, result.Summaries.Select(item => checked((int)item.VisibleCount)));
        dataAccess.SetDataList(8, result.Summaries.Select(item => checked((int)item.BlockedCount)));
        dataAccess.SetDataList(9, colors);
        dataAccess.SetData(10, result.ContentHash);
        dataAccess.SetData(11, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"ASMAN\n{result.Sensors.Count:N0} × {result.Options.SampleCount:N0}");
    }

    private void ClearPreview()
    {
        previewPoints = [];
        previewColors = [];
    }

    private static XvarnaScene? UnwrapScene(object? value) =>
        value switch
        {
            XvarnaScene scene => scene,
            GH_ObjectWrapper { Value: XvarnaScene scene } => scene,
            _ => null,
        };

    private static ulong ResolveSensorId(IReadOnlyList<string> values, int index)
    {
        if (values.Count == 0)
        {
            return checked((ulong)index + 1);
        }
        ulong parsed = ParseUnsigned(values.Count == 1 ? values[0] : values[index], "Sensor ID");
        return values.Count == 1 ? checked(parsed + (ulong)index) : parsed;
    }

    private static ulong ParseUnsigned(string value, string name)
    {
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
        {
            throw new ArgumentException($"{name} '{value}' is not an unsigned 64-bit integer.");
        }
        return parsed;
    }

    private readonly record struct SkyViewWork(
        XvarnaScene Scene,
        SkySensor[] Sensors,
        SkyViewOptions Options);
}
