//! VAYU portable GPU acceleration with precision-aware deterministic CPU fallback.

#![forbid(unsafe_code)]

use bytemuck::{Pod, Zeroable};
use std::{
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};
pub use xvarna_cache::{CacheConfig as VayuCacheConfig, CacheStats as VayuCacheStats};
use xvarna_cache::{CacheKey, CacheStore};
use xvarna_geometry::Vec3;
use xvarna_scene::{
    Hit, PortableSceneError, PortableSceneOptions, PortableSceneSnapshot, QueryRay,
    RayQueryBackend, RayQueryBatch, RayQueryBatchStats, RayQueryContext, RayQueryExecutionError,
    RayQueryExecutor, Scene,
};

const WORKGROUP_SIZE: usize = 64;
const SCRATCH_SLOT_COUNT: usize = 2;
const MAXIMUM_REUSABLE_RAYS_PER_SLOT: usize = 262_144;
const HIT_FLAG: u32 = 1;
const FRONT_FACE_FLAG: u32 = 2;

/// User preference for portable acceleration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputePreference {
    /// Prefer a discrete/high-performance adapter and fall back to CPU safely.
    Auto,
    /// Require the portable GPU path; failure is returned to the caller.
    RequirePortableGpu,
    /// Use the canonical f64 CPU backend.
    Cpu,
}

/// Power preference forwarded to adapter selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputePowerPreference {
    /// Prefer a high-performance adapter.
    HighPerformance,
    /// Prefer a lower-power adapter.
    LowPower,
}

/// Complete creation and dispatch resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputeOptions {
    /// Backend selection policy.
    pub preference: ComputePreference,
    /// Adapter power policy.
    pub power_preference: ComputePowerPreference,
    /// Optional case-insensitive adapter-name filter.
    pub adapter_name_filter: Option<String>,
    /// Maximum accepted f32 conversion error in canonical metres.
    pub maximum_precision_error_meters: f64,
    /// Maximum effective triangle count uploaded to the portable backend.
    pub maximum_expanded_triangle_count: usize,
    /// Maximum rays submitted in one dispatch/readback chunk.
    pub maximum_rays_per_dispatch: usize,
    /// Maximum static bytes represented by one streamed geometry chunk.
    pub maximum_geometry_chunk_bytes: usize,
    /// Hard cap for the retained geometry working set plus reusable ray scratch buffers.
    pub maximum_gpu_memory_bytes: usize,
    /// Enables the versioned memory/disk portable-snapshot cache.
    pub enable_scene_cache: bool,
}

impl Default for ComputeOptions {
    fn default() -> Self {
        Self {
            preference: ComputePreference::Auto,
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

/// Active execution backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveBackend {
    /// Canonical Rayon/f64 traversal.
    Cpu,
    /// VAYU wgpu/WGSL f32 traversal.
    PortableGpu,
}

/// Stable adapter description suitable for reports and public APIs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterDescription {
    /// Human-readable adapter name.
    pub name: String,
    /// Driver name.
    pub driver: String,
    /// Driver detail/version.
    pub driver_info: String,
    /// Graphics API backend.
    pub backend: String,
    /// Device class.
    pub device_type: String,
    /// PCI/vendor identifier when supplied by the backend.
    pub vendor_id: u32,
    /// Device identifier when supplied by the backend.
    pub device_id: u32,
}

/// Immutable creation metadata for one compute session.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputeSessionInfo {
    /// Backend selected at creation.
    pub backend: ActiveBackend,
    /// Selected adapter, absent for CPU.
    pub adapter: Option<AdapterDescription>,
    /// Reason Auto mode selected CPU, if applicable.
    pub fallback_reason: Option<String>,
    /// Effective triangles represented by the backend.
    pub triangle_count: usize,
    /// Portable BVH nodes, or canonical TLAS+BLAS nodes for CPU.
    pub node_count: usize,
    /// Approximate portable GPU bytes, zero for CPU.
    pub approximate_gpu_bytes: usize,
    /// Largest observed scene quantization error in metres.
    pub scene_precision_error_meters: f64,
    /// Scene upload/pipeline construction time in microseconds.
    pub initialization_microseconds: u64,
    /// Number of bounded two-level geometry chunks in the execution plan.
    pub geometry_chunk_count: usize,
    /// Number of unique triangles in the canonical scene.
    pub unique_triangle_count: usize,
    /// Number of instances in the canonical scene.
    pub instance_count: usize,
    /// True when portable snapshots were restored from the validated cache.
    pub scene_cache_hit: bool,
    /// True when geometry chunks are streamed through one bounded GPU working set.
    pub streamed_geometry: bool,
    /// Host bytes retaining the complete chunk plan for streaming/recovery.
    pub host_snapshot_bytes: usize,
}

/// Per-batch execution and transfer measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputeBatchStats {
    /// Backend that produced this batch.
    pub backend: ActiveBackend,
    /// Ordered ray count.
    pub ray_count: usize,
    /// Number of bounded GPU dispatches, or zero for CPU.
    pub dispatch_count: usize,
    /// Input upload and encoding time in microseconds.
    pub upload_microseconds: u64,
    /// Queue execution/wait time in microseconds.
    pub execution_microseconds: u64,
    /// Mapping and decode time in microseconds.
    pub readback_microseconds: u64,
    /// Largest observed scene/ray f32 conversion error in metres.
    pub precision_error_meters: f64,
    /// True when an attempted GPU query fell back to CPU for this batch.
    pub used_runtime_fallback: bool,
    /// True when this batch used VAYU's persistent scratch arena.
    pub used_reusable_buffers: bool,
    /// Runtime fallback reason, if any.
    pub fallback_reason: Option<String>,
}

/// Cumulative production telemetry for a retained compute session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComputeRuntimeStats {
    /// Closest-hit and any-hit requests received by the session.
    pub trace_count: u64,
    /// Rays received across all requests, including CPU fallback.
    pub ray_count: u64,
    /// Portable GPU dispatches submitted successfully.
    pub dispatch_count: u64,
    /// Successful GPU batches served by persistent scratch resources.
    pub reusable_buffer_batch_count: u64,
    /// Auto-mode batches retried on the canonical CPU backend.
    pub runtime_fallback_count: u64,
    /// Scratch arena generations, including first lazy allocation and growth.
    pub scratch_generation_count: u64,
    /// Current per-slot ray capacity; zero before the first non-empty GPU query.
    pub scratch_capacity_rays: u64,
    /// Current bytes retained by both dynamic GPU scratch slots.
    pub scratch_gpu_bytes: u64,
    /// Bytes uploaded through queue writes.
    pub uploaded_bytes: u64,
    /// Bytes copied back from portable GPU storage.
    pub readback_bytes: u64,
    /// Geometry chunk uploads after session initialization.
    pub geometry_upload_count: u64,
    /// Bytes uploaded while streaming geometry chunks.
    pub geometry_uploaded_bytes: u64,
    /// Device-loss notifications observed by wgpu.
    pub device_loss_count: u64,
    /// Validation, mapping, polling, or uncaptured execution errors observed.
    pub execution_error_count: u64,
    /// True while the retained logical device remains usable.
    pub device_healthy: bool,
    /// Most recent device/runtime diagnostic, when one exists.
    pub last_error: Option<String>,
}

/// Ordered closest-hit output plus provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputeBatch {
    /// One result per input ray.
    pub hits: Vec<Hit>,
    /// Backend and timing metadata.
    pub stats: ComputeBatchStats,
}

#[derive(Default)]
struct RuntimeCounters {
    trace_count: AtomicU64,
    ray_count: AtomicU64,
    dispatch_count: AtomicU64,
    reusable_buffer_batch_count: AtomicU64,
    runtime_fallback_count: AtomicU64,
    scratch_generation_count: AtomicU64,
    scratch_capacity_rays: AtomicU64,
    scratch_gpu_bytes: AtomicU64,
    uploaded_bytes: AtomicU64,
    readback_bytes: AtomicU64,
    geometry_upload_count: AtomicU64,
    geometry_uploaded_bytes: AtomicU64,
    device_loss_count: AtomicU64,
    execution_error_count: AtomicU64,
}

impl RuntimeCounters {
    fn record_trace(&self, ray_count: usize) {
        self.trace_count.fetch_add(1, Ordering::Relaxed);
        self.ray_count.fetch_add(
            u64::try_from(ray_count).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }

    fn snapshot(&self, health: Option<&DeviceHealth>) -> ComputeRuntimeStats {
        ComputeRuntimeStats {
            trace_count: self.trace_count.load(Ordering::Relaxed),
            ray_count: self.ray_count.load(Ordering::Relaxed),
            dispatch_count: self.dispatch_count.load(Ordering::Relaxed),
            reusable_buffer_batch_count: self.reusable_buffer_batch_count.load(Ordering::Relaxed),
            runtime_fallback_count: self.runtime_fallback_count.load(Ordering::Relaxed),
            scratch_generation_count: self.scratch_generation_count.load(Ordering::Relaxed),
            scratch_capacity_rays: self.scratch_capacity_rays.load(Ordering::Relaxed),
            scratch_gpu_bytes: self.scratch_gpu_bytes.load(Ordering::Relaxed),
            uploaded_bytes: self.uploaded_bytes.load(Ordering::Relaxed),
            readback_bytes: self.readback_bytes.load(Ordering::Relaxed),
            geometry_upload_count: self.geometry_upload_count.load(Ordering::Relaxed),
            geometry_uploaded_bytes: self.geometry_uploaded_bytes.load(Ordering::Relaxed),
            device_loss_count: self.device_loss_count.load(Ordering::Relaxed),
            execution_error_count: self.execution_error_count.load(Ordering::Relaxed),
            device_healthy: health.is_none_or(DeviceHealth::is_healthy),
            last_error: health.and_then(DeviceHealth::last_error),
        }
    }
}

#[derive(Default)]
struct DeviceHealth {
    lost: AtomicBool,
    last_error: Mutex<Option<String>>,
}

impl DeviceHealth {
    fn is_healthy(&self) -> bool {
        !self.lost.load(Ordering::Acquire)
    }

