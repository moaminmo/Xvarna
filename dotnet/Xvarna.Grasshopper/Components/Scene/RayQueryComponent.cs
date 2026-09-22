using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Scene;

/// <summary>Executes ordered closest-hit or visibility batches against an XVARNA scene.</summary>
public sealed class RayQueryComponent : ScheduledAnalysisComponent<RayQueryWork, RayQueryWorkResult>
{
    /// <summary>Initializes the XVARNA batched ray-query component.</summary>
    public RayQueryComponent()
        : base(
            "XVARNA Ray Query",
            "XV Ray Query",
            "Traces an ordered, parallel ray batch against an immutable XV Scene with object, instance, mesh, and triangle attribution.",
            "XVARNA",
            "01 Scene")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("ea5bd8ed-34c3-49f8-9311-2f5d23cac33e");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "ZAMYAD Ray Query";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        inputManager.AddPointParameter("Origins", "O", "Ray origins in active Rhino model units.", GH_ParamAccess.list);
        inputManager.AddVectorParameter("Directions", "D", "One direction for all origins or one direction per origin. Native validation normalizes each vector.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Minimum Distance", "Min", "Inclusive model-space minimum distance.", GH_ParamAccess.item, 0.0);
        inputManager.AddNumberParameter("Maximum Distance", "Max", "Inclusive model-space maximum distance.", GH_ParamAccess.item, 1.0e12);
        inputManager.AddTextParameter("Category Mask", "C", "Exact unsigned 64-bit instance-category filter. -1 selects all categories.", GH_ParamAccess.item, "-1");
        inputManager.AddBooleanParameter("Any Hit", "Any", "Use early-exit visibility mode. Attribution outputs are only available in closest mode.", GH_ParamAccess.item, false);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddBooleanParameter("Hit", "H", "Ordered intersection or visibility result.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Distance", "T", "Closest distance in active model units; NaN for misses or Any Hit mode.", GH_ParamAccess.list);
        outputManager.AddPointParameter("Points", "P", "Closest hit points; Unset for misses or Any Hit mode.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Object IDs", "OID", "Exact unsigned 64-bit source object IDs.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Instance IDs", "IID", "Exact unsigned 64-bit occurrence IDs.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Mesh IDs", "MID", "Exact unsigned 64-bit deduplicated mesh-resource IDs.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Triangles", "F", "Local canonical triangle indices; -1 for misses.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Barycentric U", "U", "First closest-hit barycentric coordinate.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Barycentric V", "V", "Second closest-hit barycentric coordinate.", GH_ParamAccess.list);
        outputManager.AddBooleanParameter("Front Face", "FF", "True when the closest ray hit the world-space front side.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Backend", "B", "Backend that produced this batch.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Trace Report", "R", "Dispatch, transfer, execution, precision, and runtime-fallback provenance.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess dataAccess, out RayQueryWork work)
    {
        work = default;
        object? wrappedScene = null;
        List<Point3d> origins = [];
        List<Vector3d> directions = [];
        double minimum = 0.0;
        double maximum = 1.0e12;
        string categoryMask = "-1";
        bool anyHit = false;
        if (!dataAccess.GetData(0, ref wrappedScene)
            || !dataAccess.GetDataList(1, origins)
            || origins.Count == 0
            || !dataAccess.GetDataList(2, directions)
            || directions.Count == 0)
        {
            return false;
        }
        dataAccess.GetData(3, ref minimum);
        dataAccess.GetData(4, ref maximum);
        dataAccess.GetData(5, ref categoryMask);
        dataAccess.GetData(6, ref anyHit);

        XvarnaScene? scene = UnwrapScene(wrappedScene);
        XvarnaComputeSession? compute = UnwrapCompute(wrappedScene);
        if (scene is null && compute is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Input must come from XV Scene or XV Backend.");
            return false;
        }
        if (directions.Count != 1 && directions.Count != origins.Count)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Directions must contain one vector or exactly one vector per origin.");
            return false;
        }

        try
        {
            ulong mask = string.Equals(categoryMask, "-1", StringComparison.Ordinal)
                ? ulong.MaxValue
                : ulong.Parse(categoryMask, NumberStyles.None, CultureInfo.InvariantCulture);
            SceneRay[] rays = origins
                .Select((origin, index) =>
                {
                    Vector3d direction = directions.Count == 1 ? directions[0] : directions[index];
                    return new SceneRay(
                        origin.X,
                        origin.Y,
                        origin.Z,
                        direction.X,
                        direction.Y,
                        direction.Z,
                        minimum,
                        maximum,
                        mask);
                })
                .ToArray();
            work = new(scene, compute, rays, origins.ToArray(), directions.ToArray(), anyHit);
            return true;
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or ObjectDisposedException
            or OverflowException
            or ArgumentException
            or InvalidOperationException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(RayQueryWork work) => checked((ulong)work.Rays.Length);

    /// <inheritdoc />
    protected override RayQueryWorkResult Execute(
        RayQueryWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, work.AnyHit ? "Any-hit traversal" : "Closest-hit traversal"));
        cancellationToken.ThrowIfCancellationRequested();
        if (work.AnyHit)
        {
            ComputeVisibilityResult? accelerated = work.Compute?.TraceAny(work.Rays);
            bool[] visibility = accelerated?.Hits ?? work.Scene!.TraceAny(work.Rays);
            progress.Report((total, total, "Ordered visibility result"));
            return new(null, visibility, accelerated?.Statistics);
        }
        ComputeTraceResult? acceleratedHits = work.Compute?.TraceClosest(work.Rays);
        SceneHit[] hits = acceleratedHits?.Hits ?? work.Scene!.TraceClosest(work.Rays);
        progress.Report((total, total, "Attribution result"));
        return new(hits, null, acceleratedHits?.Statistics);
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess dataAccess, RayQueryWork work, RayQueryWorkResult result)
    {
        if (result.Visibility is not null)
        {
            EmitAnyHit(dataAccess, result.Visibility);
            EmitBackend(dataAccess, result.Statistics);
            return;
        }
        SceneHit[] hits = result.Hits ?? [];
        dataAccess.SetDataList(0, hits.Select(hit => hit.Hit));
        dataAccess.SetDataList(1, hits.Select(hit => hit.Hit ? hit.Distance : double.NaN));
        dataAccess.SetDataList(2, hits.Select((hit, index) => HitPoint(hit, work.Origins[index], work.Directions.Length == 1 ? work.Directions[0] : work.Directions[index])));
        dataAccess.SetDataList(3, hits.Select(hit => hit.Hit ? hit.ObjectId.ToString() : string.Empty));
        dataAccess.SetDataList(4, hits.Select(hit => hit.Hit ? hit.InstanceId.ToString() : string.Empty));
        dataAccess.SetDataList(5, hits.Select(hit => hit.Hit ? hit.MeshId.ToString() : string.Empty));
        dataAccess.SetDataList(6, hits.Select(hit => hit.Hit ? checked((int)hit.TriangleId) : -1));
        dataAccess.SetDataList(7, hits.Select(hit => hit.Hit ? hit.BarycentricU : double.NaN));
        dataAccess.SetDataList(8, hits.Select(hit => hit.Hit ? hit.BarycentricV : double.NaN));
        dataAccess.SetDataList(9, hits.Select(hit => hit.Hit && hit.FrontFace));
        EmitBackend(dataAccess, result.Statistics);
    }

