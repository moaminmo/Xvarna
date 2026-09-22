//! VAYU portable compute-session ABI with deterministic CPU fallback.

use super::{XvStatus, output_slice};
use crate::scene::{XvHit, XvRay, convert_rays, get_ready_scene, hit_from_scene};
use core::mem;
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use xvarna_vayu::{
    ActiveBackend, AdapterDescription, ComputeBatchStats, ComputeOptions, ComputePowerPreference,
    ComputePreference, ComputeRuntimeStats, ComputeSession, VayuCacheConfig, VayuCacheStats,
    VayuError, clear_scene_cache, configure_scene_cache, enumerate_adapters, scene_cache_config,
    scene_cache_stats,
};

const INFO_FLAG_CREATION_FALLBACK: u32 = 1 << 0;
const INFO_FLAG_HAS_ADAPTER: u32 = 1 << 1;
const BATCH_FLAG_RUNTIME_FALLBACK: u32 = 1 << 0;
const BATCH_FLAG_REUSABLE_BUFFERS: u32 = 1 << 1;
const RUNTIME_FLAG_DEVICE_HEALTHY: u32 = 1 << 0;
const COMPUTE_FLAG_SCENE_CACHE: u32 = 1 << 0;
const INFO_FLAG_SCENE_CACHE_HIT: u32 = 1 << 2;
const INFO_FLAG_STREAMED_GEOMETRY: u32 = 1 << 3;

static NEXT_COMPUTE_HANDLE: AtomicU64 = AtomicU64::new(1);
static COMPUTE_SESSIONS: OnceLock<RwLock<HashMap<u64, Arc<ComputeSession>>>> = OnceLock::new();

/// Fixed-layout compute creation and resource policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvComputeOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Zero Auto, one Require Portable GPU, two CPU.
    pub preference: u32,
    /// Zero high performance, one low power.
    pub power_preference: u32,
    /// Bit zero enables the portable-scene cache.
    pub flags: u32,
    /// Maximum accepted f32 conversion error in canonical metres.
    pub maximum_precision_error_meters: f64,
    /// Hard cap on effective triangles in a portable snapshot.
    pub maximum_expanded_triangle_count: u64,
    /// Hard cap per GPU dispatch/readback chunk.
    pub maximum_rays_per_dispatch: u64,
    /// Maximum bytes in one streamed geometry chunk.
    pub maximum_geometry_chunk_bytes: u64,
    /// Hard retained GPU working-set budget.
    pub maximum_gpu_memory_bytes: u64,
    /// Reserved; must be zero.
    pub reserved: u64,
}

impl Default for XvComputeOptions {
    fn default() -> Self {
        let options = ComputeOptions::default();
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>()).expect("ABI size fits u32"),
            preference: 0,
            power_preference: 0,
            flags: COMPUTE_FLAG_SCENE_CACHE,
            maximum_precision_error_meters: options.maximum_precision_error_meters,
            maximum_expanded_triangle_count: u64::try_from(options.maximum_expanded_triangle_count)
                .expect("default triangle count fits u64"),
            maximum_rays_per_dispatch: u64::try_from(options.maximum_rays_per_dispatch)
                .expect("default ray count fits u64"),
            maximum_geometry_chunk_bytes: u64::try_from(options.maximum_geometry_chunk_bytes)
                .expect("default geometry chunk bytes fit u64"),
            maximum_gpu_memory_bytes: u64::try_from(options.maximum_gpu_memory_bytes)
                .expect("default GPU memory bytes fit u64"),
            reserved: 0,
        }
    }
}

/// Fixed-layout selected backend and initialization metadata.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvComputeInfo {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Zero CPU, one Portable GPU.
    pub backend: u32,
    /// Creation fallback/adapter flags.
    pub flags: u32,
    /// Graphics API backend code.
    pub adapter_backend: u32,
    /// Adapter device class code.
    pub device_type: u32,
    /// Adapter vendor identifier.
    pub vendor_id: u32,
    /// Adapter device identifier.
    pub device_id: u32,
    /// Bounded two-level geometry chunk count.
    pub geometry_chunk_count: u32,
    /// Effective triangle count.
    pub triangle_count: u64,
    /// Active hierarchy node count.
    pub node_count: u64,
    /// Approximate portable allocation payload bytes.
    pub approximate_gpu_bytes: u64,
    /// Session initialization microseconds.
    pub initialization_microseconds: u64,
    /// Largest scene f32 conversion error in metres.
    pub scene_precision_error_meters: f64,
    /// UTF-8 selected adapter name, or empty for CPU.
    pub adapter_name: [u8; 128],
    /// UTF-8 creation fallback/status detail.
    pub message: [u8; 256],
}

