using System.Globalization;

namespace Xvarna.Native;

/// <summary>User-facing complexity exposed by the product experience.</summary>
public enum XvarnaExperienceMode
{
    /// <summary>Shows safe defaults and concise diagnostics.</summary>
    Basic,
    /// <summary>Exposes provenance, convergence, and resource policy.</summary>
    Expert,
}

/// <summary>Named analysis-quality policy.</summary>
public enum XvarnaQualityPreset
{
    /// <summary>Fast interactive design feedback.</summary>
    Preview,
    /// <summary>Default design-iteration balance.</summary>
    Balanced,
    /// <summary>High-resolution decision support.</summary>
    High,
    /// <summary>Publication-oriented sampling and strict convergence.</summary>
    Publication,
}

/// <summary>When Grasshopper analyses are allowed to launch.</summary>
public enum XvarnaExecutionMode
{
    /// <summary>Recompute after input changes, subject to debounce.</summary>
    Live,
    /// <summary>Recompute only after the shared run generation changes.</summary>
    Manual,
}

/// <summary>Validated document-wide quality and execution contract.</summary>
public sealed record AnalysisQualityProfile
{
    /// <summary>Selected named policy.</summary>
    public required XvarnaQualityPreset Preset { get; init; }
    /// <summary>Basic or Expert product surface.</summary>
    public required XvarnaExperienceMode Experience { get; init; }
    /// <summary>Live or manual execution.</summary>
    public required XvarnaExecutionMode Execution { get; init; }
    /// <summary>Multiplier applied by quality-aware samplers.</summary>
    public required double SampleMultiplier { get; init; }
    /// <summary>Absolute convergence target for dimensionless metrics.</summary>
    public required double ConvergenceTolerance { get; init; }
    /// <summary>Maximum delay before scheduling live work.</summary>
    public required int DebounceMilliseconds { get; init; }
    /// <summary>Whether the last successful result may remain visible with an explicit stale label.</summary>
    public required bool RetainStaleResult { get; init; }
    /// <summary>Whether result reports must expose an uncertainty/convergence statement.</summary>
    public required bool RequireUncertaintyLabel { get; init; }
    /// <summary>Monotonic shared manual-run generation.</summary>
    public required long RunGeneration { get; init; }

    /// <summary>Creates a validated preset with product defaults.</summary>
    public static AnalysisQualityProfile Create(
        XvarnaQualityPreset preset,
        XvarnaExperienceMode experience = XvarnaExperienceMode.Basic,
        XvarnaExecutionMode execution = XvarnaExecutionMode.Live,
        int? debounceMilliseconds = null,
        bool retainStaleResult = true,
        bool requireUncertaintyLabel = true,
        long runGeneration = 0)
    {
        (double multiplier, double tolerance, int debounce) = preset switch
        {
            XvarnaQualityPreset.Preview => (0.25, 0.05, 300),
            XvarnaQualityPreset.Balanced => (1.0, 0.01, 150),
            XvarnaQualityPreset.High => (2.0, 0.0025, 50),
            XvarnaQualityPreset.Publication => (4.0, 0.0005, 0),
            _ => throw new ArgumentOutOfRangeException(nameof(preset)),
        };
        int resolvedDebounce = debounceMilliseconds ?? debounce;
        if (resolvedDebounce is < 0 or > 60_000)
        {
            throw new ArgumentOutOfRangeException(nameof(debounceMilliseconds), "Debounce must be between 0 and 60000 milliseconds.");
        }
        if (runGeneration < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(runGeneration));
        }
        return new()
        {
            Preset = preset,
            Experience = experience,
            Execution = execution,
            SampleMultiplier = multiplier,
            ConvergenceTolerance = tolerance,
            DebounceMilliseconds = resolvedDebounce,
            RetainStaleResult = retainStaleResult,
            RequireUncertaintyLabel = requireUncertaintyLabel,
            RunGeneration = runGeneration,
        };
    }

    /// <summary>Scales a baseline sample count with deterministic midpoint-away rounding and hard limits.</summary>
    public int ScaleSamples(int baseline, int minimum, int maximum)
    {
        if (baseline <= 0 || minimum <= 0 || maximum < minimum)
        {
            throw new ArgumentOutOfRangeException(nameof(baseline), "Sample bounds must be positive and ordered.");
        }
        double scaled = Math.Round(baseline * SampleMultiplier, MidpointRounding.AwayFromZero);
        return checked((int)Math.Clamp(scaled, minimum, maximum));
    }

    /// <summary>Human-readable policy and scientific-use boundary.</summary>
    public string Report => string.Create(
        CultureInfo.InvariantCulture,
        $"Quality: {Preset}; experience: {Experience}; execution: {Execution}{Environment.NewLine}" +
        $"Sampling multiplier: {SampleMultiplier:G6}; convergence target: {ConvergenceTolerance:G6}{Environment.NewLine}" +
        $"Debounce: {DebounceMilliseconds:N0} ms; retain labelled stale result: {RetainStaleResult}; uncertainty label required: {RequireUncertaintyLabel}{Environment.NewLine}" +
        $"Manual run generation: {RunGeneration:N0}");
}

