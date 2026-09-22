using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Builds a validated shared DAENA/HVARE optical material catalog.</summary>
public sealed class MaterialLibraryComponent : GH_Component
{
    /// <summary>Initializes the material component.</summary>
    public MaterialLibraryComponent() : base(
        "XVARNA Materials", "XV Materials",
        "Builds a physically bounded visible/solar transmittance and reflectance catalog, assigned by exact scene Object ID.",
        "XVARNA", "00 Setup")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("d9e4ec03-45e5-48f0-91e9-dd1d7054556f");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Material IDs", "MID", "Unique non-zero unsigned material IDs.", GH_ParamAccess.list);
        input.AddNumberParameter("Visible Transmittance", "Tv", "Visible fraction in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Solar Transmittance", "Ts", "Direct-solar fraction in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Reflectance", "R", "Diffuse reflectance in [0,1]; transmittance + reflectance must not exceed one.", GH_ParamAccess.list);
        input.AddTextParameter("Object IDs", "OID", "Source scene object IDs to assign.", GH_ParamAccess.list);
        input.AddTextParameter("Assigned Material IDs", "A", "Material ID per Object ID.", GH_ParamAccess.list);
        for (int index = 0; index < 6; index++) input[index].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Material Library", "M", "Immutable validated catalog for every DAENA/HVARE material-aware component.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Definitions, assignments, implicit opacity, and conservation policy.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<string> ids = [], objects = [], assigned = [];
        List<double> visible = [], solar = [], reflectance = [];
        data.GetDataList(0, ids); data.GetDataList(1, visible); data.GetDataList(2, solar);
        data.GetDataList(3, reflectance); data.GetDataList(4, objects); data.GetDataList(5, assigned);
        try
        {
            if (visible.Count != ids.Count || solar.Count != ids.Count || reflectance.Count != ids.Count)
                throw new ArgumentException("Material ID, visible, solar, and reflectance lists must have equal lengths.");
            if (objects.Count != assigned.Count)
                throw new ArgumentException("Object IDs and assigned material IDs must have equal lengths.");
            AnalysisMaterial[] materials = ids.Select((value, index) => new AnalysisMaterial(
                IntelligenceHelpers.Parse(value, "Material ID"), visible[index], solar[index], reflectance[index])).ToArray();
            MaterialAssignment[] assignments = objects.Select((value, index) => new MaterialAssignment(
                IntelligenceHelpers.Parse(value, "Object ID"), IntelligenceHelpers.Parse(assigned[index], "Assigned material ID"))).ToArray();
            AnalysisMaterialLibrary library = new(materials, assignments);
            string report = $"Material system: {materials.Length:N0} definitions; {assignments.Length:N0} object assignments{Environment.NewLine}" +
                "Policy: per geometric interaction; unassigned objects are opaque; visible/solar transmission and diffuse reflectance are energy-conserving.";
            data.SetData(0, library); data.SetData(1, report);
            Message = $"Materials\n{materials.Length:N0} · {assignments.Length:N0} assigned";
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }
}