impl Default for XvComputeInfo {
    fn default() -> Self {
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>()).expect("ABI size fits u32"),
            backend: 0,
            flags: 0,
            adapter_backend: 0,
            device_type: 0,
            vendor_id: 0,
            device_id: 0,
            geometry_chunk_count: 0,
            triangle_count: 0,
            node_count: 0,
            approximate_gpu_bytes: 0,
            initialization_microseconds: 0,
            scene_precision_error_meters: 0.0,
            adapter_name: [0; 128],
            message: [0; 256],
        }
    }
}

/// Fixed-layout per-batch timing and fallback metadata.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct XvComputeBatchStats {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Zero CPU, one Portable GPU.
    pub backend: u32,
    /// Runtime fallback flags.
    pub flags: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// Ordered ray count.
    pub ray_count: u64,
    /// GPU dispatch count; zero for CPU.
    pub dispatch_count: u64,
    /// Upload/encoding microseconds.
    pub upload_microseconds: u64,
    /// Execution/wait microseconds.
    pub execution_microseconds: u64,
    /// Readback/decode microseconds.
    pub readback_microseconds: u64,
    /// Largest scene/ray f32 conversion error in metres.
    pub precision_error_meters: f64,
}

/// Fixed-layout cumulative session throughput, scratch, fallback, and device-health telemetry.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvComputeRuntimeStats {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Device-health flags.
    pub flags: u32,
    /// Reserved; always zero.
    pub reserved_0: u32,
    /// Reserved; always zero.
    pub reserved_1: u32,
    /// Closest-hit and any-hit requests.
    pub trace_count: u64,
    /// Rays accepted by the session.
    pub ray_count: u64,
    /// Portable dispatches submitted.
    pub dispatch_count: u64,
    /// Successful batches served by reusable buffers.
    pub reusable_buffer_batch_count: u64,
    /// Auto-mode runtime fallbacks.
    pub runtime_fallback_count: u64,
    /// Scratch allocation/growth generations.
    pub scratch_generation_count: u64,
    /// Current ray capacity of each scratch slot.
    pub scratch_capacity_rays: u64,
    /// Current retained scratch bytes across both slots.
    pub scratch_gpu_bytes: u64,
    /// Total queue-write bytes.
    pub uploaded_bytes: u64,
    /// Total copied-back bytes.
    pub readback_bytes: u64,
    /// Device-loss callbacks observed.
    pub device_loss_count: u64,
    /// Execution errors observed.
    pub execution_error_count: u64,
    /// Geometry chunk uploads after initialization.
    pub geometry_upload_count: u64,
    /// Streamed geometry bytes uploaded after initialization.
    pub geometry_uploaded_bytes: u64,
    /// UTF-8 most recent error, nul-terminated when space permits.
    pub last_error: [u8; 256],
}

impl Default for XvComputeRuntimeStats {
    fn default() -> Self {
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>()).expect("ABI size fits u32"),
            flags: 0,
            reserved_0: 0,
            reserved_1: 0,
            trace_count: 0,
            ray_count: 0,
            dispatch_count: 0,
            reusable_buffer_batch_count: 0,
            runtime_fallback_count: 0,
            scratch_generation_count: 0,
            scratch_capacity_rays: 0,
            scratch_gpu_bytes: 0,
            uploaded_bytes: 0,
            readback_bytes: 0,
            device_loss_count: 0,
            execution_error_count: 0,
            geometry_upload_count: 0,
            geometry_uploaded_bytes: 0,
            last_error: [0; 256],
        }
    }
}

/// Process-wide bounded cache configuration.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvCacheOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub flags: u32,
    /// In-process payload budget.
    pub memory_budget_bytes: u64,
    /// On-disk entry budget.
    pub disk_budget_bytes: u64,
}

