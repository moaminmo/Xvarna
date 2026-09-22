using System.Globalization;
using Grasshopper2.Components;
using Grasshopper2.Parameters;
using Grasshopper2.UI;
using GrasshopperIO;
using Rhino.Geometry;
using Xvarna.Grasshopper2.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Components;

[IoId("ed3353d1-a42e-462e-aa98-b09254a8c787")]
public sealed class BackendComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    private XvarnaComputeSession? session;
    public BackendComponent() : base(new Nomen("XVARNA Backend", "Creates a bounded VAYU GPU/CPU execution session.", "XVARNA", "00 Setup")) { }
    public BackendComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs)
    {
        inputs.AddGeneric("Scene", "S", "Scene from XVARNA Scene.");
        inputs.AddText("Mode", "M", "Auto, GPU, or CPU.").Set("Auto");
        inputs.AddText("Adapter Filter", "GPU", "Optional case-insensitive adapter name filter.").Set(string.Empty);
        inputs.AddNumber("Precision mm", "E", "Maximum GPU conversion error in millimetres.").Set(0.25);
        inputs.AddInteger("Rays/Dispatch", "R", "Maximum rays per GPU dispatch.").Set(1_048_576);
        inputs.AddInteger("Geometry MB", "G", "Geometry working-set budget.").Set(512);
        inputs.AddInteger("VRAM MB", "V", "Total GPU-memory budget.").Set(1024);
        inputs.AddBoolean("Cache", "C", "Use the validated portable-plan cache.").Set(true);
    }
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddGeneric("Backend", "B", "Reusable VAYU execution session."); outputs.AddText("Selected", "M", "Actual backend.");
        outputs.AddText("Adapter", "GPU", "Selected adapter."); outputs.AddBoolean("Fallback", "F", "Auto creation used CPU fallback.");
        outputs.AddNumber("GPU MB", "MB", "Approximate resident payload."); outputs.AddText("Report", "R", "Device, cache, chunk and precision provenance.");
    }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); access.GetItem(1, out string mode); access.GetItem(2, out string filter); access.GetItem(3, out double precisionMm);
        access.GetItem(4, out int rays); access.GetItem(5, out int geometryMb); access.GetItem(6, out int vramMb); access.GetItem(7, out bool cache);
        if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try
        {
            ComputePreference preference = mode.Trim().ToUpperInvariant() switch { "AUTO" => ComputePreference.Auto, "GPU" => ComputePreference.RequirePortableGpu, "CPU" => ComputePreference.Cpu, _ => throw new ArgumentException("Mode must be Auto, GPU, or CPU.") };
            if (rays <= 0 || geometryMb <= 0 || vramMb <= 0 || !double.IsFinite(precisionMm) || precisionMm <= 0) throw new ArgumentException("All resource and precision budgets must be positive.");
            ComputeOptions options = new(preference, ComputePowerPreference.HighPerformance, precisionMm / 1000.0, 20_000_000, checked((ulong)rays), checked((ulong)geometryMb * 1_048_576), checked((ulong)vramMb * 1_048_576), cache, string.IsNullOrWhiteSpace(filter) ? null : filter.Trim());
            XvarnaComputeSession created = XvarnaComputeSession.Create(scene, options); XvarnaComputeSession? old = session; session = created; old?.Dispose();
            ComputeSessionInfo info = created.Info; access.SetItem(0, created); access.SetItem(1, info.Backend.ToString()); access.SetItem(2, info.AdapterName); access.SetItem(3, info.UsedCreationFallback);
            access.SetItem(4, info.ApproximateGpuBytes / 1_048_576.0); access.SetItem(5, $"{info.Message}; {info.TriangleCount:N0} triangles; {info.GeometryChunkCount:N0} chunks; cache {(info.SceneCacheHit ? "hit" : "miss")}; streamed {info.StreamedGeometry}; precision {info.ScenePrecisionErrorMeters:G6} m; init {info.InitializationMicroseconds / 1000.0:N3} ms");
        }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException or InvalidOperationException) { access.AddError("Backend creation failed", exception.Message); }
    }
}