/// <summary>Explicit model-to-SI conversion and tolerance provenance.</summary>
public sealed record XvarnaUnitContext
{
    /// <summary>Human-readable source unit.</summary>
    public required string ModelUnitName { get; init; }
    /// <summary>Metres represented by one model unit.</summary>
    public required double MetersPerModelUnit { get; init; }
    /// <summary>Absolute model tolerance in model units.</summary>
    public required double AbsoluteToleranceModelUnits { get; init; }
    /// <summary>Absolute model tolerance in metres.</summary>
    public double AbsoluteToleranceMeters => AbsoluteToleranceModelUnits * MetersPerModelUnit;

    /// <summary>Creates a finite positive unit contract.</summary>
    public static XvarnaUnitContext Create(string modelUnitName, double metersPerModelUnit, double absoluteToleranceModelUnits)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(modelUnitName);
        if (!double.IsFinite(metersPerModelUnit) || metersPerModelUnit <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(metersPerModelUnit));
        }
        if (!double.IsFinite(absoluteToleranceModelUnits) || absoluteToleranceModelUnits <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(absoluteToleranceModelUnits));
        }
        return new()
        {
            ModelUnitName = modelUnitName.Trim(),
            MetersPerModelUnit = metersPerModelUnit,
            AbsoluteToleranceModelUnits = absoluteToleranceModelUnits,
        };
    }

    /// <summary>Complete source/SI conversion statement.</summary>
    public string Report => string.Create(
        CultureInfo.InvariantCulture,
        $"Source units: {ModelUnitName}; 1 model unit = {MetersPerModelUnit:G17} m{Environment.NewLine}Absolute tolerance: {AbsoluteToleranceModelUnits:G17} model units = {AbsoluteToleranceMeters:G17} m{Environment.NewLine}Canonical engine geometry: f64 metres; outputs are converted back to source model units.");
}

/// <summary>Evidence strength attached to one approximate result.</summary>
public enum XvarnaEvidenceLevel
{
    /// <summary>No numerical uncertainty evidence was supplied.</summary>
    Unlabelled,
    /// <summary>Method and limitations are stated but no convergence test passed.</summary>
    MethodDeclared,
    /// <summary>A nested-sample convergence check satisfies the selected threshold.</summary>
    Converged,
    /// <summary>An external reference comparison satisfies a declared tolerance.</summary>
    ReferenceValidated,
}

/// <summary>Machine-readable uncertainty and validation label.</summary>
public sealed record XvarnaEvidenceLabel
{
    /// <summary>Evidence classification.</summary>
    public required XvarnaEvidenceLevel Level { get; init; }
    /// <summary>Metric or method to which the label applies.</summary>
    public required string Method { get; init; }
    /// <summary>Full versus nested/coarser estimate delta.</summary>
    public required double AbsoluteDelta { get; init; }
    /// <summary>Acceptance threshold.</summary>
    public required double Threshold { get; init; }
    /// <summary>Full sample count.</summary>
    public required ulong SampleCount { get; init; }
    /// <summary>Concise interpretation safe for reports.</summary>
    public required string Statement { get; init; }

    /// <summary>Builds a nested-sample convergence label.</summary>
    public static XvarnaEvidenceLabel Convergence(string method, double full, double nested, ulong sampleCount, double threshold)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(method);
        if (!double.IsFinite(full) || !double.IsFinite(nested) || !double.IsFinite(threshold) || threshold < 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(threshold));
        }
        double delta = Math.Abs(full - nested);
        bool accepted = delta <= threshold;
        return new()
        {
            Level = accepted ? XvarnaEvidenceLevel.Converged : XvarnaEvidenceLevel.MethodDeclared,
            Method = method.Trim(),
            AbsoluteDelta = delta,
            Threshold = threshold,
            SampleCount = sampleCount,
            Statement = accepted
                ? $"Nested-sample convergence passed (|Δ|={delta:G6} ≤ {threshold:G6})."
                : $"Nested-sample convergence not reached (|Δ|={delta:G6} > {threshold:G6}); increase quality or report sensitivity.",
        };
    }
}