/// Process-wide cache telemetry and active configuration.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvCacheStats {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub flags: u32,
    /// Memory hits.
    pub memory_hit_count: u64,
    /// Disk hits.
    pub disk_hit_count: u64,
    /// Misses.
    pub miss_count: u64,
    /// Successful writes.
    pub write_count: u64,
    /// Memory/disk evictions.
    pub eviction_count: u64,
    /// Corrupt entries removed.
    pub corruption_count: u64,
    /// Current memory payload bytes.
    pub memory_bytes: u64,
    /// Current disk bytes.
    pub disk_bytes: u64,
    /// Current memory entries.
    pub memory_entry_count: u64,
    /// Current disk entries.
    pub disk_entry_count: u64,
    /// Configured memory budget.
    pub memory_budget_bytes: u64,
    /// Configured disk budget.
    pub disk_budget_bytes: u64,
    /// UTF-8 active directory.
    pub directory: [u8; 256],
    /// UTF-8 latest event/recovery diagnostic.
    pub last_event: [u8; 256],
}

/// Fixed-size adapter descriptor returned by enumeration.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvAdapterInfo {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Graphics API backend code.
    pub backend: u32,
    /// Adapter device class code.
    pub device_type: u32,
    /// Reserved flags; always zero.
    pub flags: u32,
    /// Adapter vendor identifier.
    pub vendor_id: u32,
    /// Adapter device identifier.
    pub device_id: u32,
    /// Reserved; always zero.
    pub reserved_0: u32,
    /// Reserved; always zero.
    pub reserved_1: u32,
    /// UTF-8 adapter name, nul-terminated when space permits.
    pub name: [u8; 128],
    /// UTF-8 driver name/detail, nul-terminated when space permits.
    pub driver: [u8; 64],
}

impl Default for XvAdapterInfo {
    fn default() -> Self {
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>()).expect("ABI size fits u32"),
            backend: 0,
            device_type: 0,
            flags: 0,
            vendor_id: 0,
            device_id: 0,
            reserved_0: 0,
            reserved_1: 0,
            name: [0; 128],
            driver: [0; 64],
        }
    }
}

/// Configures the process-wide versioned portable-scene cache.
///
/// # Safety
///
/// `options` references one readable record and `directory_utf8` references
/// `directory_length` readable UTF-8 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_cache_configure(
    options: *const XvCacheOptions,
    directory_utf8: *const u8,
    directory_length: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if directory_length == 0 || directory_length > 32_768 {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Required readable pointer was checked above.
        let options = unsafe { options.read() };
        if options.structure_size
            != u32::try_from(mem::size_of::<XvCacheOptions>()).expect("ABI size fits u32")
            || options.flags != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: Public contract requires this readable byte range.
        let bytes = unsafe { super::input_slice(directory_utf8, directory_length)? };
        let directory = core::str::from_utf8(bytes)
            .map_err(|_| XvStatus::InvalidArgument)?
            .trim();
        if directory.is_empty() {
            return Err(XvStatus::InvalidArgument);
        }
        let config = VayuCacheConfig::try_new(
            PathBuf::from(directory),
            options.memory_budget_bytes,
            options.disk_budget_bytes,
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        configure_scene_cache(config).map_err(vayu_status)
    })
}

/// Reads cumulative cache telemetry without resetting it.
///
/// # Safety
///
/// `output_stats` references one writable [`XvCacheStats`] record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_cache_get_stats(output_stats: *mut XvCacheStats) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let stats = scene_cache_stats().map_err(vayu_status)?;
        let config = scene_cache_config().map_err(vayu_status)?;
        // SAFETY: Caller-owned output was checked for null.
        unsafe { output_stats.write(cache_stats(&stats, &config)) };
        Ok(())
    })
}

/// Clears only XVARNA `.xvc` entries in the configured directory.
#[unsafe(no_mangle)]
pub extern "C" fn xv_cache_clear() -> i32 {
    ffi_status(|| clear_scene_cache().map_err(vayu_status))
}

