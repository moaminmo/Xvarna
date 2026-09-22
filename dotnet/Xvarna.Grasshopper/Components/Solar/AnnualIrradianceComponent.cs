using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Display;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Contained annual result used by the Grasshopper task scheduler.</summary>
public sealed record AnnualIrradianceWorkOutput(
    AnnualIrradianceResult? Result,
    string? Error,
    bool Cancelled);

/// <summary>Runs EPW-driven Perez annual irradiance without blocking Grasshopper's solution thread.</summary>
public sealed class AnnualIrradianceComponent : GH_TaskCapableComponent<AnnualIrradianceWorkOutput>
{
    private Point3d[] previewPoints = [];
    private Color[] previewColors = [];

    /// <summary>Initializes the MEHR Annual Irradiance component.</summary>
    public AnnualIrradianceComponent()
        : base(
            "XVARNA Annual Irradiance",
            "XV Irradiance",
            "Computes EPW annual direct, Perez diffuse, ground-reflected, global, peak, sky-obstruction, and first-hit loss attribution.",
            "XVARNA",
            "03 Solar")
    {
        UseTasks = true;
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("c3b236c0-9d0c-4e32-b7ca-5f853f9f393e");

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
        inputManager.AddTextParameter("EPW Path", "EPW", "Path to an EnergyPlus Weather file. EPW interval-ending timestamps are converted to UTC midpoints.", GH_ParamAccess.item);
        inputManager.AddPointParameter("Sensors", "P", "Oriented analysis points in active Rhino model units.", GH_ParamAccess.list);
        inputManager.AddVectorParameter("Normals", "N", "One surface normal for all sensors or one per sensor.", GH_ParamAccess.list);
        inputManager.AddTextParameter("Sensor IDs", "ID", "Optional unsigned 64-bit IDs: none for 1..N, one start value, or one per sensor.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Ground Albedo", "ρ", "Fallback ground albedo from 0 to 1.", GH_ParamAccess.item, 0.2);
        inputManager.AddBooleanParameter("Use EPW Albedo", "EPWρ", "Prefer valid per-interval EPW albedo over the fallback.", GH_ParamAccess.item, true);
        inputManager.AddNumberParameter("Offset", "Eps", "Positive ray-origin offset in model units; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Maximum Distance", "Max", "Maximum obstruction distance in model units; 0 means unbounded.", GH_ParamAccess.item, 0.0);
        inputManager.AddTextParameter("Category Mask", "C", "Unsigned 64-bit category filter; -1 selects all categories.", GH_ParamAccess.item, "-1");
        inputManager.AddNumberParameter("North Rotation", "North", "Counter-clockwise model rotation of true north about +Z, degrees.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Delta T", "ΔT", "Terrestrial Time minus Universal Time for NREL SPA, seconds.", GH_ParamAccess.item, 69.0);
        inputManager.AddNumberParameter("Pressure", "Pr", "Atmospheric pressure for solar refraction, millibars; 0 disables refraction.", GH_ParamAccess.item, 1013.25);
        inputManager.AddNumberParameter("Temperature", "Ta", "Ambient temperature for solar refraction, °C.", GH_ParamAccess.item, 15.0);
        inputManager.AddNumberParameter("Minimum Altitude", "Alt", "Minimum apparent sun altitude for direct and circumsolar tracing, degrees.", GH_ParamAccess.item, 0.0);
        inputManager.AddBooleanParameter("Run", "Run", "Run or recompute the annual analysis.", GH_ParamAccess.item, true);
        inputManager[4].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Result", "R", "Complete immutable annual result and sensor-major timeline.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Global", "G", "Annual total plane-of-array energy, kWh/m².", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Direct", "B", "Annual received beam energy, kWh/m².", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Diffuse Sky", "D", "Annual ray-shaded Perez sky-diffuse energy, kWh/m².", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Ground", "Gr", "Annual isotropic ground-reflected energy, kWh/m².", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Peak", "Pk", "Peak interval-average global irradiance, W/m².", GH_ParamAccess.list);
        outputManager.AddTimeParameter("Peak Time", "PkT", "UTC midpoint of the first peak interval.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Dome Visibility", "DV", "Cosine-weighted visible fraction of the 144-patch sky dome.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Horizon Visibility", "HV", "Projected visible fraction of the 24-sector horizon ring.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Dominant Solar Occluder", "SO", "Object causing the greatest beam plus circumsolar energy loss.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Dominant Loss", "Loss", "Energy assigned to the dominant solar occluder, kWh/m².", GH_ParamAccess.list);
        outputManager.AddTextParameter("Dominant Sky Occluder", "DO", "Object blocking the greatest projected static sky weight.", GH_ParamAccess.list);
        outputManager.AddColourParameter("Heatmap", "Col", "Viridis colours normalized to the maximum annual global result.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Timeline Global", "TG", "Per-sensor branches of interval-average global irradiance, W/m².", GH_ParamAccess.tree);
        outputManager.AddNumberParameter("Timeline Direct", "TB", "Per-sensor branches of received direct energy, Wh/m².", GH_ParamAccess.tree);
        outputManager.AddNumberParameter("Timeline Diffuse", "TD", "Per-sensor branches of received sky-diffuse energy, Wh/m².", GH_ParamAccess.tree);
        outputManager.AddIntegerParameter("Timeline State", "TS", "Per-sensor branches: 0 inactive, 1 back-facing, 2 visible, 3 blocked.", GH_ParamAccess.tree);
        outputManager.AddTextParameter("Timeline Occluder", "TO", "Per-sensor branches of first-hit object IDs.", GH_ParamAccess.tree);
        outputManager.AddTextParameter("Weather Hash", "WH", "Semantic BLAKE3 identity of parsed EPW content.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Result Hash", "H", "Deterministic BLAKE3 analysis identity.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "Rep", "Scientific model, weather diagnostics, policy, runtime, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        if (!TryReadWork(dataAccess, out AnnualIrradianceWork work))
        {
            return;
        }
        if (InPreSolve)
        {
            Message = "MEHR\nRunning";
            ulong total = checked((ulong)work.Sensors.Length * work.Weather.WeatherCount);
            TaskList.Add(AnalysisTaskBridge.Schedule(
                "ATASH Annual Irradiance",
                $"grasshopper:{InstanceGuid:D}",
                total,
                (token, progress) =>
                {
                    progress.Report((0, total, "Annual irradiance"));
                    AnnualIrradianceWorkOutput result = Compute(work, token);
                    progress.Report((total, total, "Assembling result"));
                    return result;
                },
                CancelToken));
            return;
        }
        AnnualIrradianceWorkOutput output;
        if (!GetSolveResults(dataAccess, out output))
        {
            output = Compute(work, CancellationToken.None);
        }
        if (output.Cancelled)
        {
            Message = "MEHR\nCancelled";
            AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, "MEHR annual irradiance was cancelled before completion.");
            return;
        }
        if (output.Error is not null || output.Result is null)
        {
            Message = "MEHR\nError";
            ClearPreview();
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, output.Error ?? "MEHR returned no result.");
            return;
        }
        Emit(dataAccess, output.Result);
    }

    private bool TryReadWork(IGH_DataAccess dataAccess, out AnnualIrradianceWork work)
    {
        work = default;
        object? wrappedScene = null;
        string epwPath = string.Empty;
        List<Point3d> points = [];
        List<Vector3d> normals = [];
        List<string> sensorIds = [];
        double groundAlbedo = 0.2;
        bool useWeatherAlbedo = true;
        double offset = 0.0;
        double maximumDistance = 0.0;
        string categoryMaskText = "-1";
        double north = 0.0;
        double deltaT = 69.0;
        double pressure = 1013.25;
        double temperature = 15.0;
        double minimumAltitude = 0.0;
        bool run = true;
        if (!dataAccess.GetData(0, ref wrappedScene)
            || !dataAccess.GetData(1, ref epwPath)
            || !dataAccess.GetDataList(2, points)
            || points.Count == 0
            || !dataAccess.GetDataList(3, normals)
            || normals.Count == 0)
        {
            ClearPreview();
            return false;
        }
        dataAccess.GetDataList(4, sensorIds);
        dataAccess.GetData(5, ref groundAlbedo);
        dataAccess.GetData(6, ref useWeatherAlbedo);
        dataAccess.GetData(7, ref offset);
        dataAccess.GetData(8, ref maximumDistance);
        dataAccess.GetData(9, ref categoryMaskText);
        dataAccess.GetData(10, ref north);
        dataAccess.GetData(11, ref deltaT);
        dataAccess.GetData(12, ref pressure);
        dataAccess.GetData(13, ref temperature);
        dataAccess.GetData(14, ref minimumAltitude);
        dataAccess.GetData(15, ref run);
        if (!run)
        {
            Message = "MEHR\nPaused";
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
            EpwWeatherFile weather = EpwWeatherFile.Load(epwPath);
            double resolvedOffset = offset > 0.0 && double.IsFinite(offset)
                ? offset
                : RhinoDoc.ActiveDoc?.ModelAbsoluteTolerance ?? 1.0e-6;
            double resolvedMaximum = maximumDistance == 0.0
                ? double.PositiveInfinity
                : maximumDistance;
            ulong categoryMask = string.Equals(categoryMaskText, "-1", StringComparison.Ordinal)
                ? ulong.MaxValue
                : ParseUnsigned(categoryMaskText, "Category Mask");
            SolarSensor[] sensors = points.Select((point, index) =>
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
            }).ToArray();
            AnnualIrradianceOptions options = new(
                resolvedOffset,
                resolvedMaximum,
                categoryMask,
                groundAlbedo,
                useWeatherAlbedo,
                new(deltaT, pressure, temperature, north, minimumAltitude));
            work = new(scene, sensors, weather, options);
            return true;
        }
        catch (Exception exception) when (exception is IOException
            or UnauthorizedAccessException
            or XvarnaNativeException
            or ArgumentException
            or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    private static AnnualIrradianceWorkOutput Compute(
        AnnualIrradianceWork work,
        CancellationToken cancellationToken)
    {
        try
        {
            return new(
                work.Scene.AnalyzeAnnualIrradiance(
                    work.Sensors,
                    work.Weather,
                    work.Options,
                    cancellationToken),
                null,
                false);
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

    private void Emit(IGH_DataAccess dataAccess, AnnualIrradianceResult result)
    {
        double maximumGlobal = result.Summaries.Max(summary => summary.GlobalWhM2);
        Color[] colors = result.Summaries
            .Select(summary => SkyPalette.Viridis(maximumGlobal > 0.0
                ? summary.GlobalWhM2 / maximumGlobal
                : 0.0))
            .ToArray();
        previewPoints = result.Sensors
            .Select(sensor => new Point3d(sensor.PositionX, sensor.PositionY, sensor.PositionZ))
            .ToArray();
        previewColors = colors;

        GH_Structure<GH_Number> globalTree = [];
        GH_Structure<GH_Number> directTree = [];
        GH_Structure<GH_Number> diffuseTree = [];
        GH_Structure<GH_Integer> stateTree = [];
        GH_Structure<GH_String> occluderTree = [];
        int weatherCount = checked((int)result.Weather.WeatherCount);
        for (int sensorIndex = 0; sensorIndex < result.Sensors.Count; sensorIndex++)
        {
            GH_Path path = new(sensorIndex);
            int rowStart = checked(sensorIndex * weatherCount);
            for (int weatherIndex = 0; weatherIndex < weatherCount; weatherIndex++)
            {
                IrradianceTimelineEntry entry = result.Timeline[rowStart + weatherIndex];
                globalTree.Append(new(entry.GlobalWM2), path);
                directTree.Append(new(entry.DirectWhM2), path);
                diffuseTree.Append(new(entry.DiffuseSkyWhM2), path);
                stateTree.Append(new((int)entry.State), path);
                occluderTree.Append(new(entry.State == SunState.Blocked
                    ? entry.ObjectId.ToString(CultureInfo.InvariantCulture)
                    : string.Empty), path);
            }
        }
        ulong visible = result.Summaries.Aggregate(0UL, (total, value) => checked(total + value.VisibleCount));
        ulong blocked = result.Summaries.Aggregate(0UL, (total, value) => checked(total + value.BlockedCount));
        ulong backFacing = result.Summaries.Aggregate(0UL, (total, value) => checked(total + value.BackFacingCount));
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: MEHR annual irradiance over ZURVAN SPA, ASMAN sky, and ZAMYAD rays{Environment.NewLine}" +
            $"Weather: {result.Weather.SourcePath ?? "in-memory EPW"}; {result.Weather.WeatherCount:N0} intervals; {result.Weather.RecordsPerHour} record(s)/hour{Environment.NewLine}" +
            $"EPW location: {result.Weather.Location.LatitudeDegrees:G10}, {result.Weather.Location.LongitudeDegrees:G10}; UTC{result.Weather.TimeZoneHours:+0.##;-0.##;0}; elevation {result.Weather.Location.ElevationMeters:G8} m{Environment.NewLine}" +
            $"Sanitized radiation: GHI {result.Weather.MissingGlobalHorizontalCount:N0}; DNI {result.Weather.MissingDirectNormalCount:N0}; DHI {result.Weather.MissingDiffuseHorizontalCount:N0}{Environment.NewLine}" +
            $"Model: Perez 1990 anisotropic sky; 144 dome + 24 horizon obstruction rays/sensor; closest-hit direct/circumsolar attribution; isotropic ground reflection{Environment.NewLine}" +
            $"Sensors: {result.Sensors.Count:N0}; timeline entries: {result.Timeline.Count:N0}; states {visible:N0} visible, {blocked:N0} blocked, {backFacing:N0} back-facing{Environment.NewLine}" +
            $"Policy: albedo fallback {result.Options.GroundAlbedo:G6}; EPW albedo {result.Options.UseWeatherAlbedo}; north {result.Options.Solar.NorthRotationDegrees:G6}°; category {result.Options.CategoryMask}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; weather hash {result.Weather.ContentHash}; result hash {result.ContentHash}");

        dataAccess.SetData(0, result);
        dataAccess.SetDataList(1, result.Summaries.Select(value => value.GlobalWhM2 / 1000.0));
        dataAccess.SetDataList(2, result.Summaries.Select(value => value.DirectWhM2 / 1000.0));
        dataAccess.SetDataList(3, result.Summaries.Select(value => value.DiffuseSkyWhM2 / 1000.0));
        dataAccess.SetDataList(4, result.Summaries.Select(value => value.GroundReflectedWhM2 / 1000.0));
        dataAccess.SetDataList(5, result.Summaries.Select(value => value.PeakGlobalWM2));
        dataAccess.SetDataList(6, result.Summaries.Select(value => value.PeakTimestampUtc.UtcDateTime));
        dataAccess.SetDataList(7, result.Summaries.Select(value => value.DomeVisibilityRatio));
        dataAccess.SetDataList(8, result.Summaries.Select(value => value.HorizonVisibilityRatio));
        dataAccess.SetDataList(9, result.Summaries.Select(value => value.DominantSolarOccluderObjectId == 0
            ? string.Empty
            : value.DominantSolarOccluderObjectId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(10, result.Summaries.Select(value => value.DominantSolarOccluderLossWhM2 / 1000.0));
        dataAccess.SetDataList(11, result.Summaries.Select(value => value.DominantSkyOccluderObjectId == 0
            ? string.Empty
            : value.DominantSkyOccluderObjectId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(12, colors);
        dataAccess.SetDataTree(13, globalTree);
        dataAccess.SetDataTree(14, directTree);
        dataAccess.SetDataTree(15, diffuseTree);
        dataAccess.SetDataTree(16, stateTree);
        dataAccess.SetDataTree(17, occluderTree);
        dataAccess.SetData(18, result.Weather.ContentHash);
        dataAccess.SetData(19, result.ContentHash);
        dataAccess.SetData(20, report);
        Message = string.Create(
            CultureInfo.InvariantCulture,
            $"MEHR\n{result.Sensors.Count:N0} × {result.Weather.WeatherCount:N0}");
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

    private readonly record struct AnnualIrradianceWork(
        XvarnaScene Scene,
        SolarSensor[] Sensors,
        EpwWeatherFile Weather,
        AnnualIrradianceOptions Options);
}
