using System.Drawing;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Enumerates portable compute adapters visible to VAYU.</summary>
public sealed class ComputeDevicesComponent : GH_Component
{
    /// <summary>Initializes the device enumeration component.</summary>
    public ComputeDevicesComponent()
        : base(
            "XVARNA Devices",
            "XV Devices",
            "Enumerates DX12/Vulkan/Metal/GL adapters visible to the VAYU portable compute engine.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("73cf2718-0cf6-4a86-b04c-2671f962b731");

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
        outputManager.AddTextParameter("Names", "N", "Portable adapter names.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Drivers", "D", "Driver names and versions.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("API Codes", "API", "1 Vulkan, 2 Metal, 3 DX12, 4 GL, 5 WebGPU.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Device Types", "T", "1 integrated, 2 discrete, 3 virtual, 4 CPU.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Vendor IDs", "VID", "Hexadecimal vendor identifiers.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Device IDs", "DID", "Hexadecimal device identifiers.", GH_ParamAccess.list);
        outputManager.AddBooleanParameter("Available", "OK", "True when at least one portable adapter is visible.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        try
        {
            IReadOnlyList<ComputeAdapterInfo> adapters = XvarnaComputeSession.EnumerateAdapters();
            dataAccess.SetDataList(0, adapters.Select(value => value.Name));
            dataAccess.SetDataList(1, adapters.Select(value => value.Driver));
            dataAccess.SetDataList(2, adapters.Select(value => checked((int)value.BackendCode)));
            dataAccess.SetDataList(3, adapters.Select(value => checked((int)value.DeviceTypeCode)));
            dataAccess.SetDataList(4, adapters.Select(value => $"0x{value.VendorId:x8}"));
            dataAccess.SetDataList(5, adapters.Select(value => $"0x{value.DeviceId:x8}"));
            dataAccess.SetData(6, adapters.Count > 0);
            if (adapters.Count == 0)
            {
                AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, "No portable GPU adapter was reported. Auto mode will use the canonical CPU backend.");
            }
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException
            or InvalidOperationException
            or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }
}
