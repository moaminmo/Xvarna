namespace Xvarna.Native;

/// <summary>An immutable, fixed-offset sequence of weighted solar intervals.</summary>
public sealed record XvarnaSolarPeriod
{
    /// <summary>Creates a period from validated, interval-centred samples.</summary>
    public required IReadOnlyList<SolarTimeSample> Samples { get; init; }

    /// <summary>Inclusive period boundary in its explicit fixed UTC offset.</summary>
    public required DateTimeOffset Start { get; init; }

    /// <summary>Exclusive period boundary in its explicit fixed UTC offset.</summary>
    public required DateTimeOffset End { get; init; }

    /// <summary>Requested step in minutes. A final truncated interval may be shorter.</summary>
    public required double StepMinutes { get; init; }

    /// <summary>Sum of duration multiplied by weight across all intervals.</summary>
    public double WeightedHours => Samples.Sum(sample => sample.DurationHours * sample.Weight);
}
