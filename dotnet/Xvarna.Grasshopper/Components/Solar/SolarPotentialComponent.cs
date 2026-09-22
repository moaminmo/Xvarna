using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Maps annual MEHR results to geometry and extracts area-aware PV opportunity regions.</summary>
public sealed class SolarPotentialComponent : ScheduledAnalysisComponent<SolarPotentialWork, PvPotentialResult>
{
    private Mesh? previewMesh;

    /// <summary>Initializes the solar-surface intelligence component.</summary>
    public SolarPotentialComponent()
        : base(
            "XVARNA Solar Potential",
            "XV Solar Potential",
            "Maps MEHR annual irradiance to a false-colour surface, calculates honest area-weighted statistics, extracts connected eligible regions, and reports a transparent non-bankable PV proxy.",
            "XVARNA",
            "03 Solar")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("fe2918f0-917a-4aea-897c-cf9a7370526c");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "MEHR Solar Potential";

    /// <inheritdoc />
    public override BoundingBox ClippingBox => previewMesh is null
        ? base.ClippingBox
        : BoundingBox.Union(base.ClippingBox, previewMesh.GetBoundingBox(false));

    /// <inheritdoc />
    public override void DrawViewportMeshes(IGH_PreviewArgs args)
    {
        base.DrawViewportMeshes(args);
        if (previewMesh is not null)
        {
            args.Display.DrawMeshFalseColors(previewMesh);
        }
    }

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Sensor Grid", "Grid", "Area-aware result produced by XV Sensor Grid.", GH_ParamAccess.item);
        inputManager.AddGenericParameter("Irradiance Result", "R", "Complete annual result produced by XV Irradiance.", GH_ParamAccess.item);
        inputManager.AddNumberParameter("Minimum Irradiance", "Min", "Annual eligibility threshold in kWh/m².", GH_ParamAccess.item, 800.0);
        inputManager.AddNumberParameter("Module Efficiency", "η", "Nameplate module efficiency as percent.", GH_ParamAccess.item, 22.0);
        inputManager.AddNumberParameter("Coverage", "Cov", "Eligible surface area covered by modules as percent.", GH_ParamAccess.item, 85.0);
        inputManager.AddNumberParameter("System Losses", "Loss", "Individual downstream losses as percentages; combined multiplicatively. Empty uses one explicit 14% aggregate assumption.", GH_ParamAccess.list);
        inputManager[5].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Potential Result", "PR", "Complete immutable area-weighted distribution, region, and PV proxy result.", GH_ParamAccess.item);
        outputManager.AddMeshParameter("Solar Map", "Map", "Bakeable Viridis false-colour mesh using the full uncropped value domain.", GH_ParamAccess.item);
        outputManager.AddMeshParameter("Eligible Regions", "Reg", "One connected eligible-region mesh per native region ID.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Irradiance", "G", "Annual plane-of-array energy aligned with cells, kWh/m².", GH_ParamAccess.list);
        outputManager.AddBooleanParameter("Eligible", "OK", "Threshold mask aligned with cells.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Region IDs", "RID", "One connected-region ID per cell; zero is ineligible.", GH_ParamAccess.list);
        outputManager.AddColourParameter("Colours", "Col", "Colour aligned with each analysis cell.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Total Area", "A", "Complete analyzed geometric area in m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Eligible Area", "EA", "Geometric area meeting the threshold in m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Mean Irradiance", "Mean", "Area-weighted mean, kWh/m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("P10", "P10", "Area-weighted tenth percentile, kWh/m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Median", "P50", "Area-weighted median, kWh/m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("P90", "P90", "Area-weighted ninetieth percentile, kWh/m².", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Incident Energy", "E", "Incident solar energy on complete eligible cells, kWh.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Capacity Proxy", "kWp", "DC nameplate proxy at 1 kW/m² STC, kWp.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Yield Proxy", "Y", "Annual output proxy after declared efficiency, coverage, and losses, kWh.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Specific Yield", "SY", "Annual output proxy divided by capacity, kWh/kWp.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Combined Loss", "L", "Multiplicatively combined downstream system loss, percent.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Hash", "H", "Deterministic BLAKE3 cells/irradiance/assumptions/regions identity.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "Rep", "Area distribution, threshold, connected regions, assumptions, proxy limitation, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess dataAccess, out SolarPotentialWork work)
    {
        work = default;
        object? wrappedGrid = null;
        object? wrappedIrradiance = null;
        double minimum = 800.0;
        double moduleEfficiencyPercent = 22.0;
        double coveragePercent = 85.0;
        List<double> lossesPercent = [];
        if (!dataAccess.GetData(0, ref wrappedGrid)
            || !dataAccess.GetData(1, ref wrappedIrradiance))
        {
            ClearScheduledPreview();
            return false;
        }
        dataAccess.GetData(2, ref minimum);
        dataAccess.GetData(3, ref moduleEfficiencyPercent);
        dataAccess.GetData(4, ref coveragePercent);
        dataAccess.GetDataList(5, lossesPercent);
        try
        {
            SurfaceGridResult grid = Unwrap<SurfaceGridResult>(wrappedGrid)
                ?? throw new ArgumentException("Sensor Grid must come directly from XV Sensor Grid.");
            AnnualIrradianceResult irradiance = Unwrap<AnnualIrradianceResult>(wrappedIrradiance)
                ?? throw new ArgumentException("Irradiance Result must come directly from XV Irradiance.");
            if (lossesPercent.Count == 0)
            {
                lossesPercent.Add(14.0);
            }
            if (lossesPercent.Any(value => !double.IsFinite(value) || value is < 0.0 or > 100.0))
            {
                throw new ArgumentOutOfRangeException(nameof(lossesPercent), "Every loss must be between 0 and 100 percent.");
            }
            double combinedLoss = 1.0 - lossesPercent.Aggregate(1.0, (remaining, loss) => remaining * (1.0 - loss / 100.0));
            Dictionary<ulong, SensorIrradianceSummary> bySensor = irradiance.Summaries
                .ToDictionary(summary => summary.SensorId);
            double[] values = grid.Cells.Select(cell => bySensor.TryGetValue(cell.SensorId, out SensorIrradianceSummary summary)
                    ? summary.GlobalWhM2
                    : throw new ArgumentException($"Annual result does not contain sensor ID {cell.SensorId}. Connect XV Sensor Grid points, normals, and IDs to the same XV Irradiance run."))
                .ToArray();
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            double unitScale = RhinoMath.UnitScale(document?.ModelUnitSystem ?? UnitSystem.Meters, UnitSystem.Meters);
            PvPotentialOptions options = new(
                minimum,
                moduleEfficiencyPercent / 100.0,
                coveragePercent / 100.0,
                combinedLoss);
            work = new(grid, irradiance, values, options, unitScale, lossesPercent.ToArray(),
                moduleEfficiencyPercent, coveragePercent, combinedLoss, minimum);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException
            or InvalidOperationException
            or OverflowException
            or XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException)
        {
            ClearScheduledPreview();
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(SolarPotentialWork work) => checked((ulong)work.Grid.Cells.Count);

    /// <inheritdoc />
    protected override PvPotentialResult Execute(
        SolarPotentialWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Area-weighted analysis"));
        cancellationToken.ThrowIfCancellationRequested();
        PvPotentialResult result = SurfacePotentialAnalyzer.Analyze(work.Grid, work.Values, work.Options, work.UnitScale);
        progress.Report((total, total, "Connected regions"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess dataAccess, SolarPotentialWork work, PvPotentialResult result)
    {
        SurfaceGridResult grid = work.Grid;
        AnnualIrradianceResult irradiance = work.Irradiance;
        double[] values = work.Values;
        double minimum = work.MinimumIrradiance;
        double moduleEfficiencyPercent = work.ModuleEfficiencyPercent;
        double coveragePercent = work.CoveragePercent;
        double combinedLoss = work.CombinedLoss;
        IReadOnlyList<double> lossesPercent = work.LossesPercent;
        double minimumValue = values.Min();
        double maximumValue = values.Max();
        double range = maximumValue - minimumValue;
        Color[] colors = values.Select(value => SkyPalette.Viridis(range > 0.0
                ? (value - minimumValue) / range
                : 0.5))
            .ToArray();
        previewMesh = SensorGridComponent.CreateAnalysisMesh(grid.Cells, colors);
        Mesh[] regionMeshes = result.Regions
            .Select(region =>
            {
                List<SurfaceCell> cells = grid.Cells
                    .Zip(result.Cells)
                    .Where(item => item.Second.RegionId == region.RegionId)
                    .Select(item => item.First)
                    .ToList();
                Color color = SkyPalette.Viridis(range > 0.0
                    ? (region.MeanIrradianceWhM2 - minimumValue) / range
                    : 0.5);
                return SensorGridComponent.CreateAnalysisMesh(cells, Enumerable.Repeat(color, cells.Count).ToArray());
            })
            .ToArray();
        string lossList = string.Join(", ", lossesPercent.Select(value => value.ToString("G6", CultureInfo.InvariantCulture) + "%"));
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: MEHR surface intelligence; native area-weighted statistics and edge-connected hotspot extraction{Environment.NewLine}" +
            $"Mapping: {grid.Cells.Count:N0} stable sensor IDs; grid {grid.ContentHash}; annual result {irradiance.ContentHash}{Environment.NewLine}" +
            $"Distribution: mean {result.MeanIrradianceWhM2 / 1000.0:G8}; P10 {result.P10IrradianceWhM2 / 1000.0:G8}; P50 {result.P50IrradianceWhM2 / 1000.0:G8}; P90 {result.P90IrradianceWhM2 / 1000.0:G8} kWh/m²{Environment.NewLine}" +
            $"Domain: full uncropped [{minimumValue / 1000.0:G8}, {maximumValue / 1000.0:G8}] kWh/m²; monotonic colourblind-friendly Viridis{Environment.NewLine}" +
            $"Eligibility: threshold {minimum:G8} kWh/m²; {result.EligibleAreaM2:G8} of {result.TotalAreaM2:G8} m²; {result.Regions.Count:N0} edge-connected region(s){Environment.NewLine}" +
            $"PV assumptions: module efficiency {moduleEfficiencyPercent:G6}%; coverage {coveragePercent:G6}%; individual losses [{lossList}]; combined loss {combinedLoss * 100.0:G6}%{Environment.NewLine}" +
            $"PV proxy: eligible incident {result.EligibleIncidentEnergyKWh:G10} kWh; capacity {result.CapacityKwp:G10} kWp; output {result.ProxyYieldKWh:G10} kWh; specific {result.SpecificYieldKWhKwp:G10} kWh/kWp{Environment.NewLine}" +
            $"LIMITATION: screening proxy only—not a bankable yield model; temperature, inverter curve, module layout, horizon uncertainty, degradation, and dispatch are not independently modeled.{Environment.NewLine}" +
            $"Identity: {result.ContentHash}");
        dataAccess.SetData(0, result);
        dataAccess.SetData(1, previewMesh);
        dataAccess.SetDataList(2, regionMeshes);
        dataAccess.SetDataList(3, values.Select(value => value / 1000.0));
        dataAccess.SetDataList(4, result.Cells.Select(cell => cell.IsEligible));
        dataAccess.SetDataList(5, result.Cells.Select(cell => cell.RegionId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(6, colors);
        dataAccess.SetData(7, result.TotalAreaM2);
        dataAccess.SetData(8, result.EligibleAreaM2);
        dataAccess.SetData(9, result.MeanIrradianceWhM2 / 1000.0);
        dataAccess.SetData(10, result.P10IrradianceWhM2 / 1000.0);
        dataAccess.SetData(11, result.P50IrradianceWhM2 / 1000.0);
        dataAccess.SetData(12, result.P90IrradianceWhM2 / 1000.0);
        dataAccess.SetData(13, result.EligibleIncidentEnergyKWh);
        dataAccess.SetData(14, result.CapacityKwp);
        dataAccess.SetData(15, result.ProxyYieldKWh);
        dataAccess.SetData(16, result.SpecificYieldKWhKwp);
        dataAccess.SetData(17, combinedLoss * 100.0);
        dataAccess.SetData(18, result.ContentHash);
        dataAccess.SetData(19, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"MEHR MAP\n{result.EligibleAreaM2:N1} m² · {result.Regions.Count:N0} regions");
    }

    /// <inheritdoc />
    protected override void ClearScheduledPreview()
    {
        previewMesh = null;
    }

    private static T? Unwrap<T>(object? value) where T : class => value switch
    {
        T typed => typed,
        GH_ObjectWrapper { Value: T typed } => typed,
        _ => null,
    };
}

/// <summary>Immutable captured input for scheduled surface-potential work.</summary>
public readonly record struct SolarPotentialWork(
    SurfaceGridResult Grid,
    AnnualIrradianceResult Irradiance,
    double[] Values,
    PvPotentialOptions Options,
    double UnitScale,
    double[] LossesPercent,
    double ModuleEfficiencyPercent,
    double CoveragePercent,
    double CombinedLoss,
    double MinimumIrradiance);
