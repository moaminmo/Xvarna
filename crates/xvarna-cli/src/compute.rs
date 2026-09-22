//! VAYU adapter discovery, parity, and throughput workflows.

use std::{path::PathBuf, process::ExitCode, sync::Arc, time::Instant};
use xvarna_geometry::{MeshAuditOptions, Vec3, audit_mesh};
use xvarna_scene::{QueryRay, RayQueryBackend, RayQueryExecutionSummary, Scene};
use xvarna_vayu::{
    ActiveBackend, ComputeBatchStats, ComputeOptions, ComputePowerPreference, ComputePreference,
    ComputeSession, VayuCacheConfig, clear_scene_cache, compare_with_cpu, configure_scene_cache,
    enumerate_adapters, scene_cache_config, scene_cache_stats,
};

use super::{build_cli_scene, load_obj, micros_to_milliseconds};

#[derive(Clone, Debug)]
struct BenchmarkOptions {
    preference: ComputePreference,
    ray_count: usize,
    thread_count: usize,
    maximum_precision_error_meters: f64,
    maximum_expanded_triangle_count: usize,
    maximum_rays_per_dispatch: usize,
    maximum_geometry_chunk_bytes: usize,
    maximum_gpu_memory_bytes: usize,
    enable_scene_cache: bool,
    parity_tolerance_meters: f64,
    warmup_count: usize,
    iteration_count: usize,
    low_power: bool,
    adapter_name_filter: Option<String>,
    json: bool,
}

#[derive(Clone, Copy, Debug)]
struct TimingDistribution {
    minimum: u64,
    p50: u64,
    p95: u64,
    mean: u64,
}

/// Shared backend policy for DAENA domain commands.
#[derive(Clone, Debug)]
pub struct DomainBackendOptions {
    preference: ComputePreference,
    power_preference: ComputePowerPreference,
    adapter_name_filter: Option<String>,
    maximum_precision_error_meters: f64,
    maximum_expanded_triangle_count: usize,
    maximum_rays_per_dispatch: usize,
    maximum_geometry_chunk_bytes: usize,
    maximum_gpu_memory_bytes: usize,
    enable_scene_cache: bool,
}

impl Default for DomainBackendOptions {
    fn default() -> Self {
        Self {
            preference: ComputePreference::Cpu,
            power_preference: ComputePowerPreference::HighPerformance,
            adapter_name_filter: None,
            maximum_precision_error_meters: 0.000_25,
            maximum_expanded_triangle_count: 20_000_000,
            maximum_rays_per_dispatch: 1_048_576,
            maximum_geometry_chunk_bytes: 512 * 1024 * 1024,
            maximum_gpu_memory_bytes: 1024 * 1024 * 1024,
            enable_scene_cache: true,
        }
    }
}

impl DomainBackendOptions {
    pub const fn set_low_power(&mut self) {
        self.power_preference = ComputePowerPreference::LowPower;
    }

