using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Computes direct sun, shadow, access, and temporal occlusion attribution.</summary>
public sealed class SunHoursComponent : ScheduledAnalysisComponent<SunHoursWork, DirectSunResult>
{
    /// <summary>Initializes the HVARE direct-sun component.</summary>
    public SunHoursComponent()
        : base(
            "XVARNA Sun Hours",
            "XV Sun Hours",
            "Computes weighted Direct Sun Hours, Shadow Hours, solar access, and first-hit occluder attribution for oriented sensors.",
            "XVARNA",
            "03 Solar")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("8cb80583-9be0-483f-aa10-e6946ed4d2de");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "HVARE Sun Hours";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        inputManager.AddGenericParameter("Sun Set", "Sun", "Ordered solar positions produced by XV Sun Vectors.", GH_ParamAccess.item);
        inputManager.AddPointParameter("Sensors", "P", "Analysis points in active Rhino model units.", GH_ParamAccess.list);
        inputManager.AddVectorParameter("Normals", "N", "One oriented surface normal for all sensors or one per sensor.", GH_ParamAccess.list);
        inputManager.AddTextParameter("Sensor IDs", "ID", "Optional exact unsigned 64-bit IDs: none for 1..N, one start value, or one per sensor.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Offset", "Eps", "Positive ray-origin offset in model units; 0 uses the active document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Maximum Distance", "Max", "Maximum obstruction distance in model units; 0 means unbounded.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Maximum Incidence Angle", "Ang", "Maximum angle in degrees between sensor normal and sun vector; 90 accepts the front hemisphere.", GH_ParamAccess.item, 90.0);
        inputManager.AddTextParameter("Category Mask", "C", "Exact unsigned 64-bit occluder-category filter. -1 selects all categories.", GH_ParamAccess.item, "-1");
        inputManager[4].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Result", "R", "Complete HVARE result including summaries and sensor-major timeline.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Sun Hours", "SH", "Weighted unobstructed eligible hours per sensor.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Shadow Hours", "DH", "Weighted obstructed eligible hours per sensor.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Eligible Hours", "EH", "Weighted front-facing active hours per sensor.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Solar Access", "SA", "Sun Hours divided by Eligible Hours; zero when no interval is eligible.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Dominant Occluder", "OID", "Object ID contributing the most weighted shadow hours; empty when unobstructed.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Dominant Hours", "OH", "Weighted shadow hours attributed to the dominant object.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Visible Count", "VC", "Visible active intervals per sensor.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Blocked Count", "BC", "Blocked active intervals per sensor.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Back-Facing Count", "FC", "Active intervals rejected by the oriented-surface policy.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Timeline State", "TS", "Per-sensor branches: 0 inactive, 1 back-facing, 2 visible, 3 blocked.", GH_ParamAccess.tree);
        outputManager.AddTextParameter("Timeline Occluder", "TO", "Per-sensor branches of first-hit object IDs, aligned to Sun Set samples.", GH_ParamAccess.tree);
        outputManager.AddTextParameter("Hash", "H", "BLAKE3 analysis identity.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "Rep", "Engine, policy, counts, timing, attribution, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess dataAccess, out SunHoursWork work)
    {
        work = default;
        object? wrappedScene = null;
        object? wrappedSunSet = null;
        List<Point3d> points = [];
        List<Vector3d> normals = [];
        List<string> sensorIds = [];
        double offset = 0.0;
        double maximumDistance = 0.0;
        double maximumIncidenceAngle = 90.0;
        string categoryMaskText = "-1";
        if (!dataAccess.GetData(0, ref wrappedScene)
            || !dataAccess.GetData(1, ref wrappedSunSet)
            || !dataAccess.GetDataList(2, points)
            || points.Count == 0
            || !dataAccess.GetDataList(3, normals)
            || normals.Count == 0)
        {
            return false;
        }
        dataAccess.GetDataList(4, sensorIds);
        dataAccess.GetData(5, ref offset);
        dataAccess.GetData(6, ref maximumDistance);
        dataAccess.GetData(7, ref maximumIncidenceAngle);
        dataAccess.GetData(8, ref categoryMaskText);

        XvarnaScene? scene = UnwrapScene(wrappedScene);
        XvarnaSunSet? sunSet = UnwrapSunSet(wrappedSunSet);
        if (scene is null || sunSet is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene and Sun Set must come directly from XV Scene and XV Sun Vectors.");
            return false;
        }
        if ((normals.Count != 1 && normals.Count != points.Count)
            || (sensorIds.Count is not 0 and not 1 && sensorIds.Count != points.Count))
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Normals and Sensor IDs must contain one value or exactly one value per sensor; IDs may also be empty.");
            return false;
        }

