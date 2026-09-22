using System.Drawing;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Grasshopper.Kernel;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Scene;

/// <summary>Compiles reusable Rhino meshes and instances into an immutable native scene.</summary>
public sealed class SceneBuildComponent : GH_Component
{
    private XvarnaScene? currentScene;
    private SceneBuildOptions currentOptions;
    private List<string> currentMeshFingerprints = [];
    private string currentStateFingerprint = string.Empty;
    private double lastOperationMilliseconds;

    /// <summary>Initializes the XVARNA scene compiler component.</summary>
    public SceneBuildComponent()
        : base(
            "XVARNA Scene",
            "XV Scene",
            "Compiles meshes and affine instances into a deduplicated, origin-rebased, double-precision BLAS/TLAS scene for fast batched queries.",
            "XVARNA",
            "01 Scene")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("ce77444d-c407-44b5-8181-dc06d9704d30");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddMeshParameter("Meshes", "M", "Mesh occurrences to compile. Identical canonical meshes are stored once.", GH_ParamAccess.list);
        inputManager.AddTransformParameter("Transforms", "X", "Optional transforms: none for identity, one for all meshes, or one per mesh.", GH_ParamAccess.list);
        inputManager.AddTextParameter("Object IDs", "OID", "Optional exact unsigned 64-bit object IDs: none for 1..N, one start value, or one per mesh.", GH_ParamAccess.list);
        inputManager.AddTextParameter("Category Masks", "C", "Optional exact unsigned 64-bit category masks: none for all, one for all meshes, or one per mesh. -1 selects all.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Tolerance", "T", "Positive model-space tolerance; 0 uses the active Rhino document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddIntegerParameter("Threads", "Th", "Worker threads for query batches; 0 uses the engine default.", GH_ParamAccess.item, 0);
        inputManager.AddIntegerParameter("Leaf Size", "L", "Maximum primitives per BVH leaf, from 1 through 64.", GH_ParamAccess.item, 4);
        inputManager.AddTextParameter("Layers", "Layer", "Optional acceleration layer per occurrence: Static or Dynamic. None defaults to Static.", GH_ParamAccess.list);
        inputManager[1].Optional = true;
        inputManager[2].Optional = true;
        inputManager[3].Optional = true;
        inputManager[7].Optional = true;
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Scene", "S", "Immutable native scene snapshot for XV Ray Query.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Deterministic build, hierarchy, memory, unit, rebase, and deduplication report.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Hash", "H", "BLAKE3 identity of geometry, instances, transforms, categories, units, and rebase.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Unique Triangles", "UT", "Triangles stored once after resource deduplication.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Instanced Triangles", "IT", "Effective triangle count across all occurrences.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Memory MB", "MB", "Approximate native scene memory in mebibytes.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Build ms", "ms", "Native scene compile duration in milliseconds.", GH_ParamAccess.item);
        outputManager.AddIntegerParameter("Revision", "Rev", "Monotonic scene revision; changes only after a committed delta or rebuild.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Lifecycle", "Life", "Snapshot reuse, TLAS refit, or rebuild decision for this solve.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        List<Mesh> meshes = [];
        List<Transform> transforms = [];
        List<string> objectIds = [];
        List<string> categoryMasks = [];
        double tolerance = 0.0;
        int threads = 0;
        int leafSize = 4;
        List<string> layers = [];
        if (!dataAccess.GetDataList(0, meshes) || meshes.Count == 0)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "At least one mesh is required.");
            return;
        }
        dataAccess.GetDataList(1, transforms);
        dataAccess.GetDataList(2, objectIds);
        dataAccess.GetDataList(3, categoryMasks);
        dataAccess.GetData(4, ref tolerance);
        dataAccess.GetData(5, ref threads);
        dataAccess.GetData(6, ref leafSize);
        dataAccess.GetDataList(7, layers);

