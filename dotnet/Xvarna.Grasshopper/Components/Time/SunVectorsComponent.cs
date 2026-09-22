using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Time;

/// <summary>Calculates high-accuracy solar positions and model-space vectors.</summary>
public sealed class SunVectorsComponent : GH_Component
{
    /// <summary>Initializes the HVARE solar-position component.</summary>
    public SunVectorsComponent()
        : base(
            "XVARNA Sun Vectors",
            "XV Sun Vectors",
            "Calculates source-aligned apparent solar positions with the Reda–Andreas SPA and emits XVARNA model-space sun vectors.",
            "XVARNA",
            "02 Time")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("66413441-dd80-4c05-aeb4-303c3ca086e1");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Period", "P", "Optional schedule produced by XV Period.", GH_ParamAccess.item);
        inputManager.AddTextParameter("Timestamps", "T", "Alternative ISO 8601 timestamps with explicit Z or ±HH:MM offset.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Duration Hours", "D", "For manual timestamps: none for 1 hour, one for all, or one per timestamp.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Weights", "W", "For manual timestamps: none for 1, one for all, or one per timestamp.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Latitude", "Lat", "WGS84 latitude in degrees, from -90 through 90.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Longitude", "Lon", "WGS84 longitude in degrees east of Greenwich, from -180 through 180.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Elevation", "Elev", "Observer elevation above sea level in metres.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("True North", "N", "Counter-clockwise rotation in degrees from model +Y to true north.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Minimum Altitude", "MinA", "Sun samples below this apparent altitude in degrees are inactive.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Delta T", "dT", "TT−UT1 correction in seconds for the study epoch.", GH_ParamAccess.item, 69.0);
        inputManager.AddNumberParameter("Pressure", "Pr", "Average local pressure in millibars. Set 0 to disable atmospheric refraction.", GH_ParamAccess.item, 1013.25);
        inputManager.AddNumberParameter("Temperature", "Ta", "Average dry-bulb temperature in degrees Celsius.", GH_ParamAccess.item, 15.0);
        inputManager[0].Optional = true;
        inputManager[1].Optional = true;
        inputManager[2].Optional = true;
        inputManager[3].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Sun Set", "S", "Immutable ordered HVARE sun set for XV Sun Hours.", GH_ParamAccess.item);
        outputManager.AddVectorParameter("Vectors", "V", "Unit vectors from a sensor toward the sun: +X east, +Y model north, +Z up before north rotation.", GH_ParamAccess.list);
        outputManager.AddBooleanParameter("Active", "A", "True after minimum-altitude and non-zero-weight filtering.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Altitude", "Alt", "Apparent solar altitude in degrees.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Azimuth", "Az", "Solar azimuth in degrees clockwise from true north.", GH_ParamAccess.list);
        outputManager.AddTextParameter("UTC Timestamps", "UTC", "Source-aligned timestamps normalized to UTC.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Duration Hours", "D", "Source-aligned interval durations.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Weights", "W", "Source-aligned interval weights.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Hash", "H", "BLAKE3 identity of location, atmosphere, north, schedule, and vectors.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Method, assumptions, filter counts, and reproducibility metadata.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        object? wrappedPeriod = null;
        List<string> timestampTexts = [];
        List<double> durations = [];
        List<double> weights = [];
        double latitude = 0.0;
        double longitude = 0.0;
        double elevation = 0.0;
        double trueNorth = 0.0;
        double minimumAltitude = 0.0;
        double deltaT = 69.0;
        double pressure = 1013.25;
        double temperature = 15.0;
        dataAccess.GetData(0, ref wrappedPeriod);
        dataAccess.GetDataList(1, timestampTexts);
        dataAccess.GetDataList(2, durations);
        dataAccess.GetDataList(3, weights);
        dataAccess.GetData(4, ref latitude);
        dataAccess.GetData(5, ref longitude);
        dataAccess.GetData(6, ref elevation);
        dataAccess.GetData(7, ref trueNorth);
        dataAccess.GetData(8, ref minimumAltitude);
        dataAccess.GetData(9, ref deltaT);
        dataAccess.GetData(10, ref pressure);
        dataAccess.GetData(11, ref temperature);

        try
        {
            XvarnaSolarPeriod? period = UnwrapPeriod(wrappedPeriod);
            if (period is not null && timestampTexts.Count > 0)
            {
                throw new ArgumentException("Connect either Period or manual Timestamps, not both.");
            }
            IReadOnlyList<SolarTimeSample> timeSamples = period?.Samples
                ?? BuildManualSamples(timestampTexts, durations, weights);
            SolarPositionOptions options = new(deltaT, pressure, temperature, trueNorth, minimumAltitude);
            XvarnaSunSet sunSet = SolarEngine.Calculate(new(latitude, longitude, elevation), timeSamples, options);
            string report = string.Create(
                CultureInfo.InvariantCulture,
                $"Engine: HVARE solar position; method: Reda–Andreas SPA{Environment.NewLine}" +
                $"Location: {latitude:G12}° lat, {longitude:G12}° lon, {elevation:G12} m{Environment.NewLine}" +
                $"Samples: {sunSet.Samples.Count:N0}; active: {sunSet.ActiveSampleCount:N0}{Environment.NewLine}" +
                $"Atmosphere: ΔT {deltaT:G12} s; pressure {pressure:G12} mbar; temperature {temperature:G12} °C{Environment.NewLine}" +
                $"Orientation: true north {trueNorth:G12}° CCW from model +Y; minimum altitude {minimumAltitude:G12}°{Environment.NewLine}" +
                $"Refraction: {(sunSet.IncludesAtmosphericRefraction ? "enabled" : "disabled")}{Environment.NewLine}" +
                $"Sun-set hash: {sunSet.ContentHash}");

            dataAccess.SetData(0, sunSet);
            dataAccess.SetDataList(1, sunSet.Samples.Select(sample => new Vector3d(sample.DirectionX, sample.DirectionY, sample.DirectionZ)));
            dataAccess.SetDataList(2, sunSet.Samples.Select(sample => sample.IsActive));
            dataAccess.SetDataList(3, sunSet.Samples.Select(sample => sample.AltitudeDegrees));
            dataAccess.SetDataList(4, sunSet.Samples.Select(sample => sample.AzimuthDegrees));
            dataAccess.SetDataList(5, sunSet.Samples.Select(sample => sample.TimestampUtc.ToString("O", CultureInfo.InvariantCulture)));
            dataAccess.SetDataList(6, sunSet.Samples.Select(sample => sample.DurationHours));
            dataAccess.SetDataList(7, sunSet.Samples.Select(sample => sample.Weight));
            dataAccess.SetData(8, sunSet.ContentHash);
            dataAccess.SetData(9, report);
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ArgumentException
            or OverflowException
            or InvalidOperationException
            or DllNotFoundException
            or BadImageFormatException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static IReadOnlyList<SolarTimeSample> BuildManualSamples(
        IReadOnlyList<string> timestamps,
        IReadOnlyList<double> durations,
        IReadOnlyList<double> weights)
    {
        if (timestamps.Count == 0)
        {
            throw new ArgumentException("Connect an XV Period or supply at least one manual Timestamp.");
        }
        if (!HasValidDataMatching(timestamps.Count, durations.Count)
            || !HasValidDataMatching(timestamps.Count, weights.Count))
        {
            throw new ArgumentException("Duration Hours and Weights must contain zero, one, or exactly one value per Timestamp.");
        }
        return timestamps
            .Select((text, index) => new SolarTimeSample(
                PeriodComponent.ParseExplicitOffset(text, $"Timestamp {index}"),
                Select(durations, index, 1.0),
                Select(weights, index, 1.0)))
            .ToArray();
    }

    private static XvarnaSolarPeriod? UnwrapPeriod(object? value) =>
        value switch
        {
            XvarnaSolarPeriod period => period,
            GH_ObjectWrapper { Value: XvarnaSolarPeriod period } => period,
            _ => null,
        };

    private static bool HasValidDataMatching(int sourceCount, int valueCount) =>
        valueCount is 0 or 1 || valueCount == sourceCount;

    private static double Select(IReadOnlyList<double> values, int index, double fallback) =>
        values.Count switch
        {
            0 => fallback,
            1 => values[0],
            _ => values[index],
        };
}