[IoId("5d0d2f8c-2ff5-485f-a23a-54537da8671d")]
public sealed class ComputeRuntimeComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public ComputeRuntimeComponent() : base(new Nomen("XVARNA Runtime", "Reads live VAYU throughput and device-health telemetry.", "XVARNA", "00 Setup")) { }
    public ComputeRuntimeComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) => inputs.AddGeneric("Backend", "B", "Backend from XVARNA Backend.");
    protected override void AddOutputs(OutputAdder outputs)
    {
        outputs.AddNumber("Traces", "T", "Submitted trace batches."); outputs.AddNumber("Rays", "R", "Submitted rays."); outputs.AddNumber("Dispatches", "D", "GPU dispatches.");
        outputs.AddNumber("Reuse", "U", "Reusable-buffer batches."); outputs.AddNumber("Fallbacks", "F", "Runtime CPU fallbacks."); outputs.AddNumber("Scratch MB", "MB", "Retained scratch memory.");
        outputs.AddBoolean("Healthy", "OK", "Logical device health."); outputs.AddText("Report", "Info", "Complete cumulative telemetry.");
    }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); if (wrapped is not XvarnaComputeSession session) { access.AddError("Invalid backend", "Connect XVARNA Backend."); return; }
        try { ComputeRuntimeStatistics s = session.GetRuntimeStatistics(); access.SetItem(0, (double)s.TraceCount); access.SetItem(1, (double)s.RayCount); access.SetItem(2, (double)s.DispatchCount); access.SetItem(3, (double)s.ReusableBufferBatchCount); access.SetItem(4, (double)s.RuntimeFallbackCount); access.SetItem(5, s.ScratchGpuBytes / 1_048_576.0); access.SetItem(6, s.DeviceHealthy); access.SetItem(7, $"scratch generations {s.ScratchGenerationCount:N0}, capacity {s.ScratchCapacityRays:N0} rays; upload {s.UploadedBytes / 1_048_576.0:N3} MiB, readback {s.ReadbackBytes / 1_048_576.0:N3} MiB; geometry uploads {s.GeometryUploadCount:N0}; device losses {s.DeviceLossCount:N0}; errors {s.ExecutionErrorCount:N0}; {s.LastError}"); }
        catch (Exception exception) when (exception is XvarnaNativeException or ObjectDisposedException) { access.AddError("Runtime query failed", exception.Message); }
    }
}

[IoId("9cab45d3-0324-45de-b860-35d7027c29cb")]
public sealed class SchedulerComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SchedulerComponent() : base(new Nomen("XVARNA Scheduler", "Configures shared bounded analysis scheduling.", "XVARNA", "00 Setup")) { }
    public SchedulerComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddInteger("Workers", "W", "Maximum concurrent analyses; zero selects a safe default.").Set(0); inputs.AddInteger("Queue", "Q", "Maximum waiting jobs.").Set(64); inputs.AddBoolean("Apply", "A", "Apply configuration.").Set(true); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddNumber("Queued", "Q", "Queued jobs."); outputs.AddNumber("Running", "R", "Running jobs."); outputs.AddNumber("Completed", "C", "Completed jobs."); outputs.AddNumber("Cancelled", "X", "Cancelled jobs."); outputs.AddNumber("Failed", "F", "Failed jobs."); outputs.AddText("Active", "A", "Active job progress.", Access.Twig); outputs.AddText("Report", "Info", "Scheduler capacity and outcomes."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out int workers); access.GetItem(1, out int queue); access.GetItem(2, out bool apply);
        try { if (workers < 0 || queue <= 0) throw new ArgumentException("Workers must be non-negative and Queue positive."); if (apply) XvarnaAnalysisScheduler.Configure(new(workers == 0 ? Math.Max(1, Environment.ProcessorCount / 2) : workers, queue)); AnalysisSchedulerStatistics s = XvarnaAnalysisScheduler.GetStatistics(); access.SetItem(0, (double)s.QueuedJobs); access.SetItem(1, (double)s.RunningJobs); access.SetItem(2, (double)s.CompletedJobs); access.SetItem(3, (double)s.CancelledJobs); access.SetItem(4, (double)s.FailedJobs); Gh2Data.SetTwig(access, 5, XvarnaAnalysisScheduler.GetActiveJobs().Select(p => $"{p.Name}: {p.Fraction:P1} — {p.Phase}").ToArray()); access.SetItem(6, $"workers {s.MaximumConcurrency}; queue capacity {s.MaximumQueuedJobs}; rejected {0:N0}; stale {s.StaleJobs:N0}"); }
        catch (ArgumentException exception) { access.AddError("Scheduler configuration failed", exception.Message); }
    }
}

