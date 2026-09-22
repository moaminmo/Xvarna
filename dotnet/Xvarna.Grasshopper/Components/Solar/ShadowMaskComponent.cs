using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Extracts and previews the complete hemispherical mask of one ASMAN sensor.</summary>
public sealed class ShadowMaskComponent : GH_Component
{
    private Line[] previewLines = [];
    private Color[] previewColors = [];
    private bool previewEnabled = true;

    /// <summary>Initializes the ASMAN Shadow Mask component.</summary>
    public ShadowMaskComponent()
        : base(
            "XVARNA Shadow Mask",
            "XV Shadow Mask",
            "Extracts one sensor's visible and blocked sky directions with full first-hit geometry attribution.",
            "XVARNA",
            "03 Solar")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("757853d4-7a64-4b35-9f4e-2caecf2bb6f8");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    public override BoundingBox ClippingBox
    {
        get
        {
            BoundingBox box = base.ClippingBox;
            if (previewEnabled)
            {
                foreach (Line line in previewLines)
                {
                    box.Union(line.BoundingBox);
                }
            }
            return box;
        }
    }

    /// <inheritdoc />
    public override void DrawViewportWires(IGH_PreviewArgs args)
    {
        base.DrawViewportWires(args);
        if (!previewEnabled)
        {
            return;
        }
        for (int index = 0; index < previewLines.Length; index++)
        {
            args.Display.DrawLine(previewLines[index], previewColors[index], 1);
        }
    }

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Sky Result", "R", "Complete result produced by XV Sky View.", GH_ParamAccess.item);
        inputManager.AddIntegerParameter("Sensor Index", "I", "Zero-based sensor row to extract.", GH_ParamAccess.item, 0);
        inputManager.AddNumberParameter("Radius", "R", "Model-space dome radius for direction lines.", GH_ParamAccess.item, 1.0);
        inputManager.AddBooleanParameter("Preview", "P", "Draw the visible/blocked hemispherical mask in the Rhino viewport.", GH_ParamAccess.item, true);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddPointParameter("Origin", "O", "Selected sensor point.", GH_ParamAccess.item);
        outputManager.AddVectorParameter("Directions", "D", "Ordered world-space Fibonacci hemisphere directions.", GH_ParamAccess.list);
        outputManager.AddBooleanParameter("Visible", "V", "True for open-sky directions.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Distance", "T", "First-hit distance in model units; NaN for visible sky.", GH_ParamAccess.list);
        outputManager.AddPointParameter("Hit Points", "HP", "First-hit points; Unset for visible sky.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Object IDs", "OID", "First-hit source object IDs.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Instance IDs", "IID", "First-hit occurrence IDs.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Mesh IDs", "MID", "First-hit deduplicated mesh-resource IDs.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Triangles", "F", "First-hit local triangle indices; -1 for visible sky.", GH_ParamAccess.list);
        outputManager.AddLineParameter("Mask Rays", "L", "Equal-radius hemispherical mask rays for display or export.", GH_ParamAccess.list);
        outputManager.AddColourParameter("State Colours", "C", "Cyan visible directions and magenta blocked directions.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Report", "Rep", "Selected sensor summary and extraction identity.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        object? wrappedResult = null;
        int sensorIndex = 0;
        double radius = 1.0;
        bool preview = true;
        if (!dataAccess.GetData(0, ref wrappedResult))
        {
            ClearPreview();
            return;
        }
        dataAccess.GetData(1, ref sensorIndex);
        dataAccess.GetData(2, ref radius);
        dataAccess.GetData(3, ref preview);
        SkyViewResult? result = UnwrapResult(wrappedResult);
        if (result is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Sky Result must come directly from XV Sky View.");
            ClearPreview();
            return;
        }
        if (sensorIndex < 0 || sensorIndex >= result.Sensors.Count)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, $"Sensor Index must be between 0 and {result.Sensors.Count - 1}.");
            ClearPreview();
            return;
        }
        if (!double.IsFinite(radius) || radius <= 0.0)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Radius must be finite and positive.");
            ClearPreview();
            return;
        }

        int sampleCount = checked((int)result.Options.SampleCount);
        int rowStart = checked(sensorIndex * sampleCount);
        SkyRayEntry[] entries = result.Timeline.Skip(rowStart).Take(sampleCount).ToArray();
        SkySensor sensor = result.Sensors[sensorIndex];
        Point3d origin = new(sensor.PositionX, sensor.PositionY, sensor.PositionZ);
        Vector3d[] directions = entries.Select(item => new Vector3d(item.DirectionX, item.DirectionY, item.DirectionZ)).ToArray();
        bool[] visible = entries.Select(item => item.State == SkyRayState.Visible).ToArray();
        Color[] colors = visible
            .Select(isVisible => isVisible ? Color.FromArgb(24, 210, 190) : Color.FromArgb(230, 55, 125))
            .ToArray();
        Line[] lines = directions.Select(direction => new Line(origin, origin + (direction * radius))).ToArray();
        Point3d[] hitPoints = entries.Select((item, index) => item.State == SkyRayState.Blocked
            ? origin + (directions[index] * item.Distance)
            : Point3d.Unset).ToArray();
        SensorSkySummary summary = result.Summaries[sensorIndex];
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: ASMAN Shadow Mask; sensor index {sensorIndex}; ID {sensor.SensorId}{Environment.NewLine}" +
            $"Samples: {sampleCount:N0}; visible: {summary.VisibleCount:N0}; blocked: {summary.BlockedCount:N0}{Environment.NewLine}" +
            $"Cosine SVF: {summary.CosineWeightedSkyViewFactor:G8}; visible hemisphere: {summary.VisibleHemisphereFraction:G8}; convergence delta: {summary.CosineConvergenceDelta:G6}{Environment.NewLine}" +
            $"Dominant occluder: {(summary.DominantOccluderObjectId == 0 ? "none" : summary.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture))}; projected blocked fraction: {summary.DominantOccluderProjectedFraction:G8}{Environment.NewLine}" +
            $"Result hash: {result.ContentHash}");

        previewLines = lines;
        previewColors = colors;
        previewEnabled = preview;
        dataAccess.SetData(0, origin);
        dataAccess.SetDataList(1, directions);
        dataAccess.SetDataList(2, visible);
        dataAccess.SetDataList(3, entries.Select(item => item.State == SkyRayState.Blocked ? item.Distance : double.NaN));
        dataAccess.SetDataList(4, hitPoints);
        dataAccess.SetDataList(5, entries.Select(item => item.State == SkyRayState.Blocked ? item.ObjectId.ToString(CultureInfo.InvariantCulture) : string.Empty));
        dataAccess.SetDataList(6, entries.Select(item => item.State == SkyRayState.Blocked ? item.InstanceId.ToString(CultureInfo.InvariantCulture) : string.Empty));
        dataAccess.SetDataList(7, entries.Select(item => item.State == SkyRayState.Blocked ? item.MeshId.ToString(CultureInfo.InvariantCulture) : string.Empty));
        dataAccess.SetDataList(8, entries.Select(item => item.State == SkyRayState.Blocked ? checked((int)item.TriangleId) : -1));
        dataAccess.SetDataList(9, lines);
        dataAccess.SetDataList(10, colors);
        dataAccess.SetData(11, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"ASMAN\nSVF {summary.CosineWeightedSkyViewFactor:P1}");
    }

    private void ClearPreview()
    {
        previewLines = [];
        previewColors = [];
    }

    private static SkyViewResult? UnwrapResult(object? value) =>
        value switch
        {
            SkyViewResult result => result,
            GH_ObjectWrapper { Value: SkyViewResult result } => result,
            _ => null,
        };
}