/// Enumerates every portable adapter visible to wgpu.
///
/// # Safety
///
/// `output_count` is writable. `output` is either null with zero capacity or
/// references `capacity` writable [`XvAdapterInfo`] values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_enumerate_adapters(
    output: *mut XvAdapterInfo,
    capacity: usize,
    output_count: *mut usize,
) -> i32 {
    ffi_status(|| {
        if output_count.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let adapters = enumerate_adapters();
        // SAFETY: The required writable output pointer was validated above.
        unsafe { output_count.write(adapters.len()) };
        if output.is_null() && capacity == 0 {
            return Ok(());
        }
        if capacity < adapters.len() {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacity and nullability are validated by the shared helper.
        let destination = unsafe { output_slice(output, adapters.len())? };
        for (target, adapter) in destination.iter_mut().zip(&adapters) {
            *target = adapter_info(adapter);
        }
        Ok(())
    })
}

/// Creates a compute session over one immutable scene.
///
/// # Safety
///
/// Every pointer references one readable/writable value for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_create(
    scene_handle: u64,
    options: *const XvComputeOptions,
    output_handle: *mut u64,
    output_info: *mut XvComputeInfo,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_handle.is_null() || output_info.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: All pointers were checked and remain valid for this call.
        let native = unsafe { options.read() };
        let options = convert_options(native)?;
        let (handle, info) = create_session(scene_handle, options)?;
        // SAFETY: Caller-owned outputs were checked for null.
        unsafe {
            output_handle.write(handle);
            output_info.write(info);
        }
        Ok(())
    })
}

/// Creates a compute session on an adapter whose name contains one UTF-8 filter.
///
/// Matching is case-insensitive. A required-GPU request fails when no adapter
/// matches; Auto reports the resulting creation fallback through `output_info`.
///
/// # Safety
///
/// `adapter_name_filter` references `adapter_name_filter_length` readable UTF-8
/// bytes. Every other pointer references one readable/writable value for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_create_for_adapter(
    scene_handle: u64,
    options: *const XvComputeOptions,
    adapter_name_filter: *const u8,
    adapter_name_filter_length: usize,
    output_handle: *mut u64,
    output_info: *mut XvComputeInfo,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_handle.is_null() || output_info.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if adapter_name_filter_length == 0 || adapter_name_filter_length > 256 {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: The caller promises a readable filter buffer for this call.
        let bytes = unsafe { super::input_slice(adapter_name_filter, adapter_name_filter_length)? };
        let filter = core::str::from_utf8(bytes)
            .map_err(|_| XvStatus::InvalidArgument)?
            .trim();
        if filter.is_empty() {
            return Err(XvStatus::InvalidArgument);
        }
        // SAFETY: All fixed-layout pointers were checked and remain valid.
        let native = unsafe { options.read() };
        let mut options = convert_options(native)?;
        options.adapter_name_filter = Some(filter.to_owned());
        let (handle, info) = create_session(scene_handle, options)?;
        // SAFETY: Caller-owned outputs were checked for null.
        unsafe {
            output_handle.write(handle);
            output_info.write(info);
        }
        Ok(())
    })
}

/// Traces ordered closest-hit rays using the selected backend/fallback policy.
///
/// # Safety
///
/// Input/output arrays contain `ray_count` readable/writable entries and
/// `output_stats` references one writable value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_trace_closest(
    handle: u64,
    rays: *const XvRay,
    ray_count: usize,
    output_hits: *mut XvHit,
    hit_capacity: usize,
    output_stats: *mut XvComputeBatchStats,
) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if hit_capacity < ray_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Slice lengths and pointer validity are checked by helpers.
        let rays = unsafe { super::input_slice(rays, ray_count)? };
        // SAFETY: The capacity was checked immediately above.
        let output = unsafe { output_slice(output_hits, ray_count)? };
        let queries = convert_rays(rays)?;
        let session = get_session(handle)?;
        let batch = session.trace_closest(&queries).map_err(vayu_status)?;
        for (destination, hit) in output.iter_mut().zip(batch.hits) {
            *destination = hit_from_scene(hit);
        }
        // SAFETY: The caller-owned output was checked for null.
        unsafe { output_stats.write(batch_stats(&batch.stats)) };
        Ok(())
    })
}

/// Traces ordered early-exit visibility rays.
///
/// # Safety
///
/// Input/output arrays contain `ray_count` readable/writable entries and
/// `output_stats` references one writable value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_trace_any(
    handle: u64,
    rays: *const XvRay,
    ray_count: usize,
    output_hits: *mut u8,
    hit_capacity: usize,
    output_stats: *mut XvComputeBatchStats,
) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        if hit_capacity < ray_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Slice lengths and pointer validity are checked by helpers.
        let rays = unsafe { super::input_slice(rays, ray_count)? };
        // SAFETY: The capacity was checked immediately above.
        let output = unsafe { output_slice(output_hits, ray_count)? };
        let queries = convert_rays(rays)?;
        let session = get_session(handle)?;
        let (visibility, stats) = session.trace_any(&queries).map_err(vayu_status)?;
        for (destination, hit) in output.iter_mut().zip(visibility) {
            *destination = u8::from(hit);
        }
        // SAFETY: The caller-owned output was checked for null.
        unsafe { output_stats.write(batch_stats(&stats)) };
        Ok(())
    })
}