    fn record_device_loss(&self, counters: &RuntimeCounters, message: String) {
        if !self.lost.swap(true, Ordering::AcqRel) {
            counters.device_loss_count.fetch_add(1, Ordering::Relaxed);
        }
        self.record_error(counters, message);
    }

    fn record_error(&self, counters: &RuntimeCounters, message: String) {
        counters
            .execution_error_count
            .fetch_add(1, Ordering::Relaxed);
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = Some(message);
        }
    }

    fn record_execution_failure(&self, counters: &RuntimeCounters, message: String) {
        self.lost.store(true, Ordering::Release);
        self.record_error(counters, message);
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|value| value.clone())
    }

    fn require_healthy(&self) -> Result<(), VayuError> {
        if self.is_healthy() {
            Ok(())
        } else {
            Err(VayuError::Execution(self.last_error().unwrap_or_else(
                || "portable GPU device is lost".to_owned(),
            )))
        }
    }
}

/// Failure during portable backend creation or dispatch.
#[derive(Debug)]
pub enum VayuError {
    /// Options contain invalid limits or filters.
    InvalidOptions(String),
    /// Portable scene conversion failed its explicit contract.
    PortableScene(PortableSceneError),
    /// No adapter matched the requested policy.
    AdapterUnavailable(String),
    /// A logical device could not be opened.
    DeviceRequest(String),
    /// A ray cannot meet the requested f32 precision budget.
    RayPrecisionExceeded {
        /// Largest observed conversion error in metres.
        estimated_error_meters: f64,
        /// Configured budget in metres.
        allowed_error_meters: f64,
    },
    /// GPU mapping, device polling, or shader execution failed.
    Execution(String),
    /// Cache configuration or explicit cache-management operation failed.
    Cache(String),
}

impl fmt::Display for VayuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(formatter, "invalid VAYU options: {message}"),
            Self::PortableScene(error) => write!(formatter, "portable scene rejected: {error}"),
            Self::AdapterUnavailable(message) => {
                write!(formatter, "portable GPU adapter unavailable: {message}")
            }
            Self::DeviceRequest(message) => {
                write!(formatter, "GPU device request failed: {message}")
            }
            Self::RayPrecisionExceeded {
                estimated_error_meters,
                allowed_error_meters,
            } => write!(
                formatter,
                "ray f32 error {estimated_error_meters:.6e} m exceeds the {allowed_error_meters:.6e} m budget"
            ),
            Self::Execution(message) => write!(formatter, "GPU execution failed: {message}"),
            Self::Cache(message) => write!(formatter, "VAYU cache failed: {message}"),
        }
    }
}

impl std::error::Error for VayuError {}

impl From<PortableSceneError> for VayuError {
    fn from(value: PortableSceneError) -> Self {
        Self::PortableScene(value)
    }
}

static SCENE_CACHE: OnceLock<RwLock<Arc<CacheStore>>> = OnceLock::new();

/// Returns the platform-default cache directory without creating it.
#[must_use]
pub fn default_cache_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map_or_else(
        || std::env::temp_dir().join("XVARNA").join("cache").join("v1"),
        |root| PathBuf::from(root).join("XVARNA").join("cache").join("v1"),
    )
}

/// Replaces the process-wide portable-scene cache policy atomically.
pub fn configure_scene_cache(config: VayuCacheConfig) -> Result<(), VayuError> {
    let store =
        Arc::new(CacheStore::open(config).map_err(|error| VayuError::Cache(error.to_string()))?);
    let cache = SCENE_CACHE.get_or_init(|| RwLock::new(Arc::clone(&store)));
    *cache
        .write()
        .map_err(|_| VayuError::Cache("cache configuration lock was poisoned".to_owned()))? = store;
    Ok(())
}

/// Reads cumulative memory/disk cache telemetry.
pub fn scene_cache_stats() -> Result<VayuCacheStats, VayuError> {
    current_cache()?
        .stats()
        .map_err(|error| VayuError::Cache(error.to_string()))
}

/// Returns the active process-wide cache policy.
pub fn scene_cache_config() -> Result<VayuCacheConfig, VayuError> {
    Ok(current_cache()?.config().clone())
}

/// Clears validated XVARNA cache entries under the configured directory.
pub fn clear_scene_cache() -> Result<(), VayuError> {
    current_cache()?
        .clear()
        .map_err(|error| VayuError::Cache(error.to_string()))
}

fn current_cache() -> Result<Arc<CacheStore>, VayuError> {
    let cache = SCENE_CACHE.get_or_init(|| {
        let config = VayuCacheConfig::try_new(
            default_cache_directory(),
            256 * 1024 * 1024,
            4 * 1024 * 1024 * 1024,
        )
        .expect("default cache configuration is valid");
        let store = CacheStore::open(config).unwrap_or_else(|_| {
            CacheStore::open(
                VayuCacheConfig::try_new(std::env::temp_dir().join("xvarna-cache-fallback"), 0, 0)
                    .expect("fallback cache configuration is valid"),
            )
            .expect("disabled fallback cache opens")
        });
        RwLock::new(Arc::new(store))
    });
    cache
        .read()
        .map(|value| Arc::clone(&value))
        .map_err(|_| VayuError::Cache("cache configuration lock was poisoned".to_owned()))
}

/// Precision and identity differences between canonical CPU and VAYU GPU results.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParityReport {
    /// Compared rays.
    pub ray_count: usize,
    /// Hit/miss disagreements.
    pub state_mismatch_count: usize,
    /// Stable-identity disagreements where both backends hit.
    pub identity_mismatch_count: usize,
    /// Maximum absolute closest-distance difference in metres.
    pub maximum_distance_error_meters: f64,
    /// Mean absolute closest-distance difference over mutual hits.
    pub mean_distance_error_meters: f64,
}

/// Enumerates adapters visible to wgpu without opening a logical device.
#[must_use]
pub fn enumerate_adapters() -> Vec<AdapterDescription> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .map(|adapter| describe_adapter(&adapter.get_info()))
        .collect()
}

/// Backend session retaining the canonical scene for guaranteed fallback.
pub struct ComputeSession {
    scene: Arc<Scene>,
    gpu: Option<PortableGpu>,
    options: ComputeOptions,
    info: ComputeSessionInfo,
    runtime: Arc<RuntimeCounters>,
}