        if (!HasValidDataMatching(meshes.Count, transforms.Count)
            || !HasValidDataMatching(meshes.Count, objectIds.Count)
            || !HasValidDataMatching(meshes.Count, categoryMasks.Count))
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Transforms, Object IDs, and Category Masks must contain zero, one, or exactly one value per mesh.");
            return;
        }
        if (!HasValidDataMatching(meshes.Count, layers.Count))
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Layers must contain zero, one, or exactly one value per mesh.");
            return;
        }

        try
        {
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            XvarnaUnitContext units = ProductContextRegistry.GetUnits(OnPingDocument());
            double unitScale = units.MetersPerModelUnit;
            double modelTolerance = tolerance > 0.0 && double.IsFinite(tolerance)
                ? tolerance
                : units.AbsoluteToleranceModelUnits;
            SceneBuildOptions options = new(
                unitScale,
                modelTolerance * unitScale,
                checked((uint)leafSize),
                checked((uint)threads));
            List<PackedRhinoMesh> packedMeshes = meshes.Select(RhinoMeshAdapter.Pack).ToList();
            List<string> meshFingerprints = packedMeshes.Select(FingerprintMesh).ToList();
            List<double[]> packedTransforms = new(meshes.Count);
            List<ulong> resolvedObjectIds = new(meshes.Count);
            List<ulong> resolvedCategoryMasks = new(meshes.Count);
            List<SceneLayer> resolvedLayers = new(meshes.Count);
            for (int index = 0; index < meshes.Count; index++)
            {
                Transform transform = Select(transforms, index, Transform.Identity);
                packedTransforms.Add(PackTransform(transform));
                resolvedObjectIds.Add(ResolveObjectId(objectIds, index));
                resolvedCategoryMasks.Add(ResolveCategoryMask(Select(categoryMasks, index, "-1")));
                resolvedLayers.Add(ParseLayer(Select(layers, index, "Static")));
            }
            string stateFingerprint = FingerprintState(
                meshFingerprints,
                packedTransforms,
                resolvedObjectIds,
                resolvedCategoryMasks,
                resolvedLayers,
                options);
            string lifecycle;
            if (currentScene is not null
                && currentOptions == options
                && currentStateFingerprint == stateFingerprint)
            {
                lifecycle = "Reused immutable snapshot (input identity hit)";
                lastOperationMilliseconds = 0.0;
            }
            else if (currentScene is not null
                && currentOptions == options
                && currentMeshFingerprints.SequenceEqual(meshFingerprints))
            {
                SceneInstanceDelta[] deltas = Enumerable.Range(0, meshes.Count)
                    .Select(index => SceneInstanceDelta.Update(
                        checked((ulong)index + 1),
                        packedTransforms[index],
                        resolvedObjectIds[index],
                        resolvedCategoryMasks[index],
                        resolvedLayers[index]))
                    .ToArray();
                SceneUpdateReport update = currentScene.ApplyDeltas(deltas);
                lifecycle = string.Create(
                    CultureInfo.InvariantCulture,
                    $"Delta transaction: static TLAS {update.StaticTlasUpdate}; dynamic TLAS {update.DynamicTlasUpdate}; " +
                    $"BLAS {update.ReusedBlasCount:N0} reused/{update.RebuiltBlasCount:N0} rebuilt; " +
                    $"quality {update.MaximumRefitQualityRatio:G6}");
                lastOperationMilliseconds = update.UpdateTimeMicroseconds / 1000.0;
            }
            else
            {
                using XvarnaSceneBuilder builder = new(options);
                for (int index = 0; index < meshes.Count; index++)
                {
                    PackedRhinoMesh packed = packedMeshes[index];
                    ulong meshId = builder.AddMesh(packed.Positions, packed.Triangles);
                    builder.AddInstance(
                        meshId,
                        packedTransforms[index],
                        resolvedObjectIds[index],
                        checked((ulong)index + 1),
                        resolvedCategoryMasks[index],
                        resolvedLayers[index]);
                }
                XvarnaScene built = builder.Build();
                currentScene?.Dispose();
                currentScene = built;
                lifecycle = "Full BLAS/TLAS rebuild (geometry or build policy changed)";
                lastOperationMilliseconds = built.Statistics.BuildTimeMicroseconds / 1000.0;
            }
            currentOptions = options;
            currentMeshFingerprints = meshFingerprints;
            currentStateFingerprint = stateFingerprint;
            XvarnaScene scene = currentScene ?? throw new InvalidOperationException("Scene lifecycle did not publish a snapshot.");
            SceneStatistics stats = scene.Statistics;
            string report = BuildReport(stats, units.ModelUnitName, unitScale, lifecycle, scene.Revision);
            if (stats.MeshResourceCount < stats.InstanceCount)
            {
                AddRuntimeMessage(
                    GH_RuntimeMessageLevel.Remark,
                    $"Resource deduplication stored {stats.MeshResourceCount:N0} unique mesh(es) for {stats.InstanceCount:N0} occurrence(s).");
            }
            dataAccess.SetData(0, scene);
            dataAccess.SetData(1, report);
            dataAccess.SetData(2, stats.ContentHash);
            dataAccess.SetData(3, (double)stats.UniqueTriangleCount);
            dataAccess.SetData(4, (double)stats.InstancedTriangleCount);
            dataAccess.SetData(5, stats.ApproximateMemoryBytes / 1_048_576.0);
            dataAccess.SetData(6, lastOperationMilliseconds);
            dataAccess.SetData(7, checked((int)Math.Min(scene.Revision, int.MaxValue)));
            dataAccess.SetData(8, lifecycle);
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException
            or OverflowException
            or ArgumentException
            or InvalidOperationException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    /// <inheritdoc />
    public override void RemovedFromDocument(GH_Document document)
    {
        currentScene?.Dispose();
        currentScene = null;
        currentMeshFingerprints.Clear();
        currentStateFingerprint = string.Empty;
        base.RemovedFromDocument(document);
    }

    private static bool HasValidDataMatching(int meshCount, int valueCount) =>
        valueCount is 0 or 1 || valueCount == meshCount;

    private static T Select<T>(IReadOnlyList<T> values, int index, T fallback) =>
        values.Count switch
        {
            0 => fallback,
            1 => values[0],
            _ => values[index],
        };

    private static ulong ResolveObjectId(IReadOnlyList<string> values, int index)
    {
        if (values.Count == 0)
        {
            return checked((ulong)index + 1);
        }
        string value = Select(values, index, string.Empty);
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
        {
            throw new ArgumentException($"Object ID '{value}' is not an unsigned 64-bit integer.");
        }
        return values.Count == 1 ? checked(parsed + (ulong)index) : parsed;
    }

    private static ulong ResolveCategoryMask(string value)
    {
        if (string.Equals(value, "-1", StringComparison.Ordinal))
        {
            return ulong.MaxValue;
        }
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
        {
            throw new ArgumentException($"Category mask '{value}' is not -1 or an unsigned 64-bit integer.");
        }
        return parsed;
    }

    private static SceneLayer ParseLayer(string value) =>
        value.Trim().ToUpperInvariant() switch
        {
            "STATIC" or "S" => SceneLayer.Static,
            "DYNAMIC" or "D" => SceneLayer.Dynamic,
            _ => throw new ArgumentException($"Scene layer '{value}' must be Static or Dynamic."),
        };

    private static string FingerprintMesh(PackedRhinoMesh mesh)
    {
        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        foreach (double value in mesh.Positions)
        {
            hash.AppendData(BitConverter.GetBytes(BitConverter.DoubleToInt64Bits(value)));
        }
        foreach (uint value in mesh.Triangles)
        {
            hash.AppendData(BitConverter.GetBytes(value));
        }
        return Convert.ToHexString(hash.GetHashAndReset());
    }

    private static string FingerprintState(
        IReadOnlyList<string> meshes,
        IReadOnlyList<double[]> transforms,
        IReadOnlyList<ulong> objectIds,
        IReadOnlyList<ulong> categories,
        IReadOnlyList<SceneLayer> layers,
        SceneBuildOptions options)
    {
        StringBuilder value = new();
        value.Append(options.UnitScaleToMeters.ToString("R", CultureInfo.InvariantCulture)).Append('|')
            .Append(options.AbsoluteToleranceMeters.ToString("R", CultureInfo.InvariantCulture)).Append('|')
            .Append(options.MaximumLeafSize).Append('|').Append(options.ThreadCount).Append('|');
        for (int index = 0; index < meshes.Count; index++)
        {
            value.Append(meshes[index]).Append('|').Append(objectIds[index]).Append('|')
                .Append(categories[index]).Append('|').Append((int)layers[index]).Append('|');
            foreach (double coordinate in transforms[index])
            {
                value.Append(coordinate.ToString("R", CultureInfo.InvariantCulture)).Append(',');
            }
        }
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(value.ToString())));
    }

    private static double[] PackTransform(Transform transform) =>
    [
        transform[0, 0], transform[0, 1], transform[0, 2], transform[0, 3],
        transform[1, 0], transform[1, 1], transform[1, 2], transform[1, 3],
        transform[2, 0], transform[2, 1], transform[2, 2], transform[2, 3],
        transform[3, 0], transform[3, 1], transform[3, 2], transform[3, 3],
    ];

    private static string BuildReport(
        SceneStatistics stats,
        string sourceUnit,
        double unitScale,
        string lifecycle,
        ulong revision) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"Status: immutable scene ready; revision {revision:N0}{Environment.NewLine}" +
            $"Lifecycle: {lifecycle}{Environment.NewLine}" +
            $"Resources: {stats.MeshResourceCount:N0}; instances: {stats.InstanceCount:N0}{Environment.NewLine}" +
            $"Triangles: {stats.UniqueTriangleCount:N0} unique; {stats.InstancedTriangleCount:N0} instanced{Environment.NewLine}" +
            $"BVH: {stats.BlasNodeCount:N0} BLAS nodes; {stats.TlasNodeCount:N0} TLAS nodes; depth {stats.MaximumBvhDepth:N0}{Environment.NewLine}" +
            $"Build: {stats.BuildTimeMicroseconds / 1000.0:N3} ms; memory: {stats.ApproximateMemoryBytes / 1_048_576.0:N3} MiB; workers: {stats.ThreadCount:N0}{Environment.NewLine}" +
            $"Units: {sourceUnit}; 1 model unit = {unitScale:G12} m{Environment.NewLine}" +
            $"Rebase (m): ({stats.RebaseOriginMeters.X:G12}, {stats.RebaseOriginMeters.Y:G12}, {stats.RebaseOriginMeters.Z:G12}){Environment.NewLine}" +
            $"Scene hash: {stats.ContentHash}");
}