/// Reads cumulative production telemetry without resetting the session counters.
///
/// # Safety
///
/// `output_stats` references one writable [`XvComputeRuntimeStats`] value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_compute_get_runtime_stats(
    handle: u64,
    output_stats: *mut XvComputeRuntimeStats,
) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let session = get_session(handle)?;
        let stats = runtime_stats(&session.runtime_stats());
        // SAFETY: The caller-owned output was checked for null.
        unsafe { output_stats.write(stats) };
        Ok(())
    })
}

/// Releases one compute-session handle.
#[unsafe(no_mangle)]
pub extern "C" fn xv_compute_release(handle: u64) -> i32 {
    ffi_status(|| {
        write_registry()
            .remove(&handle)
            .map_or(Err(XvStatus::InvalidHandle), |_| Ok(()))
    })
}

fn convert_options(native: XvComputeOptions) -> Result<ComputeOptions, XvStatus> {
    if native.structure_size
        != u32::try_from(mem::size_of::<XvComputeOptions>()).expect("ABI size fits u32")
        || native.flags & !COMPUTE_FLAG_SCENE_CACHE != 0
        || native.reserved != 0
    {
        return Err(XvStatus::InvalidArgument);
    }
    let preference = match native.preference {
        0 => ComputePreference::Auto,
        1 => ComputePreference::RequirePortableGpu,
        2 => ComputePreference::Cpu,
        _ => return Err(XvStatus::InvalidArgument),
    };
    let power_preference = match native.power_preference {
        0 => ComputePowerPreference::HighPerformance,
        1 => ComputePowerPreference::LowPower,
        _ => return Err(XvStatus::InvalidArgument),
    };
    Ok(ComputeOptions {
        preference,
        power_preference,
        adapter_name_filter: None,
        maximum_precision_error_meters: native.maximum_precision_error_meters,
        maximum_expanded_triangle_count: usize::try_from(native.maximum_expanded_triangle_count)
            .map_err(|_| XvStatus::InvalidArgument)?,
        maximum_rays_per_dispatch: usize::try_from(native.maximum_rays_per_dispatch)
            .map_err(|_| XvStatus::InvalidArgument)?,
        maximum_geometry_chunk_bytes: usize::try_from(native.maximum_geometry_chunk_bytes)
            .map_err(|_| XvStatus::InvalidArgument)?,
        maximum_gpu_memory_bytes: usize::try_from(native.maximum_gpu_memory_bytes)
            .map_err(|_| XvStatus::InvalidArgument)?,
        enable_scene_cache: native.flags & COMPUTE_FLAG_SCENE_CACHE != 0,
    })
}

fn create_session(
    scene_handle: u64,
    options: ComputeOptions,
) -> Result<(u64, XvComputeInfo), XvStatus> {
    let scene = get_ready_scene(scene_handle)?;
    let session = Arc::new(ComputeSession::create(scene, options).map_err(vayu_status)?);
    let handle = next_handle()?;
    let info = compute_info(&session);
    write_registry().insert(handle, session);
    Ok((handle, info))
}

