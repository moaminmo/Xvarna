using System.Globalization;
using System.Text.Json;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

public sealed record OpticalLibraryRequest
{
    public IReadOnlyList<OpticalMaterial> Materials { get; init; } = [];
    public IReadOnlyList<MaterialAssignment> Assignments { get; init; } = [];
}

[IoId("5fd59960-81a7-4cbe-93f4-1176e7252139")]
public sealed class OpticalMaterialsComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public OpticalMaterialsComponent() : base(new Nomen("XVARNA Optical Materials", "Radiance-compatible RGB optical material catalog.", "XVARNA", "03 Environment")) { }
    public OpticalMaterialsComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) => inputs.AddText("Materials JSON", "JSON", "OpticalMaterial definitions and Object-ID assignments.").Set("{}");
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Materials", "M", "Validated optical library."); outputs.AddText("Canonical JSON", "JSON", "Portable source definition."); outputs.AddText("Report", "R", "Optical model and assignment summary."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out string json); try { OpticalLibraryRequest q = Gh2Json.Read<OpticalLibraryRequest>(json); OpticalMaterialLibrary r = new(q.Materials, q.Assignments); access.SetItem(0, r); access.SetItem(1, Gh2Json.Write(q)); access.SetItem(2, $"{q.Materials.Count:N0} Radiance-compatible RGB materials; {q.Assignments.Count:N0} exact Object-ID assignments; unassigned geometry is opaque"); } catch (Exception e) when (e is JsonException or ArgumentException) { access.AddError("Optical materials failed", e.Message); } }
}

public sealed record PointDaylightRequest
{
    public IReadOnlyList<DaylightSensor> Sensors { get; init; } = [];
    public DaylightMoment Moment { get; init; } = new(DateTimeOffset.UnixEpoch, 0, 0, 1, 0, 10_000);
    public DaylightMatrixOptions Options { get; init; } = new(512, 1e-4, double.PositiveInfinity, ulong.MaxValue, 8, 1e-4, DaylightSkyModel.CieOvercast);
    public double ExteriorHorizontalIlluminanceLux { get; init; } = 10_000;
}