    pub(crate) fn parse_value(&mut self, option: &str, value: &str) -> Result<bool, ()> {
        match option {
            "--backend" => {
                self.preference = match value.to_ascii_lowercase().as_str() {
                    "auto" => ComputePreference::Auto,
                    "gpu" => ComputePreference::RequirePortableGpu,
                    "cpu" => ComputePreference::Cpu,
                    _ => {
                        eprintln!("error: backend must be auto, gpu, or cpu");
                        return Err(());
                    }
                };
            }
            "--adapter" => self.adapter_name_filter = Some(value.to_owned()),
            "--max-error" => {
                self.maximum_precision_error_meters = parse_positive(value, "max-error")?;
            }
            "--max-triangles" => {
                self.maximum_expanded_triangle_count =
                    parse_count(value, "max-triangles", 1, u32::MAX as usize)?;
            }
            "--max-rays-per-dispatch" => {
                self.maximum_rays_per_dispatch =
                    parse_count(value, "max-rays-per-dispatch", 1, u32::MAX as usize)?;
            }
            "--geometry-chunk-mb" => {
                self.maximum_geometry_chunk_bytes = parse_mebibytes(value, "geometry-chunk-mb")?;
            }
            "--gpu-memory-mb" => {
                self.maximum_gpu_memory_bytes = parse_mebibytes(value, "gpu-memory-mb")?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(crate) fn create_session(&self, scene: Scene) -> Result<ComputeSession, String> {
        ComputeSession::create(
            Arc::new(scene),
            ComputeOptions {
                preference: self.preference,
                power_preference: self.power_preference,
                adapter_name_filter: self.adapter_name_filter.clone(),
                maximum_precision_error_meters: self.maximum_precision_error_meters,
                maximum_expanded_triangle_count: self.maximum_expanded_triangle_count,
                maximum_rays_per_dispatch: self.maximum_rays_per_dispatch,
                maximum_geometry_chunk_bytes: self.maximum_geometry_chunk_bytes,
                maximum_gpu_memory_bytes: self.maximum_gpu_memory_bytes,
                enable_scene_cache: self.enable_scene_cache,
            },
        )
        .map_err(|error| error.to_string())
    }

    pub const fn disable_cache(&mut self) {
        self.enable_scene_cache = false;
    }
}

pub fn print_domain_execution_json(execution: &RayQueryExecutionSummary) {
    print!(
        concat!(
            "\"execution\":{{\"backend\":\"{}\",\"adapter\":\"{}\",",
            "\"creationFallback\":{},\"runtimeFallbackBatches\":{},",
            "\"reusableBufferBatches\":{},\"batches\":{},\"rays\":{},\"dispatches\":{},",
            "\"uploadMicroseconds\":{},\"executionMicroseconds\":{},",
            "\"readbackMicroseconds\":{},\"precisionErrorMeters\":{},",
            "\"fallbackReason\":\"{}\"}}"
        ),
        domain_backend_name(execution.backend),
        escape_json(execution.adapter_name.as_deref().unwrap_or("")),
        execution.used_creation_fallback,
        execution.runtime_fallback_batch_count,
        execution.reusable_buffer_batch_count,
        execution.batch_count,
        execution.ray_count,
        execution.dispatch_count,
        execution.upload_microseconds,
        execution.execution_microseconds,
        execution.readback_microseconds,
        execution.maximum_precision_error_meters,
        escape_json(execution.fallback_reason.as_deref().unwrap_or("")),
    );
}

pub fn print_domain_execution_text(execution: &RayQueryExecutionSummary) {
    println!(
        "Backend: {}; adapter: {}; batches: {}; rays: {}; dispatches: {}",
        domain_backend_name(execution.backend),
        execution.adapter_name.as_deref().unwrap_or("none"),
        execution.batch_count,
        execution.ray_count,
        execution.dispatch_count
    );
    println!(
        "Upload/execute/readback: {:.3} / {:.3} / {:.3} ms; precision {:.6e} m; reusable batches={}; creation fallback={}; runtime fallback batches={}",
        micros_to_milliseconds(u128::from(execution.upload_microseconds)),
        micros_to_milliseconds(u128::from(execution.execution_microseconds)),
        micros_to_milliseconds(u128::from(execution.readback_microseconds)),
        execution.maximum_precision_error_meters,
        execution.reusable_buffer_batch_count,
        execution.used_creation_fallback,
        execution.runtime_fallback_batch_count
    );
    if let Some(reason) = &execution.fallback_reason {
        println!("Fallback reason: {reason}");
    }
}

const fn domain_backend_name(backend: RayQueryBackend) -> &'static str {
    match backend {
        RayQueryBackend::Cpu => "CPU f64",
        RayQueryBackend::PortableGpu => "VAYU Portable GPU",
        RayQueryBackend::Mixed => "Mixed GPU/CPU",
    }
}

pub fn run_devices(arguments: &[String]) -> ExitCode {
    let json = match arguments {
        [] => false,
        [value] if value == "--json" => true,
        _ => {
            eprintln!("error: devices accepts only --json");
            return ExitCode::from(2);
        }
    };
    let adapters = enumerate_adapters();
    if json {
        print!("{{\"schemaVersion\":\"0.16.0\",\"engine\":\"VAYU\",\"adapters\":[");
        for (index, adapter) in adapters.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"name\":\"{}\",\"driver\":\"{}\",\"driverInfo\":\"{}\",\"backend\":\"{}\",\"deviceType\":\"{}\",\"vendorId\":{},\"deviceId\":{}}}",
                escape_json(&adapter.name),
                escape_json(&adapter.driver),
                escape_json(&adapter.driver_info),
                escape_json(&adapter.backend),
                escape_json(&adapter.device_type),
                adapter.vendor_id,
                adapter.device_id
            );
        }
        println!("]}}");
    } else {
        println!("XVARNA VAYU portable compute adapters");
        if adapters.is_empty() {
            println!("No adapter reported; Auto sessions will use canonical CPU f64.");
        }
        for (index, adapter) in adapters.iter().enumerate() {
            println!(
                "{}: {} | {} | {} | driver={} {} | vendor=0x{:08x} device=0x{:08x}",
                index,
                adapter.name,
                adapter.backend,
                adapter.device_type,
                adapter.driver,
                adapter.driver_info,
                adapter.vendor_id,
                adapter.device_id
            );
        }
    }
    ExitCode::SUCCESS
}

