using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

/// <summary>Compiles Rhino meshes into the shared immutable ZAMYAD scene.</summary>
[IoId("ce77444d-c407-44b5-8181-dc06d9704d30")]
public sealed class SceneComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private XvarnaScene? scene;
    private SceneBuildOptions currentOptions;
    private List<string> currentMeshFingerprints = [];
    private string currentStateFingerprint = string.Empty;
    /// <summary>Initializes the component.</summary>
    public SceneComponent() : base(new Nomen("XVARNA Scene", "Immutable, deduplicated BLAS/TLAS scene.", "XVARNA", "01 Scene")) { }
    /// <summary>Deserializes the component.</summary>
    public SceneComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddMesh("Meshes", "M", "Mesh occurrences to compile.", Access.Twig);
        inputs.AddTransform("Transforms", "X", "None, one, or one per mesh.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Object IDs", "OID", "None, one start ID, or one ID per mesh.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Category Masks", "C", "None, one, or one per mesh; -1 selects all.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Meters per Unit", "m/U", "Zero uses the active Rhino document.").Set(0.0);
        inputs.AddNumber("Tolerance", "Tol", "Model-space tolerance; zero uses the active document.").Set(0.0);
        inputs.AddInteger("Threads", "Th", "Zero uses the engine default.").Set(0);
        inputs.AddInteger("Leaf Size", "L", "Maximum primitives per BVH leaf.").Set(4);
        inputs.AddText("Layers", "Layer", "None, one, or one Static/Dynamic value per mesh.", Access.Twig, Requirement.MayBeMissing);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Scene", "S", "Shared native scene for XVARNA queries.");
        outputs.AddText("Hash", "H", "Deterministic scene identity.");
        outputs.AddNumber("Unique Triangles", "UT", "Triangles stored after deduplication.");
        outputs.AddNumber("Instanced Triangles", "IT", "Effective triangles across occurrences.");
        outputs.AddNumber("Memory MB", "MB", "Approximate native memory.");
        outputs.AddText("Report", "R", "Hierarchy, units, rebase and lifecycle provenance.");
        outputs.AddNumber("Operation ms", "ms", "Reuse/delta/rebuild duration.");
        outputs.AddInteger("Revision", "Rev", "Monotonic committed scene revision.");
        outputs.AddText("Lifecycle", "Life", "Reuse, refit or rebuild decision.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        Mesh[] meshes = []; Transform[] transforms = []; string[] ids = []; string[] masks = []; string[] layers = [];
        if (!access.GetItemArray(0, out meshes) || meshes.Length == 0) return;
        access.GetItemArray(1, out transforms); access.GetItemArray(2, out ids); access.GetItemArray(3, out masks);
        access.GetItem(4, out double explicitScale); access.GetItem(5, out double explicitTolerance);
        access.GetItem(6, out int threads); access.GetItem(7, out int leaf);
        access.GetItemArray(8, out layers);
        try
        {
            ValidateBroadcast(meshes.Length, transforms.Length, "Transforms");
            ValidateBroadcast(meshes.Length, ids.Length, "Object IDs");
            ValidateBroadcast(meshes.Length, masks.Length, "Category Masks");
            ValidateBroadcast(meshes.Length, layers.Length, "Layers");
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            double scale = explicitScale > 0 ? explicitScale : RhinoMath.UnitScale(document?.ModelUnitSystem ?? UnitSystem.Meters, UnitSystem.Meters);
            double tolerance = explicitTolerance > 0 ? explicitTolerance : document?.ModelAbsoluteTolerance ?? 1e-6;
            SceneBuildOptions options = new(scale, tolerance * scale, checked((uint)leaf), checked((uint)threads));
            PackedMesh[] packedMeshes = meshes.Select(Gh2MeshAdapter.Pack).ToArray();
            List<string> meshFingerprints = packedMeshes.Select(FingerprintMesh).ToList();
            double[][] packedTransforms = new double[meshes.Length][];
            ulong[] objectIds = new ulong[meshes.Length], categoryMasks = new ulong[meshes.Length];
            SceneLayer[] sceneLayers = new SceneLayer[meshes.Length];
            for (int i = 0; i < meshes.Length; i++)
            {
                packedTransforms[i] = Pack(Select(transforms, i, Transform.Identity));
                objectIds[i] = ResolveId(ids, i); categoryMasks[i] = ResolveMask(Select(masks, i, "-1"));
                sceneLayers[i] = ParseLayer(Select(layers, i, "Static"));
            }
            string stateFingerprint = FingerprintState(meshFingerprints, packedTransforms, objectIds, categoryMasks, sceneLayers, options);
            string lifecycle; double operationMs;
            if (scene is not null && currentOptions == options && currentStateFingerprint == stateFingerprint)
            { lifecycle = "Reused immutable snapshot (input identity hit)"; operationMs = 0; }
            else if (scene is not null && currentOptions == options && currentMeshFingerprints.SequenceEqual(meshFingerprints))
            {
                SceneInstanceDelta[] deltas = Enumerable.Range(0, meshes.Length).Select(i => SceneInstanceDelta.Update(checked((ulong)i + 1), packedTransforms[i], objectIds[i], categoryMasks[i], sceneLayers[i])).ToArray();
                SceneUpdateReport update = scene.ApplyDeltas(deltas); operationMs = update.UpdateTimeMicroseconds / 1000.0;
                lifecycle = $"Delta transaction: static TLAS {update.StaticTlasUpdate}; dynamic TLAS {update.DynamicTlasUpdate}; BLAS {update.ReusedBlasCount:N0} reused/{update.RebuiltBlasCount:N0} rebuilt; quality {update.MaximumRefitQualityRatio:G6}";
            }
            else
            {
                using XvarnaSceneBuilder builder = new(options);
                for (int i = 0; i < meshes.Length; i++) { PackedMesh packed = packedMeshes[i]; ulong meshId = builder.AddMesh(packed.Positions, packed.Triangles); builder.AddInstance(meshId, packedTransforms[i], objectIds[i], checked((ulong)i + 1), categoryMasks[i], sceneLayers[i]); }
                XvarnaScene built = builder.Build(); XvarnaScene? previous = scene; scene = built; previous?.Dispose(); operationMs = built.Statistics.BuildTimeMicroseconds / 1000.0;
                lifecycle = "Full BLAS/TLAS rebuild (geometry or build policy changed)";
            }
            currentOptions = options; currentMeshFingerprints = meshFingerprints; currentStateFingerprint = stateFingerprint;
            XvarnaScene output = scene ?? throw new InvalidOperationException("Scene lifecycle produced no snapshot."); SceneStatistics stats = output.Statistics;
            access.SetItem(0, output); access.SetItem(1, stats.ContentHash);
            access.SetItem(2, (double)stats.UniqueTriangleCount); access.SetItem(3, (double)stats.InstancedTriangleCount);
            access.SetItem(4, stats.ApproximateMemoryBytes / 1_048_576.0);
            access.SetItem(5, string.Create(CultureInfo.InvariantCulture,
                $"ZAMYAD scene ready; revision {output.Revision:N0}; {lifecycle}; {stats.MeshResourceCount:N0} resources, {stats.InstanceCount:N0} instances, {stats.UniqueTriangleCount:N0} unique triangles, {stats.BlasNodeCount:N0} BLAS nodes, {stats.TlasNodeCount:N0} TLAS nodes, {scale:G12} m/unit, hash {stats.ContentHash}"));
            access.SetItem(6, operationMs); access.SetItem(7, checked((int)Math.Min(output.Revision, int.MaxValue))); access.SetItem(8, lifecycle);
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException)
        { access.AddError("Scene compilation failed", exception.Message); }
    }
    private static void ValidateBroadcast(int count, int supplied, string name) { if (supplied is not 0 and not 1 && supplied != count) throw new ArgumentException($"{name} must contain none, one, or one value per mesh."); }
    private static T Select<T>(IReadOnlyList<T> values, int index, T fallback) => values.Count switch { 0 => fallback, 1 => values[0], _ => values[index] };
    private static ulong ResolveId(IReadOnlyList<string> ids, int index) => ids.Count == 0 ? checked((ulong)index + 1) : ids.Count == 1 ? checked(Parse(ids[0], "Object ID") + (ulong)index) : Parse(ids[index], "Object ID");
    private static ulong ResolveMask(string value) => value == "-1" ? ulong.MaxValue : Parse(value, "Category mask");
    private static ulong Parse(string value, string name) => ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong result) ? result : throw new ArgumentException($"{name} '{value}' is not an unsigned 64-bit integer.");
    private static SceneLayer ParseLayer(string value) => value.Trim().ToUpperInvariant() switch { "STATIC" or "S" => SceneLayer.Static, "DYNAMIC" or "D" => SceneLayer.Dynamic, _ => throw new ArgumentException("Layer must be Static or Dynamic.") };
    private static double[] Pack(Transform t) => [t[0, 0], t[0, 1], t[0, 2], t[0, 3], t[1, 0], t[1, 1], t[1, 2], t[1, 3], t[2, 0], t[2, 1], t[2, 2], t[2, 3], t[3, 0], t[3, 1], t[3, 2], t[3, 3]];
    private static string FingerprintMesh(PackedMesh mesh)
    {
        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        foreach (double value in mesh.Positions) hash.AppendData(BitConverter.GetBytes(BitConverter.DoubleToInt64Bits(value)));
        foreach (uint value in mesh.Triangles) hash.AppendData(BitConverter.GetBytes(value));
        return Convert.ToHexString(hash.GetHashAndReset());
    }
    private static string FingerprintState(IReadOnlyList<string> meshes, IReadOnlyList<double[]> transforms, IReadOnlyList<ulong> ids, IReadOnlyList<ulong> categories, IReadOnlyList<SceneLayer> layers, SceneBuildOptions options)
    {
        StringBuilder value = new(); value.Append(options.UnitScaleToMeters.ToString("R", CultureInfo.InvariantCulture)).Append('|').Append(options.AbsoluteToleranceMeters.ToString("R", CultureInfo.InvariantCulture)).Append('|').Append(options.MaximumLeafSize).Append('|').Append(options.ThreadCount).Append('|');
        for (int i = 0; i < meshes.Count; i++) { value.Append(meshes[i]).Append('|').Append(ids[i]).Append('|').Append(categories[i]).Append('|').Append((int)layers[i]).Append('|'); foreach (double coordinate in transforms[i]) value.Append(coordinate.ToString("R", CultureInfo.InvariantCulture)).Append(','); }
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(value.ToString())));
    }
}

