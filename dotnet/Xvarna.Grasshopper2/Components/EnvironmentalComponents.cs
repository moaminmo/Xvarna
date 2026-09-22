using System.Globalization;
using System.Text.Json;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Rhino.Geometry;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

public sealed record AnnualIrradianceRequest
{
    public IReadOnlyList<SolarSensor> Sensors { get; init; } = [];
    public AnnualIrradianceOptions Options { get; init; } = AnnualIrradianceOptions.Default;
}

[IoId("c3b236c0-9d0c-4e32-b7ca-5f853f9f393e")]
public sealed class AnnualIrradianceComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public AnnualIrradianceComponent() : base(new Nomen("XVARNA Irradiance", "Annual direct, diffuse, horizon and ground irradiance.", "XVARNA", "03 Environment")) { }
    public AnnualIrradianceComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddText("EPW Path", "EPW", "EnergyPlus Weather file path.");
        inputs.AddPoint("Sensors", "P", "Sensor points.", Access.Twig); inputs.AddVector("Normals", "N", "One normal or one per sensor.", Access.Twig);
        inputs.AddText("Sensor IDs", "ID", "Empty, one first ID, or one ID per sensor.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Ground Albedo", "A", "Diffuse ground reflectance fallback.").Set(0.2); inputs.AddBoolean("Use EPW Albedo", "EA", "Prefer valid EPW albedo values.").Set(true);
        inputs.AddNumber("Offset", "E", "Ray offset.").Set(1e-4); inputs.AddNumber("Maximum Distance", "Max", "Zero means unbounded.").Set(0);
        inputs.AddText("Category Mask", "C", "-1 selects all scene categories.").Set("-1"); inputs.AddNumber("North", "N°", "Clockwise model-north rotation in degrees.").Set(0);
        inputs.AddNumber("Delta T", "dT", "Terrestrial-time minus UT1 seconds.").Set(69); inputs.AddNumber("Pressure", "hPa", "Atmospheric pressure in millibars.").Set(1013.25);
        inputs.AddNumber("Temperature", "°C", "Air temperature for refraction.").Set(15); inputs.AddNumber("Minimum Altitude", "Alt", "Solar horizon cutoff in degrees.").Set(0);
        inputs.AddText("Advanced JSON", "JSON", "Optional full SolarSensor/options override.", Access.Item, Requirement.MayBeMissing);
    }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete sensor-major annual result."); outputs.AddNumber("Global kWh/m2", "G", "Annual global plane-of-array irradiation.", Access.Twig); outputs.AddNumber("Direct kWh/m2", "D", "Annual direct component.", Access.Twig); outputs.AddNumber("Diffuse kWh/m2", "S", "Annual diffuse-sky component.", Access.Twig); outputs.AddText("Dominant Occluder", "OID", "Dominant direct-solar blocker.", Access.Twig); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Weather, method and timing provenance."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); access.GetItem(1, out string path); Point3d[] points = []; Vector3d[] normals = []; string[] ids = [];
        access.GetItemArray(2, out points); access.GetItemArray(3, out normals); if (!access.GetNull(4)) access.GetItemArray(4, out ids);
        access.GetItem(5, out double albedo); access.GetItem(6, out bool epwAlbedo); access.GetItem(7, out double offset); access.GetItem(8, out double maximum);
        access.GetItem(9, out string maskText); access.GetItem(10, out double north); access.GetItem(11, out double deltaT); access.GetItem(12, out double pressure);
        access.GetItem(13, out double temperature); access.GetItem(14, out double minimumAltitude); string? json = null; if (!access.GetNull(15)) access.GetItem(15, out json);
        if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try
        {
            AnnualIrradianceRequest q;
            if (!string.IsNullOrWhiteSpace(json)) q = Gh2Json.Read<AnnualIrradianceRequest>(json);
            else
            {
                if (points.Length == 0 || (normals.Length != 1 && normals.Length != points.Length)) throw new ArgumentException("Provide sensors and one normal or one normal per sensor.");
                if (ids.Length is not 0 and not 1 && ids.Length != points.Length) throw new ArgumentException("Sensor IDs must be empty, one first ID, or one ID per sensor.");
                ulong firstId = ids.Length == 1 ? Gh2AnalysisAdapters.ParseUnsigned(ids[0], "Sensor ID") : 1;
                SolarSensor[] sensors = points.Select((point, index) =>
                {
                    Vector3d normal = normals.Length == 1 ? normals[0] : normals[index];
                    ulong id = ids.Length == points.Length ? Gh2AnalysisAdapters.ParseUnsigned(ids[index], "Sensor ID") : checked(firstId + (ulong)index);
                    return new SolarSensor(id, point.X, point.Y, point.Z, normal.X, normal.Y, normal.Z);
                }).ToArray();
                ulong mask = maskText == "-1" ? ulong.MaxValue : Gh2AnalysisAdapters.ParseUnsigned(maskText, "Category Mask");
                q = new() { Sensors = sensors, Options = new(offset, maximum == 0 ? double.PositiveInfinity : maximum, mask, albedo, epwAlbedo, new(deltaT, pressure, temperature, north, minimumAltitude)) };
            }
            SolutionCancellationToken.ThrowIfCancellationRequested(); EpwWeatherFile weather = EpwWeatherFile.Load(System.IO.Path.GetFullPath(path)); access.SetProgress(5);
            AnnualIrradianceResult r = scene.AnalyzeAnnualIrradiance(q.Sensors, weather, q.Options, SolutionCancellationToken);
            access.SetProgress(100); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Summaries.Select(v => v.GlobalWhM2 / 1000).ToArray()); Gh2Data.SetTwig(access, 2, r.Summaries.Select(v => v.DirectWhM2 / 1000).ToArray()); Gh2Data.SetTwig(access, 3, r.Summaries.Select(v => v.DiffuseSkyWhM2 / 1000).ToArray()); Gh2Data.SetTwig(access, 4, r.Summaries.Select(v => v.DominantSolarOccluderObjectId == 0 ? string.Empty : v.DominantSolarOccluderObjectId.ToString(CultureInfo.InvariantCulture)).ToArray()); access.SetItem(5, r.ContentHash); access.SetItem(6, $"Perez 1990 transposition with traced dome/horizon/direct visibility; {r.Sensors.Count:N0} sensors × {r.Weather.WeatherCount:N0} records; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms");
        }
        catch (Exception e) when (e is JsonException or ArgumentException or IOException or UnauthorizedAccessException or XvarnaNativeException or InvalidOperationException or OverflowException or OperationCanceledException) { access.AddError("Annual irradiance failed", e.Message); }
    }
}

