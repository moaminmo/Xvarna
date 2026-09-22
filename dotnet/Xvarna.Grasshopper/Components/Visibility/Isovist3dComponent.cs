using System.Drawing;
using System.Globalization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Computes material-aware equal-solid-angle volumetric isovists.</summary>
public sealed class Isovist3dComponent : ScheduledAnalysisComponent<Isovist3dWork, Isovist3dResult>
{
    /// <summary>Initializes the 3D-isovist component.</summary>
    public Isovist3dComponent() : base(
        "XVARNA Isovist 3D", "XV Isovist 3D",
        "Computes volumetric 3D isovists, visible solid angle, general top-K/category attribution, and exact remove-one counterfactuals.",
        "XVARNA", "04 Visibility")
    { }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("47c40f7d-0c50-41b2-8559-a14fa2e6f3b9");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());
    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Isovist 3D";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable XV Scene.", GH_ParamAccess.item);
        input.AddPointParameter("Eyes", "E", "Spatial viewpoints in Rhino model units.", GH_ParamAccess.list);
        input.AddGenericParameter("Materials", "M", "Optional XV Materials catalog; empty means opaque.", GH_ParamAccess.item);
        input.AddIntegerParameter("Samples", "Q", "Equal-solid-angle Fibonacci rays per eye, 64..262144.", GH_ParamAccess.item, 4096);
        input.AddNumberParameter("Maximum Distance", "Max", "Finite clipping radius in model units.", GH_ParamAccess.item, 100.0);
        input.AddIntegerParameter("Top K", "K", "Ranked objects retained per eye.", GH_ParamAccess.item, 10);
        input.AddIntegerParameter("Counterfactual", "CF", "Leading objects retraced exactly after remove-one.", GH_ParamAccess.item, 5);
        input.AddIntegerParameter("Material Layers", "L", "Maximum transparent geometric interactions, 1..64.", GH_ParamAccess.item, 8);
        input.AddNumberParameter("Minimum Transmission", "Tmin", "Throughput cutoff in [0,1].", GH_ParamAccess.item, 0.0001);
        input.AddTextParameter("Category Mask", "C", "-1 for all or unsigned mask.", GH_ParamAccess.item, "-1");
        input.AddNumberParameter("Eye Offset", "Eps", "0 uses Rhino document tolerance.", GH_ParamAccess.item, 0.0);
        input[2].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Result", "R", "Complete immutable DAENA 3D-isovist result.", GH_ParamAccess.item);
        output.AddNumberParameter("Volume", "V", "Radial volume integral per eye, model units cubed.", GH_ParamAccess.list);
        output.AddNumberParameter("Radial Surface", "A", "Radial surface measure per eye, model units squared.", GH_ParamAccess.list);
        output.AddNumberParameter("Openness", "O", "Transmission-weighted visible solid angle divided by 4π.", GH_ParamAccess.list);
        output.AddNumberParameter("Convergence", "ΔV", "Full-versus-interleaved-half volume delta.", GH_ParamAccess.list);
        output.AddPointParameter("Endpoints", "P", "Viewpoint-major spherical endpoints.", GH_ParamAccess.tree);
        output.AddNumberParameter("Transmission", "T", "Viewpoint-major optical transmission.", GH_ParamAccess.tree);
        output.AddTextParameter("Top Objects", "OID", "Ranked source object IDs per eye.", GH_ParamAccess.tree);
        output.AddNumberParameter("Top Fractions", "F", "Ranked obstruction shares per eye.", GH_ParamAccess.tree);
        output.AddNumberParameter("Remove-One ΔVolume", "CF", "Exact recovered volume per evaluated top object.", GH_ParamAccess.tree);
        output.AddTextParameter("Category Masks", "C", "Exact non-overlapping category combinations per eye.", GH_ParamAccess.tree);
        output.AddNumberParameter("Category Fractions", "CF%", "Attributed share per exact category combination.", GH_ParamAccess.tree);
        output.AddTextParameter("Hash", "H", "BLAKE3 result identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Policy, convergence, attribution, and runtime report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out Isovist3dWork work)
    {
        work = default; object? sceneValue = null, materialValue = null; List<Point3d> eyes = [];
        int samples = 4096, topK = 10, counterfactual = 5, layers = 8;
        double maximum = 100.0, minimumTransmission = 0.0001, offset = 0.0; string mask = "-1";
        if (!data.GetData(0, ref sceneValue) || !data.GetDataList(1, eyes) || eyes.Count == 0) return false;
        data.GetData(2, ref materialValue); data.GetData(3, ref samples); data.GetData(4, ref maximum);
        data.GetData(5, ref topK); data.GetData(6, ref counterfactual); data.GetData(7, ref layers);
        data.GetData(8, ref minimumTransmission); data.GetData(9, ref mask); data.GetData(10, ref offset);
        try
        {
            samples = ProductContextRegistry.ScaleSamples(OnPingDocument(), samples, 64, 262_144);
            XvarnaScene scene = IntelligenceHelpers.Unwrap<XvarnaScene>(sceneValue)
                ?? throw new ArgumentException("Scene must come from XV Scene.");
            AnalysisMaterialLibrary materials = IntelligenceHelpers.Unwrap<AnalysisMaterialLibrary>(materialValue)
                ?? AnalysisMaterialLibrary.Opaque;
            double resolvedOffset = offset > 0 ? offset : ProductContextRegistry.GetUnits(OnPingDocument()).AbsoluteToleranceModelUnits;
            SpatialViewpoint[] viewpoints = eyes.Select((point, index) =>
                new SpatialViewpoint(checked((ulong)index + 1), point.X, point.Y, point.Z)).ToArray();
            work = new(scene, viewpoints, materials,
                new Isovist3dOptions(checked((uint)samples), maximum, resolvedOffset,
                    IntelligenceHelpers.Parse(mask, "Category mask", true), checked((uint)topK),
                    checked((uint)counterfactual), checked((uint)layers), minimumTransmission));
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or InvalidOperationException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(Isovist3dWork work) =>
        checked((ulong)work.Viewpoints.Length * work.Options.SampleCount * (1UL + work.Options.CounterfactualCount));

    /// <inheritdoc />
    protected override Isovist3dResult Execute(Isovist3dWork work, CancellationToken token,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work); progress.Report((0, total, "Spherical tracing"));
        token.ThrowIfCancellationRequested();
        Isovist3dResult result = work.Scene.AnalyzeIsovists3d(work.Viewpoints, work.Materials, work.Options);
        progress.Report((total, total, "Attribution and counterfactuals")); return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, Isovist3dWork work, Isovist3dResult result)
    {
        DataTree<Point3d> points = new(); DataTree<double> transmission = new();
        DataTree<string> objects = new(), categories = new(); DataTree<double> fractions = new(), recovered = new(), categoryFractions = new();
        int samples = checked((int)work.Options.SampleCount);
        for (int eye = 0; eye < work.Viewpoints.Length; eye++)
        {
            GH_Path path = new(eye);
            points.AddRange(result.Rays.Skip(eye * samples).Take(samples).Select(value => new Point3d(value.EndpointX, value.EndpointY, value.EndpointZ)), path);
            transmission.AddRange(result.Rays.Skip(eye * samples).Take(samples).Select(value => value.Transmission), path);
            ulong id = work.Viewpoints[eye].ViewpointId;
            IEnumerable<ObstructionAttribution> ranked = result.Attribution.Where(value => value.ObserverId == id);
            objects.AddRange(ranked.Select(value => value.ObjectId.ToString(CultureInfo.InvariantCulture)), path);
            fractions.AddRange(ranked.Select(value => value.Fraction), path);
            recovered.AddRange(ranked.Select(value => value.CounterfactualVolumeDelta), path);
            IEnumerable<CategoryBreakdown> partition = result.Categories.Where(value => value.ObserverId == id);
            categories.AddRange(partition.Select(value => value.CategoryMask.ToString(CultureInfo.InvariantCulture)), path);
            categoryFractions.AddRange(partition.Select(value => value.Fraction), path);
        }
        double maximumRelativeDelta = result.Summaries.Max(value =>
            value.Volume == 0.0 ? value.VolumeConvergenceDelta : Math.Abs(value.VolumeConvergenceDelta / value.Volume));
        AnalysisQualityProfile quality = ProductContextRegistry.GetQuality(OnPingDocument());
        XvarnaEvidenceLabel evidence = XvarnaEvidenceLabel.Convergence(
            "interleaved 3D isovist volume", maximumRelativeDelta, 0.0,
            work.Options.SampleCount, quality.ConvergenceTolerance);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA equal-solid-angle 3D isovist over ZAMYAD{Environment.NewLine}" +
            $"Study: {work.Viewpoints.Length:N0} eyes × {samples:N0} rays; top {work.Options.TopK}; exact remove-one {work.Options.CounterfactualCount}{Environment.NewLine}" +
            $"Materials: {work.Materials.Materials.Count:N0}; assignments: {work.Materials.Assignments.Count:N0}; layers: {work.Options.MaximumMaterialLayers}{Environment.NewLine}" +
            $"Evidence: {evidence.Level}; max relative Δvolume {maximumRelativeDelta:G6}; {evidence.Statement}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, result.Summaries.Select(value => value.Volume));
        data.SetDataList(2, result.Summaries.Select(value => value.RadialSurface));
        data.SetDataList(3, result.Summaries.Select(value => value.OpennessRatio));
        data.SetDataList(4, result.Summaries.Select(value => value.VolumeConvergenceDelta));
        data.SetDataTree(5, points); data.SetDataTree(6, transmission); data.SetDataTree(7, objects);
        data.SetDataTree(8, fractions); data.SetDataTree(9, recovered); data.SetDataTree(10, categories);
        data.SetDataTree(11, categoryFractions); data.SetData(12, result.ContentHash); data.SetData(13, report);
        Message = $"DAENA 3D\n{work.Viewpoints.Length:N0} × {samples:N0}";
    }
}

/// <summary>Immutable captured 3D-isovist work.</summary>
public readonly record struct Isovist3dWork(
    XvarnaScene Scene, SpatialViewpoint[] Viewpoints,
    AnalysisMaterialLibrary Materials, Isovist3dOptions Options);