[IoId("e402d486-edba-43cf-a1fb-994534793c36")]
public sealed class DiagnosticsComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public DiagnosticsComponent() : base(new Nomen("XVARNA Diagnostics", "Aggregates engine, cache and scheduler health.", "XVARNA", "00 Setup")) { }
    public DiagnosticsComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddText("Bundle Directory", "Dir", "Directory for an optional redacted support ZIP.", Access.Item, Requirement.MayBeMissing); inputs.AddBoolean("Write Bundle", "W", "Capture hardware, versions, cache, scheduler and errors.").Set(false); inputs.AddText("Error Chain", "Err", "Optional error/details to include after path/user redaction.", Access.Item, Requirement.MayBeMissing); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddBoolean("Ready", "OK", "Native ABI is ready."); outputs.AddText("Engine", "E", "Engine status."); outputs.AddText("Cache", "C", "Cache telemetry."); outputs.AddText("Scheduler", "S", "Scheduler telemetry."); outputs.AddText("Diagnostic Bundle", "ZIP", "Redacted support archive path."); outputs.AddText("Bundle Hash", "H", "SHA-256 identity of diagnostic manifest."); }
    protected override void Process(IDataAccess access) { string? directory = null, errors = null; if (!access.GetNull(0)) access.GetItem(0, out directory); access.GetItem(1, out bool write); if (!access.GetNull(2)) access.GetItem(2, out errors); EngineProbeResult e = EngineProbe.TryLoad(); CacheStatistics c = XvarnaCache.GetStatistics(); AnalysisSchedulerStatistics s = XvarnaAnalysisScheduler.GetStatistics(); access.SetItem(0, e.IsAvailable); access.SetItem(1, e.Status); access.SetItem(2, $"memory {c.MemoryHitCount:N0} hits, disk {c.DiskHitCount:N0}, misses {c.MissCount:N0}, corruption {c.CorruptionCount:N0}"); access.SetItem(3, $"queued {s.QueuedJobs:N0}, running {s.RunningJobs:N0}, completed {s.CompletedJobs:N0}, failed {s.FailedJobs:N0}"); if (write) { try { DiagnosticBundleResult bundle = XvarnaDiagnosticBundle.Create(directory ?? string.Empty, null, errors); access.SetItem(4, bundle.ArchivePath); access.SetItem(5, bundle.ContentHash); } catch (Exception exception) when (exception is ArgumentException or IOException or UnauthorizedAccessException or XvarnaNativeException or InvalidOperationException) { access.AddError("Diagnostic bundle failed", exception.Message); } } else { access.SetItem(4, string.Empty); access.SetItem(5, string.Empty); } }
}

[IoId("03011a61-e74b-41d9-bfbf-4989f1fb4aa3")]
public sealed class LegendComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public LegendComponent() : base(new Nomen("XVARNA Legend", "Builds a robust scientific legend and colour mapping.", "XVARNA", "00 Setup")) { }
    public LegendComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddNumber("Values", "V", "Finite result values.", Access.Twig); inputs.AddText("Domain", "D", "Full, Percentile, Symmetric, or Fixed.").Set("Percentile"); inputs.AddText("Palette", "P", "Viridis, Cividis, or Diverging.").Set("Viridis"); inputs.AddInteger("Steps", "N", "Legend steps.").Set(9); inputs.AddNumber("Minimum", "A", "Fixed minimum or lower percentile.").Set(2); inputs.AddNumber("Maximum", "B", "Fixed maximum or upper percentile.").Set(98); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddNumber("Normalized", "N", "Clamped normalized values.", Access.Twig); outputs.AddText("Colours", "C", "RGBA hexadecimal colours.", Access.Twig); outputs.AddNumber("Breaks", "B", "Legend break values.", Access.Twig); outputs.AddText("Report", "R", "Resolved domain and clipping."); }
    protected override void Process(IDataAccess access)
    {
        double[] values = []; access.GetItemArray(0, out values); access.GetItem(1, out string domain); access.GetItem(2, out string palette); access.GetItem(3, out int steps); access.GetItem(4, out double a); access.GetItem(5, out double b);
        try { XvarnaLegendDomainMode mode = Enum.Parse<XvarnaLegendDomainMode>(domain, true); XvarnaPalette colors = Enum.Parse<XvarnaPalette>(palette, true); XvarnaLegendResult r = XvarnaLegend.Build(values, mode == XvarnaLegendDomainMode.Percentile ? new(mode, colors, 0, 1, a / 100.0, b / 100.0, steps) : new(mode, colors, a, b, 0.02, 0.98, steps)); Gh2Data.SetTwig(access, 0, r.NormalizedValues); Gh2Data.SetTwig(access, 1, r.Colors.Select(c => $"#{c.Red:X2}{c.Green:X2}{c.Blue:X2}{c.Alpha:X2}").ToArray()); Gh2Data.SetTwig(access, 2, r.Ticks); access.SetItem(3, $"domain [{r.Minimum:G12}, {r.Maximum:G12}]; clipped low {r.ClippedLowCount:N0}, high {r.ClippedHighCount:N0}; non-finite {r.NonFiniteCount:N0}"); }
        catch (Exception exception) when (exception is ArgumentException or OverflowException) { access.AddError("Legend failed", exception.Message); }
    }
}