/// <summary>Runs ordered closest-hit or any-hit batches on a ZAMYAD scene or VAYU session.</summary>
[IoId("ea5bd8ed-34c3-49f8-9311-2f5d23cac33e")]
public sealed class RayQueryComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public RayQueryComponent() : base(new Nomen("XVARNA Ray Query", "Ordered CPU/GPU traversal with exact attribution.", "XVARNA", "01 Scene")) { }
    /// <summary>Deserializes the component.</summary>
    public RayQueryComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene or Backend", "S/B", "Scene from XVARNA Scene or bounded session from XVARNA Backend.");
        inputs.AddPoint("Origins", "O", "Ray origins.", Access.Twig);
        inputs.AddVector("Directions", "D", "One direction or one per origin.", Access.Twig);
        inputs.AddNumber("Minimum Distance", "Min", "Inclusive minimum distance.").Set(0.0);
        inputs.AddNumber("Maximum Distance", "Max", "Inclusive maximum distance.").Set(1e12);
        inputs.AddText("Category Mask", "C", "Unsigned mask or -1 for all.").Set("-1");
        inputs.AddBoolean("Any Hit", "Any", "Use early-exit visibility mode.").Set(false);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddBoolean("Hit", "H", "Ordered hit state.", Access.Twig);
        outputs.AddNumber("Distance", "T", "Closest distance; NaN for misses.", Access.Twig);
        outputs.AddPoint("Points", "P", "Closest hit points.", Access.Twig);
        outputs.AddText("Object IDs", "OID", "Exact source object IDs.", Access.Twig);
        outputs.AddText("Instance IDs", "IID", "Exact occurrence IDs.", Access.Twig);
        outputs.AddInteger("Triangles", "F", "Canonical triangle indices.", Access.Twig);
        outputs.AddText("Report", "R", "Backend and workload provenance.");
    }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); Point3d[] origins = []; Vector3d[] directions = [];
        if (!access.GetItemArray(1, out origins) || !access.GetItemArray(2, out directions) || origins.Length == 0 || directions.Length == 0) return;
        access.GetItem(3, out double min); access.GetItem(4, out double max); access.GetItem(5, out string maskText); access.GetItem(6, out bool any);
        XvarnaScene? scene = wrapped as XvarnaScene;
        XvarnaComputeSession? compute = wrapped as XvarnaComputeSession;
        if (scene is null && compute is null) { access.AddError("Invalid scene/backend", "Connect XVARNA Scene or XVARNA Backend."); return; }
        if (directions.Length != 1 && directions.Length != origins.Length) { access.AddError("Invalid directions", "Provide one direction or one direction per origin."); return; }
        try
        {
            ulong mask = maskText == "-1" ? ulong.MaxValue : ulong.Parse(maskText, NumberStyles.None, CultureInfo.InvariantCulture);
            SceneRay[] rays = origins.Select((origin, i) => { Vector3d d = directions.Length == 1 ? directions[0] : directions[i]; return new SceneRay(origin.X, origin.Y, origin.Z, d.X, d.Y, d.Z, min, max, mask); }).ToArray();
            access.SetProgress(10);
            if (any)
            {
                ComputeVisibilityResult? accelerated = compute?.TraceAny(rays);
                bool[] hits = accelerated?.Hits ?? scene!.TraceAny(rays);
                EmitAny(access, hits);
                access.SetItem(6, Report(rays.Length, "any-hit", accelerated?.Statistics));
            }
            else
            {
                ComputeTraceResult? accelerated = compute?.TraceClosest(rays);
                SceneHit[] hits = accelerated?.Hits ?? scene!.TraceClosest(rays);
                Gh2Data.SetTwig(access, 0, hits.Select(h => h.Hit).ToArray()); Gh2Data.SetTwig(access, 1, hits.Select(h => h.Hit ? h.Distance : double.NaN).ToArray());
                Gh2Data.SetTwig(access, 2, hits.Select((h, i) => Point(h, origins[i], directions.Length == 1 ? directions[0] : directions[i])).ToArray());
                Gh2Data.SetTwig(access, 3, hits.Select(h => h.Hit ? h.ObjectId.ToString(CultureInfo.InvariantCulture) : string.Empty).ToArray());
                Gh2Data.SetTwig(access, 4, hits.Select(h => h.Hit ? h.InstanceId.ToString(CultureInfo.InvariantCulture) : string.Empty).ToArray());
                Gh2Data.SetTwig(access, 5, hits.Select(h => h.Hit ? checked((int)h.TriangleId) : -1).ToArray());
                access.SetItem(6, Report(rays.Length, "closest-hit", accelerated?.Statistics));
            }
            access.SetProgress(100);
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException or ObjectDisposedException)
        { access.AddError("Ray query failed", exception.Message); }
    }
    private static Point3d Point(SceneHit h, Point3d origin, Vector3d direction) => h.Hit && direction.Unitize() ? origin + direction * h.Distance : Point3d.Unset;
    private static string Report(int rayCount, string query, ComputeBatchStatistics? statistics)
    {
        if (statistics is null) return $"CPU f64 {query}; {rayCount:N0} rays; ordered exact attribution; no transfer/readback.";
        string backend = statistics.Backend == ComputeBackend.PortableGpu ? "VAYU portable GPU" : "VAYU canonical CPU";
        return string.Create(CultureInfo.InvariantCulture,
            $"{backend} {query}; {statistics.RayCount:N0} rays; {statistics.DispatchCount:N0} dispatches; " +
            $"upload/encode {statistics.UploadMicroseconds / 1000.0:N3} ms; execute {statistics.ExecutionMicroseconds / 1000.0:N3} ms; " +
            $"readback {statistics.ReadbackMicroseconds / 1000.0:N3} ms; precision error {statistics.PrecisionErrorMeters:G6} m; " +
            $"persistent buffers {statistics.UsedReusableBuffers}; runtime fallback {statistics.UsedRuntimeFallback}.");
    }
    private static void EmitAny(IDataAccess access, IReadOnlyList<bool> hits)
    {
        Gh2Data.SetTwig(access, 0, hits); Gh2Data.SetTwig(access, 1, hits.Select(_ => double.NaN).ToArray());
        Gh2Data.SetTwig(access, 2, hits.Select(_ => Point3d.Unset).ToArray()); Gh2Data.SetTwig(access, 3, hits.Select(_ => string.Empty).ToArray());
        Gh2Data.SetTwig(access, 4, hits.Select(_ => string.Empty).ToArray()); Gh2Data.SetTwig(access, 5, hits.Select(_ => -1).ToArray());
    }
}