impl ComputeSession {
    /// Creates the requested backend, with Auto mode falling back to CPU explicitly.
    pub fn create(scene: Arc<Scene>, options: ComputeOptions) -> Result<Self, VayuError> {
        validate_options(&options)?;
        let started = Instant::now();
        let runtime = Arc::new(RuntimeCounters::default());
        if options.preference == ComputePreference::Cpu {
            let stats = scene.stats();
            return Ok(Self {
                scene,
                gpu: None,
                options,
                info: ComputeSessionInfo {
                    backend: ActiveBackend::Cpu,
                    adapter: None,
                    fallback_reason: None,
                    triangle_count: stats.instanced_triangle_count,
                    node_count: stats.blas_node_count + stats.tlas_node_count,
                    approximate_gpu_bytes: 0,
                    scene_precision_error_meters: 0.0,
                    initialization_microseconds: elapsed_microseconds(started),
                    geometry_chunk_count: 0,
                    unique_triangle_count: stats.unique_triangle_count,
                    instance_count: stats.instance_count,
                    scene_cache_hit: false,
                    streamed_geometry: false,
                    host_snapshot_bytes: 0,
                },
                runtime,
            });
        }

        match PortableGpu::create(&scene, &options, Arc::clone(&runtime)) {
            Ok(gpu) => {
                let mut info = gpu.info.clone();
                info.initialization_microseconds = elapsed_microseconds(started);
                Ok(Self {
                    scene,
                    gpu: Some(gpu),
                    options,
                    info,
                    runtime,
                })
            }
            Err(error) if options.preference == ComputePreference::Auto => {
                let stats = scene.stats();
                Ok(Self {
                    scene,
                    gpu: None,
                    options,
                    info: ComputeSessionInfo {
                        backend: ActiveBackend::Cpu,
                        adapter: None,
                        fallback_reason: Some(error.to_string()),
                        triangle_count: stats.instanced_triangle_count,
                        node_count: stats.blas_node_count + stats.tlas_node_count,
                        approximate_gpu_bytes: 0,
                        scene_precision_error_meters: 0.0,
                        initialization_microseconds: elapsed_microseconds(started),
                        geometry_chunk_count: 0,
                        unique_triangle_count: stats.unique_triangle_count,
                        instance_count: stats.instance_count,
                        scene_cache_hit: false,
                        streamed_geometry: false,
                        host_snapshot_bytes: 0,
                    },
                    runtime,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Returns immutable backend/provenance metadata.
    #[must_use]
    pub const fn info(&self) -> &ComputeSessionInfo {
        &self.info
    }

    /// Returns a lock-free cumulative snapshot of throughput, reuse, and device health.
    #[must_use]
    pub fn runtime_stats(&self) -> ComputeRuntimeStats {
        self.runtime
            .snapshot(self.gpu.as_ref().map(|gpu| gpu.health.as_ref()))
    }

    /// Executes ordered closest-hit rays, with per-batch Auto fallback on device/precision failure.
    pub fn trace_closest(&self, rays: &[QueryRay]) -> Result<ComputeBatch, VayuError> {
        self.runtime.record_trace(rays.len());
        if let Some(gpu) = &self.gpu {
            match gpu.trace(rays, false) {
                Ok(batch) => return Ok(batch),
                Err(error) if self.options.preference == ComputePreference::Auto => {
                    self.runtime
                        .runtime_fallback_count
                        .fetch_add(1, Ordering::Relaxed);
                    return Ok(cpu_batch(&self.scene, rays, true, Some(error.to_string())));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(cpu_batch(&self.scene, rays, false, None))
    }

    /// Executes ordered early-exit visibility rays.
    pub fn trace_any(
        &self,
        rays: &[QueryRay],
    ) -> Result<(Vec<bool>, ComputeBatchStats), VayuError> {
        self.runtime.record_trace(rays.len());
        if let Some(gpu) = &self.gpu {
            match gpu.trace(rays, true) {
                Ok(batch) => {
                    let visibility = batch.hits.iter().map(|hit| hit.hit).collect();
                    return Ok((visibility, batch.stats));
                }
                Err(error) if self.options.preference == ComputePreference::Auto => {
                    self.runtime
                        .runtime_fallback_count
                        .fetch_add(1, Ordering::Relaxed);
                    let started = Instant::now();
                    let visibility = self.scene.trace_any_batch(rays);
                    return Ok((
                        visibility,
                        ComputeBatchStats {
                            backend: ActiveBackend::Cpu,
                            ray_count: rays.len(),
                            dispatch_count: 0,
                            upload_microseconds: 0,
                            execution_microseconds: elapsed_microseconds(started),
                            readback_microseconds: 0,
                            precision_error_meters: 0.0,
                            used_runtime_fallback: true,
                            used_reusable_buffers: false,
                            fallback_reason: Some(error.to_string()),
                        },
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        let started = Instant::now();
        let visibility = self.scene.trace_any_batch(rays);
        Ok((
            visibility,
            ComputeBatchStats {
                backend: ActiveBackend::Cpu,
                ray_count: rays.len(),
                dispatch_count: 0,
                upload_microseconds: 0,
                execution_microseconds: elapsed_microseconds(started),
                readback_microseconds: 0,
                precision_error_meters: 0.0,
                used_runtime_fallback: false,
                used_reusable_buffers: false,
                fallback_reason: None,
            },
        ))
    }
}

impl RayQueryExecutor for ComputeSession {
    fn canonical_scene(&self) -> &Scene {
        &self.scene
    }

    fn context(&self) -> RayQueryContext {
        RayQueryContext {
            backend: query_backend(self.info.backend),
            adapter_name: self
                .info
                .adapter
                .as_ref()
                .map(|adapter| adapter.name.clone()),
            used_creation_fallback: self.info.fallback_reason.is_some(),
            fallback_reason: self.info.fallback_reason.clone(),
        }
    }

    fn trace_closest(&self, rays: &[QueryRay]) -> Result<RayQueryBatch, RayQueryExecutionError> {
        let batch = Self::trace_closest(self, rays)
            .map_err(|error| RayQueryExecutionError::new(error.to_string()))?;
        Ok(RayQueryBatch {
            hits: batch.hits,
            stats: RayQueryBatchStats {
                backend: query_backend(batch.stats.backend),
                ray_count: batch.stats.ray_count,
                dispatch_count: batch.stats.dispatch_count,
                upload_microseconds: batch.stats.upload_microseconds,
                execution_microseconds: batch.stats.execution_microseconds,
                readback_microseconds: batch.stats.readback_microseconds,
                precision_error_meters: batch.stats.precision_error_meters,
                used_runtime_fallback: batch.stats.used_runtime_fallback,
                used_reusable_buffers: batch.stats.used_reusable_buffers,
                fallback_reason: batch.stats.fallback_reason,
            },
        })
    }
}

const fn query_backend(backend: ActiveBackend) -> RayQueryBackend {
    match backend {
        ActiveBackend::Cpu => RayQueryBackend::Cpu,
        ActiveBackend::PortableGpu => RayQueryBackend::PortableGpu,
    }
}

struct PortableGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    scene_bindings: SceneBindings,
    scene_chunks: Vec<PackedScene>,
    rebase_origin_meters: Vec3,
    maximum_precision_error_meters: f64,
    maximum_rays_per_dispatch: usize,
    info: ComputeSessionInfo,
    scratch: Mutex<ScratchPool>,
    runtime: Arc<RuntimeCounters>,
    health: Arc<DeviceHealth>,
}

struct SceneBindings {
    tlas_nodes: wgpu::Buffer,
    tlas_indices: wgpu::Buffer,
    instances: wgpu::Buffer,
    blas_nodes: wgpu::Buffer,
    blas_indices: wgpu::Buffer,
    triangles: wgpu::Buffer,
}

struct PackedScene {
    tlas_nodes: Vec<GpuNode>,
    tlas_indices: Vec<u32>,
    instances: Vec<GpuInstance>,
    blas_nodes: Vec<GpuNode>,
    blas_indices: Vec<u32>,
    triangles: Vec<GpuTriangle>,
}

impl PackedScene {
    const fn payload_bytes(&self) -> usize {
        self.tlas_nodes.len() * core::mem::size_of::<GpuNode>()
            + self.tlas_indices.len() * core::mem::size_of::<u32>()
            + self.instances.len() * core::mem::size_of::<GpuInstance>()
            + self.blas_nodes.len() * core::mem::size_of::<GpuNode>()
            + self.blas_indices.len() * core::mem::size_of::<u32>()
            + self.triangles.len() * core::mem::size_of::<GpuTriangle>()
    }
}

struct ScenePlan {
    chunks: Vec<PackedScene>,
    rebase_origin_meters: Vec3,
    precision_error_meters: f64,
    cache_hit: bool,
}

struct ScratchPool {
    slots: Option<[ScratchSlot; SCRATCH_SLOT_COUNT]>,
    capacity_rays: usize,
}

struct ScratchSlot {
    rays: Vec<GpuRay>,
    ray_buffer: wgpu::Buffer,
    hit_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    parameter_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PortableGpu {
    #[allow(clippy::too_many_lines)]
    fn create(
        scene: &Scene,
        options: &ComputeOptions,
        runtime: Arc<RuntimeCounters>,
    ) -> Result<Self, VayuError> {
        let started = Instant::now();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = request_adapter(&instance, options)?;
        let adapter_info = adapter.get_info();
        let limits = adapter.limits();
        let portable_options = PortableSceneOptions {
            maximum_precision_error_meters: options.maximum_precision_error_meters,
            maximum_expanded_triangle_count: options.maximum_expanded_triangle_count,
            maximum_leaf_size: 4,
            maximum_chunk_payload_bytes: options.maximum_geometry_chunk_bytes,
        };
        let plan = load_or_build_scene_plan(scene, portable_options, options.enable_scene_cache)?;
        let rebase_origin_meters = plan.rebase_origin_meters;
        let scene_precision_error_meters = plan.precision_error_meters;
        let scene_cache_hit = plan.cache_hit;
        let scene_chunks = plan.chunks;
        let host_snapshot_bytes = scene_chunks
            .iter()
            .map(PackedScene::payload_bytes)
            .sum::<usize>();
        let approximate_gpu_bytes = scene_chunks
            .iter()
            .map(PackedScene::payload_bytes)
            .max()
            .unwrap_or(0);
        let geometry_chunk_count = scene_chunks.len();
        if approximate_gpu_bytes > options.maximum_gpu_memory_bytes {
            return Err(VayuError::InvalidOptions(format!(
                "one geometry working set needs {approximate_gpu_bytes} bytes, above the GPU memory budget of {} bytes",
                options.maximum_gpu_memory_bytes
            )));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("XVARNA VAYU device"),
            required_features: wgpu::Features::empty(),
            // The scene plan was validated against the adapter's advertised
            // storage/buffer limits. Request those limits explicitly; using
            // wgpu defaults here can create a lower-limit logical device and
            // defer an oversized-buffer failure until Queue::write_buffer.
            required_limits: limits.clone(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| VayuError::DeviceRequest(error.to_string()))?;
        let health = Arc::new(DeviceHealth::default());
        let lost_health = Arc::clone(&health);
        let lost_runtime = Arc::clone(&runtime);
        device.set_device_lost_callback(move |reason, message| {
            lost_health.record_device_loss(
                &lost_runtime,
                format!("device lost ({reason:?}): {message}"),
            );
        });
        let error_health = Arc::clone(&health);
        let error_runtime = Arc::clone(&runtime);
        device.on_uncaptured_error(Arc::new(move |error| {
            error_health
                .record_execution_failure(&error_runtime, format!("uncaptured GPU error: {error}"));
        }));
        let maximum_buffer = usize::try_from(limits.max_buffer_size).unwrap_or(usize::MAX);
        let maximum_binding = usize::try_from(limits.max_storage_buffer_binding_size)
            .unwrap_or(usize::MAX)
            .min(maximum_buffer);
        validate_scene_binding_limits(&scene_chunks, maximum_binding)?;
        let scene_bindings = SceneBindings {
            tlas_nodes: create_scene_buffer::<GpuNode>(
                &device,
                "VAYU TLAS nodes",
                maximum_count(&scene_chunks, |chunk| chunk.tlas_nodes.len()),
            )?,
            tlas_indices: create_scene_buffer::<u32>(
                &device,
                "VAYU TLAS indices",
                maximum_count(&scene_chunks, |chunk| chunk.tlas_indices.len()),
            )?,
            instances: create_scene_buffer::<GpuInstance>(
                &device,
                "VAYU instances",
                maximum_count(&scene_chunks, |chunk| chunk.instances.len()),
            )?,
            blas_nodes: create_scene_buffer::<GpuNode>(
                &device,
                "VAYU BLAS nodes",
                maximum_count(&scene_chunks, |chunk| chunk.blas_nodes.len()),
            )?,
            blas_indices: create_scene_buffer::<u32>(
                &device,
                "VAYU BLAS indices",
                maximum_count(&scene_chunks, |chunk| chunk.blas_indices.len()),
            )?,
            triangles: create_scene_buffer::<GpuTriangle>(
                &device,
                "VAYU triangles",
                maximum_count(&scene_chunks, |chunk| chunk.triangles.len()),
            )?,
        };
        upload_scene(&queue, &scene_bindings, &scene_chunks[0]);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("XVARNA VAYU traversal"),
            source: wgpu::ShaderSource::Wgsl(include_str!("trace.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("XVARNA VAYU closest/any traversal"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let hardware_ray_limit = maximum_binding
            .checked_div(core::mem::size_of::<GpuRay>().max(core::mem::size_of::<GpuHit>()))
            .unwrap_or(0)
            .min(
                usize::try_from(limits.max_compute_workgroups_per_dimension).unwrap_or(usize::MAX)
                    * WORKGROUP_SIZE,
            );
        let scratch_bytes_per_ray = (core::mem::size_of::<GpuRay>()
            + core::mem::size_of::<GpuHit>() * 2)
            * SCRATCH_SLOT_COUNT;
        let memory_ray_limit = options
            .maximum_gpu_memory_bytes
            .saturating_sub(approximate_gpu_bytes)
            .checked_div(scratch_bytes_per_ray)
            .unwrap_or(0);
        if memory_ray_limit == 0 {
            return Err(VayuError::InvalidOptions(
                "GPU memory budget leaves no room for one reusable ray slot".to_owned(),
            ));
        }
        let maximum_rays_per_dispatch = options
            .maximum_rays_per_dispatch
            .min(hardware_ray_limit)
            .min(memory_ray_limit)
            .max(1);
        let scene_stats = scene.stats();
        Ok(Self {
            device,
            queue,
            pipeline,
            scene_bindings,
            scene_chunks,
            rebase_origin_meters,
            maximum_precision_error_meters: options.maximum_precision_error_meters,
            maximum_rays_per_dispatch,
            info: ComputeSessionInfo {
                backend: ActiveBackend::PortableGpu,
                adapter: Some(describe_adapter(&adapter_info)),
                fallback_reason: None,
                triangle_count: scene_stats.instanced_triangle_count,
                node_count: scene_stats.blas_node_count + scene_stats.tlas_node_count,
                approximate_gpu_bytes,
                scene_precision_error_meters,
                initialization_microseconds: elapsed_microseconds(started),
                geometry_chunk_count,
                unique_triangle_count: scene_stats.unique_triangle_count,
                instance_count: scene_stats.instance_count,
                scene_cache_hit,
                streamed_geometry: geometry_chunk_count > 1,
                host_snapshot_bytes,
            },
            scratch: Mutex::new(ScratchPool {
                slots: None,
                capacity_rays: 0,
            }),
            runtime,
            health,
        })
    }

    #[allow(clippy::too_many_lines, clippy::significant_drop_tightening)]
    fn trace(&self, rays: &[QueryRay], any_hit: bool) -> Result<ComputeBatch, VayuError> {
        self.health.require_healthy()?;
        if rays.is_empty() {
            return Ok(ComputeBatch {
                hits: Vec::new(),
                stats: ComputeBatchStats {
                    backend: ActiveBackend::PortableGpu,
                    ray_count: 0,
                    dispatch_count: 0,
                    upload_microseconds: 0,
                    execution_microseconds: 0,
                    readback_microseconds: 0,
                    precision_error_meters: self.info.scene_precision_error_meters,
                    used_runtime_fallback: false,
                    used_reusable_buffers: false,
                    fallback_reason: None,
                },
            });
        }

        let arena_started = Instant::now();
        let maximum_slot_capacity = self
            .maximum_rays_per_dispatch
            .min(MAXIMUM_REUSABLE_RAYS_PER_SLOT);
        let requested_capacity = rays.len().min(maximum_slot_capacity).max(1);
        let mut scratch = self.scratch.lock().map_err(|_| {
            self.execution_error("portable scratch arena lock was poisoned".to_owned())
        })?;
        scratch.ensure_capacity(
            &self.device,
            &self.pipeline,
            &self.scene_bindings,
            requested_capacity,
            maximum_slot_capacity,
            &self.runtime,
        )?;
        let chunk_capacity = scratch.capacity_rays;
        let slots = scratch
            .slots
            .as_mut()
            .expect("non-empty requests initialize the scratch arena");
        let mut hits = vec![Hit::miss(); rays.len()];
        let mut upload_microseconds = elapsed_microseconds(arena_started);
        let mut execution_microseconds = 0_u64;
        let mut readback_microseconds = 0_u64;
        let mut precision_error = self.info.scene_precision_error_meters;
        let mut dispatch_count = 0_usize;
        for scene_chunk in &self.scene_chunks {
            if self.scene_chunks.len() > 1 {
                let geometry_started = Instant::now();
                upload_scene(&self.queue, &self.scene_bindings, scene_chunk);
                let geometry_bytes = u64::try_from(scene_chunk.payload_bytes()).unwrap_or(u64::MAX);
                self.runtime
                    .geometry_upload_count
                    .fetch_add(1, Ordering::Relaxed);
                self.runtime
                    .geometry_uploaded_bytes
                    .fetch_add(geometry_bytes, Ordering::Relaxed);
                upload_microseconds =
                    upload_microseconds.saturating_add(elapsed_microseconds(geometry_started));
            }
            let mut chunks = rays.chunks(chunk_capacity).enumerate();
            loop {
                let upload_started = Instant::now();
                let mut command_buffers = Vec::with_capacity(SCRATCH_SLOT_COUNT);
                let mut ray_counts = [0_usize; SCRATCH_SLOT_COUNT];
                let mut ray_offsets = [0_usize; SCRATCH_SLOT_COUNT];
                for (slot_index, slot) in slots.iter_mut().enumerate() {
                    let Some((chunk_index, chunk)) = chunks.next() else {
                        break;
                    };
                    let ray_error =
                        pack_rays_into(chunk, self.rebase_origin_meters, &mut slot.rays);
                    precision_error = precision_error.max(ray_error);
                    if ray_error > self.maximum_precision_error_meters {
                        return Err(VayuError::RayPrecisionExceeded {
                            estimated_error_meters: ray_error,
                            allowed_error_meters: self.maximum_precision_error_meters,
                        });
                    }
                    let parameters = GpuParameters {
                        ray_count: u32::try_from(chunk.len()).map_err(|_| {
                            VayuError::InvalidOptions("dispatch ray count exceeds u32".to_owned())
                        })?,
                        tlas_node_count: count_u32(
                            scene_chunk.tlas_nodes.len(),
                            "TLAS node count",
                        )?,
                        blas_node_count: count_u32(
                            scene_chunk.blas_nodes.len(),
                            "BLAS node count",
                        )?,
                        triangle_count: count_u32(scene_chunk.triangles.len(), "triangle count")?,
                        instance_count: count_u32(scene_chunk.instances.len(), "instance count")?,
                        flags: u32::from(any_hit),
                        reserved_0: 0,
                        reserved_1: 0,
                    };
                    self.queue
                        .write_buffer(&slot.ray_buffer, 0, bytemuck::cast_slice(&slot.rays));
                    self.queue.write_buffer(
                        &slot.parameter_buffer,
                        0,
                        bytemuck::bytes_of(&parameters),
                    );
                    let ray_buffer_size = byte_size::<GpuRay>(chunk.len())?;
                    let hit_buffer_size = byte_size::<GpuHit>(chunk.len())?;
                    let mut encoder =
                        self.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("VAYU two-level traversal encoder"),
                            });
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("VAYU two-level traversal pass"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(&self.pipeline);
                        pass.set_bind_group(0, &slot.bind_group, &[]);
                        let workgroups = u32::try_from(chunk.len().div_ceil(WORKGROUP_SIZE))
                            .map_err(|_| {
                                VayuError::InvalidOptions("workgroup count exceeds u32".to_owned())
                            })?;
                        pass.dispatch_workgroups(workgroups, 1, 1);
                    }
                    encoder.copy_buffer_to_buffer(
                        &slot.hit_buffer,
                        0,
                        &slot.readback_buffer,
                        0,
                        hit_buffer_size,
                    );
                    command_buffers.push(encoder.finish());
                    ray_counts[slot_index] = chunk.len();
                    ray_offsets[slot_index] = chunk_index.saturating_mul(chunk_capacity);
                    self.runtime.uploaded_bytes.fetch_add(
                        ray_buffer_size.saturating_add(
                            u64::try_from(core::mem::size_of::<GpuParameters>())
                                .unwrap_or(u64::MAX),
                        ),
                        Ordering::Relaxed,
                    );
                }
                if command_buffers.is_empty() {
                    break;
                }
                upload_microseconds =
                    upload_microseconds.saturating_add(elapsed_microseconds(upload_started));

                let execution_started = Instant::now();
                let submission = self.queue.submit(command_buffers);
                self.runtime.dispatch_count.fetch_add(
                    u64::try_from(ray_counts.iter().filter(|count| **count > 0).count())
                        .unwrap_or(u64::MAX),
                    Ordering::Relaxed,
                );
                let mut receivers = [None, None];
                for (slot_index, ray_count) in ray_counts.into_iter().enumerate() {
                    if ray_count == 0 {
                        continue;
                    }
                    let hit_buffer_size = byte_size::<GpuHit>(ray_count)?;
                    let (sender, receiver) = mpsc::sync_channel(1);
                    slots[slot_index]
                        .readback_buffer
                        .slice(0..hit_buffer_size)
                        .map_async(wgpu::MapMode::Read, move |result| {
                            let _ = sender.send(result);
                        });
                    receivers[slot_index] = Some(receiver);
                }
                self.device
                    .poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: None,
                    })
                    .map_err(|error| self.execution_error(error.to_string()))?;
                execution_microseconds =
                    execution_microseconds.saturating_add(elapsed_microseconds(execution_started));

                let readback_started = Instant::now();
                for (slot_index, receiver) in receivers.into_iter().enumerate() {
                    let Some(receiver) = receiver else {
                        continue;
                    };
                    receiver
                        .recv()
                        .map_err(|error| self.execution_error(error.to_string()))?
                        .map_err(|error| self.execution_error(error.to_string()))?;
                    let hit_buffer_size = byte_size::<GpuHit>(ray_counts[slot_index])?;
                    {
                        let mapped = slots[slot_index]
                            .readback_buffer
                            .slice(0..hit_buffer_size)
                            .get_mapped_range()
                            .map_err(|error| self.execution_error(error.to_string()))?;
                        let gpu_hits: &[GpuHit] = bytemuck::cast_slice(&mapped);
                        for (offset, candidate) in
                            gpu_hits.iter().copied().map(unpack_hit).enumerate()
                        {
                            merge_hit(
                                &mut hits[ray_offsets[slot_index].saturating_add(offset)],
                                candidate,
                            );
                        }
                    }
                    slots[slot_index].readback_buffer.unmap();
                    self.runtime
                        .readback_bytes
                        .fetch_add(hit_buffer_size, Ordering::Relaxed);
                    dispatch_count = dispatch_count.saturating_add(1);
                }
                readback_microseconds =
                    readback_microseconds.saturating_add(elapsed_microseconds(readback_started));
            }
        }

        self.runtime
            .reusable_buffer_batch_count
            .fetch_add(1, Ordering::Relaxed);
        Ok(ComputeBatch {
            hits,
            stats: ComputeBatchStats {
                backend: ActiveBackend::PortableGpu,
                ray_count: rays.len(),
                dispatch_count,
                upload_microseconds,
                execution_microseconds,
                readback_microseconds,
                precision_error_meters: precision_error,
                used_runtime_fallback: false,
                used_reusable_buffers: true,
                fallback_reason: None,
            },
        })
    }

    fn execution_error(&self, message: String) -> VayuError {
        self.health
            .record_execution_failure(&self.runtime, message.clone());
        VayuError::Execution(message)
    }
}

impl ScratchPool {
    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        scene: &SceneBindings,
        required_rays: usize,
        maximum_rays: usize,
        runtime: &RuntimeCounters,
    ) -> Result<(), VayuError> {
        if self.capacity_rays >= required_rays {
            return Ok(());
        }
        let capacity_rays = required_rays
            .checked_next_power_of_two()
            .unwrap_or(maximum_rays)
            .min(maximum_rays)
            .max(1);
        let layout = pipeline.get_bind_group_layout(0);
        let slots = [
            ScratchSlot::create(device, &layout, scene, capacity_rays, 0)?,
            ScratchSlot::create(device, &layout, scene, capacity_rays, 1)?,
        ];
        self.slots = Some(slots);
        self.capacity_rays = capacity_rays;
        let ray_bytes = byte_size::<GpuRay>(capacity_rays)?;
        let hit_bytes = byte_size::<GpuHit>(capacity_rays)?;
        let parameter_bytes =
            u64::try_from(core::mem::size_of::<GpuParameters>()).unwrap_or(u64::MAX);
        let per_slot = ray_bytes
            .saturating_add(hit_bytes.saturating_mul(2))
            .saturating_add(parameter_bytes);
        runtime
            .scratch_generation_count
            .fetch_add(1, Ordering::Relaxed);
        runtime.scratch_capacity_rays.store(
            u64::try_from(capacity_rays).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        runtime.scratch_gpu_bytes.store(
            per_slot.saturating_mul(u64::try_from(SCRATCH_SLOT_COUNT).unwrap_or(u64::MAX)),
            Ordering::Relaxed,
        );
        Ok(())
    }
}

impl ScratchSlot {
    fn create(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        scene: &SceneBindings,
        capacity_rays: usize,
        slot_index: usize,
    ) -> Result<Self, VayuError> {
        let ray_bytes = byte_size::<GpuRay>(capacity_rays)?;
        let hit_bytes = byte_size::<GpuHit>(capacity_rays)?;
        let ray_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VAYU persistent rays"),
            size: ray_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VAYU persistent hits"),
            size: hit_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VAYU persistent readback"),
            size: hit_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let parameter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VAYU persistent parameters"),
            size: u64::try_from(core::mem::size_of::<GpuParameters>()).unwrap_or(u64::MAX),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(if slot_index == 0 {
                "VAYU persistent bindings A"
            } else {
                "VAYU persistent bindings B"
            }),
            layout,
            entries: &[
                binding(0, &scene.tlas_nodes),
                binding(1, &scene.tlas_indices),
                binding(2, &scene.instances),
                binding(3, &scene.blas_nodes),
                binding(4, &scene.blas_indices),
                binding(5, &scene.triangles),
                binding(6, &ray_buffer),
                binding(7, &hit_buffer),
                binding(8, &parameter_buffer),
            ],
        });
        Ok(Self {
            rays: Vec::with_capacity(capacity_rays),
            ray_buffer,
            hit_buffer,
            readback_buffer,
            parameter_buffer,
            bind_group,
        })
    }
}

/// Compares stable state, identity, and distance against the canonical CPU backend.
#[must_use]
pub fn compare_with_cpu(cpu: &[Hit], portable: &[Hit]) -> ParityReport {
    let mut state_mismatch_count = 0;
    let mut identity_mismatch_count = 0;
    let mut maximum_distance_error_meters = 0.0_f64;
    let mut distance_sum = 0.0;
    let mut mutual_hit_count = 0_usize;
    for (left, right) in cpu.iter().zip(portable) {
        if left.hit != right.hit {
            state_mismatch_count += 1;
            continue;
        }
        if !left.hit {
            continue;
        }
        if left.object_id != right.object_id
            || left.instance_id != right.instance_id
            || left.mesh_id != right.mesh_id
            || left.triangle_id != right.triangle_id
        {
            identity_mismatch_count += 1;
        }
        let error = (left.distance - right.distance).abs();
        maximum_distance_error_meters = maximum_distance_error_meters.max(error);
        distance_sum += error;
        mutual_hit_count += 1;
    }
    ParityReport {
        ray_count: cpu.len().min(portable.len()),
        state_mismatch_count,
        identity_mismatch_count,
        maximum_distance_error_meters,
        mean_distance_error_meters: if mutual_hit_count == 0 {
            0.0
        } else {
            distance_sum / f64::from(u32::try_from(mutual_hit_count).unwrap_or(u32::MAX))
        },
    }
}

fn cpu_batch(
    scene: &Scene,
    rays: &[QueryRay],
    used_runtime_fallback: bool,
    fallback_reason: Option<String>,
) -> ComputeBatch {
    let started = Instant::now();
    let hits = scene.trace_closest_batch(rays);
    ComputeBatch {
        hits,
        stats: ComputeBatchStats {
            backend: ActiveBackend::Cpu,
            ray_count: rays.len(),
            dispatch_count: 0,
            upload_microseconds: 0,
            execution_microseconds: elapsed_microseconds(started),
            readback_microseconds: 0,
            precision_error_meters: 0.0,
            used_runtime_fallback,
            used_reusable_buffers: false,
            fallback_reason,
        },
    }
}

fn request_adapter(
    instance: &wgpu::Instance,
    options: &ComputeOptions,
) -> Result<wgpu::Adapter, VayuError> {
    if let Some(filter) = &options.adapter_name_filter {
        let normalized = filter.to_lowercase();
        return pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
            .into_iter()
            .find(|adapter| adapter.get_info().name.to_lowercase().contains(&normalized))
            .ok_or_else(|| {
                VayuError::AdapterUnavailable(format!("no adapter name contains '{filter}'"))
            });
    }
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: match options.power_preference {
            ComputePowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
            ComputePowerPreference::LowPower => wgpu::PowerPreference::LowPower,
        },
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|error| VayuError::AdapterUnavailable(error.to_string()))
}

fn validate_options(options: &ComputeOptions) -> Result<(), VayuError> {
    if !options.maximum_precision_error_meters.is_finite()
        || options.maximum_precision_error_meters <= 0.0
    {
        return Err(VayuError::InvalidOptions(
            "precision budget must be finite and positive".to_owned(),
        ));
    }
    if options.maximum_expanded_triangle_count == 0
        || options.maximum_rays_per_dispatch == 0
        || options.maximum_geometry_chunk_bytes == 0
        || options.maximum_gpu_memory_bytes == 0
    {
        return Err(VayuError::InvalidOptions(
            "triangle, ray, geometry-chunk, and GPU-memory limits must be positive".to_owned(),
        ));
    }
    if options.maximum_geometry_chunk_bytes > options.maximum_gpu_memory_bytes {
        return Err(VayuError::InvalidOptions(
            "geometry chunk budget cannot exceed the total GPU memory budget".to_owned(),
        ));
    }
    if options
        .adapter_name_filter
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(VayuError::InvalidOptions(
            "adapter filter cannot be empty".to_owned(),
        ));
    }
    if options.preference == ComputePreference::Cpu && options.adapter_name_filter.is_some() {
        return Err(VayuError::InvalidOptions(
            "an adapter filter cannot be combined with the CPU backend".to_owned(),
        ));
    }
    Ok(())
}

fn describe_adapter(info: &wgpu::AdapterInfo) -> AdapterDescription {
    AdapterDescription {
        name: info.name.clone(),
        driver: info.driver.clone(),
        driver_info: info.driver_info.clone(),
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        vendor_id: info.vendor,
        device_id: info.device,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuNode {
    minimum: [f32; 4],
    maximum: [f32; 4],
    data: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuTriangle {
    first: [f32; 4],
    edge_one: [f32; 4],
    edge_two: [f32; 4],
    data: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuInstance {
    inverse_0: [f32; 4],
    inverse_1: [f32; 4],
    inverse_2: [f32; 4],
    object_instance: [u32; 4],
    mesh_category: [u32; 4],
    data: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuRay {
    origin_minimum: [f32; 4],
    direction_maximum: [f32; 4],
    category: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuHit {
    values: [f32; 4],
    object_instance: [u32; 4],
    mesh_data: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParameters {
    ray_count: u32,
    tlas_node_count: u32,
    blas_node_count: u32,
    triangle_count: u32,
    instance_count: u32,
    flags: u32,
    reserved_0: u32,
    reserved_1: u32,
}

fn pack_scene(snapshot: &PortableSceneSnapshot) -> PackedScene {
    PackedScene {
        tlas_nodes: pack_nodes(&snapshot.tlas_nodes),
        tlas_indices: snapshot.tlas_primitive_indices.clone(),
        instances: snapshot
            .instances
            .iter()
            .map(|instance| GpuInstance {
                inverse_0: instance.inverse_transform[0],
                inverse_1: instance.inverse_transform[1],
                inverse_2: instance.inverse_transform[2],
                object_instance: split_pair(instance.object_id, instance.instance_id),
                mesh_category: split_pair(instance.mesh_id, instance.category_mask),
                data: [
                    instance.blas_root_node,
                    u32::from(instance.mirrored),
                    instance.layer,
                    0,
                ],
            })
            .collect(),
        blas_nodes: pack_nodes(&snapshot.blas_nodes),
        blas_indices: snapshot.blas_primitive_indices.clone(),
        triangles: pack_triangles(snapshot),
    }
}

fn pack_nodes(nodes: &[xvarna_scene::PortableBvhNode]) -> Vec<GpuNode> {
    nodes
        .iter()
        .map(|node| GpuNode {
            minimum: [node.minimum[0], node.minimum[1], node.minimum[2], 0.0],
            maximum: [node.maximum[0], node.maximum[1], node.maximum[2], 0.0],
            data: [node.left, node.right, node.start, node.count],
        })
        .collect()
}

fn pack_triangles(snapshot: &PortableSceneSnapshot) -> Vec<GpuTriangle> {
    snapshot
        .triangles
        .iter()
        .map(|triangle| GpuTriangle {
            first: [triangle.first[0], triangle.first[1], triangle.first[2], 0.0],
            edge_one: [
                triangle.edge_one[0],
                triangle.edge_one[1],
                triangle.edge_one[2],
                0.0,
            ],
            edge_two: [
                triangle.edge_two[0],
                triangle.edge_two[1],
                triangle.edge_two[2],
                0.0,
            ],
            data: [triangle.triangle_id, 0, 0, 0],
        })
        .collect()
}

fn load_or_build_scene_plan(
    scene: &Scene,
    options: PortableSceneOptions,
    cache_enabled: bool,
) -> Result<ScenePlan, VayuError> {
    let key = scene_plan_key(scene, options);
    let cache = cache_enabled.then(current_cache).transpose()?;
    if let Some(store) = &cache
        && let Ok(Some(hit)) = store.get(key)
    {
        if let Some(mut plan) = decode_scene_plan(&hit.payload, scene.stats().content_hash) {
            plan.cache_hit = true;
            return Ok(plan);
        }
        let _ = store.recover_corrupt(key, "portable scene payload schema rejected");
    }
    let snapshots = scene.portable_snapshots(options)?;
    let precision_error_meters = snapshots
        .iter()
        .map(|snapshot| snapshot.stats.maximum_precision_error_meters)
        .fold(0.0_f64, f64::max);
    let plan = ScenePlan {
        rebase_origin_meters: snapshots[0].rebase_origin_meters,
        chunks: snapshots.iter().map(pack_scene).collect(),
        precision_error_meters,
        cache_hit: false,
    };
    if let Some(store) = cache
        && let Some(payload) = encode_scene_plan(&plan, scene.stats().content_hash)
    {
        let _ = store.put(key, payload);
    }
    Ok(plan)
}

fn scene_plan_key(scene: &Scene, options: PortableSceneOptions) -> CacheKey {
    let scene_hash = scene.stats().content_hash;
    CacheKey::from_parts(
        b"VAYU_TWO_LEVEL_SCENE_0_13",
        &[
            &scene_hash,
            &options
                .maximum_precision_error_meters
                .to_bits()
                .to_le_bytes(),
            &u64::try_from(options.maximum_expanded_triangle_count)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
            &u64::try_from(options.maximum_leaf_size)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
            &u64::try_from(options.maximum_chunk_payload_bytes)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        ],
    )
}

fn encode_scene_plan(plan: &ScenePlan, scene_hash: [u8; 32]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(b"XVSCN13\0");
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&scene_hash);
    for value in [
        plan.rebase_origin_meters.x,
        plan.rebase_origin_meters.y,
        plan.rebase_origin_meters.z,
        plan.precision_error_meters,
    ] {
        output.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    output.extend_from_slice(&u64::try_from(plan.chunks.len()).ok()?.to_le_bytes());
    for chunk in &plan.chunks {
        write_pod_vec(&mut output, &chunk.tlas_nodes)?;
        write_pod_vec(&mut output, &chunk.tlas_indices)?;
        write_pod_vec(&mut output, &chunk.instances)?;
        write_pod_vec(&mut output, &chunk.blas_nodes)?;
        write_pod_vec(&mut output, &chunk.blas_indices)?;
        write_pod_vec(&mut output, &chunk.triangles)?;
    }
    Some(output)
}

fn decode_scene_plan(payload: &[u8], expected_hash: [u8; 32]) -> Option<ScenePlan> {
    let mut reader = ByteReader::new(payload);
    if reader.read_array::<8>()? != *b"XVSCN13\0"
        || reader.read_u32()? != 1
        || reader.read_u32()? != 0
        || reader.read_array::<32>()? != expected_hash
    {
        return None;
    }
    let rebase_origin_meters = Vec3::new(
        f64::from_bits(reader.read_u64()?),
        f64::from_bits(reader.read_u64()?),
        f64::from_bits(reader.read_u64()?),
    );
    let precision_error_meters = f64::from_bits(reader.read_u64()?);
    if !rebase_origin_meters.is_finite()
        || !precision_error_meters.is_finite()
        || precision_error_meters < 0.0
    {
        return None;
    }
    let chunk_count = usize::try_from(reader.read_u64()?).ok()?;
    if chunk_count == 0 || chunk_count > 1_000_000 {
        return None;
    }
    let mut chunks = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        let chunk = PackedScene {
            tlas_nodes: reader.read_pod_vec()?,
            tlas_indices: reader.read_pod_vec()?,
            instances: reader.read_pod_vec()?,
            blas_nodes: reader.read_pod_vec()?,
            blas_indices: reader.read_pod_vec()?,
            triangles: reader.read_pod_vec()?,
        };
        if chunk.tlas_nodes.is_empty()
            || chunk.tlas_indices.is_empty()
            || chunk.instances.is_empty()
            || chunk.blas_nodes.is_empty()
            || chunk.blas_indices.is_empty()
            || chunk.triangles.is_empty()
        {
            return None;
        }
        chunks.push(chunk);
    }
    if !reader.finished() {
        return None;
    }
    Some(ScenePlan {
        chunks,
        rebase_origin_meters,
        precision_error_meters,
        cache_hit: true,
    })
}

fn write_pod_vec<T: Pod>(output: &mut Vec<u8>, values: &[T]) -> Option<()> {
    output.extend_from_slice(&u64::try_from(values.len()).ok()?.to_le_bytes());
    output.extend_from_slice(bytemuck::cast_slice(values));
    Some(())
}

struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.offset.checked_add(N)?;
        let value = self.bytes.get(self.offset..end)?.try_into().ok()?;
        self.offset = end;
        Some(value)
    }

    fn read_u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.read_array()?))
    }

    fn read_pod_vec<T: Pod>(&mut self) -> Option<Vec<T>> {
        let count = usize::try_from(self.read_u64()?).ok()?;
        let byte_count = count.checked_mul(core::mem::size_of::<T>())?;
        let end = self.offset.checked_add(byte_count)?;
        let bytes = self.bytes.get(self.offset..end)?;
        let mut output = Vec::with_capacity(count);
        for chunk in bytes.chunks_exact(core::mem::size_of::<T>()) {
            output.push(bytemuck::pod_read_unaligned(chunk));
        }
        if output.len() != count {
            return None;
        }
        self.offset = end;
        Some(output)
    }

    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn pack_rays_into(rays: &[QueryRay], rebase: Vec3, packed: &mut Vec<GpuRay>) -> f64 {
    let mut maximum_error = 0.0_f64;
    packed.clear();
    packed.extend(rays.iter().map(|ray| {
        let origin = ray.origin - rebase;
        let origin_minimum = quantize_four(
            [origin.x, origin.y, origin.z, ray.t_min],
            &mut maximum_error,
        );
        let direction_maximum = quantize_four(
            [ray.direction.x, ray.direction.y, ray.direction.z, ray.t_max],
            &mut maximum_error,
        );
        GpuRay {
            origin_minimum,
            direction_maximum,
            category: [
                lower_word(ray.category_mask),
                upper_word(ray.category_mask),
                0,
                0,
            ],
        }
    }));
    maximum_error
}

#[allow(clippy::cast_possible_truncation)]
fn quantize_four(values: [f64; 4], maximum_error: &mut f64) -> [f32; 4] {
    let converted = values.map(|value| value as f32);
    for (source, portable) in values.into_iter().zip(converted) {
        *maximum_error = maximum_error.max((source - f64::from(portable)).abs());
    }
    converted
}

fn unpack_hit(value: GpuHit) -> Hit {
    let flags = value.mesh_data[3];
    if flags & HIT_FLAG == 0 {
        return Hit::miss();
    }
    Hit {
        hit: true,
        distance: f64::from(value.values[0]),
        barycentric_u: f64::from(value.values[1]),
        barycentric_v: f64::from(value.values[2]),
        object_id: xvarna_types::ObjectId::new(join_words(
            value.object_instance[0],
            value.object_instance[1],
        )),
        instance_id: xvarna_types::InstanceId::new(join_words(
            value.object_instance[2],
            value.object_instance[3],
        )),
        mesh_id: xvarna_types::MeshId::new(join_words(value.mesh_data[0], value.mesh_data[1])),
        triangle_id: value.mesh_data[2],
        front_face: flags & FRONT_FACE_FLAG != 0,
    }
}

const fn split_pair(first: u64, second: u64) -> [u32; 4] {
    [
        lower_word(first),
        upper_word(first),
        lower_word(second),
        upper_word(second),
    ]
}

const fn lower_word(value: u64) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

const fn upper_word(value: u64) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
}

fn join_words(lower: u32, upper: u32) -> u64 {
    u64::from(lower) | (u64::from(upper) << 32)
}

fn create_scene_buffer<T: Pod>(
    device: &wgpu::Device,
    label: &str,
    capacity: usize,
) -> Result<wgpu::Buffer, VayuError> {
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: byte_size::<T>(capacity.max(1))?,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }))
}

fn upload_scene(queue: &wgpu::Queue, bindings: &SceneBindings, scene: &PackedScene) {
    queue.write_buffer(
        &bindings.tlas_nodes,
        0,
        bytemuck::cast_slice(&scene.tlas_nodes),
    );
    queue.write_buffer(
        &bindings.tlas_indices,
        0,
        bytemuck::cast_slice(&scene.tlas_indices),
    );
    queue.write_buffer(
        &bindings.instances,
        0,
        bytemuck::cast_slice(&scene.instances),
    );
    queue.write_buffer(
        &bindings.blas_nodes,
        0,
        bytemuck::cast_slice(&scene.blas_nodes),
    );
    queue.write_buffer(
        &bindings.blas_indices,
        0,
        bytemuck::cast_slice(&scene.blas_indices),
    );
    queue.write_buffer(
        &bindings.triangles,
        0,
        bytemuck::cast_slice(&scene.triangles),
    );
}

fn maximum_count(scenes: &[PackedScene], select: impl Fn(&PackedScene) -> usize) -> usize {
    scenes.iter().map(select).max().unwrap_or(1).max(1)
}

fn validate_scene_binding_limits(
    scenes: &[PackedScene],
    maximum_binding_bytes: usize,
) -> Result<(), VayuError> {
    for (chunk_index, scene) in scenes.iter().enumerate() {
        for (name, bytes) in [
            (
                "TLAS nodes",
                scene.tlas_nodes.len() * core::mem::size_of::<GpuNode>(),
            ),
            (
                "TLAS indices",
                scene.tlas_indices.len() * core::mem::size_of::<u32>(),
            ),
            (
                "instances",
                scene.instances.len() * core::mem::size_of::<GpuInstance>(),
            ),
            (
                "BLAS nodes",
                scene.blas_nodes.len() * core::mem::size_of::<GpuNode>(),
            ),
            (
                "BLAS indices",
                scene.blas_indices.len() * core::mem::size_of::<u32>(),
            ),
            (
                "triangles",
                scene.triangles.len() * core::mem::size_of::<GpuTriangle>(),
            ),
        ] {
            if bytes > maximum_binding_bytes {
                return Err(VayuError::InvalidOptions(format!(
                    "geometry chunk {chunk_index} {name} buffer needs {bytes} bytes, above adapter binding limit {maximum_binding_bytes}"
                )));
            }
        }
    }
    Ok(())
}

fn count_u32(value: usize, name: &str) -> Result<u32, VayuError> {
    u32::try_from(value)
        .map_err(|_| VayuError::InvalidOptions(format!("portable {name} exceeds u32")))
}

fn merge_hit(best: &mut Hit, candidate: Hit) {
    if candidate.hit && (!best.hit || candidate.distance < best.distance) {
        *best = candidate;
    }
}

const fn binding(index: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding: index,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: None,
        }),
    }
}

fn byte_size<T>(count: usize) -> Result<u64, VayuError> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| VayuError::InvalidOptions("buffer byte size overflow".to_owned()))?;
    u64::try_from(bytes)
        .map_err(|_| VayuError::InvalidOptions("buffer byte size exceeds u64".to_owned()))
}

fn elapsed_microseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::{Mesh, Vec3};
    use xvarna_scene::{SceneBuildOptions, SceneBuilder, Transform};
    use xvarna_types::{InstanceId, ObjectId};