[IoId("b75b855d-232e-4820-832a-176f58cbce7a")]
public sealed class PointDaylightComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public PointDaylightComponent() : base(new Nomen("XVARNA Daylight", "Point illuminance or CIE-overcast Daylight Factor.", "XVARNA", "03 Environment")) { }
    public PointDaylightComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Optical Materials", "M", "XVARNA Optical Materials.", Access.Item, Requirement.MayBeMissing);
        inputs.AddText("Mode", "Mode", "Point or Factor.").Set("Point"); inputs.AddPoint("Sensors", "P", "Workplane points.", Access.Twig);
        inputs.AddVector("Normals", "N", "Empty, one, or one per sensor.", Access.Twig, Requirement.MayBeMissing); inputs.AddNumber("Areas", "A", "Empty, one, or one per sensor.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Timestamp UTC", "T", "ISO-8601 timestamp.").Set("2026-06-21T08:30:00Z"); inputs.AddVector("Sun Direction", "Sun", "Vector from sensor toward sun.").Set(Rhino.Geometry.Vector3d.ZAxis);
        inputs.AddNumber("Direct Normal Lux", "DNI", "Direct-normal illuminance.").Set(80000); inputs.AddNumber("Diffuse Horizontal Lux", "DHI", "Diffuse-horizontal illuminance.").Set(10000);
        inputs.AddInteger("Sky Patches", "Q", "Diffuse coefficient directions.").Set(576); inputs.AddNumber("Exterior DF Lux", "DFext", "Exterior illuminance for Daylight Factor.").Set(10000);
        inputs.AddText("Advanced JSON", "JSON", "Optional full request override.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Typed daylight result."); outputs.AddNumber("Total Lux", "Lux", "Point total or DF interior illuminance.", Access.Twig); outputs.AddNumber("Direct Lux", "Dir", "Point direct illuminance; zero in DF mode.", Access.Twig); outputs.AddNumber("Diffuse Lux", "Dif", "Point diffuse illuminance; zero in DF mode.", Access.Twig); outputs.AddNumber("Daylight Factor %", "DF", "Daylight Factor percentage; NaN in Point mode.", Access.Twig); outputs.AddBoolean("Matrix Reused", "Cache", "Daylight-coefficient matrix cache hit."); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Method, matrix and timing provenance."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object so); object? mo = null; if (!access.GetNull(1)) access.GetItem(1, out mo); access.GetItem(2, out string mode);
        Rhino.Geometry.Point3d[] points = []; Rhino.Geometry.Vector3d[] normals = []; double[] areas = []; access.GetItemArray(3, out points); if (!access.GetNull(4)) access.GetItemArray(4, out normals); if (!access.GetNull(5)) access.GetItemArray(5, out areas);
        access.GetItem(6, out string timestamp); access.GetItem(7, out Rhino.Geometry.Vector3d sun); access.GetItem(8, out double dni); access.GetItem(9, out double dhi);
        access.GetItem(10, out int patches); access.GetItem(11, out double exterior); string? json = null; if (!access.GetNull(12)) access.GetItem(12, out json);
        if (so is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try { PointDaylightRequest q = !string.IsNullOrWhiteSpace(json) ? Gh2Json.Read<PointDaylightRequest>(json) : new() { Sensors = Gh2AnalysisAdapters.Sensors(points, normals, areas), Moment = new(DateTimeOffset.Parse(timestamp, CultureInfo.InvariantCulture), sun.X, sun.Y, sun.Z, dni, dhi), Options = new(checked((uint)patches)), ExteriorHorizontalIlluminanceLux = exterior }; OpticalMaterialLibrary materials = mo as OpticalMaterialLibrary ?? OpticalMaterialLibrary.Opaque; SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(5); if (mode.Trim().StartsWith("F", StringComparison.OrdinalIgnoreCase)) { DaylightFactorResult r = scene.AnalyzeDaylightFactor(q.Sensors, materials, q.ExteriorHorizontalIlluminanceLux, q.Options); SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(100); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Entries.Select(v => v.InteriorIlluminanceLux).ToArray()); Gh2Data.SetTwig(access, 2, r.Entries.Select(_ => 0.0).ToArray()); Gh2Data.SetTwig(access, 3, r.Entries.Select(_ => 0.0).ToArray()); Gh2Data.SetTwig(access, 4, r.Entries.Select(v => v.DaylightFactorPercent).ToArray()); access.SetItem(5, r.MatrixReused); access.SetItem(6, r.ContentHash); access.SetItem(7, $"CIE-overcast Daylight Factor; {r.Entries.Count:N0} sensors; exterior {r.ExteriorHorizontalIlluminanceLux:G8} lux; matrix {r.MatrixHash}; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms"); } else { PointIlluminanceResult r = scene.AnalyzePointIlluminance(q.Sensors, materials, q.Moment, q.Options, SolutionCancellationToken); access.SetProgress(100); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Entries.Select(v => v.TotalLux).ToArray()); Gh2Data.SetTwig(access, 2, r.Entries.Select(v => v.DirectLux).ToArray()); Gh2Data.SetTwig(access, 3, r.Entries.Select(v => v.DiffuseLux).ToArray()); Gh2Data.SetTwig(access, 4, r.Entries.Select(_ => double.NaN).ToArray()); access.SetItem(5, r.MatrixReused); access.SetItem(6, r.ContentHash); access.SetItem(7, $"point-in-time illuminance; {r.Entries.Count:N0} sensors; matrix {r.MatrixHash}; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms"); } }
        catch (Exception e) when (e is JsonException or ArgumentException or XvarnaNativeException or InvalidOperationException or OperationCanceledException) { access.AddError("Daylight failed", e.Message); }
    }
}

public sealed record AnnualDaylightRequest
{
    public IReadOnlyList<DaylightSensor> Sensors { get; init; } = [];
    public AnnualDaylightOptions Options { get; init; } = AnnualDaylightOptions.Default;
}