[IoId("ee467ab9-54ed-4b71-a35f-a747689736e9")]
public sealed class SensorGridComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SensorGridComponent() : base(new Nomen("XVARNA Sensor Grid", "Area-preserving longest-edge surface subdivision.", "XVARNA", "03 Environment")) { }
    public SensorGridComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddMesh("Mesh", "M", "Triangulated analysis surface."); inputs.AddNumber("Cell Size", "L", "Maximum target edge length.").Set(1); inputs.AddNumber("Offset", "E", "Normal sensor offset.").Set(1e-4); inputs.AddText("First ID", "ID", "First sensor ID.").Set("1"); inputs.AddInteger("Maximum Cells", "Max", "Hard resource limit.").Set(1_000_000); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Grid", "G", "Complete area-aware grid."); outputs.AddPoint("Points", "P", "Oriented sensor points.", Access.Twig); outputs.AddVector("Normals", "N", "Sensor normals.", Access.Twig); outputs.AddNumber("Areas", "A", "Cell areas.", Access.Twig); outputs.AddText("IDs", "ID", "Stable sensor IDs.", Access.Twig); outputs.AddText("Hash", "H", "Grid identity."); outputs.AddText("Report", "R", "Area conservation and subdivision summary."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out Mesh mesh); access.GetItem(1, out double size); access.GetItem(2, out double offset); access.GetItem(3, out string first); access.GetItem(4, out int max); try { PackedMesh p = Gh2MeshAdapter.Pack(mesh); SurfaceGridResult r = SurfaceGridGenerator.Generate(p.Positions, p.Triangles, new(size, offset, ulong.Parse(first, CultureInfo.InvariantCulture), checked((ulong)max))); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Cells.Select(v => new Point3d(v.PositionX, v.PositionY, v.PositionZ)).ToArray()); Gh2Data.SetTwig(access, 2, r.Cells.Select(v => new Vector3d(v.NormalX, v.NormalY, v.NormalZ)).ToArray()); Gh2Data.SetTwig(access, 3, r.Cells.Select(v => v.Area).ToArray()); Gh2Data.SetTwig(access, 4, r.Cells.Select(v => v.SensorId.ToString(CultureInfo.InvariantCulture)).ToArray()); access.SetItem(5, r.ContentHash); access.SetItem(6, $"{r.SourceFaceCount:N0} source triangles → {r.Cells.Count:N0} cells; area {r.SampledArea:G12}/{r.SourceArea:G12}; max depth {r.MaximumSubdivisionDepth:N0}; skipped {r.SkippedFaceCount:N0}"); } catch (Exception e) when (e is ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException) { access.AddError("Sensor grid failed", e.Message); } }
}