/// <summary>Builds a physically bounded DAENA/HVARE material library.</summary>
[IoId("d9e4ec03-45e5-48f0-91e9-dd1d7054556f")]
public sealed class MaterialsComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    /// <summary>Initializes the component.</summary>
    public MaterialsComponent() : base(new Nomen("XVARNA Materials", "Visible/solar transmission and reflectance by Object ID.", "XVARNA", "03 Environment")) { }
    /// <summary>Deserializes the component.</summary>
    public MaterialsComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Material IDs", "MID", "Unsigned material IDs.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Visible Transmittance", "Tv", "Fractions in [0,1].", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Solar Transmittance", "Ts", "Fractions in [0,1].", Access.Twig, Requirement.MayBeMissing);
        inputs.AddNumber("Reflectance", "R", "Diffuse fractions in [0,1].", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Object IDs", "OID", "Objects to assign.", Access.Twig, Requirement.MayBeMissing);
        inputs.AddText("Assigned Material IDs", "A", "Material per object.", Access.Twig, Requirement.MayBeMissing);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Materials", "M", "Validated immutable material library."); outputs.AddText("Report", "R", "Conservation and assignment policy."); }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        string[] ids = [], objects = [], assigned = []; double[] visible = [], solar = [], reflectance = [];
        access.GetItemArray(0, out ids); access.GetItemArray(1, out visible); access.GetItemArray(2, out solar); access.GetItemArray(3, out reflectance); access.GetItemArray(4, out objects); access.GetItemArray(5, out assigned);
        try
        {
            if (visible.Length != ids.Length || solar.Length != ids.Length || reflectance.Length != ids.Length) throw new ArgumentException("Material definition lists must have equal lengths.");
            if (objects.Length != assigned.Length) throw new ArgumentException("Object and assignment lists must have equal lengths.");
            AnalysisMaterial[] materials = ids.Select((id, i) => new AnalysisMaterial(Parse(id), visible[i], solar[i], reflectance[i])).ToArray();
            MaterialAssignment[] assignments = objects.Select((id, i) => new MaterialAssignment(Parse(id), Parse(assigned[i]))).ToArray();
            AnalysisMaterialLibrary library = new(materials, assignments);
            access.SetItem(0, library); access.SetItem(1, $"{materials.Length:N0} definitions; {assignments.Length:N0} exact Object-ID assignments; unassigned geometry is opaque; transmission + reflectance <= 1.");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException) { access.AddError("Invalid materials", exception.Message); }
    }
    private static ulong Parse(string value) => ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);
}

