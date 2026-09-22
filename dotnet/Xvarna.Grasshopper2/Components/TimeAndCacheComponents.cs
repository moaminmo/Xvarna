using System.Globalization;
using System.Text.RegularExpressions;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Rhino.Geometry;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

/// <summary>Configures the bounded checksummed memory/disk cache and exposes telemetry.</summary>
[IoId("c84040e1-739c-4a65-8eef-9c57277bbb23")]
public sealed class CacheComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private CacheConfiguration? current;
    private bool previousClear;
    /// <summary>Initializes the component.</summary>
    public CacheComponent() : base(new Nomen("XVARNA Cache", "Memory/disk LRU policy and telemetry.", "XVARNA", "00 Setup")) { }
    /// <summary>Deserializes the component.</summary>
    public CacheComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Directory", "Dir", "Empty uses LocalAppData/XVARNA/cache/v1.").Set(string.Empty);
        inputs.AddNumber("Memory MB", "RAM", "Process-memory LRU budget.").Set(256.0);
        inputs.AddNumber("Disk GB", "Disk", "Persistent disk budget.").Set(4.0);
        inputs.AddBoolean("Apply", "Apply", "Apply policy changes.").Set(true);
        inputs.AddBoolean("Clear", "Clear", "Rising edge clears XVARNA cache entries.").Set(false);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddNumber("Memory Hits", "MH", "Memory-tier hits."); outputs.AddNumber("Disk Hits", "DH", "Disk-tier hits.");
        outputs.AddNumber("Misses", "Miss", "Cache misses."); outputs.AddNumber("Writes", "W", "Atomic writes.");
        outputs.AddNumber("Evictions", "E", "Budget evictions."); outputs.AddNumber("Recoveries", "C", "Corrupt entries rejected.");
        outputs.AddText("Report", "Info", "Budgets, occupancy, schema and directory.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string directory); access.GetItem(1, out double memoryMb); access.GetItem(2, out double diskGb); access.GetItem(3, out bool apply); access.GetItem(4, out bool clear);
        try
        {
            string path = string.IsNullOrWhiteSpace(directory) ? System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "XVARNA", "cache", "v1") : System.IO.Path.GetFullPath(directory.Trim());
            CacheConfiguration configuration = new(path, Bytes(memoryMb, 1_048_576), Bytes(diskGb, 1_073_741_824));
            if (apply && current != configuration) { XvarnaCache.Configure(configuration); current = configuration; }
            if (clear && !previousClear) XvarnaCache.Clear(); previousClear = clear;
            CacheStatistics s = XvarnaCache.GetStatistics();
            access.SetItem(0, (double)s.MemoryHitCount); access.SetItem(1, (double)s.DiskHitCount); access.SetItem(2, (double)s.MissCount);
            access.SetItem(3, (double)s.WriteCount); access.SetItem(4, (double)s.EvictionCount); access.SetItem(5, (double)s.CorruptionCount);
            access.SetItem(6, $"BLAKE3 cache v1; memory {s.MemoryBytes / 1_048_576.0:N3}/{s.MemoryBudgetBytes / 1_048_576.0:N3} MiB; disk {s.DiskBytes / 1_073_741_824.0:N3}/{s.DiskBudgetBytes / 1_073_741_824.0:N3} GiB; entries {s.MemoryEntryCount:N0}+{s.DiskEntryCount:N0}; {s.Directory}");
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or OverflowException or XvarnaNativeException) { access.AddError("Cache operation failed", exception.Message); }
    }
    private static ulong Bytes(double value, double scale) => double.IsFinite(value) && value >= 0 && value <= ulong.MaxValue / scale ? checked((ulong)Math.Ceiling(value * scale)) : throw new ArgumentOutOfRangeException(nameof(value));
}