pub fn run_cache(arguments: &[String]) -> ExitCode {
    match arguments {
        [command] if command == "status" => print_cache_status(false),
        [command, flag] if command == "status" && flag == "--json" => print_cache_status(true),
        [command] if command == "clear" => match clear_scene_cache() {
            Ok(()) => {
                println!("XVARNA cache cleared");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
        [command, directory, memory, disk] if command == "configure" => {
            let Ok(memory_budget_bytes) = parse_byte_budget(memory, "memory bytes") else {
                return ExitCode::from(2);
            };
            let Ok(disk_budget_bytes) = parse_byte_budget(disk, "disk bytes") else {
                return ExitCode::from(2);
            };
            match configure_scene_cache(VayuCacheConfig {
                directory: PathBuf::from(directory),
                memory_budget_bytes,
                disk_budget_bytes,
            }) {
                Ok(()) => print_cache_status(false),
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "error: cache expects status [--json], clear, or configure <DIR> <MEMORY_BYTES> <DISK_BYTES>"
            );
            ExitCode::from(2)
        }
    }
}

fn print_cache_status(json: bool) -> ExitCode {
    let stats = match scene_cache_stats() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let config = match scene_cache_config() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!(
            "{{\"schemaVersion\":\"0.16.0\",\"directory\":\"{}\",\"memoryHits\":{},\"diskHits\":{},\"misses\":{},\"writes\":{},\"evictions\":{},\"corruptions\":{},\"memoryBytes\":{},\"diskBytes\":{},\"memoryEntries\":{},\"diskEntries\":{},\"lastEvent\":\"{}\"}}",
            escape_json(&config.directory.to_string_lossy()),
            stats.memory_hit_count,
            stats.disk_hit_count,
            stats.miss_count,
            stats.write_count,
            stats.eviction_count,
            stats.corruption_count,
            stats.memory_bytes,
            stats.disk_bytes,
            stats.memory_entry_count,
            stats.disk_entry_count,
            escape_json(&stats.last_event),
        );
    } else {
        println!("XVARNA VAYU cache");
        println!("Directory: {}", config.directory.display());
        println!(
            "Hits: {} memory + {} disk; misses: {}; writes: {}",
            stats.memory_hit_count, stats.disk_hit_count, stats.miss_count, stats.write_count
        );
        println!(
            "Occupancy: {} bytes/{} entries memory; {} bytes/{} entries disk",
            stats.memory_bytes, stats.memory_entry_count, stats.disk_bytes, stats.disk_entry_count
        );
        println!(
            "Evictions: {}; corruption recoveries: {}; last event: {}",
            stats.eviction_count, stats.corruption_count, stats.last_event
        );
    }
    ExitCode::SUCCESS
}

fn parse_byte_budget(value: &str, name: &str) -> Result<u64, ()> {
    value
        .parse::<u64>()
        .map_err(|_| eprintln!("error: {name} must be a non-negative integer"))
}

#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
pub fn run_backend_benchmark(input: &str, arguments: &[String]) -> ExitCode {
    let Ok(options) = parse_options(arguments) else {
        return ExitCode::from(2);
    };
    let Some(imported) = load_obj(input) else {
        return ExitCode::FAILURE;
    };
    let audit = match audit_mesh(
        &imported.mesh,
        MeshAuditOptions::try_new(1.0e-6, 0.25).expect("fixed audit options valid"),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: mesh audit failed: {error:?}");
            return ExitCode::FAILURE;
        }
    };
    let Some(bounds) = audit.bounds else {
        eprintln!("error: benchmark mesh has no finite bounds");
        return ExitCode::FAILURE;
    };
    let Some(scene) = build_cli_scene(input, options.thread_count, "backend-benchmark") else {
        return ExitCode::FAILURE;
    };
    let scene = Arc::new(scene);
    let rays = benchmark_rays(bounds.min, bounds.max, options.ray_count);
    let cpu_started = Instant::now();
    let cpu_hits = scene.trace_closest_batch(&rays);
    let mut cpu_samples =
        vec![u64::try_from(cpu_started.elapsed().as_micros()).unwrap_or(u64::MAX)];
    for _ in 1..options.iteration_count {
        let started = Instant::now();
        let repeated = scene.trace_closest_batch(&rays);
        cpu_samples.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
        drop(repeated);
    }
    let cpu_distribution = timing_distribution(cpu_samples);
    let session = match ComputeSession::create(
        Arc::clone(&scene),
        ComputeOptions {
            preference: options.preference,
            power_preference: if options.low_power {
                ComputePowerPreference::LowPower
            } else {
                ComputePowerPreference::HighPerformance
            },
            adapter_name_filter: options.adapter_name_filter.clone(),
            maximum_precision_error_meters: options.maximum_precision_error_meters,
            maximum_expanded_triangle_count: options.maximum_expanded_triangle_count,
            maximum_rays_per_dispatch: options.maximum_rays_per_dispatch,
            maximum_geometry_chunk_bytes: options.maximum_geometry_chunk_bytes,
            maximum_gpu_memory_bytes: options.maximum_gpu_memory_bytes,
            enable_scene_cache: options.enable_scene_cache,
        },
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: VAYU session creation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let cold_batch = match session.trace_closest(&rays) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("error: VAYU query failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let parity = compare_with_cpu(&cpu_hits, &cold_batch.hits);
    for _ in 0..options.warmup_count {
        if let Err(error) = session.trace_closest(&rays) {
            eprintln!("error: VAYU warm-up failed: {error}");
            return ExitCode::FAILURE;
        }
    }
    let mut warm_total_samples = Vec::with_capacity(options.iteration_count);
    let mut warm_upload_samples = Vec::with_capacity(options.iteration_count);
    let mut warm_execution_samples = Vec::with_capacity(options.iteration_count);
    let mut warm_readback_samples = Vec::with_capacity(options.iteration_count);
    for _ in 0..options.iteration_count {
        let value = match session.trace_closest(&rays) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("error: VAYU measured query failed: {error}");
                return ExitCode::FAILURE;
            }
        };
        warm_total_samples.push(batch_microseconds(&value.stats));
        warm_upload_samples.push(value.stats.upload_microseconds);
        warm_execution_samples.push(value.stats.execution_microseconds);
        warm_readback_samples.push(value.stats.readback_microseconds);
    }
    let cold_microseconds = batch_microseconds(&cold_batch.stats);
    let warm_distribution = timing_distribution(warm_total_samples);
    let warm_upload = timing_distribution(warm_upload_samples);
    let warm_execution = timing_distribution(warm_execution_samples);
    let warm_readback = timing_distribution(warm_readback_samples);
    let runtime = session.runtime_stats();
    let cpu_mrays = million_rays_per_second(rays.len(), cpu_distribution.p50);
    let backend_mrays = million_rays_per_second(rays.len(), warm_distribution.p50);
    let (identity_match_rate, accepted) =
        parity_acceptance(parity, options.parity_tolerance_meters);
    if options.json {
        println!(
            concat!(
                "{{\"schemaVersion\":\"0.16.0\",\"engine\":\"VAYU\",",
                "\"backend\":\"{}\",\"adapter\":\"{}\",\"creationFallback\":{},",
                "\"creationMessage\":\"{}\",\"rays\":{},\"warmupCount\":{},\"iterationCount\":{},",
                "\"triangles\":{},\"nodes\":{},\"sceneGpuBytes\":{},",
                "\"geometryChunks\":{},\"uniqueTriangles\":{},\"instances\":{},",
                "\"sceneCacheHit\":{},\"streamedGeometry\":{},\"hostSnapshotBytes\":{},",
                "\"cold\":{{\"totalMilliseconds\":{},\"uploadMilliseconds\":{},",
                "\"executionMilliseconds\":{},\"readbackMilliseconds\":{}}},",
                "\"warm\":{{\"minimumMilliseconds\":{},\"p50Milliseconds\":{},",
                "\"p95Milliseconds\":{},\"meanMilliseconds\":{},\"uploadP50Milliseconds\":{},",
                "\"executionP50Milliseconds\":{},\"readbackP50Milliseconds\":{}}},",
                "\"cpu\":{{\"p50Milliseconds\":{},\"millionRaysPerSecond\":{}}},",
                "\"backendMillionRaysPerSecond\":{},",
                "\"runtime\":{{\"traces\":{},\"dispatches\":{},\"reusableBatches\":{},",
                "\"runtimeFallbacks\":{},\"scratchGenerations\":{},\"scratchCapacityRays\":{},",
                "\"scratchGpuBytes\":{},\"uploadedBytes\":{},\"readbackBytes\":{},",
                "\"geometryUploads\":{},\"geometryUploadedBytes\":{},",
                "\"deviceHealthy\":{},\"deviceLosses\":{},\"executionErrors\":{},\"lastError\":\"{}\"}},",
                "\"parity\":{{\"stateMismatches\":{},\"identityMismatches\":{},\"identityMatchRate\":{},",
                "\"maximumDistanceErrorMeters\":{},\"meanDistanceErrorMeters\":{},",
                "\"precisionErrorMeters\":{},\"accepted\":{}}}}}"
            ),
            backend_name(cold_batch.stats.backend),
            escape_json(
                session
                    .info()
                    .adapter
                    .as_ref()
                    .map_or("", |adapter| adapter.name.as_str())
            ),
            session.info().fallback_reason.is_some(),
            escape_json(session.info().fallback_reason.as_deref().unwrap_or("ready")),
            rays.len(),
            options.warmup_count,
            options.iteration_count,
            session.info().triangle_count,
            session.info().node_count,
            session.info().approximate_gpu_bytes,
            session.info().geometry_chunk_count,
            session.info().unique_triangle_count,
            session.info().instance_count,
            session.info().scene_cache_hit,
            session.info().streamed_geometry,
            session.info().host_snapshot_bytes,
            micros_to_milliseconds(u128::from(cold_microseconds)),
            micros_to_milliseconds(u128::from(cold_batch.stats.upload_microseconds)),
            micros_to_milliseconds(u128::from(cold_batch.stats.execution_microseconds)),
            micros_to_milliseconds(u128::from(cold_batch.stats.readback_microseconds)),
            micros_to_milliseconds(u128::from(warm_distribution.minimum)),
            micros_to_milliseconds(u128::from(warm_distribution.p50)),
            micros_to_milliseconds(u128::from(warm_distribution.p95)),
            micros_to_milliseconds(u128::from(warm_distribution.mean)),
            micros_to_milliseconds(u128::from(warm_upload.p50)),
            micros_to_milliseconds(u128::from(warm_execution.p50)),
            micros_to_milliseconds(u128::from(warm_readback.p50)),
            micros_to_milliseconds(u128::from(cpu_distribution.p50)),
            cpu_mrays,
            backend_mrays,
            runtime.trace_count,
            runtime.dispatch_count,
            runtime.reusable_buffer_batch_count,
            runtime.runtime_fallback_count,
            runtime.scratch_generation_count,
            runtime.scratch_capacity_rays,
            runtime.scratch_gpu_bytes,
            runtime.uploaded_bytes,
            runtime.readback_bytes,
            runtime.geometry_upload_count,
            runtime.geometry_uploaded_bytes,
            runtime.device_healthy,
            runtime.device_loss_count,
            runtime.execution_error_count,
            escape_json(runtime.last_error.as_deref().unwrap_or("")),
            parity.state_mismatch_count,
            parity.identity_mismatch_count,
            identity_match_rate,
            parity.maximum_distance_error_meters,
            parity.mean_distance_error_meters,
            cold_batch.stats.precision_error_meters,
            accepted
        );
    } else {
        println!("XVARNA VAYU portable backend benchmark");
        println!("Backend: {}", backend_name(cold_batch.stats.backend));
        if let Some(adapter) = &session.info().adapter {
            println!(
                "Adapter: {} | {} | {}",
                adapter.name, adapter.backend, adapter.driver
            );
        }
        if let Some(reason) = &session.info().fallback_reason {
            println!("Creation fallback: {reason}");
        }
        println!(
            "Scene: {:} triangles; {:} nodes; {:.3} MiB portable payload; {} chunk(s); cache hit={}; streamed={}",
            session.info().triangle_count,
            session.info().node_count,
            session.info().approximate_gpu_bytes as f64 / 1_048_576.0,
            session.info().geometry_chunk_count,
            session.info().scene_cache_hit,
            session.info().streamed_geometry
        );
        println!(
            "Rays: {}; warm-up: {}; measured iterations: {}; CPU p50 {:.3} Mray/s; backend p50 {:.3} Mray/s",
            rays.len(),
            options.warmup_count,
            options.iteration_count,
            cpu_mrays,
            backend_mrays
        );
        println!(
            "Cold total: {:.3} ms; warm min/p50/p95/mean: {:.3} / {:.3} / {:.3} / {:.3} ms",
            micros_to_milliseconds(u128::from(cold_microseconds)),
            micros_to_milliseconds(u128::from(warm_distribution.minimum)),
            micros_to_milliseconds(u128::from(warm_distribution.p50)),
            micros_to_milliseconds(u128::from(warm_distribution.p95)),
            micros_to_milliseconds(u128::from(warm_distribution.mean))
        );
        println!(
            "Warm upload/execute/readback p50: {:.3} / {:.3} / {:.3} ms",
            micros_to_milliseconds(u128::from(warm_upload.p50)),
            micros_to_milliseconds(u128::from(warm_execution.p50)),
            micros_to_milliseconds(u128::from(warm_readback.p50))
        );
        println!(
            "Persistent arena: {} rays/slot; {:.3} MiB; generations={}; reusable batches={}; device healthy={}",
            runtime.scratch_capacity_rays,
            runtime.scratch_gpu_bytes as f64 / 1_048_576.0,
            runtime.scratch_generation_count,
            runtime.reusable_buffer_batch_count,
            runtime.device_healthy
        );
        println!(
            "Geometry streaming: {} uploads; {:.3} MiB uploaded; host plan {:.3} MiB",
            runtime.geometry_upload_count,
            runtime.geometry_uploaded_bytes as f64 / 1_048_576.0,
            session.info().host_snapshot_bytes as f64 / 1_048_576.0
        );
        println!(
            "Parity: state={} identity={} identity-rate={:.6} max-distance={:.6e} m mean-distance={:.6e} m accepted={}",
            parity.state_mismatch_count,
            parity.identity_mismatch_count,
            identity_match_rate,
            parity.maximum_distance_error_meters,
            parity.mean_distance_error_meters,
            accepted
        );
    }
    if accepted {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

const fn batch_microseconds(stats: &ComputeBatchStats) -> u64 {
    stats
        .upload_microseconds
        .saturating_add(stats.execution_microseconds)
        .saturating_add(stats.readback_microseconds)
}

fn timing_distribution(mut samples: Vec<u64>) -> TimingDistribution {
    debug_assert!(!samples.is_empty());
    samples.sort_unstable();
    let p50_index = samples.len().div_ceil(2).saturating_sub(1);
    let p95_index = samples
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    let sum = samples.iter().fold(0_u128, |total, value| {
        total.saturating_add(u128::from(*value))
    });
    let mean = sum / u128::try_from(samples.len()).unwrap_or(u128::MAX);
    TimingDistribution {
        minimum: samples[0],
        p50: samples[p50_index],
        p95: samples[p95_index],
        mean: u64::try_from(mean).unwrap_or(u64::MAX),
    }
}

#[allow(clippy::cast_precision_loss)]
fn million_rays_per_second(ray_count: usize, microseconds: u64) -> f64 {
    ray_count as f64 / microseconds.max(1) as f64
}

#[allow(clippy::cast_precision_loss)]
fn parity_acceptance(report: xvarna_vayu::ParityReport, distance_tolerance: f64) -> (f64, bool) {
    let identity_match_rate = if report.ray_count == 0 {
        1.0
    } else {
        1.0 - report.identity_mismatch_count as f64 / report.ray_count as f64
    };
    (
        identity_match_rate,
        report.state_mismatch_count == 0
            && identity_match_rate >= 0.999
            && report.maximum_distance_error_meters <= distance_tolerance,
    )
}

fn parse_options(arguments: &[String]) -> Result<BenchmarkOptions, ()> {
    let mut output = BenchmarkOptions {
        preference: ComputePreference::Auto,
        ray_count: 262_144,
        thread_count: 0,
        maximum_precision_error_meters: 0.000_25,
        maximum_expanded_triangle_count: 20_000_000,
        maximum_rays_per_dispatch: 1_048_576,
        maximum_geometry_chunk_bytes: 512 * 1024 * 1024,
        maximum_gpu_memory_bytes: 1024 * 1024 * 1024,
        enable_scene_cache: true,
        parity_tolerance_meters: 0.000_1,
        warmup_count: 2,
        iteration_count: 5,
        low_power: false,
        adapter_name_filter: None,
        json: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => {
                output.json = true;
                index += 1;
            }
            "--low-power" => {
                output.low_power = true;
                index += 1;
            }
            "--no-cache" => {
                output.enable_scene_cache = false;
                index += 1;
            }
            option => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("error: {option} requires a value");
                    return Err(());
                };
                match option {
                    "--backend" => {
                        output.preference = match value.to_ascii_lowercase().as_str() {
                            "auto" => ComputePreference::Auto,
                            "gpu" => ComputePreference::RequirePortableGpu,
                            "cpu" => ComputePreference::Cpu,
                            _ => {
                                eprintln!("error: backend must be auto, gpu, or cpu");
                                return Err(());
                            }
                        };
                    }
                    "--adapter" => output.adapter_name_filter = Some(value.clone()),
                    "--rays" => output.ray_count = parse_count(value, "rays", 1, 100_000_000)?,
                    "--warmup" => output.warmup_count = parse_count(value, "warmup", 0, 100)?,
                    "--iterations" => {
                        output.iteration_count = parse_count(value, "iterations", 1, 100)?;
                    }
                    "--threads" => output.thread_count = parse_count(value, "threads", 0, 256)?,
                    "--max-triangles" => {
                        output.maximum_expanded_triangle_count =
                            parse_count(value, "max-triangles", 1, u32::MAX as usize)?;
                    }
                    "--max-rays-per-dispatch" => {
                        output.maximum_rays_per_dispatch =
                            parse_count(value, "max-rays-per-dispatch", 1, u32::MAX as usize)?;
                    }
                    "--geometry-chunk-mb" => {
                        output.maximum_geometry_chunk_bytes =
                            parse_mebibytes(value, "geometry-chunk-mb")?;
                    }
                    "--gpu-memory-mb" => {
                        output.maximum_gpu_memory_bytes = parse_mebibytes(value, "gpu-memory-mb")?;
                    }
                    "--max-error" => {
                        output.maximum_precision_error_meters = parse_positive(value, "max-error")?;
                    }
                    "--parity-tolerance" => {
                        output.parity_tolerance_meters = parse_positive(value, "parity-tolerance")?;
                    }
                    unknown => {
                        eprintln!("error: unknown backend-benchmark option '{unknown}'");
                        return Err(());
                    }
                }
                index += 2;
            }
        }
    }
    Ok(output)
}