    private static XvarnaScene? UnwrapScene(object? value) =>
        value switch
        {
            XvarnaScene scene => scene,
            GH_ObjectWrapper { Value: XvarnaScene scene } => scene,
            _ => null,
        };

    private static XvarnaComputeSession? UnwrapCompute(object? value) =>
        value switch
        {
            XvarnaComputeSession session => session,
            GH_ObjectWrapper { Value: XvarnaComputeSession session } => session,
            _ => null,
        };

    private static Point3d HitPoint(SceneHit hit, Point3d origin, Vector3d direction)
    {
        if (!hit.Hit || !direction.Unitize())
        {
            return Point3d.Unset;
        }
        return origin + direction * hit.Distance;
    }

    private static void EmitAnyHit(IGH_DataAccess dataAccess, IReadOnlyList<bool> hits)
    {
        dataAccess.SetDataList(0, hits);
        dataAccess.SetDataList(1, hits.Select(_ => double.NaN));
        dataAccess.SetDataList(2, hits.Select(_ => Point3d.Unset));
        dataAccess.SetDataList(3, hits.Select(_ => string.Empty));
        dataAccess.SetDataList(4, hits.Select(_ => string.Empty));
        dataAccess.SetDataList(5, hits.Select(_ => string.Empty));
        dataAccess.SetDataList(6, hits.Select(_ => -1));
        dataAccess.SetDataList(7, hits.Select(_ => double.NaN));
        dataAccess.SetDataList(8, hits.Select(_ => double.NaN));
        dataAccess.SetDataList(9, hits.Select(_ => false));
    }

    private static void EmitBackend(IGH_DataAccess dataAccess, ComputeBatchStatistics? stats)
    {
        if (stats is null)
        {
            dataAccess.SetData(10, "CPU f64");
            dataAccess.SetData(11, "Canonical scene traversal; no transfer or readback.");
            return;
        }
        dataAccess.SetData(10, stats.Backend == ComputeBackend.PortableGpu ? "VAYU Portable GPU" : "CPU f64");
        dataAccess.SetData(
            11,
            string.Create(
                CultureInfo.InvariantCulture,
                $"Rays: {stats.RayCount:N0}; dispatches: {stats.DispatchCount:N0}{Environment.NewLine}" +
                $"Upload/encode: {stats.UploadMicroseconds / 1000.0:N3} ms; execute: {stats.ExecutionMicroseconds / 1000.0:N3} ms; readback: {stats.ReadbackMicroseconds / 1000.0:N3} ms{Environment.NewLine}" +
                $"Precision error: {stats.PrecisionErrorMeters:G6} m; persistent arena: {stats.UsedReusableBuffers}; runtime fallback: {stats.UsedRuntimeFallback}"));
    }
}

/// <summary>Immutable captured input for one scheduled ray-query batch.</summary>
public readonly record struct RayQueryWork(
    XvarnaScene? Scene,
    XvarnaComputeSession? Compute,
    SceneRay[] Rays,
    Point3d[] Origins,
    Vector3d[] Directions,
    bool AnyHit);

/// <summary>Closest or any-hit scheduled payload.</summary>
public sealed record RayQueryWorkResult(
    SceneHit[]? Hits,
    bool[]? Visibility,
    ComputeBatchStatistics? Statistics);
