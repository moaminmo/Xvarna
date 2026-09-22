using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

#pragma warning disable CS1591 // Component input/output documentation is registered in Grasshopper below.

namespace Xvarna.Grasshopper.Components.Daylight;

/// <summary>Creates the shared Fast Path/Radiance RGB optical material catalog.</summary>
public sealed class OpticalMaterialsComponent : GH_Component
{
    public OpticalMaterialsComponent() : base(
        "XVARNA Optical Materials", "XV Optical",
        "Builds energy-conserving Radiance-compatible RGB plastic, glass, metal, trans, and mirror materials with exact Object-ID assignments.",
        "XVARNA", "03 Daylight")
    { }

    public override Guid ComponentGuid => new("5fd59960-81a7-4cbe-93f4-1176e7252139");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Material IDs", "MID", "Unique non-zero material IDs.", GH_ParamAccess.list);
        input.AddTextParameter("Kinds", "K", "plastic, glass, metal, trans, or mirror per material.", GH_ParamAccess.list);
        input.AddColourParameter("Reflectance RGB", "R", "Linear-RGB reflectance; colour channels are mapped from 0..255 to 0..1.", GH_ParamAccess.list);
        input.AddColourParameter("Transmittance RGB", "T", "Linear-RGB visible transmittance.", GH_ParamAccess.list);
        input.AddNumberParameter("Specularity", "S", "Specular fraction per material in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Roughness", "Ro", "Roughness per material in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Refractive Index", "IOR", "Index of refraction per material, normally 1.52 for glass.", GH_ParamAccess.list);
        input.AddTextParameter("Object IDs", "OID", "Exact source Object IDs to assign.", GH_ParamAccess.list);
        input.AddTextParameter("Assigned Material IDs", "A", "Material ID for every Object ID.", GH_ParamAccess.list);
        for (int index = 0; index < 9; index++) input[index].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Optical Materials", "M", "Validated immutable catalog for XV Daylight, Annual Daylight, and Radiance.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Material families, assignment count, and energy-conservation policy.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<string> ids = [], kinds = [], objects = [], assigned = [];
        List<Color> reflectance = [], transmittance = [];
        List<double> specularity = [], roughness = [], ior = [];
        data.GetDataList(0, ids); data.GetDataList(1, kinds); data.GetDataList(2, reflectance);
        data.GetDataList(3, transmittance); data.GetDataList(4, specularity);
        data.GetDataList(5, roughness); data.GetDataList(6, ior);
        data.GetDataList(7, objects); data.GetDataList(8, assigned);
        try
        {
            int count = ids.Count;
            if (new[] { kinds.Count, reflectance.Count, transmittance.Count, specularity.Count, roughness.Count, ior.Count }.Any(value => value != count))
                throw new ArgumentException("All seven material-definition lists must have equal lengths.");
            if (objects.Count != assigned.Count)
                throw new ArgumentException("Object IDs and assigned Material IDs must have equal lengths.");
            OpticalMaterial[] materials = ids.Select((value, index) =>
            {
                Color r = reflectance[index], t = transmittance[index];
                return new OpticalMaterial(ParseId(value, "Material ID"), ParseKind(kinds[index]),
                    r.R / 255.0, r.G / 255.0, r.B / 255.0,
                    t.R / 255.0, t.G / 255.0, t.B / 255.0,
                    specularity[index], roughness[index], ior[index]);
            }).ToArray();
            MaterialAssignment[] assignments = objects.Select((value, index) => new MaterialAssignment(
                ParseId(value, "Object ID"), ParseId(assigned[index], "Assigned Material ID"))).ToArray();
            OpticalMaterialLibrary library = new(materials, assignments);
            data.SetData(0, library);
            data.SetData(1, $"HVARE/Radiance optical catalog: {materials.Length:N0} RGB definitions; {assignments.Length:N0} exact Object-ID assignments.{Environment.NewLine}" +
                "Energy policy: channel-wise reflectance + transmittance ≤ 1; unassigned geometry is opaque; RGB is converted photopically only in Fast Path and remains RGB in Radiance.");
            Message = $"Optical\n{materials.Length:N0} · {assignments.Length:N0}";
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static OpticalMaterialKind ParseKind(string value) => value.Trim().ToLowerInvariant() switch
    {
        "plastic" => OpticalMaterialKind.Plastic,
        "glass" => OpticalMaterialKind.Glass,
        "metal" => OpticalMaterialKind.Metal,
        "trans" => OpticalMaterialKind.Trans,
        "mirror" => OpticalMaterialKind.Mirror,
        _ => throw new ArgumentException($"Unknown optical material kind '{value}'."),
    };

    internal static ulong ParseId(string value, string name) => ulong.TryParse(
        value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed)
        ? parsed
        : throw new ArgumentException($"{name} '{value}' is not an unsigned 64-bit integer.");
}

public readonly record struct PointDaylightWork(
    XvarnaScene Scene, DaylightSensor[] Sensors, OpticalMaterialLibrary Materials,
    DaylightMoment Moment, DaylightMatrixOptions Options, double ExteriorLux);

public sealed record PointDaylightBundle(PointIlluminanceResult Point, DaylightFactorResult Factor);

/// <summary>Computes point-in-time illuminance and CIE-overcast daylight factor.</summary>
public sealed class PointDaylightComponent : ScheduledAnalysisComponent<PointDaylightWork, PointDaylightBundle>
{
    public PointDaylightComponent() : base(
        "XVARNA Daylight", "XV Daylight",
        "Runs material-aware direct/diffuse point illuminance plus CIE-overcast Daylight Factor using a reusable coefficient matrix.",
        "XVARNA", "03 Daylight")
    { }