/// <summary>Builds an explicit fixed-offset interval-centred ZURVAN schedule.</summary>
[IoId("17918bd8-b275-4cad-a194-d4ae84e7839c")]
public sealed class PeriodComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public PeriodComponent() : base(new Nomen("XVARNA Period", "Deterministic fixed-offset environmental schedule.", "XVARNA", "02 Time")) { }
    /// <summary>Deserializes the component.</summary>
    public PeriodComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Start", "A", "Inclusive ISO 8601 boundary with explicit offset.").Set("2026-06-21T06:00:00+00:00");
        inputs.AddText("End", "B", "Exclusive boundary with the same offset.").Set("2026-06-21T18:00:00+00:00");
        inputs.AddNumber("Step Minutes", "dt", "Positive interval length.").Set(60.0); inputs.AddNumber("Weight", "W", "Non-negative interval weight.").Set(1.0);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Period", "P", "Immutable ZURVAN schedule."); outputs.AddText("Timestamps", "T", "Interval-centre timestamps.", Access.Twig);
        outputs.AddNumber("Duration Hours", "D", "Actual durations.", Access.Twig); outputs.AddNumber("Weights", "W", "Interval weights.", Access.Twig);
        outputs.AddNumber("Weighted Hours", "H", "Weighted total."); outputs.AddText("Report", "R", "Boundary convention and sample count.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string startText); access.GetItem(1, out string endText); access.GetItem(2, out double minutes); access.GetItem(3, out double weight);
        try
        {
            DateTimeOffset start = Parse(startText, "Start"), end = Parse(endText, "End");
            if (start.Offset != end.Offset || end <= start) throw new ArgumentException("End must follow Start and use the same fixed UTC offset.");
            if (!double.IsFinite(minutes) || minutes <= 0 || !double.IsFinite(weight) || weight < 0) throw new ArgumentException("Step and weight must be finite; step positive and weight non-negative.");
            int capacity = checked((int)Math.Ceiling((end - start).TotalMinutes / minutes)); if (capacity > 1_000_000) throw new ArgumentException("Schedule exceeds the one-million sample limit.");
            List<SolarTimeSample> samples = new(capacity); DateTimeOffset cursor = start;
            while (cursor < end) { double seconds = Math.Min(minutes * 60, (end - cursor).TotalSeconds); samples.Add(new(cursor.AddSeconds(seconds / 2), seconds / 3600, weight)); cursor = cursor.AddSeconds(seconds); }
            XvarnaSolarPeriod period = new() { Samples = samples, Start = start, End = end, StepMinutes = minutes };
            access.SetItem(0, period); Gh2Data.SetTwig(access, 1, samples.Select(s => s.Timestamp.ToString("O", CultureInfo.InvariantCulture)).ToArray());
            Gh2Data.SetTwig(access, 2, samples.Select(s => s.DurationHours).ToArray()); Gh2Data.SetTwig(access, 3, samples.Select(s => s.Weight).ToArray());
            access.SetItem(4, period.WeightedHours); access.SetItem(5, $"Start inclusive; End exclusive; interval centres; {samples.Count:N0} samples; {period.WeightedHours:G12} weighted hours.");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException) { access.AddError("Invalid period", exception.Message); }
    }
    internal static DateTimeOffset Parse(string value, string name)
    {
        if (!Regex.IsMatch(value, @"(?:Z|[+-]\d{2}:\d{2})\s*$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant) || !DateTimeOffset.TryParse(value, CultureInfo.InvariantCulture, DateTimeStyles.AllowWhiteSpaces | DateTimeStyles.RoundtripKind, out DateTimeOffset result)) throw new ArgumentException($"{name} must be ISO 8601 with explicit Z or ±HH:MM offset.");
        return result;
    }
}

/// <summary>Calculates source-aligned apparent solar positions with the Reda-Andreas SPA.</summary>
[IoId("66413441-dd80-4c05-aeb4-303c3ca086e1")]
public sealed class SunVectorsComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public SunVectorsComponent() : base(new Nomen("XVARNA Sun Vectors", "High-accuracy model-space solar vectors.", "XVARNA", "02 Time")) { }
    /// <summary>Deserializes the component.</summary>
    public SunVectorsComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Period", "P", "Optional XVARNA Period.", Access.Item, Requirement.MayBeMissing);
        inputs.AddText("Timestamps", "T", "Alternative explicit-offset timestamps.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Duration Hours", "D", "None, one, or one per timestamp.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Weights", "W", "None, one, or one per timestamp.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Latitude", "Lat", "WGS84 latitude degrees.").Set(0.0); inputs.AddNumber("Longitude", "Lon", "Degrees east of Greenwich.").Set(0.0);
        inputs.AddNumber("Elevation", "Elev", "Metres above sea level.").Set(0.0); inputs.AddNumber("True North", "N", "CCW degrees from model +Y.").Set(0.0);
        inputs.AddNumber("Minimum Altitude", "MinA", "Inactive below this apparent altitude.").Set(0.0); inputs.AddNumber("Delta T", "dT", "TT-UT1 seconds.").Set(69.0);
        inputs.AddNumber("Pressure", "Pr", "Millibars; zero disables refraction.").Set(1013.25); inputs.AddNumber("Temperature", "Ta", "Degrees Celsius.").Set(15.0);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Sun Set", "S", "Immutable ordered HVARE sun set."); outputs.AddVector("Vectors", "V", "Sensor-to-sun unit vectors.", Access.Twig);
        outputs.AddBoolean("Active", "A", "Altitude/weight activity.", Access.Twig); outputs.AddNumber("Altitude", "Alt", "Apparent altitude degrees.", Access.Twig);
        outputs.AddNumber("Azimuth", "Az", "Clockwise from true north.", Access.Twig); outputs.AddText("UTC", "UTC", "Normalized timestamps.", Access.Twig);
        outputs.AddText("Hash", "H", "Location/atmosphere/schedule identity."); outputs.AddText("Report", "R", "Method and assumptions.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        object? wrapped = null; string[] times = []; double[] durations = [], weights = [];
        if (!access.GetNull(0)) access.GetItem(0, out wrapped); access.GetItemArray(1, out times); access.GetItemArray(2, out durations); access.GetItemArray(3, out weights);
        access.GetItem(4, out double lat); access.GetItem(5, out double lon); access.GetItem(6, out double elev); access.GetItem(7, out double north);
        access.GetItem(8, out double minAlt); access.GetItem(9, out double deltaT); access.GetItem(10, out double pressure); access.GetItem(11, out double temperature);
        try
        {
            XvarnaSolarPeriod? period = wrapped as XvarnaSolarPeriod; if (period is not null && times.Length > 0) throw new ArgumentException("Connect Period or Timestamps, not both.");
            IReadOnlyList<SolarTimeSample> samples = period?.Samples ?? Manual(times, durations, weights);
            XvarnaSunSet sun = SolarEngine.Calculate(new(lat, lon, elev), samples, new SolarPositionOptions(deltaT, pressure, temperature, north, minAlt));
            access.SetItem(0, sun); Gh2Data.SetTwig(access, 1, sun.Samples.Select(s => new Vector3d(s.DirectionX, s.DirectionY, s.DirectionZ)).ToArray());
            Gh2Data.SetTwig(access, 2, sun.Samples.Select(s => s.IsActive).ToArray()); Gh2Data.SetTwig(access, 3, sun.Samples.Select(s => s.AltitudeDegrees).ToArray());
            Gh2Data.SetTwig(access, 4, sun.Samples.Select(s => s.AzimuthDegrees).ToArray()); Gh2Data.SetTwig(access, 5, sun.Samples.Select(s => s.TimestampUtc.ToString("O", CultureInfo.InvariantCulture)).ToArray());
            access.SetItem(6, sun.ContentHash); access.SetItem(7, $"Reda-Andreas SPA; {sun.Samples.Count:N0} samples, {sun.ActiveSampleCount:N0} active; north {north:G6} degrees; refraction {(sun.IncludesAtmosphericRefraction ? "on" : "off")}; hash {sun.ContentHash}");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException) { access.AddError("Solar position failed", exception.Message); }
    }
    private static IReadOnlyList<SolarTimeSample> Manual(IReadOnlyList<string> times, IReadOnlyList<double> durations, IReadOnlyList<double> weights)
    {
        if (times.Count == 0) throw new ArgumentException("Connect Period or at least one Timestamp.");
        if ((durations.Count is not 0 and not 1 && durations.Count != times.Count) || (weights.Count is not 0 and not 1 && weights.Count != times.Count)) throw new ArgumentException("Durations and Weights must contain none, one, or one per Timestamp.");
        return times.Select((t, i) => new SolarTimeSample(PeriodComponent.Parse(t, $"Timestamp {i}"), Select(durations, i, 1), Select(weights, i, 1))).ToArray();
    }
    private static double Select(IReadOnlyList<double> values, int index, double fallback) => values.Count switch { 0 => fallback, 1 => values[0], _ => values[index] };
}
