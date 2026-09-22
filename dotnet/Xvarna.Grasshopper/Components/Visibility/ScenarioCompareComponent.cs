using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Compares two aligned 3D-isovist scenarios with attribution deltas.</summary>
public sealed class ScenarioCompareComponent : GH_Component
{
    /// <summary>Initializes the spatial scenario comparison.</summary>
    public ScenarioCompareComponent() : base(
        "XVARNA Scenario Compare", "XV Compare",
        "Compares aligned DAENA 3D-isovist results with per-viewpoint volume/openness/depth and per-object attribution deltas.",
        "XVARNA", "05 Study")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("e41c6334-13ca-4ca5-bd65-cf95e6d51398");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Baseline", "A", "Baseline XV Isovist 3D result.", GH_ParamAccess.item);
        input.AddTextParameter("Baseline Name", "AN", "Baseline scenario label.", GH_ParamAccess.item, "baseline");
        input.AddGenericParameter("Candidate", "B", "Candidate XV Isovist 3D result with aligned viewpoint IDs.", GH_ParamAccess.item);
        input.AddTextParameter("Candidate Name", "BN", "Candidate scenario label.", GH_ParamAccess.item, "candidate");
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Comparison", "R", "Aligned scenario delta result.", GH_ParamAccess.item);
        output.AddNumberParameter("Δ Volume", "ΔV", "Candidate minus baseline volume per viewpoint.", GH_ParamAccess.list);
        output.AddNumberParameter("Δ Openness", "ΔO", "Candidate minus baseline openness per viewpoint.", GH_ParamAccess.list);
        output.AddNumberParameter("Δ Mean Radius", "ΔR", "Candidate minus baseline mean radius per viewpoint.", GH_ParamAccess.list);
        output.AddTextParameter("Object IDs", "OID", "Per-viewpoint object union.", GH_ParamAccess.tree);
        output.AddNumberParameter("Δ Attribution", "ΔF", "Candidate minus baseline obstruction share.", GH_ParamAccess.tree);
        output.AddTextParameter("Report", "Rep", "Scenario alignment and delta summary.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess data)
    {
        object? baselineValue = null, candidateValue = null; string baselineName = "baseline", candidateName = "candidate";
        if (!data.GetData(0, ref baselineValue) || !data.GetData(2, ref candidateValue)) return;
        data.GetData(1, ref baselineName); data.GetData(3, ref candidateName);
        try
        {
            Isovist3dResult baseline = IntelligenceHelpers.Unwrap<Isovist3dResult>(baselineValue)
                ?? throw new ArgumentException("Baseline must come from XV Isovist 3D.");
            Isovist3dResult candidate = IntelligenceHelpers.Unwrap<Isovist3dResult>(candidateValue)
                ?? throw new ArgumentException("Candidate must come from XV Isovist 3D.");
            SpatialScenarioComparison comparison = DaenaScenarios.Compare(baselineName, baseline, candidateName, candidate);
            DataTree<string> objects = new(); DataTree<double> deltas = new();
            for (int index = 0; index < comparison.Metrics.Count; index++)
            {
                GH_Path path = new(index); ulong viewpoint = comparison.Metrics[index].ViewpointId;
                IEnumerable<AttributionDelta> rows = comparison.AttributionDeltas.Where(value => value.ViewpointId == viewpoint);
                objects.AddRange(rows.Select(value => value.ObjectId.ToString(CultureInfo.InvariantCulture)), path);
                deltas.AddRange(rows.Select(value => value.DeltaFraction), path);
            }
            string report = $"DAENA scenario delta: {baselineName} → {candidateName}{Environment.NewLine}" +
                $"Aligned viewpoints: {comparison.Metrics.Count:N0}; attribution rows: {comparison.AttributionDeltas.Count:N0}{Environment.NewLine}" +
                "Signs: positive metric deltas improve volume/openness/depth; positive attribution delta means a larger obstruction share.";
            data.SetData(0, comparison); data.SetDataList(1, comparison.Metrics.Select(value => value.VolumeDelta));
            data.SetDataList(2, comparison.Metrics.Select(value => value.OpennessDelta));
            data.SetDataList(3, comparison.Metrics.Select(value => value.MeanRadialDelta));
            data.SetDataTree(4, objects); data.SetDataTree(5, deltas); data.SetData(6, report);
            Message = $"Scenario Δ\n{comparison.Metrics.Count:N0} views";
        }
        catch (Exception exception) when (exception is ArgumentException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}