    public override Guid ComponentGuid => new("b75b855d-232e-4820-832a-176f58cbce7a");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    protected override string AnalysisName => "HVARE Daylight";

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene snapshot.", GH_ParamAccess.item);
        input.AddGenericParameter("Optical Materials", "M", "XV Optical Materials; empty means implicit opaque geometry.", GH_ParamAccess.item);
        input.AddPointParameter("Sensors", "P", "Workplane points in Rhino model units.", GH_ParamAccess.list);
        input.AddVectorParameter("Normals", "N", "One normal or one per sensor.", GH_ParamAccess.list);
        input.AddNumberParameter("Areas", "A", "One represented area or one per sensor, in squared model units.", GH_ParamAccess.list);
        input.AddTimeParameter("Timestamp UTC", "T", "UTC timestamp retained in provenance.", GH_ParamAccess.item, new DateTime(2026, 6, 21, 8, 30, 0, DateTimeKind.Utc));
        input.AddVectorParameter("Sun Direction", "Sun", "Unit vector from sensors toward sun.", GH_ParamAccess.item, Vector3d.ZAxis);
        input.AddNumberParameter("Direct Normal Illuminance", "DNI", "Direct-normal illuminance in lux.", GH_ParamAccess.item, 80000.0);
        input.AddNumberParameter("Diffuse Horizontal Illuminance", "DHI", "Diffuse-horizontal illuminance in lux.", GH_ParamAccess.item, 10000.0);
        input.AddIntegerParameter("Sky Patches", "Q", "Equal-solid-angle upper-hemisphere coefficient directions.", GH_ParamAccess.item, 576);
        input.AddNumberParameter("Exterior DF Illuminance", "DFext", "CIE-overcast exterior horizontal illuminance in lux.", GH_ParamAccess.item, 10000.0);
        input.AddBooleanParameter("Run", "Run", "Run or recompute.", GH_ParamAccess.item, true);
        input[1].Optional = true; input[4].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Point Result", "R", "Complete point-in-time result.", GH_ParamAccess.item);
        output.AddNumberParameter("Total Lux", "E", "Direct plus diffuse illuminance per sensor.", GH_ParamAccess.list);
        output.AddNumberParameter("Direct Lux", "Ed", "Received direct illuminance.", GH_ParamAccess.list);
        output.AddNumberParameter("Diffuse Lux", "Es", "Received diffuse-sky illuminance.", GH_ParamAccess.list);
        output.AddNumberParameter("Daylight Factor", "DF", "CIE-overcast daylight factor percent.", GH_ParamAccess.list);
        output.AddBooleanParameter("Matrix Reused", "Hit", "True when the exact process-memory coefficient matrix was reused.", GH_ParamAccess.item);
        output.AddTextParameter("Matrix Hash", "MH", "Reusable coefficient identity.", GH_ParamAccess.item);
        output.AddTextParameter("Result Hash", "H", "Deterministic result provenance.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Photometric model, material boundary, cache state, timing, and scientific scope.", GH_ParamAccess.item);
    }

