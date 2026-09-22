using System.Globalization;
using Grasshopper2.Components;
using GrasshopperIO;
using Grasshopper2.UI;
using Rhino;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

/// <summary>Reports native engine and managed-host compatibility.</summary>
[IoId("bd48e779-b716-4f31-b625-a72bff99a1c5")]
public sealed class EngineInfoComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public EngineInfoComponent() : base(new Nomen("XVARNA Info", "Native engine, ABI and runtime health.", "XVARNA", "00 Setup")) { }
    /// <summary>Deserializes the component.</summary>
    public EngineInfoComponent(IReader reader) : base(reader) { }

    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs) { }

    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddText("Engine", "E", "Native XVARNA engine version.");
        outputs.AddText("ABI", "A", "Native and expected ABI version.");
        outputs.AddText("Runtime", "R", "Active managed runtime.");
        outputs.AddText("Status", "S", "Compatibility status.");
        outputs.AddBoolean("Ready", "OK", "True when native engine and ABI are compatible.");
    }

    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        EngineProbeResult result = EngineProbe.TryLoad();
        access.SetItem(0, result.EngineVersion?.ToString() ?? "Unavailable");
        access.SetItem(1, result.NativeAbi is { } native ? $"native {native}; expected {AbiVersion.Expected}" : $"expected {AbiVersion.Expected}");
        access.SetItem(2, result.Runtime);
        access.SetItem(3, result.Status);
        access.SetItem(4, result.IsAvailable);
        if (!result.IsAvailable) access.AddWarning("Native engine unavailable", result.Status);
    }
}

/// <summary>Enumerates VAYU compute devices without assuming a GPU vendor.</summary>
[IoId("73cf2718-0cf6-4a86-b04c-2671f962b731")]
public sealed class ComputeDevicesComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public ComputeDevicesComponent() : base(new Nomen("XVARNA Devices", "Portable GPU/CPU adapters visible to VAYU.", "XVARNA", "00 Setup")) { }
    /// <summary>Deserializes the component.</summary>
    public ComputeDevicesComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs) { }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddText("Devices", "D", "One auditable device record per line.");
        outputs.AddInteger("Count", "N", "Number of visible adapters.");
        outputs.AddBoolean("Available", "OK", "True when at least one portable adapter is visible.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        try
        {
            IReadOnlyList<ComputeAdapterInfo> devices = XvarnaComputeSession.EnumerateAdapters();
            string report = string.Join(Environment.NewLine, devices.Select(value =>
                $"{value.Name} | {value.Driver} | backend {value.BackendCode} | type {value.DeviceTypeCode} | vendor 0x{value.VendorId:x8} | device 0x{value.DeviceId:x8}"));
            access.SetItem(0, report);
            access.SetItem(1, devices.Count);
            access.SetItem(2, devices.Count > 0);
            if (devices.Count == 0) access.AddWarning("No portable adapter", "XVARNA remains usable through the canonical CPU backend.");
        }
        catch (Exception exception) when (exception is XvarnaNativeException or DllNotFoundException or BadImageFormatException)
        {
            access.AddError("Device enumeration failed", exception.Message);
        }
    }
}

/// <summary>Declares one explicit model-unit to SI conversion contract.</summary>
[IoId("b121be1d-f579-42f6-b91c-311604522fb4")]
public sealed class UnitsComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public UnitsComponent() : base(new Nomen("XVARNA Units", "Authoritative model-unit and tolerance contract.", "XVARNA", "00 Setup")) { }
    /// <summary>Deserializes the component.</summary>
    public UnitsComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Unit Name", "Unit", "Empty uses the active Rhino document.").Set(string.Empty);
        inputs.AddNumber("Meters per Unit", "m/U", "Zero uses the active Rhino document conversion.").Set(0.0);
        inputs.AddNumber("Absolute Tolerance", "Tol", "Zero uses active document tolerance.").Set(0.0);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Units", "U", "Validated XVARNA unit context.");
        outputs.AddNumber("Meters per Unit", "m/U", "Metres represented by one model unit.");
        outputs.AddNumber("Tolerance m", "Tol", "Absolute tolerance in metres.");
        outputs.AddText("Report", "Info", "Complete conversion provenance.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string name);
        access.GetItem(1, out double scale);
        access.GetItem(2, out double tolerance);
        try
        {
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            Rhino.UnitSystem system = document?.ModelUnitSystem ?? Rhino.UnitSystem.Meters;
            double resolvedScale = scale > 0 ? scale : RhinoMath.UnitScale(system, Rhino.UnitSystem.Meters);
            double modelTolerance = tolerance > 0 ? tolerance : document?.ModelAbsoluteTolerance ?? 1e-6;
            XvarnaUnitContext units = XvarnaUnitContext.Create(string.IsNullOrWhiteSpace(name) ? system.ToString() : name.Trim(), resolvedScale, modelTolerance);
            access.SetItem(0, units);
            access.SetItem(1, units.MetersPerModelUnit);
            access.SetItem(2, units.AbsoluteToleranceMeters);
            access.SetItem(3, units.Report);
        }
        catch (ArgumentException exception) { access.AddError("Invalid unit contract", exception.Message); }
    }
}