/// <summary>Legend-domain selection.</summary>
public enum XvarnaLegendDomainMode
{
    /// <summary>Finite minimum and maximum.</summary>
    FullRange,
    /// <summary>Lower and upper quantiles suppress display outliers.</summary>
    Percentile,
    /// <summary>Symmetric domain around zero.</summary>
    Symmetric,
    /// <summary>Caller-provided bounds.</summary>
    Fixed,
}

/// <summary>Colour map used by the unified legend.</summary>
public enum XvarnaPalette
{
    /// <summary>Perceptually uniform Viridis.</summary>
    Viridis,
    /// <summary>Colour-vision-deficiency-aware Cividis.</summary>
    Cividis,
    /// <summary>Blue-white-red diverging map.</summary>
    Diverging,
}

/// <summary>Portable opaque colour value.</summary>
public readonly record struct XvarnaColor(byte Red, byte Green, byte Blue, byte Alpha = 255);

/// <summary>Inputs controlling a unified deterministic legend.</summary>
public readonly record struct XvarnaLegendOptions(
    XvarnaLegendDomainMode DomainMode = XvarnaLegendDomainMode.FullRange,
    XvarnaPalette Palette = XvarnaPalette.Viridis,
    double Minimum = 0.0,
    double Maximum = 1.0,
    double LowerPercentile = 0.02,
    double UpperPercentile = 0.98,
    int TickCount = 5);

/// <summary>Finite-domain normalized values, colours, ticks, and clipping diagnostics.</summary>
public sealed record XvarnaLegendResult
{
    /// <summary>Resolved display lower bound.</summary>
    public required double Minimum { get; init; }
    /// <summary>Resolved display upper bound.</summary>
    public required double Maximum { get; init; }
    /// <summary>One normalized value per source value; non-finite entries are NaN.</summary>
    public required IReadOnlyList<double> NormalizedValues { get; init; }
    /// <summary>One colour per source value; non-finite entries are transparent.</summary>
    public required IReadOnlyList<XvarnaColor> Colors { get; init; }
    /// <summary>Ordered numeric tick values.</summary>
    public required IReadOnlyList<double> Ticks { get; init; }
    /// <summary>Count below the resolved domain.</summary>
    public required int ClippedLowCount { get; init; }
    /// <summary>Count above the resolved domain.</summary>
    public required int ClippedHighCount { get; init; }
    /// <summary>Count of NaN or infinite entries.</summary>
    public required int NonFiniteCount { get; init; }
}

/// <summary>Shared colour/domain implementation for every product visualization.</summary>
public static class XvarnaLegend
{
    private static readonly (double Position, XvarnaColor Color)[] Viridis =
    [
        (0.00, new(68, 1, 84)), (0.25, new(59, 82, 139)),
        (0.50, new(33, 145, 140)), (0.75, new(94, 201, 98)), (1.00, new(253, 231, 37)),
    ];
    private static readonly (double Position, XvarnaColor Color)[] Cividis =
    [
        (0.00, new(0, 32, 77)), (0.25, new(66, 78, 108)),
        (0.50, new(124, 124, 120)), (0.75, new(188, 173, 108)), (1.00, new(255, 233, 69)),
    ];
    private static readonly (double Position, XvarnaColor Color)[] Diverging =
    [
        (0.00, new(33, 102, 172)), (0.50, new(247, 247, 247)), (1.00, new(178, 24, 43)),
    ];