[IoId("64c77232-a554-4bd6-a03a-b28246203a30")]
public sealed class AnnualDaylightComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public AnnualDaylightComponent() : base(new Nomen("XVARNA Annual Daylight", "Climate-based sDA, ASE and four-bin UDI.", "XVARNA", "03 Environment")) { }
    public AnnualDaylightComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddText("EPW Path", "EPW", "EnergyPlus Weather file."); inputs.AddGeneric("Optical Materials", "M", "XVARNA Optical Materials.", Access.Item, Requirement.MayBeMissing);
        inputs.AddPoint("Sensors", "P", "Workplane sensor points.", Access.Twig); inputs.AddVector("Normals", "N", "Empty, one, or one per sensor.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Areas", "A", "Empty, one, or one represented area per sensor.", Access.Twig, Requirement.MayBeMissing); inputs.AddInteger("Sky Patches", "Q", "Static diffuse directions.").Set(576);
        inputs.AddNumber("Occupied Start", "Start", "Local standard-time hour.").Set(8); inputs.AddNumber("Occupied End", "End", "Local standard-time hour.").Set(18);
        inputs.AddNumber("sDA Lux", "sDA", "sDA threshold.").Set(300); inputs.AddNumber("ASE Lux", "ASE", "Direct ASE threshold.").Set(1000);
        inputs.AddNumber("ASE Hours", "h", "Maximum exceedance hours.").Set(250); inputs.AddNumber("North", "N°", "Model rotation from true north.").Set(0);
        inputs.AddText("Advanced JSON", "JSON", "Optional full sensors/options override.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete annual daylight result."); outputs.AddNumber("sDA Area %", "sDA", "Area satisfying the sDA criterion."); outputs.AddNumber("ASE Area %", "ASE", "Area exceeding the ASE criterion."); outputs.AddNumber("UDI Useful %", "UDI", "Area/time useful range summary."); outputs.AddNumber("Sensor sDA", "S", "Per-sensor qualified occupied fraction.", Access.Twig); outputs.AddBoolean("Sensor ASE", "A", "Per-sensor ASE exceedance.", Access.Twig); outputs.AddBoolean("Matrix Reused", "Cache", "Annual coefficient matrix cache hit."); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Metrics, weather and timing provenance."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object so); access.GetItem(1, out string path); object? mo = null; if (!access.GetNull(2)) access.GetItem(2, out mo);
        Rhino.Geometry.Point3d[] points = []; Rhino.Geometry.Vector3d[] normals = []; double[] areas = []; access.GetItemArray(3, out points); if (!access.GetNull(4)) access.GetItemArray(4, out normals); if (!access.GetNull(5)) access.GetItemArray(5, out areas);
        access.GetItem(6, out int patches); access.GetItem(7, out double start); access.GetItem(8, out double end); access.GetItem(9, out double sda); access.GetItem(10, out double ase);
        access.GetItem(11, out double aseHours); access.GetItem(12, out double north); string? json = null; if (!access.GetNull(13)) access.GetItem(13, out json);
        if (so is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try
        {
            AnnualDaylightRequest q = !string.IsNullOrWhiteSpace(json) ? Gh2Json.Read<AnnualDaylightRequest>(json) : new()
            {
                Sensors = Gh2AnalysisAdapters.Sensors(points, normals, areas),
                Options = AnnualDaylightOptions.Default with { Matrix = new(checked((uint)patches)), OccupiedStartHour = start, OccupiedEndHour = end, SdaThresholdLux = sda, AseThresholdLux = ase, AseMaximumHours = aseHours, Solar = SolarPositionOptions.Default with { NorthRotationDegrees = north } },
            };
            EpwWeatherFile weather = EpwWeatherFile.Load(System.IO.Path.GetFullPath(path)); SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(5);
            AnnualDaylightResult r = scene.AnalyzeAnnualDaylight(q.Sensors, weather, mo as OpticalMaterialLibrary ?? OpticalMaterialLibrary.Opaque, q.Options, SolutionCancellationToken);
            access.SetProgress(100); access.SetItem(0, r); access.SetItem(1, r.Project.SdaAreaPercent); access.SetItem(2, r.Project.AseAreaPercent); access.SetItem(3, r.Project.UdiUsefulPercent);
            Gh2Data.SetTwig(access, 4, r.Summaries.Select(v => v.SdaOccupiedFraction * 100).ToArray()); Gh2Data.SetTwig(access, 5, r.Summaries.Select(v => v.AseFails).ToArray()); access.SetItem(6, r.MatrixReused); access.SetItem(7, r.ContentHash);
            access.SetItem(8, $"climate-based annual daylight; {q.Sensors.Count:N0} sensors × {r.WeatherCount:N0} records; matrix {r.MatrixHash}; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms; Fast Path screening—validate publication evidence with Radiance");
        }
        catch (Exception e) when (e is JsonException or ArgumentException or IOException or XvarnaNativeException or InvalidOperationException or OperationCanceledException or UnauthorizedAccessException or FormatException) { access.AddError("Annual daylight failed", e.Message); }
    }
}

public sealed record RadianceRequest
{
    public IReadOnlyList<DaylightSensor> Sensors { get; init; } = [];
}