    fn scene() -> Arc<Scene> {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                triangles: vec![[0, 1, 2]],
            })
            .expect("mesh valid");
        builder
            .add_instance(
                mesh_id,
                Transform::IDENTITY,
                ObjectId::new(u64::from(u32::MAX) + 19),
                InstanceId::new(u64::from(u32::MAX) + 23),
                4,
            )
            .expect("instance valid");
        Arc::new(builder.build().expect("scene valid"))
    }

    fn instanced_scene(count: u64) -> Arc<Scene> {
        let mut builder = SceneBuilder::new(SceneBuildOptions::default()).expect("options valid");
        let mesh_id = builder
            .add_mesh(Mesh {
                positions: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                triangles: vec![[0, 1, 2]],
            })
            .expect("mesh valid");
        for index in 0..count {
            let transform = Transform::try_from_row_major([
                1.0,
                0.0,
                0.0,
                f64::from(u32::try_from(index).expect("test instance count fits u32")) * 2.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ])
            .expect("transform valid");
            builder
                .add_instance(
                    mesh_id,
                    transform,
                    ObjectId::new(index + 1),
                    InstanceId::new(index + 1),
                    4,
                )
                .expect("instance valid");
        }
        Arc::new(builder.build().expect("scene valid"))
    }

    #[test]
    fn cpu_preference_is_a_first_class_session() {
        let session = ComputeSession::create(
            scene(),
            ComputeOptions {
                preference: ComputePreference::Cpu,
                ..ComputeOptions::default()
            },
        )
        .expect("CPU session valid");
        let ray = QueryRay::try_new(
            Vec3::new(0.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            2.0,
            4,
        )
        .expect("ray valid");
        let batch = session.trace_closest(&[ray]).expect("query valid");
        assert_eq!(batch.stats.backend, ActiveBackend::Cpu);
        assert!(batch.hits[0].hit);
        assert_eq!(batch.hits[0].object_id.get(), u64::from(u32::MAX) + 19);
    }

    #[test]
    fn identity_words_round_trip_full_u64_range() {
        let values = [0, 1, u64::from(u32::MAX), u64::MAX, 0x1234_5678_9abc_def0];
        for value in values {
            assert_eq!(join_words(lower_word(value), upper_word(value)), value);
        }
    }

    #[test]
    fn cached_two_level_plan_round_trips_exact_gpu_payloads() {
        let scene = instanced_scene(5);
        let plan = load_or_build_scene_plan(
            &scene,
            PortableSceneOptions {
                maximum_chunk_payload_bytes: 900,
                ..PortableSceneOptions::default()
            },
            false,
        )
        .expect("plan builds");
        assert!(plan.chunks.len() > 1);
        let encoded = encode_scene_plan(&plan, scene.stats().content_hash).expect("encodes");
        let decoded =
            decode_scene_plan(&encoded, scene.stats().content_hash).expect("decodes exactly");
        assert_eq!(decoded.chunks.len(), plan.chunks.len());
        assert_eq!(
            bytemuck::cast_slice::<GpuInstance, u8>(&decoded.chunks[0].instances),
            bytemuck::cast_slice::<GpuInstance, u8>(&plan.chunks[0].instances)
        );
        assert_eq!(decoded.rebase_origin_meters, plan.rebase_origin_meters);
        assert!(decode_scene_plan(&encoded, [99; 32]).is_none());
    }

    #[test]
    fn parity_report_separates_state_identity_and_distance() {
        let cpu = [Hit {
            hit: true,
            distance: 1.0,
            barycentric_u: 0.2,
            barycentric_v: 0.3,
            object_id: ObjectId::new(1),
            instance_id: InstanceId::new(2),
            mesh_id: xvarna_types::MeshId::new(3),
            triangle_id: 4,
            front_face: true,
        }];
        let mut portable = cpu;
        portable[0].distance += 1.0e-6;
        let report = compare_with_cpu(&cpu, &portable);
        assert_eq!(report.state_mismatch_count, 0);
        assert_eq!(report.identity_mismatch_count, 0);
        assert!(report.maximum_distance_error_meters > 0.0);
    }

    #[test]
    fn portable_gpu_matches_cpu_when_an_adapter_is_available() {
        let scene = scene();
        let Some(adapter) = enumerate_adapters().into_iter().next() else {
            return;
        };
        let session = match ComputeSession::create(
            Arc::clone(&scene),
            ComputeOptions {
                preference: ComputePreference::RequirePortableGpu,
                adapter_name_filter: Some(adapter.name.clone()),
                maximum_rays_per_dispatch: 2048,
                ..ComputeOptions::default()
            },
        ) {
            Ok(session) => session,
            Err(VayuError::AdapterUnavailable(_) | VayuError::DeviceRequest(_)) => return,
            Err(error) => panic!("unexpected VAYU creation failure: {error}"),
        };
        assert!(
            session
                .info()
                .adapter
                .as_ref()
                .is_some_and(|selected| selected.name == adapter.name)
        );
        let rays = (0..4096)
            .map(|index| {
                let x = f64::from(index % 64) / 63.0;
                let y = f64::from(index / 64) / 63.0;
                QueryRay::try_new(Vec3::new(x, y, 2.0), Vec3::new(0.0, 0.0, -1.0), 0.0, 4.0, 4)
                    .expect("ray valid")
            })
            .collect::<Vec<_>>();
        let cpu = scene.trace_closest_batch(&rays);
        let cold = session
            .trace_closest(&rays[..1024])
            .expect("small cold GPU query succeeds");
        assert_eq!(cold.stats.backend, ActiveBackend::PortableGpu);
        assert!(cold.stats.used_reusable_buffers);
        let after_cold = session.runtime_stats();
        assert_eq!(after_cold.scratch_generation_count, 1);
        assert!(after_cold.scratch_capacity_rays >= 1024);
        let portable = session
            .trace_closest(&rays)
            .expect("larger GPU query grows the arena");
        let after_growth = session.runtime_stats();
        assert_eq!(after_growth.scratch_generation_count, 2);
        assert_eq!(after_growth.scratch_capacity_rays, 2048);
        assert_eq!(portable.stats.dispatch_count, 2);
        let warm = session
            .trace_closest(&rays)
            .expect("warm reusable GPU query succeeds");
        assert!(warm.stats.used_reusable_buffers);
        let after_warm = session.runtime_stats();
        assert_eq!(after_warm.scratch_generation_count, 2);
        assert_eq!(after_warm.reusable_buffer_batch_count, 3);
        assert_eq!(after_warm.trace_count, 3);
        assert_eq!(after_warm.dispatch_count, 5);
        let parity = compare_with_cpu(&cpu, &portable.hits);
        assert_eq!(parity.state_mismatch_count, 0);
        assert_eq!(parity.identity_mismatch_count, 0);
        assert!(parity.maximum_distance_error_meters <= 2.0e-5);
    }

    #[test]
    fn streamed_two_level_gpu_chunks_merge_closest_hits_with_cpu_parity() {
        let scene = instanced_scene(20);
        let Some(adapter) = enumerate_adapters().into_iter().next() else {
            return;
        };
        let session = match ComputeSession::create(
            Arc::clone(&scene),
            ComputeOptions {
                preference: ComputePreference::RequirePortableGpu,
                adapter_name_filter: Some(adapter.name),
                maximum_geometry_chunk_bytes: 900,
                maximum_gpu_memory_bytes: 2 * 1024 * 1024,
                maximum_rays_per_dispatch: 64,
                enable_scene_cache: false,
                ..ComputeOptions::default()
            },
        ) {
            Ok(session) => session,
            Err(VayuError::AdapterUnavailable(_) | VayuError::DeviceRequest(_)) => return,
            Err(error) => panic!("unexpected streamed VAYU creation failure: {error}"),
        };
        assert!(session.info().geometry_chunk_count > 1);
        assert!(session.info().streamed_geometry);
        assert_eq!(session.info().unique_triangle_count, 1);
        assert_eq!(session.info().instance_count, 20);
        let rays = (0..20)
            .map(|index| {
                QueryRay::try_new(
                    Vec3::new(f64::from(index).mul_add(2.0, 0.25), 0.25, 2.0),
                    Vec3::new(0.0, 0.0, -1.0),
                    0.0,
                    4.0,
                    4,
                )
                .expect("ray valid")
            })
            .collect::<Vec<_>>();
        let cpu = scene.trace_closest_batch(&rays);
        let gpu = session
            .trace_closest(&rays)
            .expect("streamed query succeeds");
        let parity = compare_with_cpu(&cpu, &gpu.hits);
        assert_eq!(parity.state_mismatch_count, 0);
        assert_eq!(parity.identity_mismatch_count, 0);
        let runtime = session.runtime_stats();
        assert_eq!(
            runtime.geometry_upload_count,
            u64::try_from(session.info().geometry_chunk_count).expect("chunk count fits")
        );
        assert!(runtime.geometry_uploaded_bytes > 0);
    }

    #[test]
    fn device_loss_is_idempotent_and_preserves_exact_diagnostic() {
        let counters = RuntimeCounters::default();
        let health = DeviceHealth::default();
        health.record_device_loss(&counters, "device lost: first".to_owned());
        health.record_device_loss(&counters, "device lost: confirmed".to_owned());
        assert!(!health.is_healthy());
        assert_eq!(counters.device_loss_count.load(Ordering::Relaxed), 1);
        assert_eq!(counters.execution_error_count.load(Ordering::Relaxed), 2);
        assert_eq!(
            health.last_error().as_deref(),
            Some("device lost: confirmed")
        );
        assert!(health.require_healthy().is_err());

        let execution_counters = RuntimeCounters::default();
        let execution_health = DeviceHealth::default();
        execution_health.record_execution_failure(&execution_counters, "mapping failed".to_owned());
        assert!(!execution_health.is_healthy());
        assert_eq!(
            execution_counters.device_loss_count.load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            execution_counters
                .execution_error_count
                .load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn injected_lost_health_drives_auto_runtime_fallback_when_gpu_is_available() {
        let Some(adapter) = enumerate_adapters().into_iter().next() else {
            return;
        };
        let session = match ComputeSession::create(
            scene(),
            ComputeOptions {
                preference: ComputePreference::Auto,
                adapter_name_filter: Some(adapter.name),
                ..ComputeOptions::default()
            },
        ) {
            Ok(session) if session.gpu.is_some() => session,
            Ok(_) | Err(VayuError::AdapterUnavailable(_) | VayuError::DeviceRequest(_)) => return,
            Err(error) => panic!("unexpected VAYU creation failure: {error}"),
        };
        let gpu = session.gpu.as_ref().expect("guarded above");
        gpu.health
            .record_device_loss(&session.runtime, "injected device loss".to_owned());
        let ray = QueryRay::try_new(
            Vec3::new(0.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            2.0,
            4,
        )
        .expect("ray valid");
        let batch = session
            .trace_closest(&[ray])
            .expect("Auto falls back after injected loss");
        assert_eq!(batch.stats.backend, ActiveBackend::Cpu);
        assert!(batch.stats.used_runtime_fallback);
        assert!(batch.hits[0].hit);
        let runtime = session.runtime_stats();
        assert_eq!(runtime.device_loss_count, 1);
        assert_eq!(runtime.runtime_fallback_count, 1);
        assert!(!runtime.device_healthy);
        assert_eq!(runtime.last_error.as_deref(), Some("injected device loss"));
    }
}