[IoId("3a43edab-dc24-4e89-86a4-0c1c929935a7")]
public sealed class MeshCheckComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public MeshCheckComponent() : base(new Nomen("XVARNA Mesh Check", "Audits mesh validity, topology and numeric precision.", "XVARNA", "01 Scene")) { }
    public MeshCheckComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddMesh("Mesh", "M", "Mesh to audit."); inputs.AddNumber("Tolerance", "T", "Positive model-space tolerance.").Set(1e-6); inputs.AddNumber("f32 Ratio", "R", "Maximum rounding-error fraction of tolerance.").Set(0.25); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Audit", "A", "Complete immutable audit result."); outputs.AddBoolean("Ready", "OK", "Safe for analysis without conservative repair."); outputs.AddBoolean("Watertight", "W", "Closed manifold with consistent winding."); outputs.AddText("Face Issues", "F", "Per-face issue flags.", Access.Twig); outputs.AddText("Hash", "H", "BLAKE3 geometry identity."); outputs.AddText("Report", "R", "Complete diagnostic report."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out Mesh mesh); access.GetItem(1, out double tolerance); access.GetItem(2, out double ratio); try { PackedMesh p = Gh2MeshAdapter.Pack(mesh); MeshAuditResult r = MeshAuditor.Audit(p.Positions, p.Triangles, tolerance, ratio); access.SetItem(0, r); access.SetItem(1, r.IsAnalysisReady); access.SetItem(2, r.IsWatertight); Gh2Data.SetTwig(access, 3, r.FaceIssues.Select(v => v.ToString()).ToArray()); access.SetItem(4, r.ContentHash); access.SetItem(5, r.ToReport()); } catch (Exception exception) when (exception is ArgumentException or XvarnaNativeException or InvalidOperationException) { access.AddError("Mesh audit failed", exception.Message); } }
}