[IoId("582cdf0c-57ce-4dda-a614-c97084fdc1f1")]
public sealed class RadianceExportComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public RadianceExportComponent() : base(new Nomen("XVARNA Radiance", "Exports a canonical Radiance scene bundle.", "XVARNA", "03 Environment")) { }
    public RadianceExportComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Optical Materials", "M", "XVARNA Optical Materials.", Access.Item, Requirement.MayBeMissing); inputs.AddPoint("Sensors", "P", "Radiance sensor points.", Access.Twig); inputs.AddVector("Normals", "N", "Empty, one, or one per sensor.", Access.Twig, Requirement.MayBeMissing); inputs.AddText("Directory", "Dir", "Output directory."); inputs.AddBoolean("Write", "W", "Write the deterministic bundle.").Set(false); inputs.AddText("Advanced JSON", "JSON", "Optional sensor request override.", Access.Item, Requirement.MayBeMissing); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Bundle", "B", "Canonical in-memory bundle."); outputs.AddText("Materials", "M", "materials.rad path."); outputs.AddText("Geometry", "G", "geometry.rad path."); outputs.AddText("Sensors", "S", "sensors.pts path."); outputs.AddText("Manifest", "JSON", "manifest path."); outputs.AddText("Hash", "H", "Bundle identity."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object so); object? mo = null; if (!access.GetNull(1)) access.GetItem(1, out mo); Rhino.Geometry.Point3d[] points = []; Rhino.Geometry.Vector3d[] normals = []; access.GetItemArray(2, out points); if (!access.GetNull(3)) access.GetItemArray(3, out normals); access.GetItem(4, out string directory); access.GetItem(5, out bool write); string? json = null; if (!access.GetNull(6)) access.GetItem(6, out json); if (so is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; } try { RadianceRequest q = !string.IsNullOrWhiteSpace(json) ? Gh2Json.Read<RadianceRequest>(json) : new() { Sensors = Gh2AnalysisAdapters.Sensors(points, normals, []) }; SolutionCancellationToken.ThrowIfCancellationRequested(); RadianceExportBundle r = scene.ExportRadiance(q.Sensors, mo as OpticalMaterialLibrary ?? OpticalMaterialLibrary.Opaque); string root = string.IsNullOrWhiteSpace(directory) ? string.Empty : System.IO.Path.GetFullPath(directory); if (write) { if (root.Length == 0) throw new ArgumentException("Directory is required when Write is true."); r.Write(root); } access.SetItem(0, r); access.SetItem(1, write ? System.IO.Path.Combine(root, "materials.rad") : string.Empty); access.SetItem(2, write ? System.IO.Path.Combine(root, "geometry.rad") : string.Empty); access.SetItem(3, write ? System.IO.Path.Combine(root, "sensors.pts") : string.Empty); access.SetItem(4, write ? System.IO.Path.Combine(root, "manifest.json") : string.Empty); access.SetItem(5, r.ContentHash); } catch (Exception e) when (e is JsonException or ArgumentException or IOException or XvarnaNativeException or InvalidOperationException or OperationCanceledException or UnauthorizedAccessException) { access.AddError("Radiance export failed", e.Message); } }
}

[IoId("65e91cde-2398-4591-9260-9be17c49dd97")]
public sealed class DaylightValidationComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public DaylightValidationComponent() : base(new Nomen("XVARNA Daylight Delta", "Compares aligned Fast Path and Radiance illuminance.", "XVARNA", "03 Environment")) { }
    public DaylightValidationComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddNumber("Fast Lux", "F", "XVARNA Fast Path values.", Access.Twig); inputs.AddNumber("Radiance Lux", "R", "Aligned Radiance reference values.", Access.Twig); inputs.AddNumber("Absolute Tolerance", "A", "Lux tolerance.").Set(50); inputs.AddNumber("Relative Tolerance", "P", "Fractional tolerance.").Set(0.1); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Validation", "V", "Complete statistical report."); outputs.AddBoolean("Accepted", "OK", "All declared acceptance rules passed."); outputs.AddNumber("Bias", "Bias", "Mean signed error in lux."); outputs.AddNumber("MAE", "MAE", "Mean absolute error in lux."); outputs.AddNumber("RMSE", "RMSE", "Root mean square error in lux."); outputs.AddNumber("R2", "R2", "Coefficient of determination."); outputs.AddNumber("Accepted Fraction", "A", "Fraction within the tolerance envelope."); outputs.AddText("Hash", "H", "Comparison identity."); outputs.AddText("Report", "Info", "Scientific difference summary."); }
    protected override void Process(IDataAccess access) { double[] fast = [], reference = []; access.GetItemArray(0, out fast); access.GetItemArray(1, out reference); access.GetItem(2, out double absolute); access.GetItem(3, out double relative); try { DaylightValidationReport r = DaylightValidation.Compare(fast, reference, absolute, relative); access.SetItem(0, r); access.SetItem(1, r.Accepted); access.SetItem(2, r.MeanBiasLux); access.SetItem(3, r.MeanAbsoluteErrorLux); access.SetItem(4, r.RootMeanSquareErrorLux); access.SetItem(5, r.RSquared); access.SetItem(6, r.AcceptedFraction); access.SetItem(7, r.ContentHash); access.SetItem(8, string.Create(CultureInfo.InvariantCulture, $"n={r.Count:N0}; bias {r.MeanBiasLux:G8} lux; MAE {r.MeanAbsoluteErrorLux:G8}; RMSE {r.RootMeanSquareErrorLux:G8}; MAPE {r.MeanAbsolutePercentageError:P3}; max {r.MaximumAbsoluteErrorLux:G8}; R² {r.RSquared:G8}; accepted {r.AcceptedFraction:P2}")); } catch (Exception e) when (e is ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Daylight comparison failed", e.Message); } }
}