fn compute_info(session: &ComputeSession) -> XvComputeInfo {
    let info = session.info();
    let adapter = info.adapter.as_ref();
    let mut flags = 0;
    if info.fallback_reason.is_some() {
        flags |= INFO_FLAG_CREATION_FALLBACK;
    }
    if adapter.is_some() {
        flags |= INFO_FLAG_HAS_ADAPTER;
    }
    if info.scene_cache_hit {
        flags |= INFO_FLAG_SCENE_CACHE_HIT;
    }
    if info.streamed_geometry {
        flags |= INFO_FLAG_STREAMED_GEOMETRY;
    }
    let mut output = XvComputeInfo {
        structure_size: u32::try_from(mem::size_of::<XvComputeInfo>()).expect("ABI size fits u32"),
        backend: backend_code(info.backend),
        flags,
        adapter_backend: adapter.map_or(0, |value| adapter_backend_code(&value.backend)),
        device_type: adapter.map_or(0, |value| device_type_code(&value.device_type)),
        vendor_id: adapter.map_or(0, |value| value.vendor_id),
        device_id: adapter.map_or(0, |value| value.device_id),
        geometry_chunk_count: u32::try_from(info.geometry_chunk_count).unwrap_or(u32::MAX),
        triangle_count: u64::try_from(info.triangle_count).unwrap_or(u64::MAX),
        node_count: u64::try_from(info.node_count).unwrap_or(u64::MAX),
        approximate_gpu_bytes: u64::try_from(info.approximate_gpu_bytes).unwrap_or(u64::MAX),
        initialization_microseconds: info.initialization_microseconds,
        scene_precision_error_meters: info.scene_precision_error_meters,
        adapter_name: [0; 128],
        message: [0; 256],
    };
    if let Some(adapter) = adapter {
        write_text(&mut output.adapter_name, &adapter.name);
    }
    if let Some(reason) = &info.fallback_reason {
        write_text(&mut output.message, reason);
    } else if let Some(adapter) = adapter {
        write_text(
            &mut output.message,
            &format!("VAYU portable GPU ready via {}", adapter.backend),
        );
    } else {
        write_text(&mut output.message, "Canonical f64 CPU backend selected");
    }
    output
}

fn batch_stats(stats: &ComputeBatchStats) -> XvComputeBatchStats {
    let mut flags = 0;
    if stats.used_runtime_fallback {
        flags |= BATCH_FLAG_RUNTIME_FALLBACK;
    }
    if stats.used_reusable_buffers {
        flags |= BATCH_FLAG_REUSABLE_BUFFERS;
    }
    XvComputeBatchStats {
        structure_size: u32::try_from(mem::size_of::<XvComputeBatchStats>())
            .expect("ABI size fits u32"),
        backend: backend_code(stats.backend),
        flags,
        reserved: 0,
        ray_count: u64::try_from(stats.ray_count).unwrap_or(u64::MAX),
        dispatch_count: u64::try_from(stats.dispatch_count).unwrap_or(u64::MAX),
        upload_microseconds: stats.upload_microseconds,
        execution_microseconds: stats.execution_microseconds,
        readback_microseconds: stats.readback_microseconds,
        precision_error_meters: stats.precision_error_meters,
    }
}

fn runtime_stats(stats: &ComputeRuntimeStats) -> XvComputeRuntimeStats {
    let mut output = XvComputeRuntimeStats {
        flags: if stats.device_healthy {
            RUNTIME_FLAG_DEVICE_HEALTHY
        } else {
            0
        },
        trace_count: stats.trace_count,
        ray_count: stats.ray_count,
        dispatch_count: stats.dispatch_count,
        reusable_buffer_batch_count: stats.reusable_buffer_batch_count,
        runtime_fallback_count: stats.runtime_fallback_count,
        scratch_generation_count: stats.scratch_generation_count,
        scratch_capacity_rays: stats.scratch_capacity_rays,
        scratch_gpu_bytes: stats.scratch_gpu_bytes,
        uploaded_bytes: stats.uploaded_bytes,
        readback_bytes: stats.readback_bytes,
        device_loss_count: stats.device_loss_count,
        execution_error_count: stats.execution_error_count,
        geometry_upload_count: stats.geometry_upload_count,
        geometry_uploaded_bytes: stats.geometry_uploaded_bytes,
        ..XvComputeRuntimeStats::default()
    };
    if let Some(error) = &stats.last_error {
        write_text(&mut output.last_error, error);
    }
    output
}

fn cache_stats(stats: &VayuCacheStats, config: &VayuCacheConfig) -> XvCacheStats {
    let mut output = XvCacheStats {
        structure_size: u32::try_from(mem::size_of::<XvCacheStats>()).expect("ABI size fits u32"),
        flags: 0,
        memory_hit_count: stats.memory_hit_count,
        disk_hit_count: stats.disk_hit_count,
        miss_count: stats.miss_count,
        write_count: stats.write_count,
        eviction_count: stats.eviction_count,
        corruption_count: stats.corruption_count,
        memory_bytes: stats.memory_bytes,
        disk_bytes: stats.disk_bytes,
        memory_entry_count: stats.memory_entry_count,
        disk_entry_count: stats.disk_entry_count,
        memory_budget_bytes: config.memory_budget_bytes,
        disk_budget_bytes: config.disk_budget_bytes,
        directory: [0; 256],
        last_event: [0; 256],
    };
    write_text(&mut output.directory, &config.directory.to_string_lossy());
    write_text(&mut output.last_event, &stats.last_event);
    output
}

