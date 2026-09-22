using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Configures document-wide quality, complexity, execution, and stale-result policy.</summary>
public sealed class QualityComponent : GH_Component
{
    private bool previousRun;
    private long runGeneration;

    /// <summary>Initializes the product-quality component.</summary>
    public QualityComponent() : base(
        "XVARNA Quality", "XV Quality",
        "Controls document-wide Preview/Balanced/High/Publication sampling, Basic/Expert UX, live/manual execution, debounce, uncertainty labels, and stale-result policy.",
        "XVARNA", "00 Setup")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("6aeeb503-ea7b-4fe1-bdf9-ec3df8d8641e");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Preset", "Q", "Preview, Balanced, High, or Publication.", GH_ParamAccess.item, "Balanced");
        input.AddTextParameter("Experience", "UX", "Basic or Expert.", GH_ParamAccess.item, "Basic");
        input.AddTextParameter("Execution", "Mode", "Live or Manual.", GH_ParamAccess.item, "Live");
        input.AddBooleanParameter("Run", "Run", "In Manual mode, a false-to-true pulse launches one new shared generation.", GH_ParamAccess.item, false);
        input.AddIntegerParameter("Debounce ms", "ms", "-1 uses the preset; otherwise 0..60000 milliseconds.", GH_ParamAccess.item, -1);
        input.AddBooleanParameter("Retain Stale", "Stale", "Keep the last successful viewport/result after cancellation or failure, always labelled STALE.", GH_ParamAccess.item, true);
        input.AddBooleanParameter("Require Evidence", "Evidence", "Require convergence/uncertainty statements in scientific reports.", GH_ParamAccess.item, true);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Quality", "Q", "Validated document-wide quality profile.", GH_ParamAccess.item);
        output.AddNumberParameter("Sample Multiplier", "×N", "Multiplier used by quality-aware samplers.", GH_ParamAccess.item);
        output.AddNumberParameter("Convergence", "Δ", "Dimensionless target for nested-sample convergence.", GH_ParamAccess.item);
        output.AddIntegerParameter("Run Generation", "Gen", "Monotonic manual execution generation.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Info", "Complete quality, execution, and evidence policy.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        string preset = "Balanced", experience = "Basic", execution = "Live";
        bool run = false, retain = true, evidence = true;
        int debounce = -1;
        data.GetData(0, ref preset); data.GetData(1, ref experience); data.GetData(2, ref execution);
        data.GetData(3, ref run); data.GetData(4, ref debounce); data.GetData(5, ref retain); data.GetData(6, ref evidence);
        try
        {
            XvarnaExecutionMode executionMode = ParseExecution(execution);
            if (executionMode == XvarnaExecutionMode.Manual && run && !previousRun)
            {
                runGeneration = checked(runGeneration + 1);
            }
            previousRun = run;
            AnalysisQualityProfile profile = AnalysisQualityProfile.Create(
                ParsePreset(preset), ParseExperience(experience), executionMode,
                debounce < 0 ? null : debounce, retain, evidence, runGeneration);
            ProductContextRegistry.SetQuality(OnPingDocument(), InstanceGuid, profile);
            data.SetData(0, profile); data.SetData(1, profile.SampleMultiplier);
            data.SetData(2, profile.ConvergenceTolerance);
            data.SetData(3, checked((int)Math.Min(profile.RunGeneration, int.MaxValue)));
            data.SetData(4, profile.Report);
            Message = $"{profile.Preset}\n{profile.Execution} · {profile.Experience}";
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        {
            previousRun = run;
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static XvarnaQualityPreset ParsePreset(string value) => value.Trim().ToUpperInvariant() switch
    {
        "PREVIEW" or "DRAFT" => XvarnaQualityPreset.Preview,
        "BALANCED" or "STANDARD" => XvarnaQualityPreset.Balanced,
        "HIGH" => XvarnaQualityPreset.High,
        "PUBLICATION" or "PUBLISH" => XvarnaQualityPreset.Publication,
        _ => throw new ArgumentException("Preset must be Preview, Balanced, High, or Publication."),
    };

    private static XvarnaExperienceMode ParseExperience(string value) => value.Trim().ToUpperInvariant() switch
    {
        "BASIC" => XvarnaExperienceMode.Basic,
        "EXPERT" => XvarnaExperienceMode.Expert,
        _ => throw new ArgumentException("Experience must be Basic or Expert."),
    };

    private static XvarnaExecutionMode ParseExecution(string value) => value.Trim().ToUpperInvariant() switch
    {
        "LIVE" or "AUTO" => XvarnaExecutionMode.Live,
        "MANUAL" => XvarnaExecutionMode.Manual,
        _ => throw new ArgumentException("Execution must be Live or Manual."),
    };
}

/// <summary>Declares one authoritative Rhino-model-to-SI conversion contract.</summary>
public sealed class UnitsComponent : GH_Component
{
    /// <summary>Initializes the units component.</summary>
    public UnitsComponent() : base(
        "XVARNA Units", "XV Units",
        "Publishes the authoritative model-unit, metre scale, and absolute-tolerance provenance used by XVARNA scene construction.",
        "XVARNA", "00 Setup")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("b121be1d-f579-42f6-b91c-311604522fb4");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Unit Name", "Unit", "Empty uses the active Rhino document unit name.", GH_ParamAccess.item, string.Empty);
        input.AddNumberParameter("Meters per Unit", "m/U", "0 uses Rhino's exact active-document conversion.", GH_ParamAccess.item, 0.0);
        input.AddNumberParameter("Absolute Tolerance", "Tol", "0 uses the active Rhino document tolerance in model units.", GH_ParamAccess.item, 0.0);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Units", "U", "Validated unit context.", GH_ParamAccess.item);
        output.AddNumberParameter("Meters per Unit", "m/U", "Metres represented by one model unit.", GH_ParamAccess.item);
        output.AddNumberParameter("Tolerance m", "Tol m", "Absolute tolerance in metres.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Info", "Canonical/source conversion and tolerance statement.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        string name = string.Empty;
        double scale = 0.0, tolerance = 0.0;
        data.GetData(0, ref name); data.GetData(1, ref scale); data.GetData(2, ref tolerance);
        try
        {
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            UnitSystem system = document?.ModelUnitSystem ?? UnitSystem.Meters;
            string resolvedName = string.IsNullOrWhiteSpace(name) ? system.ToString() : name.Trim();
            double resolvedScale = scale > 0.0 ? scale : RhinoMath.UnitScale(system, UnitSystem.Meters);
            double resolvedTolerance = tolerance > 0.0 ? tolerance : document?.ModelAbsoluteTolerance ?? 1.0e-6;
            XvarnaUnitContext units = XvarnaUnitContext.Create(resolvedName, resolvedScale, resolvedTolerance);
            ProductContextRegistry.SetUnits(OnPingDocument(), InstanceGuid, units);
            data.SetData(0, units); data.SetData(1, units.MetersPerModelUnit);
            data.SetData(2, units.AbsoluteToleranceMeters); data.SetData(3, units.Report);
            Message = $"{units.ModelUnitName}\n1 U = {units.MetersPerModelUnit:G4} m";
            if (scale > 0.0 && document is not null
                && Math.Abs(scale - RhinoMath.UnitScale(system, UnitSystem.Meters)) > Math.Abs(scale) * 1.0e-12)
            {
                AddRuntimeMessage(GH_RuntimeMessageLevel.Warning,
                    "The explicit unit scale differs from the active Rhino document. This is allowed but must be intentional and reported.");
            }
        }
        catch (ArgumentException exception)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }
}

/// <summary>Aggregates product, scheduler, cache, backend, and evidence health.</summary>
public sealed class DiagnosticsComponent : GH_Component
{
    /// <summary>Initializes the diagnostics component.</summary>
    public DiagnosticsComponent() : base(
        "XVARNA Diagnostics", "XV Diagnostics",
        "Runs a document-wide preflight over quality, units, scheduler, cache, GPU health, fallbacks, failures, and recent analysis events.",
        "XVARNA", "00 Setup")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("e402d486-edba-43cf-a1fb-994534793c36");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Backend", "B", "Optional XV Backend session for device and transfer health.", GH_ParamAccess.item);
        input.AddTextParameter("Bundle Directory", "Dir", "Directory for an optional redacted diagnostic ZIP.", GH_ParamAccess.item);
        input.AddBooleanParameter("Write Bundle", "W", "Capture hardware, versions, cache, scheduler, recent events and errors.", GH_ParamAccess.item, false);
        input.AddTextParameter("Error Chain", "Err", "Optional error/details; user names and profile paths are redacted.", GH_ParamAccess.item, string.Empty);
        input[0].Optional = true; input[1].Optional = true; input[3].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddBooleanParameter("Healthy", "OK", "True when no blocking preflight issue exists.", GH_ParamAccess.item);
        output.AddTextParameter("Severity", "Level", "OK, NOTICE, WARNING, or ERROR.", GH_ParamAccess.item);
        output.AddTextParameter("Issues", "Issues", "Actionable preflight findings.", GH_ParamAccess.list);
        output.AddTextParameter("Active Jobs", "Jobs", "Current bounded-scheduler jobs and progress.", GH_ParamAccess.list);
        output.AddTextParameter("Recent Events", "Events", "Recent document-scoped XVARNA execution events.", GH_ParamAccess.list);
        output.AddTextParameter("Report", "Info", "Shareable diagnostic preflight report.", GH_ParamAccess.item);
        output.AddTextParameter("Diagnostic Bundle", "ZIP", "Redacted support archive path when Write Bundle is true.", GH_ParamAccess.item);
        output.AddTextParameter("Bundle Hash", "H", "SHA-256 diagnostic-manifest identity.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        object? wrapped = null; string directory = string.Empty, errorChain = string.Empty; bool writeBundle = false;
        data.GetData(0, ref wrapped); data.GetData(1, ref directory); data.GetData(2, ref writeBundle); data.GetData(3, ref errorChain);
        XvarnaComputeSession? backend = wrapped switch
        {
            XvarnaComputeSession session => session,
            GH_ObjectWrapper { Value: XvarnaComputeSession session } => session,
            _ => null,
        };
        try
        {
            ProductContextSnapshot context = ProductContextRegistry.Snapshot(OnPingDocument());
            AnalysisSchedulerStatistics scheduler = XvarnaAnalysisScheduler.GetStatistics();
            CacheStatistics cache = XvarnaCache.GetStatistics();
            ComputeRuntimeStatistics? runtime = backend?.GetRuntimeStatistics();
            List<string> issues = [];
            if (!context.QualityConfigured) issues.Add("NOTICE: XV Quality is absent; Balanced/Basic/Live defaults are active.");
            if (!context.UnitsConfigured) issues.Add("NOTICE: XV Units is absent; active Rhino document units are inferred.");
            if (scheduler.FailedJobs > 0) issues.Add($"WARNING: scheduler has recorded {scheduler.FailedJobs:N0} failed job(s).");
            if (scheduler.StaleJobs > 0) issues.Add($"NOTICE: scheduler suppressed {scheduler.StaleJobs:N0} stale generation(s).");
            if (cache.CorruptionCount > 0) issues.Add($"WARNING: cache recovered {cache.CorruptionCount:N0} corrupt entry/entries.");
            if (runtime is not null && !runtime.DeviceHealthy) issues.Add($"ERROR: GPU device is unhealthy: {runtime.LastError}");
            if (runtime is not null && runtime.RuntimeFallbackCount > 0) issues.Add($"WARNING: {runtime.RuntimeFallbackCount:N0} GPU batch(es) fell back to CPU.");
            string severity = issues.Any(value => value.StartsWith("ERROR", StringComparison.Ordinal)) ? "ERROR"
                : issues.Any(value => value.StartsWith("WARNING", StringComparison.Ordinal)) ? "WARNING"
                : issues.Count > 0 ? "NOTICE" : "OK";
            bool healthy = severity != "ERROR";
            IReadOnlyList<AnalysisProgress> jobs = XvarnaAnalysisScheduler.GetActiveJobs();
            string[] jobLines = jobs.Select(job => string.Create(CultureInfo.InvariantCulture,
                $"{job.Name} | {job.State} | {job.Fraction:P1} | {job.Phase}")).ToArray();
            string report = string.Create(CultureInfo.InvariantCulture,
                $"XVARNA preflight: {severity}; generated {DateTimeOffset.UtcNow:O}{Environment.NewLine}" +
                $"Quality: {context.Quality.Preset}/{context.Quality.Experience}/{context.Quality.Execution}; evidence required {context.Quality.RequireUncertaintyLabel}{Environment.NewLine}" +
                $"Units: {context.Units.ModelUnitName}; {context.Units.MetersPerModelUnit:G17} m/unit; tolerance {context.Units.AbsoluteToleranceMeters:G8} m{Environment.NewLine}" +
                $"Scheduler: {scheduler.RunningJobs:N0} running, {scheduler.QueuedJobs:N0} queued, {scheduler.CompletedJobs:N0} completed, {scheduler.FailedJobs:N0} failed, {scheduler.StaleJobs:N0} stale{Environment.NewLine}" +
                $"Cache: {cache.MemoryHitCount + cache.DiskHitCount:N0} hits, {cache.MissCount:N0} misses, {cache.EvictionCount:N0} evictions, {cache.CorruptionCount:N0} recoveries{Environment.NewLine}" +
                $"Backend: {(runtime is null ? "not connected" : $"healthy {runtime.DeviceHealthy}; fallbacks {runtime.RuntimeFallbackCount}; losses {runtime.DeviceLossCount}")}{Environment.NewLine}" +
                $"Findings: {(issues.Count == 0 ? "none" : string.Join(" | ", issues))}");
            data.SetData(0, healthy); data.SetData(1, severity); data.SetDataList(2, issues);
            data.SetDataList(3, jobLines); data.SetDataList(4, context.RecentEvents); data.SetData(5, report);
            if (writeBundle)
            {
                DiagnosticBundleResult bundle = XvarnaDiagnosticBundle.Create(directory, context.RecentEvents, errorChain);
                data.SetData(6, bundle.ArchivePath); data.SetData(7, bundle.ContentHash);
            }
            else { data.SetData(6, string.Empty); data.SetData(7, string.Empty); }
            Message = $"PREFLIGHT\n{severity}";
            if (severity == "ERROR") AddRuntimeMessage(GH_RuntimeMessageLevel.Error, issues.First(value => value.StartsWith("ERROR", StringComparison.Ordinal)));
            else if (severity == "WARNING") AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, "Preflight found warnings; inspect Issues.");
        }
        catch (Exception exception) when (exception is XvarnaNativeException or ObjectDisposedException or InvalidOperationException
            or ArgumentException or IOException or UnauthorizedAccessException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }
}

/// <summary>Builds a reusable, colourblind-aware numerical legend contract.</summary>
public sealed class LegendComponent : GH_Component
{
    /// <summary>Initializes the unified legend component.</summary>
    public LegendComponent() : base(
        "XVARNA Legend", "XV Legend",
        "Creates one deterministic Full/Percentile/Symmetric/Fixed domain, normalized values, colourblind-aware colours, ticks, clipping counts, and provenance.",
        "XVARNA", "06 Display")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("03011a61-e74b-41d9-bfbf-4989f1fb4aa3");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddNumberParameter("Values", "V", "Values to map without reordering.", GH_ParamAccess.list);
        input.AddTextParameter("Domain", "D", "Full, Percentile, Symmetric, or Fixed.", GH_ParamAccess.item, "Full");
        input.AddTextParameter("Palette", "P", "Viridis, Cividis, or Diverging.", GH_ParamAccess.item, "Viridis");
        input.AddNumberParameter("Minimum", "Min", "Fixed minimum or lower percentile in 0..1.", GH_ParamAccess.item, 0.02);
        input.AddNumberParameter("Maximum", "Max", "Fixed maximum or upper percentile in 0..1.", GH_ParamAccess.item, 0.98);
        input.AddIntegerParameter("Ticks", "N", "Tick count from 2 through 64.", GH_ParamAccess.item, 5);
        input.AddTextParameter("Unit", "U", "Optional label unit.", GH_ParamAccess.item, string.Empty);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Legend", "L", "Portable resolved legend result.", GH_ParamAccess.item);
        output.AddColourParameter("Colors", "C", "One colour per input; non-finite values are transparent.", GH_ParamAccess.list);
        output.AddNumberParameter("Normalized", "t", "Clamped 0..1 positions; non-finite values remain NaN.", GH_ParamAccess.list);
        output.AddNumberParameter("Domain", "D", "Resolved minimum and maximum.", GH_ParamAccess.list);
        output.AddNumberParameter("Tick Values", "TV", "Ordered numeric ticks.", GH_ParamAccess.list);
        output.AddTextParameter("Tick Labels", "TL", "Invariant numeric labels with optional unit.", GH_ParamAccess.list);
        output.AddTextParameter("Report", "Info", "Domain, palette, clipping, and missing-value provenance.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<double> values = [];
        string domain = "Full", palette = "Viridis", unit = string.Empty;
        double minimum = 0.02, maximum = 0.98;
        int ticks = 5;
        if (!data.GetDataList(0, values) || values.Count == 0) return;
        data.GetData(1, ref domain); data.GetData(2, ref palette); data.GetData(3, ref minimum);
        data.GetData(4, ref maximum); data.GetData(5, ref ticks); data.GetData(6, ref unit);
        try
        {
            XvarnaLegendDomainMode mode = ParseDomain(domain);
            XvarnaLegendOptions options = mode == XvarnaLegendDomainMode.Percentile
                ? new(mode, ParsePalette(palette), LowerPercentile: minimum, UpperPercentile: maximum, TickCount: ticks)
                : new(mode, ParsePalette(palette), Minimum: minimum, Maximum: maximum, TickCount: ticks);
            XvarnaLegendResult result = XvarnaLegend.Build(values, options);
            Color[] colors = result.Colors.Select(value => Color.FromArgb(value.Alpha, value.Red, value.Green, value.Blue)).ToArray();
            string suffix = string.IsNullOrWhiteSpace(unit) ? string.Empty : $" {unit.Trim()}";
            string[] labels = result.Ticks.Select(value => value.ToString("G6", CultureInfo.InvariantCulture) + suffix).ToArray();
            string report = string.Create(CultureInfo.InvariantCulture,
                $"Legend: {mode}; palette {options.Palette}; finite domain [{result.Minimum:G17}, {result.Maximum:G17}]{Environment.NewLine}Values: {values.Count:N0}; clipped low {result.ClippedLowCount:N0}; clipped high {result.ClippedHighCount:N0}; non-finite {result.NonFiniteCount:N0}{Environment.NewLine}Colour maps are monotonic and colour-vision-deficiency-aware; geometry/value ordering is preserved.");
            data.SetData(0, result); data.SetDataList(1, colors); data.SetDataList(2, result.NormalizedValues);
            data.SetDataList(3, new[] { result.Minimum, result.Maximum }); data.SetDataList(4, result.Ticks);
            data.SetDataList(5, labels); data.SetData(6, report);
            Message = $"{options.Palette}\n{result.Minimum:G3} … {result.Maximum:G3}";
        }
        catch (ArgumentException exception)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static XvarnaLegendDomainMode ParseDomain(string value) => value.Trim().ToUpperInvariant() switch
    {
        "FULL" or "RANGE" or "FULLRANGE" => XvarnaLegendDomainMode.FullRange,
        "PERCENTILE" or "ROBUST" => XvarnaLegendDomainMode.Percentile,
        "SYMMETRIC" or "DIVERGING" => XvarnaLegendDomainMode.Symmetric,
        "FIXED" => XvarnaLegendDomainMode.Fixed,
        _ => throw new ArgumentException("Domain must be Full, Percentile, Symmetric, or Fixed."),
    };

    private static XvarnaPalette ParsePalette(string value) => value.Trim().ToUpperInvariant() switch
    {
        "VIRIDIS" => XvarnaPalette.Viridis,
        "CIVIDIS" => XvarnaPalette.Cividis,
        "DIVERGING" or "BLUE-RED" or "BLUERED" => XvarnaPalette.Diverging,
        _ => throw new ArgumentException("Palette must be Viridis, Cividis, or Diverging."),
    };
}