/// <summary>Seals a complete RASHNU evidence passport supplied as versioned JSON.</summary>
[IoId("a34cb04c-94b7-4672-81cb-92f710ad1554")]
public sealed class EvidenceComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, PropertyNameCaseInsensitive = true, WriteIndented = true, Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) } };
    /// <summary>Initializes the component.</summary>
    public EvidenceComponent() : base(new Nomen("XVARNA Evidence", "Hash-sealed scientific provenance passport.", "XVARNA", "05 Study")) { }
    /// <summary>Deserializes the component.</summary>
    public EvidenceComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Passport JSON", "JSON", "Evidence Passport without a content hash."); }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Passport", "E", "Typed sealed evidence passport."); outputs.AddText("JSON", "JSON", "Shareable sealed JSON."); outputs.AddText("Hash", "H", "BLAKE3 content seal."); }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string json);
        try
        {
            EvidencePassport input = JsonSerializer.Deserialize<EvidencePassport>(json, Json) ?? throw new ArgumentException("Passport JSON is empty.");
            EvidencePassport sealedPassport = EvidenceEngine.Seal(input);
            access.SetItem(0, sealedPassport); access.SetItem(1, JsonSerializer.Serialize(sealedPassport, Json)); access.SetItem(2, sealedPassport.ContentHash);
        }
        catch (Exception exception) when (exception is JsonException or ArgumentException or XvarnaNativeException) { access.AddError("Evidence sealing failed", exception.Message); }
    }
}