        try
        {
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            double resolvedOffset = offset > 0.0 && double.IsFinite(offset)
                ? offset
                : document?.ModelAbsoluteTolerance ?? 1.0e-6;
            double resolvedMaximum = maximumDistance == 0.0
                ? double.PositiveInfinity
                : maximumDistance;
            if (!double.IsFinite(maximumIncidenceAngle)
                || maximumIncidenceAngle < 0.0
                || maximumIncidenceAngle > 180.0)
            {
                throw new ArgumentException("Maximum Incidence Angle must be finite and between 0 and 180 degrees.");
            }
            ulong categoryMask = ResolveCategoryMask(categoryMaskText);
            SolarSensor[] sensors = points
                .Select((point, index) =>
                {
                    Vector3d normal = normals.Count == 1 ? normals[0] : normals[index];
                    return new SolarSensor(
                        ResolveSensorId(sensorIds, index),
                        point.X,
                        point.Y,
                        point.Z,
                        normal.X,
                        normal.Y,
                        normal.Z);
                })
                .ToArray();
            DirectSunOptions options = new(
                resolvedOffset,
                resolvedMaximum,
                Math.Cos(RhinoMath.ToRadians(maximumIncidenceAngle)),
                categoryMask);
            work = new(scene, sunSet, sensors, options, maximumIncidenceAngle);
            return true;
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ObjectDisposedException
            or ArgumentException
            or OverflowException
            or InvalidOperationException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(SunHoursWork work) =>
        checked((ulong)work.Sensors.Length * (ulong)work.SunSet.Samples.Count);

    /// <inheritdoc />
    protected override DirectSunResult Execute(
        SunHoursWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Tracing sun visibility"));
        cancellationToken.ThrowIfCancellationRequested();
        DirectSunResult result = work.Scene.AnalyzeDirectSun(work.Sensors, work.SunSet, work.Options);
        progress.Report((total, total, "Temporal attribution"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess dataAccess, SunHoursWork work, DirectSunResult result)
    {
        GH_Structure<GH_Integer> stateTree = [];
        GH_Structure<GH_String> occluderTree = [];
        int sunCount = checked((int)result.SunCount);
        for (int sensorIndex = 0; sensorIndex < work.Sensors.Length; sensorIndex++)
        {
            GH_Path path = new(sensorIndex);
            int rowStart = checked(sensorIndex * sunCount);
            for (int sunIndex = 0; sunIndex < sunCount; sunIndex++)
            {
                SunTimelineEntry entry = result.Timeline[rowStart + sunIndex];
                stateTree.Append(new((int)entry.State), path);
                occluderTree.Append(new(entry.State == SunState.Blocked
                    ? entry.ObjectId.ToString(CultureInfo.InvariantCulture)
                    : string.Empty), path);
            }
        }
        ulong totalVisible = result.Summaries.Aggregate(0UL, (total, summary) => checked(total + summary.VisibleCount));
        ulong totalBlocked = result.Summaries.Aggregate(0UL, (total, summary) => checked(total + summary.BlockedCount));
        ulong totalBackFacing = result.Summaries.Aggregate(0UL, (total, summary) => checked(total + summary.BackFacingCount));
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: HVARE direct sun over ZAMYAD closest-hit attribution{Environment.NewLine}" +
            $"Sensors: {work.Sensors.Length:N0}; sun samples: {sunCount:N0}; evaluated pairs: {(long)work.Sensors.Length * sunCount:N0}{Environment.NewLine}" +
            $"States: {totalVisible:N0} visible; {totalBlocked:N0} blocked; {totalBackFacing:N0} back-facing{Environment.NewLine}" +
            $"Policy: offset {work.Options.SensorOffset:G12} model units; max distance {(double.IsPositiveInfinity(work.Options.MaximumDistance) ? "unbounded" : work.Options.MaximumDistance.ToString("G12", CultureInfo.InvariantCulture))}; incidence ≤ {work.MaximumIncidenceAngle:G12}°{Environment.NewLine}" +
            $"Category mask: {work.Options.CategoryMask}; analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms{Environment.NewLine}" +
            $"Result hash: {result.ContentHash}");
        dataAccess.SetData(0, result);
        dataAccess.SetDataList(1, result.Summaries.Select(summary => summary.DirectSunHours));
        dataAccess.SetDataList(2, result.Summaries.Select(summary => summary.ShadowHours));
        dataAccess.SetDataList(3, result.Summaries.Select(summary => summary.EligibleHours));
        dataAccess.SetDataList(4, result.Summaries.Select(summary => summary.SolarAccessRatio));
        dataAccess.SetDataList(5, result.Summaries.Select(summary => summary.DominantOccluderObjectId == 0
            ? string.Empty : summary.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(6, result.Summaries.Select(summary => summary.DominantOccluderHours));
        dataAccess.SetDataList(7, result.Summaries.Select(summary => checked((int)summary.VisibleCount)));
        dataAccess.SetDataList(8, result.Summaries.Select(summary => checked((int)summary.BlockedCount)));
        dataAccess.SetDataList(9, result.Summaries.Select(summary => checked((int)summary.BackFacingCount)));
        dataAccess.SetDataTree(10, stateTree);
        dataAccess.SetDataTree(11, occluderTree);
        dataAccess.SetData(12, result.ContentHash);
        dataAccess.SetData(13, report);
    }

    private static XvarnaScene? UnwrapScene(object? value) =>
        value switch
        {
            XvarnaScene scene => scene,
            GH_ObjectWrapper { Value: XvarnaScene scene } => scene,
            _ => null,
        };

    private static XvarnaSunSet? UnwrapSunSet(object? value) =>
        value switch
        {
            XvarnaSunSet sunSet => sunSet,
            GH_ObjectWrapper { Value: XvarnaSunSet sunSet } => sunSet,
            _ => null,
        };

    private static ulong ResolveSensorId(IReadOnlyList<string> values, int index)
    {
        if (values.Count == 0)
        {
            return checked((ulong)index + 1);
        }
        string value = values.Count == 1 ? values[0] : values[index];
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
        {
            throw new ArgumentException($"Sensor ID '{value}' is not an unsigned 64-bit integer.");
        }
        return values.Count == 1 ? checked(parsed + (ulong)index) : parsed;
    }

    private static ulong ResolveCategoryMask(string value)
    {
        if (string.Equals(value, "-1", StringComparison.Ordinal))
        {
            return ulong.MaxValue;
        }
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
        {
            throw new ArgumentException($"Category mask '{value}' is not -1 or an unsigned 64-bit integer.");
        }
        return parsed;
    }
}

/// <summary>Immutable captured input for scheduled direct-sun attribution.</summary>
public readonly record struct SunHoursWork(
    XvarnaScene Scene,
    XvarnaSunSet SunSet,
    SolarSensor[] Sensors,
    DirectSunOptions Options,
    double MaximumIncidenceAngle);