fn adapter_info(adapter: &AdapterDescription) -> XvAdapterInfo {
    let mut output = XvAdapterInfo {
        backend: adapter_backend_code(&adapter.backend),
        device_type: device_type_code(&adapter.device_type),
        vendor_id: adapter.vendor_id,
        device_id: adapter.device_id,
        ..XvAdapterInfo::default()
    };
    write_text(&mut output.name, &adapter.name);
    let driver = if adapter.driver_info.is_empty() {
        adapter.driver.clone()
    } else {
        format!("{} {}", adapter.driver, adapter.driver_info)
    };
    write_text(&mut output.driver, &driver);
    output
}

fn write_text(destination: &mut [u8], value: &str) {
    destination.fill(0);
    let length = value.len().min(destination.len().saturating_sub(1));
    destination[..length].copy_from_slice(&value.as_bytes()[..length]);
}

const fn backend_code(value: ActiveBackend) -> u32 {
    match value {
        ActiveBackend::Cpu => 0,
        ActiveBackend::PortableGpu => 1,
    }
}

fn adapter_backend_code(value: &str) -> u32 {
    match value {
        "Vulkan" => 1,
        "Metal" => 2,
        "Dx12" => 3,
        "Gl" => 4,
        "BrowserWebGpu" => 5,
        _ => 0,
    }
}