/// <summary>Generates a valid Saltelli/Jansen or Morris global-sensitivity design.</summary>
[IoId("b2500814-feca-4168-bae6-825ce98fcafd")]
public sealed class SensitivityDesignComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, PropertyNameCaseInsensitive = true, WriteIndented = true, Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) } };
    /// <summary>Initializes the component.</summary>
    public SensitivityDesignComponent() : base(new Nomen("XVARNA Sensitivity+", "Saltelli/Jansen or Morris design and validated output analysis.", "XVARNA", "05 Study")) { }
    /// <summary>Deserializes the component.</summary>
    public SensitivityDesignComponent(IReader reader) : base(reader) { }
    /// <inheritdoc />
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddText("Variables JSON", "V", "JSON array of VAHMAN DesignVariable objects."); inputs.AddText("Method", "M", "Saltelli or Morris.").Set("Saltelli");
        inputs.AddInteger("Base Samples", "N", "Saltelli base samples or Morris trajectories.").Set(256); inputs.AddInteger("Levels", "L", "Even Morris grid levels.").Set(6); inputs.AddText("Seed", "Seed", "Unsigned deterministic seed.").Set("42");
        inputs.AddText("Outputs JSON", "Y", "Optional row-aligned output matrix; empty emits the design only.").Set(string.Empty);
    }
    /// <inheritdoc />
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddText("Design JSON", "JSON", "Versioned design with row identities."); outputs.AddInteger("Rows", "N", "Required evaluator runs."); outputs.AddText("Hash", "H", "BLAKE3 design identity."); outputs.AddGeneric("Analysis", "A", "Sobol or Morris result when outputs are supplied."); outputs.AddText("Analysis JSON", "AJ", "Portable sensitivity indices/effects."); }
    /// <inheritdoc />
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out string variablesJson); access.GetItem(1, out string methodText); access.GetItem(2, out int samples); access.GetItem(3, out int levels); access.GetItem(4, out string seedText); access.GetItem(5, out string outputsJson);
        try
        {
            DesignVariable[] variables = JsonSerializer.Deserialize<DesignVariable[]>(variablesJson, Json) ?? throw new ArgumentException("Variables JSON is empty.");
            GlobalSensitivityMethod method = methodText.Trim().StartsWith("M", StringComparison.OrdinalIgnoreCase) ? GlobalSensitivityMethod.Morris : GlobalSensitivityMethod.SaltelliJansen;
            GlobalSensitivityDesign design = EvidenceEngine.CreateSensitivityDesign(variables, method, samples, levels, ulong.Parse(seedText, CultureInfo.InvariantCulture));
            access.SetItem(0, JsonSerializer.Serialize(design, Json)); access.SetItem(1, design.Rows.Count); access.SetItem(2, design.ContentHash);
            if (!string.IsNullOrWhiteSpace(outputsJson))
            {
                double[][] values = JsonSerializer.Deserialize<double[][]>(outputsJson, Json) ?? throw new ArgumentException("Outputs JSON is empty.");
                IReadOnlyList<IReadOnlyList<double>> rows = values.Select(value => (IReadOnlyList<double>)value).ToArray();
                GlobalSensitivityResult analysis = EvidenceEngine.AnalyzeSensitivity(design, rows);
                access.SetItem(3, analysis); access.SetItem(4, JsonSerializer.Serialize(analysis, Json));
            }
        }
        catch (Exception exception) when (exception is JsonException or ArgumentException or OverflowException or XvarnaNativeException) { access.AddError("Sensitivity design failed", exception.Message); }
    }
}
