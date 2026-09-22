using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper.Components.Visibility;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Compares material-aware solar alternatives over one scene and schedule.</summary>
public sealed class SolarScenariosComponent : ScheduledAnalysisComponent<SolarScenariosWork, SolarScenarioComparison>
{
    /// <summary>Initializes the solar scenario component.</summary>
    public SolarScenariosComponent() : base(
        "XVARNA Solar Scenarios", "XV Solar Scenarios",
        "Compares optical material alternatives with full transmission timeline, top-K/category loss attribution, and baseline deltas.",
        "XVARNA", "03 Solar")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("fae40110-d73a-4949-bd4a-ac2256dde02b");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override string AnalysisName => "HVARE Solar Scenarios";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene.", GH_ParamAccess.item);
        input.AddGenericParameter("Sun Set", "Sun", "XV Sun Vectors result.", GH_ParamAccess.item);
        input.AddPointParameter("Sensors", "P", "Protected oriented sensor points.", GH_ParamAccess.list);
        input.AddVectorParameter("Normals", "N", "One normal or one per sensor.", GH_ParamAccess.list);
        input.AddGenericParameter("Material Scenarios", "M", "Two or more XV Materials catalogs; first is baseline.", GH_ParamAccess.list);
        input.AddTextParameter("Scenario Names", "Name", "One name per material catalog.", GH_ParamAccess.list);
        input.AddTextParameter("Scenario IDs", "ID", "Optional unsigned IDs; empty uses 1..N.", GH_ParamAccess.list);
        input.AddIntegerParameter("Top K", "K", "Ranked loss sources per scenario/sensor.", GH_ParamAccess.item, 10);
        input.AddIntegerParameter("Material Layers", "L", "Transparent geometric layer cap.", GH_ParamAccess.item, 8);
        input.AddNumberParameter("Minimum Transmission", "Tmin", "Throughput cutoff.", GH_ParamAccess.item, 0.0001);
        input.AddTextParameter("Category Mask", "C", "-1 for all or unsigned mask.", GH_ParamAccess.item, "-1");
        input.AddNumberParameter("Offset", "Eps", "0 uses Rhino document tolerance.", GH_ParamAccess.item, 0.0);
        input[6].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Comparison", "R", "Complete material-aware solar comparison.", GH_ParamAccess.item);
        output.AddNumberParameter("Received Hours", "Sun", "Scenario branches, sensor values.", GH_ParamAccess.tree);
        output.AddNumberParameter("Lost Hours", "Loss", "Scenario branches, sensor values.", GH_ParamAccess.tree);
        output.AddNumberParameter("Solar Access", "SA", "Scenario branches, sensor values.", GH_ParamAccess.tree);
        output.AddNumberParameter("Δ Received", "ΔSun", "Non-baseline scenario branches, sensor deltas.", GH_ParamAccess.tree);
        output.AddTextParameter("Top Objects", "OID", "Scenario/sensor paths of ranked loss objects.", GH_ParamAccess.tree);
        output.AddNumberParameter("Top Lost Hours", "OH", "Scenario/sensor paths of attributed loss.", GH_ParamAccess.tree);
        output.AddTextParameter("Hash", "H", "BLAKE3 result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Dimensions, material traversal, attribution, delta, and runtime report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out SolarScenariosWork work)
    {
        work = default; object? sceneValue = null, sunValue = null; List<object> materialValues = [];
        List<Point3d> points = []; List<Vector3d> normals = []; List<string> names = [], ids = [];
        int topK = 10, layers = 8; double minimumTransmission = 0.0001, offset = 0; string mask = "-1";
        if (!data.GetData(0, ref sceneValue) || !data.GetData(1, ref sunValue)
            || !data.GetDataList(2, points) || points.Count == 0 || !data.GetDataList(3, normals)
            || !data.GetDataList(4, materialValues) || materialValues.Count == 0
            || !data.GetDataList(5, names)) return false;
        data.GetDataList(6, ids); data.GetData(7, ref topK); data.GetData(8, ref layers);
        data.GetData(9, ref minimumTransmission); data.GetData(10, ref mask); data.GetData(11, ref offset);
        try
        {
            if (normals.Count is not 1 && normals.Count != points.Count) throw new ArgumentException("Normals require one value or one per sensor.");
            if (names.Count != materialValues.Count || (ids.Count != 0 && ids.Count != materialValues.Count))
                throw new ArgumentException("Scenario names and optional IDs must align with Material Scenarios.");
            XvarnaScene scene = IntelligenceHelpers.Unwrap<XvarnaScene>(sceneValue) ?? throw new ArgumentException("Scene must come from XV Scene.");
            XvarnaSunSet sun = IntelligenceHelpers.Unwrap<XvarnaSunSet>(sunValue) ?? throw new ArgumentException("Sun Set must come from XV Sun Vectors.");
            SolarSensor[] sensors = points.Select((point, index) =>
            {
                Vector3d normal = normals[normals.Count == 1 ? 0 : index];
                return new SolarSensor(checked((ulong)index + 1), point.X, point.Y, point.Z, normal.X, normal.Y, normal.Z);
            }).ToArray();
            SolarMaterialScenario[] scenarios = materialValues.Select((value, index) => new SolarMaterialScenario
            {
                ScenarioId = ids.Count == 0 ? checked((ulong)index + 1) : IntelligenceHelpers.Parse(ids[index], "Scenario ID"),
                Name = names[index],
                Materials = IntelligenceHelpers.Unwrap<AnalysisMaterialLibrary>(value)
                    ?? throw new ArgumentException($"Material Scenario {index} must come from XV Materials."),
            }).ToArray();
            double resolvedOffset = offset > 0 ? offset : RhinoDoc.ActiveDoc?.ModelAbsoluteTolerance ?? 1.0e-6;
            work = new(scene, sun, sensors, scenarios,
                new SolarScenarioOptions(resolvedOffset, double.PositiveInfinity, 0.0,
                    IntelligenceHelpers.Parse(mask, "Category mask", true), checked((uint)layers),
                    minimumTransmission, checked((uint)topK)));
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(SolarScenariosWork work) => checked(
        (ulong)work.Scenarios.Length * (ulong)work.Sensors.Length * (ulong)work.SunSet.Samples.Count);

    /// <inheritdoc />
    protected override SolarScenarioComparison Execute(SolarScenariosWork work, CancellationToken token,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Optical solar traversal"));
        token.ThrowIfCancellationRequested();
        SolarScenarioComparison result = work.Scene.CompareSolarScenarios(work.Sensors, work.SunSet, work.Scenarios, work.Options);
        progress.Report((total, total, "Scenario deltas and attribution")); return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, SolarScenariosWork work, SolarScenarioComparison result)
    {
        DataTree<double> received = new(), lost = new(), access = new(), deltas = new(), topHours = new();
        DataTree<string> topObjects = new();
        for (int scenario = 0; scenario < work.Scenarios.Length; scenario++)
        {
            GH_Path path = new(scenario); IReadOnlyList<SolarScenarioSummary> summaries = result.Summaries
                .Skip(scenario * work.Sensors.Length).Take(work.Sensors.Length).ToArray();
            received.AddRange(summaries.Select(value => value.ReceivedSunHours), path);
            lost.AddRange(summaries.Select(value => value.LostSunHours), path);
            access.AddRange(summaries.Select(value => value.SolarAccessRatio), path);
            if (scenario > 0) deltas.AddRange(result.Deltas.Where(value => value.ScenarioId == work.Scenarios[scenario].ScenarioId)
                .Select(value => value.ReceivedSunHoursDelta), path);
            for (int sensor = 0; sensor < work.Sensors.Length; sensor++)
            {
                GH_Path rankedPath = new(scenario, sensor);
                IEnumerable<SolarLossAttribution> ranked = result.Attribution.Where(value =>
                    value.ScenarioId == work.Scenarios[scenario].ScenarioId && value.SensorId == work.Sensors[sensor].SensorId);
                topObjects.AddRange(ranked.Select(value => value.ObjectId.ToString(CultureInfo.InvariantCulture)), rankedPath);
                topHours.AddRange(ranked.Select(value => value.LostSunHours), rankedPath);
            }
        }
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: HVARE material-aware solar scenario comparison{Environment.NewLine}" +
            $"Study: {work.Scenarios.Length:N0} scenarios × {work.Sensors.Length:N0} sensors × {work.SunSet.Samples.Count:N0} sun intervals{Environment.NewLine}" +
            $"Rows: {result.Attribution.Count:N0} top-object; {result.Categories.Count:N0} exact-category; {result.Deltas.Count:N0} deltas{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataTree(1, received); data.SetDataTree(2, lost);
        data.SetDataTree(3, access); data.SetDataTree(4, deltas); data.SetDataTree(5, topObjects);
        data.SetDataTree(6, topHours); data.SetData(7, result.ContentHash); data.SetData(8, report);
        Message = $"Solar Scenarios\n{work.Scenarios.Length:N0} × {work.Sensors.Length:N0}";
    }
}

/// <summary>Immutable captured solar-scenario work.</summary>
public readonly record struct SolarScenariosWork(
    XvarnaScene Scene, XvarnaSunSet SunSet, SolarSensor[] Sensors,
    SolarMaterialScenario[] Scenarios, SolarScenarioOptions Options);