[IoId("8cb80583-9be0-483f-aa10-e6946ed4d2de")]
public sealed class SunHoursComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SunHoursComponent() : base(new Nomen("XVARNA Sun Hours", "Direct-sun duration with first-blocker attribution.", "XVARNA", "03 Environment")) { }
    public SunHoursComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddGeneric("Sun Set", "Sun", "XVARNA Sun Vectors result."); inputs.AddPoint("Sensors", "P", "Sensor points.", Access.Twig); inputs.AddVector("Normals", "N", "One or one per sensor.", Access.Twig); inputs.AddText("Sensor IDs", "ID", "None, one start, or one per sensor.", Access.Twig, Requirement.MayBeMissing); inputs.AddNumber("Offset", "E", "Ray offset in model units.").Set(1e-4); inputs.AddNumber("Maximum Distance", "Max", "Zero means unbounded.").Set(0); inputs.AddNumber("Maximum Incidence", "A", "Maximum incidence angle in degrees.").Set(90); inputs.AddText("Category Mask", "C", "-1 selects all.").Set("-1"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete temporal result."); outputs.AddNumber("Direct Hours", "Sun", "Weighted direct-sun hours.", Access.Twig); outputs.AddNumber("Shadow Hours", "Shade", "Weighted shadow hours.", Access.Twig); outputs.AddNumber("Access", "A", "Solar access ratio.", Access.Twig); outputs.AddText("Dominant Occluder", "OID", "Dominant blocking Object ID.", Access.Twig); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Sampling and timing provenance."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object sceneObject); access.GetItem(1, out object sunObject); Point3d[] points = []; Vector3d[] normals = []; string[] ids = []; access.GetItemArray(2, out points); access.GetItemArray(3, out normals); access.GetItemArray(4, out ids); access.GetItem(5, out double offset); access.GetItem(6, out double max); access.GetItem(7, out double angle); access.GetItem(8, out string maskText);
        if (sceneObject is not XvarnaScene scene || sunObject is not XvarnaSunSet sun) { access.AddError("Invalid input", "Connect XVARNA Scene and Sun Vectors."); return; }
        try { if (points.Length == 0 || (normals.Length != 1 && normals.Length != points.Length) || (ids.Length is not 0 and not 1 && ids.Length != points.Length)) throw new ArgumentException("Provide sensors plus one normal or one per sensor; IDs may be empty, one start, or one per sensor."); ulong mask = maskText == "-1" ? ulong.MaxValue : ulong.Parse(maskText, CultureInfo.InvariantCulture); SolarSensor[] sensors = points.Select((p, i) => { Vector3d n = normals.Length == 1 ? normals[0] : normals[i]; ulong id = ids.Length == 0 ? (ulong)i + 1 : ids.Length == 1 ? ulong.Parse(ids[0], CultureInfo.InvariantCulture) + (ulong)i : ulong.Parse(ids[i], CultureInfo.InvariantCulture); return new SolarSensor(id, p.X, p.Y, p.Z, n.X, n.Y, n.Z); }).ToArray(); DirectSunResult r = scene.AnalyzeDirectSun(sensors, sun, new(offset, max == 0 ? double.PositiveInfinity : max, Math.Cos(angle * Math.PI / 180.0), mask)); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Summaries.Select(v => v.DirectSunHours).ToArray()); Gh2Data.SetTwig(access, 2, r.Summaries.Select(v => v.ShadowHours).ToArray()); Gh2Data.SetTwig(access, 3, r.Summaries.Select(v => v.SolarAccessRatio).ToArray()); Gh2Data.SetTwig(access, 4, r.Summaries.Select(v => v.DominantOccluderObjectId == 0 ? string.Empty : v.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture)).ToArray()); access.SetItem(5, r.ContentHash); access.SetItem(6, $"{sensors.Length:N0} sensors × {sun.Samples.Count:N0} times; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms; CPU f64 canonical; full temporal attribution"); }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException or ObjectDisposedException) { access.AddError("Sun Hours failed", exception.Message); }
    }
}

[IoId("fe598553-6b7f-4820-b0af-5b4402d426f5")]
public sealed class SkyViewComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public SkyViewComponent() : base(new Nomen("XVARNA Sky View", "Sky-view factor, convergence and obstruction attribution.", "XVARNA", "03 Environment")) { }
    public SkyViewComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Scene", "S", "XVARNA Scene."); inputs.AddPoint("Sensors", "P", "Sensor points.", Access.Twig); inputs.AddVector("Normals", "N", "One or one per sensor.", Access.Twig); inputs.AddInteger("Samples", "Q", "Fibonacci samples per sensor.").Set(2048); inputs.AddText("Seed", "Seed", "Unsigned seed.").Set("0"); inputs.AddNumber("Offset", "E", "Ray offset.").Set(1e-4); inputs.AddNumber("Maximum Distance", "Max", "Zero means unbounded.").Set(0); inputs.AddText("Category Mask", "C", "-1 selects all.").Set("-1"); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddGeneric("Result", "R", "Complete ASMAN result."); outputs.AddNumber("Sky View Factor", "SVF", "Cosine-weighted visible fraction.", Access.Twig); outputs.AddNumber("Visible Hemisphere", "VH", "Unweighted visible fraction.", Access.Twig); outputs.AddNumber("Convergence Delta", "D", "Interleaved nested estimate delta.", Access.Twig); outputs.AddText("Dominant Occluder", "OID", "Dominant Object ID.", Access.Twig); outputs.AddText("Hash", "H", "Result identity."); outputs.AddText("Report", "Info", "Estimator and timing provenance."); }
    protected override void Process(IDataAccess access)
    {
        access.GetItem(0, out object wrapped); Point3d[] points = []; Vector3d[] normals = []; access.GetItemArray(1, out points); access.GetItemArray(2, out normals); access.GetItem(3, out int samples); access.GetItem(4, out string seedText); access.GetItem(5, out double offset); access.GetItem(6, out double max); access.GetItem(7, out string maskText);
        if (wrapped is not XvarnaScene scene) { access.AddError("Invalid scene", "Connect XVARNA Scene."); return; }
        try { if (points.Length == 0 || (normals.Length != 1 && normals.Length != points.Length) || samples is < 16 or > 262144) throw new ArgumentException("Provide sensors, one or per-sensor normal, and 16..262144 samples."); SkySensor[] sensors = points.Select((p, i) => { Vector3d n = normals.Length == 1 ? normals[0] : normals[i]; return new SkySensor((ulong)i + 1, p.X, p.Y, p.Z, n.X, n.Y, n.Z); }).ToArray(); SolutionCancellationToken.ThrowIfCancellationRequested(); access.SetProgress(5); SkyViewResult r = scene.AnalyzeSkyView(sensors, new((uint)samples, ulong.Parse(seedText, CultureInfo.InvariantCulture), offset, max == 0 ? double.PositiveInfinity : max, maskText == "-1" ? ulong.MaxValue : ulong.Parse(maskText, CultureInfo.InvariantCulture)), SolutionCancellationToken); access.SetProgress(100); access.SetItem(0, r); Gh2Data.SetTwig(access, 1, r.Summaries.Select(v => v.CosineWeightedSkyViewFactor).ToArray()); Gh2Data.SetTwig(access, 2, r.Summaries.Select(v => v.VisibleHemisphereFraction).ToArray()); Gh2Data.SetTwig(access, 3, r.Summaries.Select(v => v.CosineConvergenceDelta).ToArray()); Gh2Data.SetTwig(access, 4, r.Summaries.Select(v => v.DominantOccluderObjectId == 0 ? string.Empty : v.DominantOccluderObjectId.ToString(CultureInfo.InvariantCulture)).ToArray()); access.SetItem(5, r.ContentHash); access.SetItem(6, $"equal-solid-angle Fibonacci hemisphere; {sensors.Length:N0} × {samples:N0}; {r.AnalysisTimeMicroseconds / 1000.0:N3} ms"); }
        catch (Exception exception) when (exception is ArgumentException or OverflowException or XvarnaNativeException or ObjectDisposedException or OperationCanceledException) { access.AddError("Sky View failed", exception.Message); }
    }
}

