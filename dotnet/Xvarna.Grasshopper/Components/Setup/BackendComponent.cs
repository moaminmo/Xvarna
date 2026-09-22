using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Types;
using Rhino;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Creates and caches a VAYU compute session for one immutable scene.</summary>
public sealed class BackendComponent : GH_Component
{
    private XvarnaComputeSession? currentSession;
    private XvarnaScene? currentScene;
    private ComputeOptions currentOptions;
    private ulong currentSceneRevision = ulong.MaxValue;

    /// <summary>Initializes the portable backend component.</summary>
    public BackendComponent()
        : base(
            "XVARNA Backend",
            "XV Backend",
            "Creates a cached VAYU portable-GPU session with explicit precision, memory, chunking, and canonical CPU fallback.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("ed3353d1-a42e-462e-aa98-b09254a8c787");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGenericParameter("Scene", "S", "Immutable snapshot produced by XV Scene.", GH_ParamAccess.item);
        inputManager.AddTextParameter("Preference", "P", "Auto, GPU, or CPU. GPU forbids fallback.", GH_ParamAccess.item, "Auto");
        inputManager.AddNumberParameter("Maximum Error mm", "E", "Maximum accepted f32 coordinate conversion error in millimetres.", GH_ParamAccess.item, 0.25);
        inputManager.AddNumberParameter("Triangle Limit", "T", "Maximum effective triangles expanded for portable traversal.", GH_ParamAccess.item, 20_000_000.0);
        inputManager.AddNumberParameter("Rays per Dispatch", "R", "Maximum rays per bounded upload/dispatch/readback chunk.", GH_ParamAccess.item, 1_048_576.0);
        inputManager.AddBooleanParameter("Low Power", "LP", "Prefer an integrated/low-power adapter.", GH_ParamAccess.item, false);
        inputManager.AddTextParameter("Adapter Filter", "GPU", "Optional case-insensitive device-name substring from XV Devices; empty uses the power preference.", GH_ParamAccess.item, string.Empty);
        inputManager.AddNumberParameter("Geometry Chunk MB", "Chunk", "Maximum resident two-level geometry chunk in MiB.", GH_ParamAccess.item, 512.0);
        inputManager.AddNumberParameter("GPU Budget MB", "VRAM", "Hard VAYU working-set budget in MiB, including geometry and ray scratch.", GH_ParamAccess.item, 1024.0);
        inputManager.AddBooleanParameter("Scene Cache", "Cache", "Use the checksummed memory/disk portable-plan cache.", GH_ParamAccess.item, true);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Backend", "B", "Cached compute session accepted by XV Ray Query.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Active", "A", "CPU or Portable GPU.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Adapter", "GPU", "Selected adapter name.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Backend, fallback, precision, memory, hierarchy, and initialization provenance.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Fallback", "F", "True when Auto creation selected CPU after a portable-path rejection.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("GPU Memory MB", "MB", "Approximate static portable payload in mebibytes.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Initialization ms", "ms", "Snapshot, device, upload, shader, and pipeline initialization milliseconds.", GH_ParamAccess.item);
        outputManager.AddIntegerParameter("Geometry Chunks", "Chunks", "Number of bounded two-level geometry chunks.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Cache Hit", "Hit", "True when the portable scene plan was restored from cache.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Streamed", "Stream", "True when geometry exceeds one resident chunk and is streamed.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        object? wrappedScene = null;
        string preference = "Auto";
        double maximumError = 0.25;
        double triangleLimit = 20_000_000.0;
        double raysPerDispatch = 1_048_576.0;
        bool lowPower = false;
        string adapterFilter = string.Empty;
        double geometryChunkMb = 512.0;
        double gpuBudgetMb = 1024.0;
        bool enableSceneCache = true;
        if (!dataAccess.GetData(0, ref wrappedScene))
        {
            return;
        }
        dataAccess.GetData(1, ref preference);
        dataAccess.GetData(2, ref maximumError);
        dataAccess.GetData(3, ref triangleLimit);
        dataAccess.GetData(4, ref raysPerDispatch);
        dataAccess.GetData(5, ref lowPower);
        dataAccess.GetData(6, ref adapterFilter);
        dataAccess.GetData(7, ref geometryChunkMb);
        dataAccess.GetData(8, ref gpuBudgetMb);
        dataAccess.GetData(9, ref enableSceneCache);
        XvarnaScene? scene = UnwrapScene(wrappedScene);
        if (scene is null)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Scene must come directly from XV Scene.");
            return;
        }

        try
        {
            double unitScale = ProductContextRegistry.GetUnits(OnPingDocument()).MetersPerModelUnit;
            ComputeOptions options = new(
                Preference: ParsePreference(preference),
                PowerPreference: lowPower ? ComputePowerPreference.LowPower : ComputePowerPreference.HighPerformance,
                MaximumPrecisionErrorMeters: maximumError * 0.001,
                MaximumExpandedTriangleCount: checked((ulong)triangleLimit),
                MaximumRaysPerDispatch: checked((ulong)raysPerDispatch),
                MaximumGeometryChunkBytes: ToBytes(geometryChunkMb, "Geometry Chunk MB"),
                MaximumGpuMemoryBytes: ToBytes(gpuBudgetMb, "GPU Budget MB"),
                EnableSceneCache: enableSceneCache,
                AdapterNameFilter: string.IsNullOrWhiteSpace(adapterFilter) ? null : adapterFilter.Trim());
            if (currentSession is null
                || !ReferenceEquals(currentScene, scene)
                || currentSceneRevision != scene.Revision
                || currentOptions != options)
            {
                XvarnaComputeSession built = XvarnaComputeSession.Create(scene, options);
                currentSession?.Dispose();
                currentSession = built;
                currentScene = scene;
                currentSceneRevision = scene.Revision;
                currentOptions = options;
            }
            ComputeSessionInfo info = currentSession.Info;
            string report = BuildReport(info, unitScale);
            dataAccess.SetData(0, currentSession);
            dataAccess.SetData(1, info.Backend == ComputeBackend.PortableGpu ? "Portable GPU" : "CPU f64");
            dataAccess.SetData(2, info.AdapterName);
            dataAccess.SetData(3, report);
            dataAccess.SetData(4, info.UsedCreationFallback);
            dataAccess.SetData(5, info.ApproximateGpuBytes / 1_048_576.0);
            dataAccess.SetData(6, info.InitializationMicroseconds / 1000.0);
            dataAccess.SetData(7, checked((int)Math.Min(info.GeometryChunkCount, int.MaxValue)));
            dataAccess.SetData(8, info.SceneCacheHit);
            dataAccess.SetData(9, info.StreamedGeometry);
            if (info.UsedCreationFallback)
            {
                AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, info.Message);
            }
            else if (info.Backend == ComputeBackend.PortableGpu)
            {
                AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, $"VAYU active on {info.AdapterName}.");
            }
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException
            or ArgumentException
            or InvalidOperationException
            or OverflowException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    /// <inheritdoc />
    public override void RemovedFromDocument(GH_Document document)
    {
        currentSession?.Dispose();
        currentSession = null;
        currentScene = null;
        currentSceneRevision = ulong.MaxValue;
        base.RemovedFromDocument(document);
    }

