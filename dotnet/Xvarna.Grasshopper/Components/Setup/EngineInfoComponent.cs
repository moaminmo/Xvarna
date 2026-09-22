using System.Drawing;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Reports the active managed runtime and native-engine compatibility.</summary>
public sealed class EngineInfoComponent : GH_Component
{
    /// <summary>Initializes the XVARNA engine-info component.</summary>
    public EngineInfoComponent()
        : base(
            "XVARNA Info",
            "XV Info",
            "Reports XVARNA engine, ABI, runtime, and compatibility status.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("bd48e779-b716-4f31-b625-a72bff99a1c5");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        _ = inputManager;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddTextParameter("Engine", "E", "Native XVARNA engine version.", GH_ParamAccess.item);
        outputManager.AddTextParameter("ABI", "A", "Native and expected ABI version.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Runtime", "R", "Active .NET runtime used by Rhino.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Status", "S", "Native engine compatibility status.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Ready", "OK", "True when the engine is loadable and ABI-compatible.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        EngineProbeResult result = EngineProbe.TryLoad();

        dataAccess.SetData(0, result.EngineVersion?.ToString() ?? "Unavailable");
        dataAccess.SetData(1, result.NativeAbi is { } native
            ? $"native {native}; expected {AbiVersion.Expected}"
            : $"expected {AbiVersion.Expected}");
        dataAccess.SetData(2, result.Runtime);
        dataAccess.SetData(3, result.Status);
        dataAccess.SetData(4, result.IsAvailable);

        if (!result.IsAvailable)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, result.Status);
        }
    }
}
