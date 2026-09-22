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

/// <summary>Computes a directed view and privacy matrix.</summary>
public sealed class IntervisibilityComponent : ScheduledAnalysisComponent<IntervisibilityWork, IntervisibilityResult>
{
    /// <summary>Initializes the intervisibility/privacy component.</summary>
    public IntervisibilityComponent() : base("XVARNA Intervisibility + Privacy", "XV Intervisibility",
        "Computes a complete DAENA observer-target visibility matrix with visible/blocked geometry, first blockers, directional exposure, and transparent distance-weighted privacy risk.",
        "XVARNA", "04 Visibility")
    { }
    /// <inheritdoc />
    public override Guid ComponentGuid => new("5f88d25b-34de-4946-b315-c7288f101e80");
    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;
    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "DAENA Intervisibility";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        input.AddPointParameter("Observers", "O", "Observer/eye positions.", GH_ParamAccess.list);
        input.AddPointParameter("Targets", "T", "View or privacy target positions.", GH_ParamAccess.list);
        input.AddVectorParameter("Target Facings", "F", "Optional one facing or one per target; empty makes targets omnidirectional.", GH_ParamAccess.list);
        input.AddNumberParameter("Observer Weights", "OW", "Empty uses 1; one value or one per observer, in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Target Sensitivities", "TS", "Empty uses 1; one value or one per target, in [0,1].", GH_ParamAccess.list);
        input.AddNumberParameter("Endpoint Clearance", "Eps", "Clearance at both line endpoints; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        input.AddNumberParameter("Maximum Distance", "Max", "0 means unbounded; otherwise pair cutoff in model units.", GH_ParamAccess.item, 0.0);
        input.AddNumberParameter("Privacy Reference Distance", "Ref", "Distance at which privacy distance factor equals 0.5.", GH_ParamAccess.item, 10.0);
        input.AddNumberParameter("Facing Exponent", "Exp", "Non-negative exponent controlling directional sensitivity.", GH_ParamAccess.item, 1.0);
        input.AddTextParameter("Category Mask", "C", "Unsigned 64-bit category filter; -1 selects all.", GH_ParamAccess.item, "-1");
        input.AddBooleanParameter("Run", "Run", "Compute the directed matrix.", GH_ParamAccess.item, true);
        input[3].Optional = true; input[4].Optional = true; input[5].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Intervisibility Result", "I", "Immutable DAENA matrix for downstream automation.", GH_ParamAccess.item);
        output.AddLineParameter("Visible Lines", "V", "Directly visible observer-target segments.", GH_ParamAccess.list);
        output.AddLineParameter("Blocked Lines", "B", "Obstructed observer-target segments.", GH_ParamAccess.list);
        output.AddIntegerParameter("States", "S", "Row-major state tree: 0 visible, 1 blocked, 2 out of range, 3 coincident.", GH_ParamAccess.tree);
        output.AddNumberParameter("Distances", "D", "Row-major endpoint distances.", GH_ParamAccess.tree);
        output.AddNumberParameter("Privacy Risk", "PR", "Row-major transparent pair risk.", GH_ParamAccess.tree);
        output.AddTextParameter("Blocker IDs", "OID", "Row-major first blocking object IDs.", GH_ParamAccess.tree);
        output.AddNumberParameter("Observer Visibility", "OV", "Visible fraction per observer.", GH_ParamAccess.list);
        output.AddNumberParameter("Target Exposure", "TE", "Visible-observer fraction per target.", GH_ParamAccess.list);
        output.AddNumberParameter("Combined Privacy", "CP", "Complement-product combined risk per target.", GH_ParamAccess.list);
        output.AddTextParameter("Hash", "H", "Deterministic matrix identity.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "R", "Matrix dimensions, states, risk formula, runtime, and identity.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess data, out IntervisibilityWork work)
    {
        work = default;
        object? wrapped = null; List<Point3d> observers = []; List<Point3d> targets = [];
        List<Vector3d> facings = []; List<double> observerWeights = []; List<double> sensitivities = [];
        double clearance = 0.0, maximum = 0.0, reference = 10.0, exponent = 1.0;
        string category = "-1"; bool run = true;
        if (!data.GetData(0, ref wrapped) || !data.GetDataList(1, observers) || observers.Count == 0
            || !data.GetDataList(2, targets) || targets.Count == 0) return false;
        data.GetDataList(3, facings); data.GetDataList(4, observerWeights); data.GetDataList(5, sensitivities);
        data.GetData(6, ref clearance); data.GetData(7, ref maximum); data.GetData(8, ref reference);
        data.GetData(9, ref exponent); data.GetData(10, ref category); data.GetData(11, ref run);
        if (!run) { Message = "DAENA\nPaused"; return false; }
        XvarnaScene? scene = Unwrap(wrapped);
        if (scene is null) { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene must come directly from XV Scene."); return false; }
        if (!Compatible(facings.Count, targets.Count) || !Compatible(observerWeights.Count, observers.Count) || !Compatible(sensitivities.Count, targets.Count))
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Facings, weights, and sensitivities must be empty, one item, or aligned with their source list."); return false; }
        try
        {
            double resolvedClearance = clearance > 0.0 && double.IsFinite(clearance)
                ? clearance : RhinoDoc.ActiveDoc?.ModelAbsoluteTolerance ?? 1.0e-6;
            VisibilityObserverPoint[] observerValues = observers.Select((point, index) => new VisibilityObserverPoint(
                checked((ulong)index + 1), point.X, point.Y, point.Z, Select(observerWeights, index, 1.0))).ToArray();
            VisibilityTargetPoint[] targetValues = targets.Select((point, index) =>
            {
                Vector3d facing = Select(facings, index, Vector3d.Zero);
                bool hasFacing = facings.Count > 0;
                return new VisibilityTargetPoint(checked((ulong)index + 1), point.X, point.Y, point.Z,
                    Select(sensitivities, index, 1.0), hasFacing, facing.X, facing.Y, facing.Z);
            }).ToArray();
            work = new(scene, observerValues, targetValues,
                new IntervisibilityOptions(resolvedClearance, maximum == 0.0 ? double.PositiveInfinity : maximum,
                    reference, exponent, ParseMask(category)), observers.ToArray(), targets.ToArray());
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException
            or InvalidOperationException or XvarnaNativeException or ObjectDisposedException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); return false; }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(IntervisibilityWork work) =>
        checked((ulong)work.Observers.Length * (ulong)work.Targets.Length);

    /// <inheritdoc />
    protected override IntervisibilityResult Execute(
        IntervisibilityWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Tracing directed pairs"));
        cancellationToken.ThrowIfCancellationRequested();
        IntervisibilityResult result = work.Scene.AnalyzeIntervisibility(work.Observers, work.Targets, work.Options);
        progress.Report((total, total, "Privacy aggregation"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess data, IntervisibilityWork work, IntervisibilityResult result)
    {
        IReadOnlyList<Point3d> observers = work.ObserverPoints;
        IReadOnlyList<Point3d> targets = work.TargetPoints;
        List<Line> visible = []; List<Line> blocked = [];
        DataTree<int> states = new(); DataTree<double> distances = new(); DataTree<double> risks = new(); DataTree<string> blockers = new();
        for (int row = 0; row < observers.Count; row++)
        {
            GH_Path path = new(row);
            for (int column = 0; column < targets.Count; column++)
            {
                IntervisibilityEntry entry = result.Entries[row * targets.Count + column];
                Line line = new(observers[row], targets[column]);
                if (entry.State == VisibilityState.Visible) visible.Add(line);
                else if (entry.State == VisibilityState.Blocked) blocked.Add(line);
                states.Add((int)entry.State, path); distances.Add(entry.Distance, path); risks.Add(entry.PrivacyRisk, path);
                blockers.Add(entry.BlockerObjectId == 0 ? string.Empty : entry.BlockerObjectId.ToString(CultureInfo.InvariantCulture), path);
            }
        }
        int visibleCount = result.Entries.Count(value => value.State == VisibilityState.Visible);
        int blockedCount = result.Entries.Count(value => value.State == VisibilityState.Blocked);
        double peak = result.Entries.Max(value => value.PrivacyRisk);
        string maximum = double.IsPositiveInfinity(result.Options.MaximumDistance) ? "unbounded" : result.Options.MaximumDistance.ToString("G10", CultureInfo.InvariantCulture);
        string report = string.Create(CultureInfo.InvariantCulture,
            $"Engine: DAENA directed intervisibility/privacy over ZAMYAD{Environment.NewLine}" +
            $"Matrix: {observers.Count:N0} observers × {targets.Count:N0} targets = {result.Entries.Count:N0} pairs{Environment.NewLine}" +
            $"States: {visibleCount:N0} visible; {blockedCount:N0} blocked; {result.Entries.Count - visibleCount - blockedCount:N0} excluded/coincident{Environment.NewLine}" +
            $"Risk: observer weight × target sensitivity × facing^({result.Options.FacingExponent:G6}) × 1/(1+(distance/{result.Options.PrivacyReferenceDistance:G6})²); peak {peak:G6}{Environment.NewLine}" +
            $"Policy: clearance {result.Options.EndpointClearance:G10}; max {maximum}; category {result.Options.CategoryMask}{Environment.NewLine}" +
            $"Analysis: {result.AnalysisTimeMicroseconds / 1000.0:N3} ms; hash: {result.ContentHash}");
        data.SetData(0, result); data.SetDataList(1, visible); data.SetDataList(2, blocked);
        data.SetDataTree(3, states); data.SetDataTree(4, distances); data.SetDataTree(5, risks); data.SetDataTree(6, blockers);
        data.SetDataList(7, result.ObserverSummaries.Select(value => value.VisibleFraction));
        data.SetDataList(8, result.TargetSummaries.Select(value => value.ExposureFraction));
        data.SetDataList(9, result.TargetSummaries.Select(value => value.CombinedPrivacyRisk));
        data.SetData(10, result.ContentHash); data.SetData(11, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"DAENA\n{observers.Count:N0} × {targets.Count:N0}");
    }

    private static bool Compatible(int count, int sourceCount) => count is 0 or 1 || count == sourceCount;
    private static T Select<T>(IReadOnlyList<T> values, int index, T fallback) => values.Count == 0 ? fallback : values[values.Count == 1 ? 0 : index];
    private static ulong ParseMask(string value) => value == "-1" ? ulong.MaxValue : ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);
    private static XvarnaScene? Unwrap(object? value) => value switch { XvarnaScene scene => scene, GH_ObjectWrapper { Value: XvarnaScene scene } => scene, _ => null };
}

/// <summary>Immutable captured input for scheduled directed visibility work.</summary>
public readonly record struct IntervisibilityWork(
    XvarnaScene Scene,
    VisibilityObserverPoint[] Observers,
    VisibilityTargetPoint[] Targets,
    IntervisibilityOptions Options,
    Point3d[] ObserverPoints,
    Point3d[] TargetPoints);