fn benchmark_rays(minimum: Vec3, maximum: Vec3, count: usize) -> Vec<QueryRay> {
    const ROTATION_X: f64 = 0.754_877_666_246_692_7;
    const ROTATION_Y: f64 = 0.569_840_290_998_053_2;
    let extent = maximum - minimum;
    let diagonal = extent.length_squared().sqrt().max(1.0);
    let origin_z = maximum.z + diagonal * 0.25 + 1.0e-3;
    let maximum_distance = extent.z + diagonal * 0.5 + 2.0e-3;
    // A raw radical-inverse sequence repeatedly lands on exact integer grid lines
    // for power-of-two benchmark extents.  Those points have two equally valid
    // closest triangles and therefore cannot provide meaningful identity parity.
    // Cranley-Patterson rotations keep the deterministic low-discrepancy sequence
    // while moving the performance corpus away from shared-edge degeneracies.
    (0..count)
        .map(|index| {
            let x = (radical_inverse(index + 1, 2) + ROTATION_X).fract();
            let y = (radical_inverse(index + 1, 3) + ROTATION_Y).fract();
            QueryRay::try_new(
                Vec3::new(
                    extent.x.mul_add(x, minimum.x),
                    extent.y.mul_add(y, minimum.y),
                    origin_z,
                ),
                Vec3::new(0.0, 0.0, -1.0),
                0.0,
                maximum_distance,
                u64::MAX,
            )
            .expect("generated benchmark ray is finite")
        })
        .collect()
}

