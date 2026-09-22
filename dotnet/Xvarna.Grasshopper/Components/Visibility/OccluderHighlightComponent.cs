using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Rhino.Display;
using Rhino.Geometry;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Maps exact DAENA source-object IDs back to Rhino viewport mesh highlights.</summary>
public sealed class OccluderHighlightComponent : GH_Component
{
    private Mesh[] normalMeshes = [];
    private Mesh[] highlightedMeshes = [];
    private Color normalColor = Color.FromArgb(90, 120, 130, 140);
    private Color highlightColor = Color.FromArgb(220, 255, 70, 35);

    /// <summary>Initializes the occluder highlighter.</summary>
    public OccluderHighlightComponent() : base(
        "XVARNA Highlight Occluders", "XV Highlight",
        "Highlights Rhino source meshes by the same exact Object IDs returned by DAENA/HVARE attribution.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("a9dce77d-2ed9-430a-877a-33d159cb7e6f");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    public override BoundingBox ClippingBox => normalMeshes.Concat(highlightedMeshes).Any()
        ? normalMeshes.Concat(highlightedMeshes).Select(value => value.GetBoundingBox(false)).Aggregate(BoundingBox.Union)
        : base.ClippingBox;

    /// <inheritdoc />
    public override void DrawViewportMeshes(IGH_PreviewArgs args)
    {
        base.DrawViewportMeshes(args);
        DisplayMaterial normal = new(normalColor); DisplayMaterial highlighted = new(highlightColor);
        foreach (Mesh mesh in normalMeshes) args.Display.DrawMeshShaded(mesh, normal);
        foreach (Mesh mesh in highlightedMeshes) args.Display.DrawMeshShaded(mesh, highlighted);
    }

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddMeshParameter("Source Meshes", "M", "Rhino meshes used to build the scene, in source order.", GH_ParamAccess.list);
        input.AddTextParameter("Object IDs", "OID", "Exact source Object ID per mesh.", GH_ParamAccess.list);
        input.AddTextParameter("Highlight IDs", "HID", "Object IDs from DAENA/HVARE top attribution.", GH_ParamAccess.list);
        input.AddColourParameter("Normal Colour", "N", "Viewport colour for non-selected meshes.", GH_ParamAccess.item, Color.FromArgb(90, 120, 130, 140));
        input.AddColourParameter("Highlight Colour", "H", "Viewport colour for selected occluders.", GH_ParamAccess.item, Color.FromArgb(220, 255, 70, 35));
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddMeshParameter("Highlighted Meshes", "H", "Bakeable meshes whose Object IDs matched.", GH_ParamAccess.list);
        output.AddMeshParameter("Other Meshes", "O", "Bakeable non-matching meshes.", GH_ParamAccess.list);
        output.AddBooleanParameter("Selected", "S", "Boolean selection mask aligned with Source Meshes.", GH_ParamAccess.list);
        output.AddTextParameter("Report", "Rep", "Resolved and missing ID counts.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<Mesh> meshes = []; List<string> ids = [], selected = []; Color normal = normalColor, highlight = highlightColor;
        if (!data.GetDataList(0, meshes) || !data.GetDataList(1, ids) || !data.GetDataList(2, selected)) return;
        data.GetData(3, ref normal); data.GetData(4, ref highlight);
        try
        {
            if (meshes.Count != ids.Count) throw new ArgumentException("Source Meshes and Object IDs must have equal lengths.");
            ulong[] objectIds = ids.Select(value => IntelligenceHelpers.Parse(value, "Object ID")).ToArray();
            HashSet<ulong> highlightIds = selected.Select(value => IntelligenceHelpers.Parse(value, "Highlight ID")).ToHashSet();
            bool[] mask = objectIds.Select(highlightIds.Contains).ToArray();
            highlightedMeshes = meshes.Where((_, index) => mask[index]).Select(value => value.DuplicateMesh()).ToArray();
            normalMeshes = meshes.Where((_, index) => !mask[index]).Select(value => value.DuplicateMesh()).ToArray();
            normalColor = normal; highlightColor = highlight;
            int missing = highlightIds.Count(value => !objectIds.Contains(value));
            string report = string.Create(CultureInfo.InvariantCulture,
                $"Viewport attribution: {highlightedMeshes.Length:N0}/{meshes.Count:N0} source meshes highlighted; {missing:N0} requested IDs absent.");
            data.SetDataList(0, highlightedMeshes); data.SetDataList(1, normalMeshes);
            data.SetDataList(2, mask); data.SetData(3, report); Message = $"Highlight\n{highlightedMeshes.Length:N0} meshes";
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}