[IoId("fe2918f0-917a-4aea-897c-cf9a7370526c")]
public sealed class SolarPotentialComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SolarPotentialComponent() : base(new Nomen("XVARNA Solar Potential", "Area-weighted hotspots, connected regions and PV proxy.", "XVARNA", "03 Environment")) { }
    public SolarPotentialComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Grid", "G", "XVARNA Sensor Grid."); inputs.AddGeneric("Irradiance", "I", "XVARNA Irradiance result."); inputs.AddNumber("Meters/Unit", "m/U", "Model-unit scale to metres.").Set(1); inputs.AddNumber("Threshold kWh/m2", "T", "Eligibility threshold.").Set(800); inputs.AddNumber("Efficiency", "Eff", "Module efficiency fraction.").Set(0.22); inputs.AddNumber("Coverage", "C", "Geometric coverage fraction.").Set(0.85); inputs.AddNumber("Loss", "L", "Aggregate system loss fraction.").Set(0.14); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete potential result."); outputs.AddBoolean("Eligible", "E", "Per-cell threshold state.", Access.Twig); outputs.AddText("Region IDs", "RID", "Connected eligible region IDs.", Access.Twig); outputs.AddNumber("Yield kWh", "Y", "Per-cell proxy yield.", Access.Twig); outputs.AddNumber("Total Yield kWh", "Sum", "Annual screening proxy."); outputs.AddNumber("Capacity kWp", "kWp", "Installed-capacity proxy."); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Explicit non-bankable assumptions."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object go); access.GetItem(1, out object io); access.GetItem(2, out double scale); access.GetItem(3, out double threshold); access.GetItem(4, out double efficiency); access.GetItem(5, out double coverage); access.GetItem(6, out double loss); if (go is not SurfaceGridResult grid || io is not AnnualIrradianceResult irr) { access.AddError("Invalid inputs", "Connect Sensor Grid and Irradiance results."); return; } try { Dictionary<ulong, double> byId = irr.Summaries.ToDictionary(v => v.SensorId, v => v.GlobalWhM2); double[] aligned = grid.Cells.Select(v => byId.TryGetValue(v.SensorId, out double value) ? value : throw new ArgumentException($"Irradiance is missing Sensor ID {v.SensorId}.")).ToArray(); PvPotentialResult r = SurfacePotentialAnalyzer.Analyze(grid, aligned, new(threshold, efficiency, coverage, loss), scale); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Cells.Select(v => v.IsEligible).ToArray()); Gh2Data.SetTwig(access, 2, r.Cells.Select(v => v.RegionId == 0 ? string.Empty : v.RegionId.ToString(CultureInfo.InvariantCulture)).ToArray()); Gh2Data.SetTwig(access, 3, r.Cells.Select(v => v.ProxyYieldKWh).ToArray()); access.SetItem(4, r.ProxyYieldKWh); access.SetItem(5, r.CapacityKwp); access.SetItem(6, r.ContentHash); access.SetItem(7, $"screening proxy only; area {r.EligibleAreaM2:G8}/{r.TotalAreaM2:G8} m²; efficiency {efficiency:P1}, coverage {coverage:P1}, loss {loss:P1}; not bankable energy yield"); } catch (Exception e) when (e is ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException) { access.AddError("Solar potential failed", e.Message); } }
}