[IoId("757853d4-7a64-4b35-9f4e-2caecf2bb6f8")]
public sealed class ShadowMaskComponent : Xvarna.Grasshopper2.Interop.XvarnaComponent
{
    public ShadowMaskComponent() : base(new Nomen("XVARNA Shadow Mask", "Extracts complete sampled sky-direction states.", "XVARNA", "03 Environment")) { }
    public ShadowMaskComponent(IReader reader) : base(reader) { }
    protected override void AddInputs(InputAdder inputs) { inputs.AddGeneric("Sky Result", "R", "Result from XVARNA Sky View."); inputs.AddInteger("Sensor Index", "i", "Zero-based sensor index.").Set(0); }
    protected override void AddOutputs(OutputAdder outputs) { outputs.AddVector("Directions", "D", "Sampled hemisphere directions.", Access.Twig); outputs.AddBoolean("Visible", "V", "Open-sky states.", Access.Twig); outputs.AddText("Occluder IDs", "OID", "First blocker Object IDs.", Access.Twig); outputs.AddNumber("Distance", "T", "First-hit distance.", Access.Twig); outputs.AddText("Report", "R", "Selected sensor and sample count."); }
    protected override void Process(IDataAccess access) { access.GetItem(0, out object wrapped); access.GetItem(1, out int index); if (wrapped is not SkyViewResult result) { access.AddError("Invalid result", "Connect XVARNA Sky View."); return; } try { int count = checked((int)result.Options.SampleCount); if (index < 0 || index >= result.Sensors.Count) throw new ArgumentOutOfRangeException(nameof(index)); SkyRayEntry[] row = result.Timeline.Skip(index * count).Take(count).ToArray(); Gh2Data.SetTwig(access, 0, row.Select(v => new Vector3d(v.DirectionX, v.DirectionY, v.DirectionZ)).ToArray()); Gh2Data.SetTwig(access, 1, row.Select(v => v.State == SkyRayState.Visible).ToArray()); Gh2Data.SetTwig(access, 2, row.Select(v => v.ObjectId == 0 ? string.Empty : v.ObjectId.ToString(CultureInfo.InvariantCulture)).ToArray()); Gh2Data.SetTwig(access, 3, row.Select(v => v.State == SkyRayState.Blocked ? v.Distance : double.NaN).ToArray()); access.SetItem(4, $"sensor {index}; {row.Length:N0} deterministic directions; complete first-hit attribution"); } catch (Exception exception) when (exception is ArgumentException or OverflowException) { access.AddError("Shadow Mask failed", exception.Message); } }
}