    private static XvarnaScene? UnwrapScene(object? value) =>
        value switch
        {
            XvarnaScene scene => scene,
            GH_ObjectWrapper { Value: XvarnaScene scene } => scene,
            _ => null,
        };

    private static ComputePreference ParsePreference(string value) =>
        value.Trim().ToUpperInvariant() switch
        {
            "AUTO" => ComputePreference.Auto,
            "GPU" or "PORTABLE GPU" or "PORTABLEGPU" => ComputePreference.RequirePortableGpu,
            "CPU" => ComputePreference.Cpu,
            _ => throw new ArgumentException("Preference must be Auto, GPU, or CPU."),
        };

    private static ulong ToBytes(double mebibytes, string name)
    {
        if (!double.IsFinite(mebibytes) || mebibytes <= 0.0 || mebibytes > ulong.MaxValue / 1_048_576.0)
        {
            throw new ArgumentOutOfRangeException(name, "Budget must be a finite positive number of MiB.");
        }
        return checked((ulong)Math.Ceiling(mebibytes * 1_048_576.0));
    }

    private static string BuildReport(ComputeSessionInfo info, double unitScale) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"Backend: {(info.Backend == ComputeBackend.PortableGpu ? "VAYU Portable GPU" : "Canonical CPU f64")}{Environment.NewLine}" +
            $"Adapter: {(string.IsNullOrEmpty(info.AdapterName) ? "none" : info.AdapterName)}; API code {info.AdapterBackendCode}; type {info.DeviceTypeCode}{Environment.NewLine}" +
            $"Device: vendor 0x{info.VendorId:x8}; id 0x{info.DeviceId:x8}{Environment.NewLine}" +
            $"Portable scene: {info.TriangleCount:N0} triangles; {info.NodeCount:N0} nodes; {info.ApproximateGpuBytes / 1_048_576.0:N3} MiB{Environment.NewLine}" +
            $"Geometry: {info.GeometryChunkCount:N0} chunk(s); streamed {info.StreamedGeometry}; cache hit {info.SceneCacheHit}{Environment.NewLine}" +
            $"Precision: {info.ScenePrecisionErrorMeters / unitScale:G6} model units ({info.ScenePrecisionErrorMeters:G6} m){Environment.NewLine}" +
            $"Initialization: {info.InitializationMicroseconds / 1000.0:N3} ms{Environment.NewLine}" +
            $"Status: {info.Message}");
}
