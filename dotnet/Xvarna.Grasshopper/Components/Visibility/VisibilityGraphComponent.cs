using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

/// <summary>Builds a DAENA visibility graph with topology metrics.</summary>
public sealed class VisibilityGraphComponent : ScheduledAnalysisComponent<VisibilityGraphWork, VisibilityGraphResult>
{
    /// <summary>Initializes the visibility graph component.</summary>
    public VisibilityGraphComponent() : base("XVARNA Visibility Graph", "XV Visibility Graph",
        "Builds either an exact all-pairs or spatially indexed radius/degree-bounded DAENA graph with blockers and topology metrics.",
        "XVARNA", "04 Visibility")
    { }
    /// <inheritdoc />
    public override Guid ComponentGuid => new("e9bcdb86-b0af-46fc-9c02-e3852c0fc6e6");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Visibility Graph";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        input.AddPointParameter("Nodes", "P", "Visibility graph nodes in active Rhino units.", GH_ParamAccess.list);
        input.AddNumberParameter("Endpoint Clearance", "Eps", "Clearance at both endpoints; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        input.AddNumberParameter("Maximum Distance", "Max", "0 means unbounded; otherwise maximum edge length.", GH_ParamAccess.item, 0.0);
        input.AddTextParameter("Category Mask", "C", "Unsigned 64-bit category filter; -1 selects all.", GH_ParamAccess.item, "-1");
        input.AddBooleanParameter("Centrality", "Cent", "Compute harmonic closeness and Brandes betweenness.", GH_ParamAccess.item, true);
        input.AddBooleanParameter("Run", "Run", "Build the graph.", GH_ParamAccess.item, true);
        input.AddBooleanParameter("Sparse", "Sparse", "Use the scalable spatial radius/neighbor graph instead of materializing all pairs.", GH_ParamAccess.item, false);
        input.AddIntegerParameter("Maximum Neighbors", "K", "Sparse candidate degree cap from 1 through 1024.", GH_ParamAccess.item, 16);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Visibility Graph", "G", "Immutable DAENA graph result.", GH_ParamAccess.item);
        output.AddLineParameter("Visible Edges", "E", "Directly visible unordered node pairs.", GH_ParamAccess.list);
        output.AddLineParameter("Blocked Pairs", "B", "Pairs blocked by scene geometry.", GH_ParamAccess.list);
        output.AddIntegerParameter("Degree", "D", "Visible adjacency count per node.", GH_ParamAccess.list);
        output.AddNumberParameter("Degree Centrality", "DC", "Degree divided by maximum possible degree.", GH_ParamAccess.list);
        output.AddIntegerParameter("Component", "C", "Deterministic connected-component label.", GH_ParamAccess.list);
        output.AddNumberParameter("Harmonic Closeness", "HC", "Normalized reciprocal hop-distance accessibility.", GH_ParamAccess.list);
        output.AddNumberParameter("Betweenness", "BC", "Normalized undirected Brandes betweenness.", GH_ParamAccess.list);
        output.AddTextParameter("Blocker IDs", "OID", "First blocking object for each blocked pair.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "Deterministic graph identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "R", "Graph size, density, components, policy, runtime, and identity.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out VisibilityGraphWork work)
    {
        work = default;
        object? wrapped = null; List<Point3d> points = [];
        double clearance = 0.0, maximum = 0.0; string category = "-1"; bool centrality = true, run = true, sparse = false;
        int maximumNeighbors = 16;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, points) || points.Count == 0) return false;
        data.GetData(2, ref clearance); data.GetData(3, ref maximum); data.GetData(4, ref category);
        data.GetData(5, ref centrality); data.GetData(6, ref run);
        data.GetData(7, ref sparse); data.GetData(8, ref maximumNeighbors);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = Unwrap(wrapped);
        if (scene is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene must come directly from XV Scene."); return false; }
        try
        {
            if (sparse && (!(maximum > 0.0) || !double.IsFinite(maximum)))
                throw new ArgumentException("Sparse mode requires a finite positive Maximum Distance in model units.");
            if (sparse && maximumNeighbors is < 1 or > 1024)
                throw new ArgumentOutOfRangeException(nameof(maximumNeighbors), "Maximum Neighbors must be 1..1024.");
            double resolvedClearance = clearance > 0.0 && double.IsFinite(clearance)
                ? clearance : RhinoDoc.ActiveDoc?.ModelAbsoluteTolerance ?? 1.0e-6;
            VisibilityNodePoint[] nodes = points.Select((point, index) => new VisibilityNodePoint(
                checked((ulong)index + 1), point.X, point.Y, point.Z)).ToArray();
            work = new(scene, nodes,
                new VisibilityGraphOptions(resolvedClearance, maximum == 0.0 ? double.PositiveInfinity : maximum,
                    ParseMask(category), centrality), sparse, checked((uint)maximumNeighbors), points.ToArray());
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(VisibilityGraphWork work)
    {
        ulong nodes = checked((ulong)work.Nodes.Length);
        if (nodes == 0) return 0;
        ulong exact = checked(nodes * (nodes - 1) / 2);
        return work.Sparse ? Math.Min(exact, checked(nodes * work.MaximumNeighbors / 2)) : exact;
    }

    /// <inheritdoc />
    protected override VisibilityGraphResult Execute(
        VisibilityGraphWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Tracing node pairs"));
        cancellationToken.ThrowIfCancellationRequested();
        VisibilityGraphResult result = work.Sparse
            ? work.Scene.AnalyzeSparseVisibilityGraph(work.Nodes,
                new SparseVisibilityGraphOptions(work.Options.EndpointClearance,
                    work.Options.MaximumDistance, work.Options.CategoryMask,
                    work.MaximumNeighbors, work.Options.ComputeCentrality))
            : work.Scene.AnalyzeVisibilityGraph(work.Nodes, work.Options);
        progress.Report((total, total, "Topology and centrality"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, VisibilityGraphWork work, VisibilityGraphResult result)
    {
        IReadOnlyList<Point3d> points = work.Points;
        List<Line> visible = []; List<Line> blocked = []; List<string> blockers = [];
        foreach (VisibilityGraphPair pair in result.Pairs)
        {
            Line line = new(points[checked((int)pair.FirstIndex)], points[checked((int)pair.SecondIndex)]);
            if (pair.State == VisibilityState.Visible) visible.Add(line);
            else if (pair.State == VisibilityState.Blocked) { blocked.Add(line); blockers.Add(pair.BlockerObjectId.ToString(CultureInfo.InvariantCulture)); }
        }
        double possible = points.Count < 2 ? 0.0 : points.Count * (points.Count - 1) / 2.0;
        double density = possible == 0.0 ? 0.0 : result.VisibleEdgeCount / possible;
        string maxText = double.IsPositiveInfinity(result.Options.MaximumDistance) ? "unbounded" : result.Options.MaximumDistance.ToString("G10", CultureInfo.InvariantCulture);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA {(result.IsSparse ? "spatial radius/degree sparse" : "exact all-pairs")} visibility graph over ZAMYAD{Environment.NewLine}" +
            $"Graph: {points.Count:N0} nodes; {result.Pairs.Count:N0} traced pairs of {possible:N0} possible; {result.VisibleEdgeCount:N0} visible edges; full-matrix density {density:G6}{Environment.NewLine}" +
            $"Topology: {result.ConnectedComponentCount:N0} connected component(s); centrality {(result.Options.ComputeCentrality ? "computed" : "disabled")}{Environment.NewLine}" +
            $"Policy: clearance {result.Options.EndpointClearance:G10}; max {maxText}; category {result.Options.CategoryMask}; neighbor cap {(result.IsSparse ? result.MaximumNeighbors.ToString(CultureInfo.InvariantCulture) : "none")}; node cap {(result.IsSparse ? "100,000" : "4,096")}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, visible); data.SetDataList(2, blocked);
        data.SetDataList(3, result.Metrics.Select(value => checked((int)value.Degree)));
        data.SetDataList(4, result.Metrics.Select(value => value.DegreeCentrality));
        data.SetDataList(5, result.Metrics.Select(value => checked((int)value.ComponentIndex)));
        data.SetDataList(6, result.Metrics.Select(value => value.HarmonicCloseness));
        data.SetDataList(7, result.Metrics.Select(value => value.BetweennessCentrality));
        data.SetDataList(8, blockers); data.SetData(9, result.ContentHash); data.SetData(10, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"DAENA\n{points.Count:N0} nodes");
    }

    private static ulong ParseMask(string value) => value == "-1" ? ulong.MaxValue : ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);
    private static XvarnaScene? Unwrap(object? value) => value switch { XvarnaScene scene => scene, GH_ObjectWrapper { Value: XvarnaScene scene } => scene, _ => null };
}

/// <summary>Immutable captured input for scheduled all-pairs visibility-graph work.</summary>
public readonly record struct VisibilityGraphWork(
    XvarnaScene Scene,
    VisibilityNodePoint[] Nodes,
    VisibilityGraphOptions Options,
    bool Sparse,
    uint MaximumNeighbors,
    Point3d[] Points);
