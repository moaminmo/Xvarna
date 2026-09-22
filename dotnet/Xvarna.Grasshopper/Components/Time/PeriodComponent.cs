using System.Drawing;
using System.Globalization;
using System.Text.RegularExpressions;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Time;

/// <summary>Builds a deterministic fixed-offset schedule of interval-centred samples.</summary>
public sealed partial class PeriodComponent : GH_Component
{
    private const int MaximumSampleCount = 1_000_000;

    /// <summary>Initializes the ZURVAN period component.</summary>
    public PeriodComponent()
        : base(
            "XVARNA Period",
            "XV Period",
            "Builds an explicit fixed-offset, interval-centred ZURVAN schedule for reproducible environmental studies.",
            "XVARNA",
            "02 Time")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("17918bd8-b275-4cad-a194-d4ae84e7839c");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddTextParameter("Start", "A", "Inclusive ISO 8601 boundary with explicit Z or ±HH:MM offset.", GH_ParamAccess.item, "2026-06-21T06:00:00+00:00");
        inputManager.AddTextParameter("End", "B", "Exclusive ISO 8601 boundary with the same explicit UTC offset as Start.", GH_ParamAccess.item, "2026-06-21T18:00:00+00:00");
        inputManager.AddNumberParameter("Step Minutes", "dt", "Positive interval length in minutes. The final interval is truncated at End when needed.", GH_ParamAccess.item, 60.0);
        inputManager.AddNumberParameter("Weight", "W", "Non-negative multiplier applied to every interval.", GH_ParamAccess.item, 1.0);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Period", "P", "Immutable ZURVAN schedule for XV Sun Vectors.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Timestamps", "T", "ISO 8601 interval-centre timestamps with explicit fixed offset.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Duration Hours", "D", "Actual interval durations in hours.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Weights", "W", "Per-interval weights.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Weighted Hours", "H", "Sum of duration multiplied by weight.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Schedule boundaries, convention, sample count, and modeled hours.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        string startText = string.Empty;
        string endText = string.Empty;
        double stepMinutes = 60.0;
        double weight = 1.0;
        if (!dataAccess.GetData(0, ref startText) || !dataAccess.GetData(1, ref endText))
        {
            return;
        }
        dataAccess.GetData(2, ref stepMinutes);
        dataAccess.GetData(3, ref weight);

        try
        {
            DateTimeOffset start = ParseExplicitOffset(startText, "Start");
            DateTimeOffset end = ParseExplicitOffset(endText, "End");
            if (start.Offset != end.Offset)
            {
                throw new ArgumentException("Start and End must use the same UTC offset. Split daylight-saving transitions into separate fixed-offset periods.");
            }
            if (end <= start)
            {
                throw new ArgumentException("End must be later than Start.");
            }
            if (!double.IsFinite(stepMinutes) || stepMinutes <= 0.0)
            {
                throw new ArgumentException("Step Minutes must be finite and greater than zero.");
            }
            if (!double.IsFinite(weight) || weight < 0.0)
            {
                throw new ArgumentException("Weight must be finite and non-negative.");
            }

            double stepSeconds = stepMinutes * 60.0;
            double estimatedCount = Math.Ceiling((end - start).TotalSeconds / stepSeconds);
            if (!double.IsFinite(estimatedCount) || estimatedCount > MaximumSampleCount)
            {
                throw new ArgumentException($"The period exceeds the {MaximumSampleCount:N0}-sample safety limit. Increase Step Minutes or split the study.");
            }

            List<SolarTimeSample> samples = new(checked((int)estimatedCount));
            DateTimeOffset cursor = start;
            while (cursor < end)
            {
                double remainingSeconds = (end - cursor).TotalSeconds;
                double intervalSeconds = Math.Min(stepSeconds, remainingSeconds);
                DateTimeOffset centre = cursor.AddSeconds(intervalSeconds * 0.5);
                samples.Add(new(centre, intervalSeconds / 3600.0, weight));
                cursor = cursor.AddSeconds(intervalSeconds);
            }

            XvarnaSolarPeriod period = new()
            {
                Samples = samples,
                Start = start,
                End = end,
                StepMinutes = stepMinutes,
            };
            string report = string.Create(
                CultureInfo.InvariantCulture,
                $"Engine: ZURVAN fixed-offset schedule{Environment.NewLine}" +
                $"Boundary convention: Start inclusive; End exclusive; timestamps at interval centres{Environment.NewLine}" +
                $"Period: {start:O} to {end:O}{Environment.NewLine}" +
                $"Samples: {samples.Count:N0}; requested step: {stepMinutes:G12} min{Environment.NewLine}" +
                $"Elapsed hours: {(end - start).TotalHours:G12}; weighted hours: {period.WeightedHours:G12}");

            dataAccess.SetData(0, period);
            dataAccess.SetDataList(1, samples.Select(sample => sample.Timestamp.ToString("O", CultureInfo.InvariantCulture)));
            dataAccess.SetDataList(2, samples.Select(sample => sample.DurationHours));
            dataAccess.SetDataList(3, samples.Select(sample => sample.Weight));
            dataAccess.SetData(4, period.WeightedHours);
            dataAccess.SetData(5, report);
        }
        catch (ArgumentException exception)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    internal static DateTimeOffset ParseExplicitOffset(string value, string parameterName)
    {
        if (!ExplicitOffsetRegex().IsMatch(value)
            || !DateTimeOffset.TryParse(
                value,
                CultureInfo.InvariantCulture,
                DateTimeStyles.AllowWhiteSpaces | DateTimeStyles.RoundtripKind,
                out DateTimeOffset parsed))
        {
            throw new ArgumentException($"{parameterName} must be ISO 8601 with an explicit Z or ±HH:MM UTC offset; received '{value}'.");
        }
        return parsed;
    }

    [GeneratedRegex(@"(?:Z|[+-]\d{2}:\d{2})\s*$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    private static partial Regex ExplicitOffsetRegex();
}