fn radical_inverse(mut value: usize, base: usize) -> f64 {
    let inverse = 1.0 / f64::from(u32::try_from(base).expect("small base"));
    let mut denominator = inverse;
    let mut result = 0.0;
    while value > 0 {
        result = f64::from(u32::try_from(value % base).expect("digit fits u32"))
            .mul_add(denominator, result);
        value /= base;
        denominator *= inverse;
    }
    result
}

fn parse_count(value: &str, name: &str, minimum: usize, maximum: usize) -> Result<usize, ()> {
    let parsed = value.parse::<usize>().map_err(|_| {
        eprintln!("error: {name} must be an integer");
    })?;
    if !(minimum..=maximum).contains(&parsed) {
        eprintln!("error: {name} must be between {minimum} and {maximum}");
        return Err(());
    }
    Ok(parsed)
}

fn parse_positive(value: &str, name: &str) -> Result<f64, ()> {
    let parsed = value.parse::<f64>().map_err(|_| {
        eprintln!("error: {name} must be a number");
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        eprintln!("error: {name} must be finite and positive");
        return Err(());
    }
    Ok(parsed)
}

fn parse_mebibytes(value: &str, name: &str) -> Result<usize, ()> {
    let mebibytes = parse_count(value, name, 1, usize::MAX / 1_048_576)?;
    mebibytes.checked_mul(1_048_576).ok_or_else(|| {
        eprintln!("error: {name} exceeds the platform address space");
    })
}

const fn backend_name(backend: ActiveBackend) -> &'static str {
    match backend {
        ActiveBackend::Cpu => "CPU f64",
        ActiveBackend::PortableGpu => "VAYU Portable GPU",
    }
}