    protected override bool TryReadWork(IGH_DataAccess data, out PointDaylightWork work)
    {
        work = default;
        object? sceneValue = null, materialValue = null;
        List<Point3d> points = []; List<Vector3d> normals = []; List<double> areas = [];
        DateTime timestamp = default; Vector3d sun = Vector3d.ZAxis;
        double dni = 80000, dhi = 10000, exterior = 10000; int patches = 576; bool run = true;
        if (!data.GetData(0, ref sceneValue) || !data.GetDataList(2, points) || points.Count == 0
            || !data.GetDataList(3, normals) || normals.Count == 0) return false;
        data.GetData(1, ref materialValue); data.GetDataList(4, areas); data.GetData(5, ref timestamp);
        data.GetData(6, ref sun); data.GetData(7, ref dni); data.GetData(8, ref dhi);
        data.GetData(9, ref patches); data.GetData(10, ref exterior); data.GetData(11, ref run);
        if (!run) { Message = "Daylight\nPaused"; return false; }
        try
        {
            patches = ProductContextRegistry.ScaleSamples(OnPingDocument(), patches, 64, 65_536);
            XvarnaScene scene = DaylightGh.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            OpticalMaterialLibrary materials = DaylightGh.Unwrap<OpticalMaterialLibrary>(materialValue) ?? OpticalMaterialLibrary.Opaque;
            DaylightSensor[] sensors = DaylightGh.Sensors(points, normals, areas);
            double tolerance = ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            work = new(scene, sensors, materials,
                new(new DateTimeOffset(DateTime.SpecifyKind(timestamp, DateTimeKind.Utc)), sun.X, sun.Y, sun.Z, dni, dhi),
                new((uint)patches, tolerance), exterior);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    protected override ulong EstimateWorkUnits(PointDaylightWork work) =>
        checked((ulong)work.Sensors.Length * (work.Options.SkyPatchCount + 2));

    protected override PointDaylightBundle Execute(PointDaylightWork work, CancellationToken token, IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        progress.Report((0, EstimateWorkUnits(work), "Daylight coefficients and direct rays"));
        PointIlluminanceResult point = work.Scene.AnalyzePointIlluminance(work.Sensors, work.Materials, work.Moment, work.Options, token);
        token.ThrowIfCancellationRequested();
        DaylightFactorResult factor = work.Scene.AnalyzeDaylightFactor(work.Sensors, work.Materials, work.ExteriorLux,
            work.Options with { SkyModel = DaylightSkyModel.CieOvercast });
        progress.Report((EstimateWorkUnits(work), EstimateWorkUnits(work), "Assembling daylight result"));
        return new(point, factor);
    }

    protected override void Emit(IGH_DataAccess data, PointDaylightWork work, PointDaylightBundle result)
    {
        data.SetData(0, result.Point);
        data.SetDataList(1, result.Point.Entries.Select(value => value.TotalLux));
        data.SetDataList(2, result.Point.Entries.Select(value => value.DirectLux));
        data.SetDataList(3, result.Point.Entries.Select(value => value.DiffuseLux));
        data.SetDataList(4, result.Factor.Entries.Select(value => value.DaylightFactorPercent));
        data.SetData(5, result.Point.MatrixReused); data.SetData(6, result.Point.MatrixHash);
        data.SetData(7, result.Point.ContentHash);
        data.SetData(8, $"HVARE deterministic Fast Path: {work.Options.SkyPatchCount:N0} equal-solid-angle patches; direct ray + diffuse coefficients; RGB materials reduced by photopic weighting.{Environment.NewLine}" +
            $"Matrix: {result.Point.MatrixHash}; reused {result.Point.MatrixReused}; point {result.Point.AnalysisTimeMicroseconds / 1000.0:N3} ms; DF {result.Factor.AnalysisTimeMicroseconds / 1000.0:N3} ms.{Environment.NewLine}" +
            "Scientific boundary: screening path has no multi-bounce interreflection; use XV Radiance Export/reference and XV Daylight Validate for publication-grade comparison.");
        Message = result.Point.MatrixReused ? "Daylight\nCache hit" : $"Daylight\n{work.Sensors.Length:N0} sensors";
    }
}

public readonly record struct AnnualDaylightWork(
    XvarnaScene Scene, DaylightSensor[] Sensors, EpwWeatherFile Weather,
    OpticalMaterialLibrary Materials, AnnualDaylightOptions Options);

/// <summary>Computes climate-based sDA, ASE, and UDI with reusable daylight coefficients.</summary>
public sealed class AnnualDaylightComponent : ScheduledAnalysisComponent<AnnualDaylightWork, AnnualDaylightResult>
{
    public AnnualDaylightComponent() : base(
        "XVARNA Annual Daylight", "XV Annual Daylight",
        "Computes sensor and area-weighted sDA, ASE, four-bin UDI, and occupied illuminance from EPW with cancellable matrix reuse.",
        "XVARNA", "03 Daylight")
    { }

    public override Guid ComponentGuid => new("64c77232-a554-4bd6-a03a-b28246203a30");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    protected override string AnalysisName => "HVARE Annual";

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene.", GH_ParamAccess.item);
        input.AddTextParameter("EPW Path", "EPW", "EnergyPlus Weather file.", GH_ParamAccess.item);
        input.AddGenericParameter("Optical Materials", "M", "XV Optical Materials; empty means opaque.", GH_ParamAccess.item);
        input.AddPointParameter("Sensors", "P", "Workplane points.", GH_ParamAccess.list);
        input.AddVectorParameter("Normals", "N", "One normal or one per sensor.", GH_ParamAccess.list);
        input.AddNumberParameter("Areas", "A", "One area or one per sensor in squared model units.", GH_ParamAccess.list);
        input.AddIntegerParameter("Sky Patches", "Q", "Static diffuse coefficient directions.", GH_ParamAccess.item, 576);
        input.AddNumberParameter("Occupied Start", "Start", "Local standard-time occupied start hour.", GH_ParamAccess.item, 8.0);
        input.AddNumberParameter("Occupied End", "End", "Local standard-time occupied end hour.", GH_ParamAccess.item, 18.0);
        input.AddNumberParameter("sDA Lux", "sDA", "sDA illuminance threshold.", GH_ParamAccess.item, 300.0);
        input.AddNumberParameter("ASE Lux", "ASE", "Direct-only ASE threshold.", GH_ParamAccess.item, 1000.0);
        input.AddNumberParameter("ASE Hours", "h", "Maximum permitted direct threshold exceedance hours.", GH_ParamAccess.item, 250.0);
        input.AddNumberParameter("North", "N°", "Model rotation from true north in degrees.", GH_ParamAccess.item, 0.0);
        input.AddBooleanParameter("Run", "Run", "Run or recompute.", GH_ParamAccess.item, true);
        input[2].Optional = true; input[5].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Result", "R", "Complete sensor-major annual result.", GH_ParamAccess.item);
        output.AddNumberParameter("sDA Area", "sDA", "Area meeting sDA, percent.", GH_ParamAccess.item);
        output.AddNumberParameter("ASE Area", "ASE", "Area failing ASE, percent.", GH_ParamAccess.item);
        output.AddNumberParameter("UDI Useful", "UDI", "Area-weighted preferred/useful UDI, percent.", GH_ParamAccess.item);
        output.AddNumberParameter("Sensor sDA", "sDAi", "Occupied sDA percentage per sensor.", GH_ParamAccess.list);
        output.AddNumberParameter("Sensor ASE Hours", "ASEh", "Direct exceedance hours per sensor.", GH_ParamAccess.list);
        output.AddBooleanParameter("ASE Fails", "Fail", "ASE failure per sensor.", GH_ParamAccess.list);
        output.AddNumberParameter("Mean Lux", "Eavg", "Occupied mean lux per sensor.", GH_ParamAccess.list);
        output.AddBooleanParameter("Matrix Reused", "Hit", "Exact coefficient cache hit.", GH_ParamAccess.item);
        output.AddTextParameter("Matrix Hash", "MH", "Coefficient identity.", GH_ParamAccess.item);
        output.AddTextParameter("Result Hash", "H", "Complete annual identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Thresholds, weather, matrix reuse, scientific boundary, and provenance.", GH_ParamAccess.item);
    }

    protected override bool TryReadWork(IGH_DataAccess data, out AnnualDaylightWork work)
    {
        work = default;
        object? sceneValue = null, materialValue = null; string epw = string.Empty;
        List<Point3d> points = []; List<Vector3d> normals = []; List<double> areas = [];
        int patches = 576; double start = 8, end = 18, sda = 300, ase = 1000, aseHours = 250, north = 0; bool run = true;
        if (!data.GetData(0, ref sceneValue) || !data.GetData(1, ref epw)
            || !data.GetDataList(3, points) || points.Count == 0
            || !data.GetDataList(4, normals) || normals.Count == 0) return false;
        data.GetData(2, ref materialValue); data.GetDataList(5, areas); data.GetData(6, ref patches);
        data.GetData(7, ref start); data.GetData(8, ref end); data.GetData(9, ref sda);
        data.GetData(10, ref ase); data.GetData(11, ref aseHours); data.GetData(12, ref north); data.GetData(13, ref run);
        if (!run) { Message = "Annual\nPaused"; return false; }
        try
        {
            patches = ProductContextRegistry.ScaleSamples(OnPingDocument(), patches, 64, 65_536);
            XvarnaScene scene = DaylightGh.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            OpticalMaterialLibrary materials = DaylightGh.Unwrap<OpticalMaterialLibrary>(materialValue) ?? OpticalMaterialLibrary.Opaque;
            DaylightSensor[] sensors = DaylightGh.Sensors(points, normals, areas);
            double tolerance = ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            AnnualDaylightOptions options = AnnualDaylightOptions.Default with
            {
                Matrix = new((uint)patches, tolerance),
                OccupiedStartHour = start,
                OccupiedEndHour = end,
                SdaThresholdLux = sda,
                AseThresholdLux = ase,
                AseMaximumHours = aseHours,
                Solar = SolarPositionOptions.Default with { NorthRotationDegrees = north },
            };
            work = new(scene, sensors, EpwWeatherFile.Load(epw), materials, options);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or UnauthorizedAccessException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    protected override ulong EstimateWorkUnits(AnnualDaylightWork work) =>
        checked((ulong)work.Sensors.Length * (work.Weather.WeatherCount + work.Options.Matrix.SkyPatchCount));

    protected override AnnualDaylightResult Execute(AnnualDaylightWork work, CancellationToken token, IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Coefficient matrix and EPW timeline"));
        AnnualDaylightResult result = work.Scene.AnalyzeAnnualDaylight(work.Sensors, work.Weather, work.Materials, work.Options, token);
        progress.Report((total, total, "sDA / ASE / UDI aggregation")); return result;
    }

    protected override void Emit(IGH_DataAccess data, AnnualDaylightWork work, AnnualDaylightResult result)
    {
        data.SetData(0, result); data.SetData(1, result.Project.SdaAreaPercent); data.SetData(2, result.Project.AseAreaPercent);
        data.SetData(3, result.Project.UdiUsefulPercent);
        data.SetDataList(4, result.Summaries.Select(value => value.SdaOccupiedFraction * 100.0));
        data.SetDataList(5, result.Summaries.Select(value => value.AseExceedanceHours));
        data.SetDataList(6, result.Summaries.Select(value => value.AseFails));
        data.SetDataList(7, result.Summaries.Select(value => value.MeanOccupiedLux));
        data.SetData(8, result.MatrixReused); data.SetData(9, result.MatrixHash); data.SetData(10, result.ContentHash);
        data.SetData(11, $"HVARE annual daylight: {work.Sensors.Length:N0} sensors × {result.WeatherCount:N0} EPW intervals; {result.Timeline.Count:N0} cells.{Environment.NewLine}" +
            $"sDA {work.Options.SdaThresholdLux:G6} lux / {work.Options.SdaRequiredFraction:P0}; ASE {work.Options.AseThresholdLux:G6} lux / {work.Options.AseMaximumHours:G6} h; UDI {work.Options.UdiLowerLux:G6}/{work.Options.UdiPreferredLux:G6}/{work.Options.UdiUpperLux:G6} lux.{Environment.NewLine}" +
            $"Matrix {result.MatrixHash}; reused {result.MatrixReused}; analysis {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; result {result.ContentHash}.{Environment.NewLine}" +
            "Fast Path is a reproducible screening model, not a substitute for validated multi-bounce Radiance/LM-83 evidence.");
        Message = result.MatrixReused ? $"Annual\nCache hit · sDA {result.Project.SdaAreaPercent:N1}%" : $"Annual\nsDA {result.Project.SdaAreaPercent:N1}%";
    }
}

/// <summary>Exports exact world geometry, RGB materials, sensors, and a manifest to Radiance.</summary>
public sealed class RadianceExportComponent : GH_Component
{
    public RadianceExportComponent() : base(
        "XVARNA Radiance Export", "XV Radiance",
        "Exports materials.rad, geometry.rad, sensors.pts, and manifest.json with exact world-space Object-ID mapping.",
        "XVARNA", "03 Daylight")
    { }

    public override Guid ComponentGuid => new("582cdf0c-57ce-4dda-a614-c97084fdc1f1");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene.", GH_ParamAccess.item);
        input.AddGenericParameter("Optical Materials", "M", "XV Optical Materials.", GH_ParamAccess.item);
        input.AddPointParameter("Sensors", "P", "Radiance sensor points.", GH_ParamAccess.list);
        input.AddVectorParameter("Normals", "N", "One normal or one per sensor.", GH_ParamAccess.list);
        input.AddTextParameter("Directory", "D", "Output directory for the deterministic bundle.", GH_ParamAccess.item);
        input.AddBooleanParameter("Write", "W", "Write or overwrite the four named bundle files.", GH_ParamAccess.item, false);
        input[1].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Bundle", "B", "In-memory Radiance export bundle.", GH_ParamAccess.item);
        output.AddTextParameter("Files", "F", "Written file paths.", GH_ParamAccess.list);
        output.AddTextParameter("Export Hash", "H", "Stable export identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Mapping, units, and next-step reference command guidance.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        object? sceneValue = null, materialValue = null; List<Point3d> points = []; List<Vector3d> normals = [];
        string directory = string.Empty; bool write = false;
        if (!data.GetData(0, ref sceneValue) || !data.GetDataList(2, points) || !data.GetDataList(3, normals)) return;
        data.GetData(1, ref materialValue); data.GetData(4, ref directory); data.GetData(5, ref write);
        try
        {
            XvarnaScene scene = DaylightGh.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            OpticalMaterialLibrary materials = DaylightGh.Unwrap<OpticalMaterialLibrary>(materialValue) ?? OpticalMaterialLibrary.Opaque;
            RadianceExportBundle bundle = scene.ExportRadiance(DaylightGh.Sensors(points, normals, []), materials);
            string[] files = [];
            if (write)
            {
                if (string.IsNullOrWhiteSpace(directory)) throw new ArgumentException("Directory is required when Write is true.");
                directory = Path.GetFullPath(directory); bundle.Write(directory);
                files = new[] { "materials.rad", "geometry.rad", "sensors.pts", "manifest.json" }.Select(name => Path.Combine(directory, name)).ToArray();
            }
            data.SetData(0, bundle); data.SetDataList(1, files); data.SetData(2, bundle.ContentHash);
            data.SetData(3, $"Radiance-compatible canonical metre export; exact scene Object ID → RGB primitive mapping; {points.Count:N0} sensors.{Environment.NewLine}" +
                $"Export hash: {bundle.ContentHash}.{Environment.NewLine}" +
                "Use the packaged xvarna radiance-point/radiance-annual runners for cancellable oconv/rtrace or rfluxmtx/gendaymtx/dctimestep execution with version and command provenance.");
            Message = write ? "Radiance\nWritten" : "Radiance\nPreview";
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or UnauthorizedAccessException or XvarnaNativeException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

/// <summary>Produces a scientific Fast Path versus Radiance agreement report.</summary>
public sealed class DaylightValidationComponent : GH_Component
{
    public DaylightValidationComponent() : base(
        "XVARNA Daylight Validate", "XV Daylight Δ",
        "Compares aligned Fast Path and Radiance lux values with bias, MAE, RMSE, MAPE, maximum error, R², and an explicit tolerance envelope.",
        "XVARNA", "05 Study")
    { }

    public override Guid ComponentGuid => new("65e91cde-2398-4591-9260-9be17c49dd97");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddNumberParameter("Fast Path Lux", "F", "Aligned HVARE Fast Path values.", GH_ParamAccess.list);
        input.AddNumberParameter("Radiance Lux", "R", "Aligned Radiance reference values.", GH_ParamAccess.list);
        input.AddNumberParameter("Absolute Tolerance", "Abs", "Absolute acceptance tolerance in lux.", GH_ParamAccess.item, 50.0);
        input.AddNumberParameter("Relative Tolerance", "Rel", "Relative acceptance tolerance in [0,1].", GH_ParamAccess.item, 0.1);
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Validation", "V", "Immutable scientific comparison record.", GH_ParamAccess.item);
        output.AddBooleanParameter("Accepted", "OK", "True when every pair is inside max(abs, relative) tolerance.", GH_ParamAccess.item);
        output.AddNumberParameter("Bias Lux", "Bias", "Mean signed Fast minus Radiance bias.", GH_ParamAccess.item);
        output.AddNumberParameter("MAE Lux", "MAE", "Mean absolute error.", GH_ParamAccess.item);
        output.AddNumberParameter("RMSE Lux", "RMSE", "Root mean square error.", GH_ParamAccess.item);
        output.AddNumberParameter("MAPE", "MAPE", "Mean absolute percentage error, percent.", GH_ParamAccess.item);
        output.AddNumberParameter("Max Error Lux", "Max", "Maximum absolute error.", GH_ParamAccess.item);
        output.AddNumberParameter("R Squared", "R²", "Coefficient of determination.", GH_ParamAccess.item);
        output.AddNumberParameter("Accepted Fraction", "AF", "Fraction inside the tolerance envelope.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Publication-ready numerical summary and interpretation boundary.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<double> fast = [], reference = []; double absolute = 50, relative = 0.1;
        if (!data.GetDataList(0, fast) || !data.GetDataList(1, reference)) return;
        data.GetData(2, ref absolute); data.GetData(3, ref relative);
        try
        {
            DaylightValidationReport report = DaylightValidation.Compare(fast, reference, absolute, relative);
            data.SetData(0, report); data.SetData(1, report.Accepted); data.SetData(2, report.MeanBiasLux);
            data.SetData(3, report.MeanAbsoluteErrorLux); data.SetData(4, report.RootMeanSquareErrorLux);
            data.SetData(5, report.MeanAbsolutePercentageError * 100.0); data.SetData(6, report.MaximumAbsoluteErrorLux);
            data.SetData(7, report.RSquared); data.SetData(8, report.AcceptedFraction);
            data.SetData(9, $"Fast Path vs Radiance: n={report.Count:N0}; bias {report.MeanBiasLux:N3} lux; MAE {report.MeanAbsoluteErrorLux:N3}; RMSE {report.RootMeanSquareErrorLux:N3}; MAPE {report.MeanAbsolutePercentageError:P3}; max {report.MaximumAbsoluteErrorLux:N3}; R² {report.RSquared:N6}.{Environment.NewLine}" +
                $"Tolerance: max({absolute:G6} lux, {relative:P2} of reference); accepted {report.AcceptedFraction:P3}; all accepted {report.Accepted}; hash {report.ContentHash}.{Environment.NewLine}" +
                "Agreement statistics establish numerical evidence for this dataset; they do not by themselves establish universal model validity.");
            Message = report.Accepted ? $"Validation\nPASS · {report.AcceptedFraction:P0}" : $"Validation\nFAIL · {report.AcceptedFraction:P0}";
        }
        catch (Exception exception) when (exception is ArgumentException or XvarnaNativeException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

internal static class DaylightGh
{
    internal static T? Unwrap<T>(object? value) where T : class => value switch
    {
        T direct => direct,
        GH_ObjectWrapper { Value: T wrapped } => wrapped,
        _ => null,
    };

    internal static DaylightSensor[] Sensors(IReadOnlyList<Point3d> points, IReadOnlyList<Vector3d> normals, IReadOnlyList<double> areas)
    {
        if (points.Count == 0) throw new ArgumentException("At least one sensor is required.");
        if (normals.Count != 1 && normals.Count != points.Count) throw new ArgumentException("Normals need one value or one per sensor.");
        if (areas.Count is not 0 and not 1 && areas.Count != points.Count) throw new ArgumentException("Areas need zero, one, or one per sensor.");
        return points.Select((point, index) =>
        {
            Vector3d normal = normals.Count == 1 ? normals[0] : normals[index];
            double area = areas.Count == 0 ? 1.0 : areas.Count == 1 ? areas[0] : areas[index];
            return new DaylightSensor(checked((ulong)index + 1), point.X, point.Y, point.Z,
                normal.X, normal.Y, normal.Z, area);
        }).ToArray();
    }
}

#pragma warning restore CS1591