fn device_type_code(value: &str) -> u32 {
    match value {
        "IntegratedGpu" => 1,
        "DiscreteGpu" => 2,
        "VirtualGpu" => 3,
        "Cpu" => 4,
        _ => 0,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn vayu_status(error: VayuError) -> XvStatus {
    match error {
        VayuError::InvalidOptions(_)
        | VayuError::PortableScene(_)
        | VayuError::RayPrecisionExceeded { .. } => XvStatus::InvalidArgument,
        VayuError::AdapterUnavailable(_)
        | VayuError::DeviceRequest(_)
        | VayuError::Execution(_)
        | VayuError::Cache(_) => XvStatus::InvalidState,
    }
}

fn registry() -> &'static RwLock<HashMap<u64, Arc<ComputeSession>>> {
    COMPUTE_SESSIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn write_registry() -> std::sync::RwLockWriteGuard<'static, HashMap<u64, Arc<ComputeSession>>> {
    registry()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn get_session(handle: u64) -> Result<Arc<ComputeSession>, XvStatus> {
    registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .map(Arc::clone)
        .ok_or(XvStatus::InvalidHandle)
}

fn next_handle() -> Result<u64, XvStatus> {
    let handle = NEXT_COMPUTE_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 || handle == u64::MAX {
        return Err(XvStatus::InvalidState);
    }
    Ok(handle)
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{
        XvSceneOptions, XvSceneStats, xv_scene_add_instance, xv_scene_add_mesh, xv_scene_build,
        xv_scene_create, xv_scene_release,
    };
    use xvarna_scene::Transform;

    #[test]
    fn layouts_are_stable() {
        assert_eq!(mem::size_of::<XvComputeOptions>(), 64);
        assert_eq!(mem::size_of::<XvComputeInfo>(), 456);
        assert_eq!(mem::size_of::<XvComputeBatchStats>(), 64);
        assert_eq!(mem::size_of::<XvComputeRuntimeStats>(), 384);
        assert_eq!(mem::size_of::<XvCacheOptions>(), 24);
        assert_eq!(mem::size_of::<XvCacheStats>(), 616);
        assert_eq!(mem::size_of::<XvAdapterInfo>(), 224);
    }

    #[test]
    fn adapter_filtered_creation_rejects_invalid_utf8() {
        let options = XvComputeOptions::default();
        let invalid_utf8 = [0xff_u8];
        let mut handle = 0_u64;
        let mut info = XvComputeInfo::default();
        // SAFETY: All buffers are live; the deliberately invalid byte is bounded.
        let status = unsafe {
            xv_compute_create_for_adapter(
                0,
                &raw const options,
                invalid_utf8.as_ptr(),
                invalid_utf8.len(),
                &raw mut handle,
                &raw mut info,
            )
        };
        assert_eq!(status, XvStatus::InvalidArgument as i32);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn explicit_cpu_session_crosses_the_complete_abi() {
        let scene_options = XvSceneOptions::default();
        let mut scene_handle = 0_u64;
        // SAFETY: Every pointer references a correctly sized live value.
        assert_eq!(
            unsafe { xv_scene_create(&raw const scene_options, &raw mut scene_handle) },
            0
        );
        let positions = [0.0_f64, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let triangles = [0_u32, 1, 2];
        let mut mesh_id = 0_u64;
        // SAFETY: Input and output buffers are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    scene_handle,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut mesh_id,
                )
            },
            0
        );
        let transform = Transform::IDENTITY.to_row_major();
        // SAFETY: Matrix pointer references sixteen values.
        assert_eq!(
            unsafe { xv_scene_add_instance(scene_handle, mesh_id, transform.as_ptr(), 11, 12, 1) },
            0
        );
        let mut scene_stats = XvSceneStats {
            structure_size: 0,
            thread_count: 0,
            mesh_resource_count: 0,
            instance_count: 0,
            unique_triangle_count: 0,
            instanced_triangle_count: 0,
            blas_node_count: 0,
            tlas_node_count: 0,
            maximum_bvh_depth: 0,
            approximate_memory_bytes: 0,
            build_time_microseconds: 0,
            rebase_origin_x: 0.0,
            rebase_origin_y: 0.0,
            rebase_origin_z: 0.0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        };
        // SAFETY: Output is writable.
        assert_eq!(
            unsafe { xv_scene_build(scene_handle, &raw mut scene_stats) },
            0
        );
        let options = XvComputeOptions {
            preference: 2,
            ..XvComputeOptions::default()
        };
        let mut compute_handle = 0_u64;
        let mut info = XvComputeInfo::default();
        // SAFETY: Every pointer references a correctly sized live value.
        assert_eq!(
            unsafe {
                xv_compute_create(
                    scene_handle,
                    &raw const options,
                    &raw mut compute_handle,
                    &raw mut info,
                )
            },
            0
        );
        assert_eq!(info.backend, 0);
        let rays = [XvRay {
            origin_x: 0.25,
            origin_y: 0.25,
            origin_z: 1.0,
            direction_x: 0.0,
            direction_y: 0.0,
            direction_z: -1.0,
            t_min: 0.0,
            t_max: 2.0,
            category_mask: 1,
        }];
        let mut hits = [XvHit {
            structure_size: 0,
            flags: 0,
            distance: 0.0,
            barycentric_u: 0.0,
            barycentric_v: 0.0,
            object_id: 0,
            instance_id: 0,
            mesh_id: 0,
            triangle_id: 0,
            reserved: 0,
        }];
        let mut batch_stats = XvComputeBatchStats::default();
        // SAFETY: Input/output arrays and stats are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_compute_trace_closest(
                    compute_handle,
                    rays.as_ptr(),
                    1,
                    hits.as_mut_ptr(),
                    1,
                    &raw mut batch_stats,
                )
            },
            0
        );
        assert_eq!(hits[0].object_id, 11);
        assert_eq!(batch_stats.backend, 0);
        let mut runtime_stats = XvComputeRuntimeStats::default();
        // SAFETY: Output is live and correctly sized.
        assert_eq!(
            unsafe { xv_compute_get_runtime_stats(compute_handle, &raw mut runtime_stats) },
            0
        );
        assert_eq!(runtime_stats.trace_count, 1);
        assert_eq!(runtime_stats.ray_count, 1);
        assert_eq!(runtime_stats.dispatch_count, 0);
        assert_ne!(runtime_stats.flags & RUNTIME_FLAG_DEVICE_HEALTHY, 0);
        assert_eq!(xv_compute_release(compute_handle), 0);
        assert_eq!(xv_scene_release(scene_handle), 0);
    }

    #[test]
    fn text_copy_always_terminates() {
        let mut destination = [9_u8; 4];
        write_text(&mut destination, "abcdefgh");
        assert_eq!(destination, [b'a', b'b', b'c', 0]);
        let null = core::ptr::null_mut::<XvAdapterInfo>();
        let mut count = 0_usize;
        // SAFETY: Null output with zero capacity is the documented count-query form.
        let status = unsafe { xv_compute_enumerate_adapters(null, 0, &raw mut count) };
        assert_eq!(status, 0);
    }
}