fn escape_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control.is_control() => output.push(' '),
            other => output.push(other),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_options_parse_backend_precision_and_chunking() {
        let arguments = [
            "--backend",
            "gpu",
            "--rays",
            "4096",
            "--max-rays-per-dispatch",
            "1024",
            "--max-error",
            "0.0005",
            "--geometry-chunk-mb",
            "64",
            "--gpu-memory-mb",
            "256",
            "--no-cache",
            "--warmup",
            "3",
            "--iterations",
            "7",
            "--json",
        ]
        .map(str::to_owned);
        let options = parse_options(&arguments).expect("options valid");
        assert_eq!(options.preference, ComputePreference::RequirePortableGpu);
        assert_eq!(options.ray_count, 4096);
        assert_eq!(options.maximum_rays_per_dispatch, 1024);
        assert_eq!(options.maximum_geometry_chunk_bytes, 64 * 1024 * 1024);
        assert_eq!(options.maximum_gpu_memory_bytes, 256 * 1024 * 1024);
        assert!(!options.enable_scene_cache);
        assert_eq!(options.warmup_count, 3);
        assert_eq!(options.iteration_count, 7);
        assert!(options.json);
    }

    #[test]
    fn radical_inverse_is_bounded_and_deterministic() {
        let values = (1..100)
            .map(|index| radical_inverse(index, 2))
            .collect::<Vec<_>>();
        assert!(values.iter().all(|value| (0.0..1.0).contains(value)));
        assert!((values[0] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn timing_distribution_uses_sorted_nearest_rank_percentiles() {
        let distribution = timing_distribution(vec![100, 10, 50, 20, 90]);
        assert_eq!(distribution.minimum, 10);
        assert_eq!(distribution.p50, 50);
        assert_eq!(distribution.p95, 100);
        assert_eq!(distribution.mean, 54);
    }

    #[test]
    fn parity_gate_preserves_strict_state_distance_and_999_identity_contract() {
        let base = xvarna_vayu::ParityReport {
            ray_count: 10_000,
            state_mismatch_count: 0,
            identity_mismatch_count: 10,
            maximum_distance_error_meters: 0.000_25,
            mean_distance_error_meters: 0.0,
        };
        let (rate, accepted) = parity_acceptance(base, 0.000_25);
        assert!((rate - 0.999).abs() < f64::EPSILON);
        assert!(accepted);
        assert!(
            !parity_acceptance(
                xvarna_vayu::ParityReport {
                    identity_mismatch_count: 11,
                    ..base
                },
                0.000_25
            )
            .1
        );
        assert!(
            !parity_acceptance(
                xvarna_vayu::ParityReport {
                    state_mismatch_count: 1,
                    ..base
                },
                0.000_25
            )
            .1
        );
        assert!(!parity_acceptance(base, 0.000_249).1);
    }
}