/// <summary>Creates a deterministic quality and interaction policy shared by analyses.</summary>
[IoId("6aeeb503-ea7b-4fe1-bdf9-ec3df8d8641e")]
public sealed class QualityComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private bool previousRun;
    private long generation;
    /// <summary>Initializes the component.</summary>
    public QualityComponent() : base(new Nomen("XVARNA Quality", "Sampling, UX, execution and evidence policy.", "XVARNA", "00 Setup")) { }
    /// <summary>Deserializes the component.</summary>
    public QualityComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Preset", "Q", "Preview, Balanced, High, or Publication.").Set("Balanced");
        inputs.AddText("Experience", "UX", "Basic or Expert.").Set("Basic");
        inputs.AddText("Execution", "Mode", "Live or Manual.").Set("Live");
        inputs.AddBoolean("Run", "Run", "False-to-true pulse in Manual mode.").Set(false);
        inputs.AddInteger("Debounce ms", "ms", "-1 uses the preset default.").Set(-1);
        inputs.AddBoolean("Retain Stale", "Stale", "Retain a clearly labelled last good result.").Set(true);
        inputs.AddBoolean("Require Evidence", "Evidence", "Require uncertainty/convergence statements.").Set(true);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Quality", "Q", "Validated quality profile.");
        outputs.AddNumber("Sample Multiplier", "xN", "Sampling multiplier.");
        outputs.AddNumber("Convergence", "d", "Convergence target.");
        outputs.AddInteger("Generation", "Gen", "Manual execution generation.");
        outputs.AddText("Report", "Info", "Complete policy.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string preset); access.GetItem(1, out string experience); access.GetItem(2, out string execution);
        access.GetItem(3, out bool run); access.GetItem(4, out int debounce); access.GetItem(5, out bool stale); access.GetItem(6, out bool evidence);
        try
        {
            XvarnaExecutionMode mode = execution.Trim().Equals("Manual", StringComparison.OrdinalIgnoreCase) ? XvarnaExecutionMode.Manual : XvarnaExecutionMode.Live;
            if (mode == XvarnaExecutionMode.Manual && run && !previousRun) generation++;
            previousRun = run;
            AnalysisQualityProfile profile = AnalysisQualityProfile.Create(ParsePreset(preset), ParseExperience(experience), mode, debounce < 0 ? null : debounce, stale, evidence, generation);
            access.SetItem(0, profile); access.SetItem(1, profile.SampleMultiplier); access.SetItem(2, profile.ConvergenceTolerance);
            access.SetItem(3, checked((int)Math.Min(generation, int.MaxValue))); access.SetItem(4, profile.Report);
        }
        catch (ArgumentException exception) { access.AddError("Invalid quality policy", exception.Message); }
    }
    private static XvarnaQualityPreset ParsePreset(string value) => value.Trim().ToUpperInvariant() switch
    {
        "PREVIEW" => XvarnaQualityPreset.Preview,
        "BALANCED" => XvarnaQualityPreset.Balanced,
        "HIGH" => XvarnaQualityPreset.High,
        "PUBLICATION" => XvarnaQualityPreset.Publication,
        _ => throw new ArgumentException("Preset must be Preview, Balanced, High, or Publication."),
    };
    private static XvarnaExperienceMode ParseExperience(string value) => value.Trim().ToUpperInvariant() switch
    {
        "BASIC" => XvarnaExperienceMode.Basic,
        "EXPERT" => XvarnaExperienceMode.Expert,
        _ => throw new ArgumentException("Experience must be Basic or Expert."),
    };
}