    /// <summary>Resolves a finite domain and maps every input without changing its order.</summary>
    public static XvarnaLegendResult Build(IReadOnlyList<double> values, XvarnaLegendOptions options)
    {
        ArgumentNullException.ThrowIfNull(values);
        if (options.TickCount is < 2 or > 64
            || !double.IsFinite(options.LowerPercentile)
            || !double.IsFinite(options.UpperPercentile)
            || options.LowerPercentile < 0.0
            || options.UpperPercentile > 1.0
            || options.LowerPercentile >= options.UpperPercentile)
        {
            throw new ArgumentOutOfRangeException(nameof(options));
        }
        double[] finite = values.Where(double.IsFinite).Order().ToArray();
        if (finite.Length == 0)
        {
            throw new ArgumentException("Legend requires at least one finite value.", nameof(values));
        }
        (double minimum, double maximum) = options.DomainMode switch
        {
            XvarnaLegendDomainMode.FullRange => (finite[0], finite[^1]),
            XvarnaLegendDomainMode.Percentile =>
                (Quantile(finite, options.LowerPercentile), Quantile(finite, options.UpperPercentile)),
            XvarnaLegendDomainMode.Symmetric => Symmetric(finite),
            XvarnaLegendDomainMode.Fixed when double.IsFinite(options.Minimum)
                && double.IsFinite(options.Maximum) && options.Maximum > options.Minimum =>
                (options.Minimum, options.Maximum),
            XvarnaLegendDomainMode.Fixed => throw new ArgumentOutOfRangeException(nameof(options), "Fixed legend bounds must be finite and increasing."),
            _ => throw new ArgumentOutOfRangeException(nameof(options)),
        };
        if (!(maximum > minimum))
        {
            double padding = Math.Max(Math.Abs(minimum) * 1.0e-9, 1.0e-12);
            minimum -= padding;
            maximum += padding;
        }
        int low = 0;
        int high = 0;
        int nonFinite = 0;
        double range = maximum - minimum;
        List<double> normalized = new(values.Count);
        List<XvarnaColor> colors = new(values.Count);
        foreach (double value in values)
        {
            if (!double.IsFinite(value))
            {
                nonFinite++;
                normalized.Add(double.NaN);
                colors.Add(new(0, 0, 0, 0));
                continue;
            }
            if (value < minimum) low++;
            if (value > maximum) high++;
            double position = Math.Clamp((value - minimum) / range, 0.0, 1.0);
            normalized.Add(position);
            colors.Add(Map(position, options.Palette));
        }
        double[] ticks = Enumerable.Range(0, options.TickCount)
            .Select(index => minimum + range * index / (options.TickCount - 1.0))
            .ToArray();
        return new()
        {
            Minimum = minimum,
            Maximum = maximum,
            NormalizedValues = normalized,
            Colors = colors,
            Ticks = ticks,
            ClippedLowCount = low,
            ClippedHighCount = high,
            NonFiniteCount = nonFinite,
        };
    }

    /// <summary>Maps one normalized value through the shared palette.</summary>
    public static XvarnaColor Color(double normalizedValue, XvarnaPalette palette) =>
        Map(Math.Clamp(double.IsFinite(normalizedValue) ? normalizedValue : 0.0, 0.0, 1.0), palette);

    private static (double Minimum, double Maximum) Symmetric(IReadOnlyList<double> values)
    {
        double magnitude = Math.Max(Math.Abs(values[0]), Math.Abs(values[^1]));
        return (-magnitude, magnitude);
    }

    private static double Quantile(IReadOnlyList<double> sorted, double probability)
    {
        double position = probability * (sorted.Count - 1);
        int lower = checked((int)Math.Floor(position));
        int upper = checked((int)Math.Ceiling(position));
        double fraction = position - lower;
        return sorted[lower] + (sorted[upper] - sorted[lower]) * fraction;
    }

    private static XvarnaColor Map(double value, XvarnaPalette palette)
    {
        (double Position, XvarnaColor Color)[] anchors = palette switch
        {
            XvarnaPalette.Viridis => Viridis,
            XvarnaPalette.Cividis => Cividis,
            XvarnaPalette.Diverging => Diverging,
            _ => throw new ArgumentOutOfRangeException(nameof(palette)),
        };
        for (int index = 1; index < anchors.Length; index++)
        {
            if (value <= anchors[index].Position)
            {
                (double start, XvarnaColor a) = anchors[index - 1];
                (double end, XvarnaColor b) = anchors[index];
                double fraction = (value - start) / (end - start);
                return new(
                    Channel(a.Red, b.Red, fraction),
                    Channel(a.Green, b.Green, fraction),
                    Channel(a.Blue, b.Blue, fraction));
            }
        }
        return anchors[^1].Color;
    }

    private static byte Channel(byte start, byte end, double fraction) => checked((byte)Math.Clamp(
        Math.Round(start + (end - start) * fraction, MidpointRounding.AwayFromZero), byte.MinValue, byte.MaxValue));
}
