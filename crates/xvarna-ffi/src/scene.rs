//! Handle-based scene compilation and ray-query ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use xvarna_geometry::{Mesh, Vec3};
use xvarna_scene::{
    HierarchyUpdateKind, Hit, QueryRay, Scene, SceneBuildOptions, SceneBuilder, SceneDelta,
    SceneError, SceneLayer, SceneStats, SceneUpdatePolicy, SceneUpdateReport, Transform,
};
use xvarna_types::{InstanceId, MeshId, ObjectId};

const HIT_FLAG_HIT: u32 = 1 << 0;
const HIT_FLAG_FRONT_FACE: u32 = 1 << 1;

static NEXT_SCENE_HANDLE: AtomicU64 = AtomicU64::new(1);
static SCENES: OnceLock<RwLock<HashMap<u64, SceneSlot>>> = OnceLock::new();

enum SceneSlot {
    Building(SceneBuilder),
    Ready(Arc<Scene>),
}

/// Fixed-layout options for a native scene snapshot.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSceneOptions {
    /// Byte size of this structure for layout diagnostics.
    pub structure_size: u32,
    /// Maximum primitives per BLAS/TLAS leaf, from 1 through 64.
    pub maximum_leaf_size: u32,
    /// Number of worker threads, or zero for the engine default.
    pub thread_count: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Source model-unit scale in metres.
    pub unit_scale_to_meters: f64,
    /// Absolute canonical tolerance in metres.
    pub absolute_tolerance_meters: f64,
}

impl Default for XvSceneOptions {
    fn default() -> Self {
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>())
                .expect("ABI option size fits u32"),
            maximum_leaf_size: 4,
            thread_count: 0,
            reserved: 0,
            unit_scale_to_meters: 1.0,
            absolute_tolerance_meters: 1.0e-6,
        }
    }
}

/// Fixed-layout normalized ray in canonical metre coordinates.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvRay {
    /// Origin X coordinate.
    pub origin_x: f64,
    /// Origin Y coordinate.
    pub origin_y: f64,
    /// Origin Z coordinate.
    pub origin_z: f64,
    /// Direction X coordinate.
    pub direction_x: f64,
    /// Direction Y coordinate.
    pub direction_y: f64,
    /// Direction Z coordinate.
    pub direction_z: f64,
    /// Inclusive minimum distance.
    pub t_min: f64,
    /// Inclusive maximum distance.
    pub t_max: f64,
    /// Instance-category filter; zero means all categories.
    pub category_mask: u64,
}

/// Fixed-layout closest-hit result.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvHit {
    /// Byte size of this structure for layout diagnostics.
    pub structure_size: u32,
    /// Hit and front-face flags.
    pub flags: u32,
    /// Canonical hit distance in metres, or infinity for a miss.
    pub distance: f64,
    /// First barycentric coordinate.
    pub barycentric_u: f64,
    /// Second barycentric coordinate.
    pub barycentric_v: f64,
    /// Stable source-object identifier.
    pub object_id: u64,
    /// Stable occurrence identifier.
    pub instance_id: u64,
    /// Shared mesh-resource identifier.
    pub mesh_id: u64,
    /// Local triangle index, or `UINT32_MAX` for a miss.
    pub triangle_id: u32,
    /// Reserved; always zero.
    pub reserved: u32,
}

/// Fixed-layout build and memory statistics.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSceneStats {
    /// Byte size of this structure for layout diagnostics.
    pub structure_size: u32,
    /// Worker threads assigned to the snapshot.
    pub thread_count: u32,
    /// Unique mesh resources after content deduplication.
    pub mesh_resource_count: u64,
    /// Geometry instance count.
    pub instance_count: u64,
    /// Unique stored triangle count.
    pub unique_triangle_count: u64,
    /// Effective instanced triangle count.
    pub instanced_triangle_count: u64,
    /// Total bottom-level BVH nodes.
    pub blas_node_count: u64,
    /// Top-level BVH node count.
    pub tlas_node_count: u64,
    /// Maximum depth across every acceleration structure.
    pub maximum_bvh_depth: u64,
    /// Approximate owned CPU bytes.
    pub approximate_memory_bytes: u64,
    /// Total scene compilation time in microseconds.
    pub build_time_microseconds: u64,
    /// Internal world-origin rebase X in metres.
    pub rebase_origin_x: f64,
    /// Internal world-origin rebase Y in metres.
    pub rebase_origin_y: f64,
    /// Internal world-origin rebase Z in metres.
    pub rebase_origin_z: f64,
    /// First little-endian word of the 256-bit scene hash.
    pub content_hash_0: u64,
    /// Second little-endian word of the 256-bit scene hash.
    pub content_hash_1: u64,
    /// Third little-endian word of the 256-bit scene hash.
    pub content_hash_2: u64,
    /// Fourth little-endian word of the 256-bit scene hash.
    pub content_hash_3: u64,
}

/// One fixed-layout instance lifecycle operation.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSceneInstanceDelta {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Zero update, one add, two remove.
    pub operation: u32,
    /// Update flags: transform, object, category, layer in bits zero through three.
    pub flags: u32,
    /// Zero static or one dynamic.
    pub layer: u32,
    /// Stable occurrence identifier.
    pub instance_id: u64,
    /// Existing mesh identifier for add.
    pub mesh_id: u64,
    /// Stable object identifier.
    pub object_id: u64,
    /// Category bits; zero means all.
    pub category_mask: u64,
    /// Source-model affine row-major matrix.
    pub transform: [f64; 16],
}

/// Refit/rebuild decision policy for a scene delta transaction.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSceneUpdateOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Maximum consecutive refits before rebuild.
    pub maximum_consecutive_refits: u32,
    /// Maximum accepted new/old hierarchy cost.
    pub maximum_refit_quality_ratio: f64,
}

impl Default for XvSceneUpdateOptions {
    fn default() -> Self {
        let policy = SceneUpdatePolicy::default();
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>()).expect("ABI size fits u32"),
            maximum_consecutive_refits: policy.maximum_consecutive_refits,
            maximum_refit_quality_ratio: policy.maximum_refit_quality_ratio,
        }
    }
}

/// Fixed-layout incremental lifecycle provenance.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSceneUpdateReport {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Static action in low two bits, dynamic action in bits two and three.
    pub flags: u32,
    /// Requested deltas.
    pub delta_count: u64,
    /// Static changes.
    pub static_change_count: u64,
    /// Dynamic changes.
    pub dynamic_change_count: u64,
    /// Reused BLAS resources.
    pub reused_blas_count: u64,
    /// Rebuilt BLAS resources.
    pub rebuilt_blas_count: u64,
    /// Transition duration.
    pub update_time_microseconds: u64,
    /// Worst refit quality ratio.
    pub maximum_refit_quality_ratio: f64,
    /// Previous scene hash words.
    pub previous_hash: [u64; 4],
    /// Current scene hash words.
    pub current_hash: [u64; 4],
}

/// Creates a mutable scene builder and returns its opaque handle.
///
/// # Safety
///
/// `options` must point to a readable [`XvSceneOptions`] and `output_handle`
/// must point to writable `u64` storage. Both remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_create(
    options: *const XvSceneOptions,
    output_handle: *mut u64,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_handle.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Non-null caller-owned pointers are required by the public contract.
        let options = unsafe { options.read() };
        if options.structure_size
            != u32::try_from(mem::size_of::<XvSceneOptions>()).expect("ABI option size fits u32")
            || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let builder = SceneBuilder::new(SceneBuildOptions {
            unit_scale_to_meters: options.unit_scale_to_meters,
            absolute_tolerance_meters: options.absolute_tolerance_meters,
            maximum_leaf_size: usize::try_from(options.maximum_leaf_size)
                .map_err(|_| XvStatus::InvalidArgument)?,
            thread_count: usize::try_from(options.thread_count)
                .map_err(|_| XvStatus::InvalidArgument)?,
        })
        .map_err(scene_error_status)?;
        let handle = next_handle()?;
        write_registry().insert(handle, SceneSlot::Building(builder));
        // SAFETY: The output pointer was checked and is caller-owned writable memory.
        unsafe { output_handle.write(handle) };
        Ok(())
    })
}

/// Adds or deduplicates a triangle mesh in a building scene.
///
/// # Safety
///
/// `positions` references `vertex_count * 3` readable `f64` values,
/// `triangles` references `face_count * 3` readable `u32` values, and
/// `output_mesh_id` references writable `u64` storage for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_add_mesh(
    handle: u64,
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    face_count: usize,
    output_mesh_id: *mut u64,
) -> i32 {
    ffi_status(|| {
        if output_mesh_id.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let position_count = vertex_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        let triangle_count = face_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        // SAFETY: Length arithmetic and nullability are validated by `input_slice`.
        let positions = unsafe { input_slice(positions, position_count)? };
        // SAFETY: Length arithmetic and nullability are validated by `input_slice`.
        let triangles = unsafe { input_slice(triangles, triangle_count)? };
        let mesh = Mesh {
            positions: positions
                .chunks_exact(3)
                .map(|value| Vec3::new(value[0], value[1], value[2]))
                .collect(),
            triangles: triangles
                .chunks_exact(3)
                .map(|value| [value[0], value[1], value[2]])
                .collect(),
        };
        let mesh_id = {
            let mut registry = write_registry();
            let result = {
                let slot = registry.get_mut(&handle).ok_or(XvStatus::InvalidHandle)?;
                let SceneSlot::Building(builder) = slot else {
                    return Err(XvStatus::InvalidState);
                };
                builder.add_mesh(mesh).map_err(scene_error_status)
            };
            drop(registry);
            result?
        };
        // SAFETY: The output pointer was checked and is caller-owned writable memory.
        unsafe { output_mesh_id.write(mesh_id.get()) };
        Ok(())
    })
}

/// Adds a transformed mesh occurrence to a building scene.
///
/// # Safety
///
/// `row_major_transform` must reference exactly 16 readable `f64` values for
/// the duration of this call.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_scene_add_instance(
    handle: u64,
    mesh_id: u64,
    row_major_transform: *const f64,
    object_id: u64,
    instance_id: u64,
    category_mask: u64,
) -> i32 {
    ffi_status(|| {
        // SAFETY: The fixed matrix length and pointer are validated by `input_slice`.
        let values = unsafe { input_slice(row_major_transform, 16)? };
        let transform = Transform::try_from_row_major(
            values
                .try_into()
                .expect("validated transform has sixteen values"),
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        {
            let mut registry = write_registry();
            let result = {
                let slot = registry.get_mut(&handle).ok_or(XvStatus::InvalidHandle)?;
                let SceneSlot::Building(builder) = slot else {
                    return Err(XvStatus::InvalidState);
                };
                builder
                    .add_instance(
                        MeshId::new(mesh_id),
                        transform,
                        ObjectId::new(object_id),
                        InstanceId::new(instance_id),
                        category_mask,
                    )
                    .map_err(scene_error_status)
            };
            drop(registry);
            result?;
        };
        Ok(())
    })
}

/// Adds a transformed occurrence to an explicit static or dynamic layer.
///
/// # Safety
///
/// `row_major_transform` references sixteen readable `f64` values for this call.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_scene_add_instance_layered(
    handle: u64,
    mesh_id: u64,
    row_major_transform: *const f64,
    object_id: u64,
    instance_id: u64,
    category_mask: u64,
    layer: u32,
) -> i32 {
    ffi_status(|| {
        // SAFETY: The fixed matrix length and pointer are validated by `input_slice`.
        let values = unsafe { input_slice(row_major_transform, 16)? };
        let transform = Transform::try_from_row_major(
            values
                .try_into()
                .expect("validated transform has sixteen values"),
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        let layer = scene_layer(layer)?;
        let mut registry = write_registry();
        let slot = registry.get_mut(&handle).ok_or(XvStatus::InvalidHandle)?;
        let SceneSlot::Building(builder) = slot else {
            return Err(XvStatus::InvalidState);
        };
        let result = builder.add_instance_in_layer(
            MeshId::new(mesh_id),
            transform,
            ObjectId::new(object_id),
            InstanceId::new(instance_id),
            category_mask,
            layer,
        );
        drop(registry);
        result.map_err(scene_error_status)
    })
}

/// Compiles a building scene into an immutable, concurrently queryable snapshot.
///
/// # Safety
///
/// `output_stats` must reference writable [`XvSceneStats`] storage for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_build(handle: u64, output_stats: *mut XvSceneStats) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let builder = take_builder(handle)?;
        let scene = Arc::new(builder.build().map_err(scene_error_status)?);
        let stats = stats_from_scene(scene.stats());
        write_registry().insert(handle, SceneSlot::Ready(scene));
        // SAFETY: The output pointer was checked and is caller-owned writable memory.
        unsafe { output_stats.write(stats) };
        Ok(())
    })
}

/// Reads current snapshot statistics after any number of lifecycle transactions.
///
/// # Safety
///
/// `output_stats` references writable [`XvSceneStats`] storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_get_stats(handle: u64, output_stats: *mut XvSceneStats) -> i32 {
    ffi_status(|| {
        if output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let scene = get_ready_scene(handle)?;
        // SAFETY: Caller-owned output was checked for null.
        unsafe { output_stats.write(stats_from_scene(scene.stats())) };
        Ok(())
    })
}

/// Applies an ordered atomic instance-delta transaction and publishes a new snapshot.
///
/// Existing compute sessions safely retain their previous immutable scene.
///
/// # Safety
///
/// `deltas` references `delta_count` readable records; options and both outputs
/// reference one readable/writable record for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_apply_instance_deltas(
    handle: u64,
    deltas: *const XvSceneInstanceDelta,
    delta_count: usize,
    options: *const XvSceneUpdateOptions,
    output_report: *mut XvSceneUpdateReport,
    output_stats: *mut XvSceneStats,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_report.is_null() || output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Pointer and count follow the public slice contract.
        let deltas = unsafe { input_slice(deltas, delta_count)? };
        // SAFETY: Required readable pointer was checked above.
        let options = unsafe { options.read() };
        if options.structure_size
            != u32::try_from(mem::size_of::<XvSceneUpdateOptions>()).expect("ABI size fits u32")
        {
            return Err(XvStatus::InvalidArgument);
        }
        let converted = deltas
            .iter()
            .map(convert_instance_delta)
            .collect::<Result<Vec<_>, _>>()?;
        let previous = get_ready_scene(handle)?;
        let previous_hash = previous.stats().content_hash;
        let (updated, report) = previous
            .apply_deltas(
                &converted,
                SceneUpdatePolicy {
                    maximum_refit_quality_ratio: options.maximum_refit_quality_ratio,
                    maximum_consecutive_refits: options.maximum_consecutive_refits,
                },
            )
            .map_err(scene_error_status)?;
        let updated = Arc::new(updated);
        {
            let mut registry = write_registry();
            let Some(SceneSlot::Ready(current)) = registry.get(&handle) else {
                return Err(XvStatus::InvalidState);
            };
            if current.stats().content_hash != previous_hash {
                return Err(XvStatus::InvalidState);
            }
            registry.insert(handle, SceneSlot::Ready(Arc::clone(&updated)));
        }
        // SAFETY: Both caller-owned outputs were checked for null.
        unsafe {
            output_report.write(update_report(report));
            output_stats.write(stats_from_scene(updated.stats()));
        }
        Ok(())
    })
}

/// Replaces one shared mesh, rebuilding only its BLAS before refitting/rebuilding affected TLASes.
///
/// # Safety
///
/// Position/triangle arrays and all fixed-layout pointers follow the same contracts
/// as `xv_scene_add_mesh` and `xv_scene_apply_instance_deltas`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_scene_replace_mesh(
    handle: u64,
    mesh_id: u64,
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    face_count: usize,
    options: *const XvSceneUpdateOptions,
    output_report: *mut XvSceneUpdateReport,
    output_stats: *mut XvSceneStats,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_report.is_null() || output_stats.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let position_count = vertex_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        let triangle_count = face_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        // SAFETY: Length arithmetic and pointer contracts were validated.
        let positions = unsafe { input_slice(positions, position_count)? };
        // SAFETY: Length arithmetic and pointer contracts were validated.
        let triangles = unsafe { input_slice(triangles, triangle_count)? };
        // SAFETY: Required readable pointer was checked above.
        let policy = unsafe { options.read() };
        if policy.structure_size
            != u32::try_from(mem::size_of::<XvSceneUpdateOptions>()).expect("ABI size fits u32")
        {
            return Err(XvStatus::InvalidArgument);
        }
        let mesh = Mesh {
            positions: positions
                .chunks_exact(3)
                .map(|value| Vec3::new(value[0], value[1], value[2]))
                .collect(),
            triangles: triangles
                .chunks_exact(3)
                .map(|value| [value[0], value[1], value[2]])
                .collect(),
        };
        let previous = get_ready_scene(handle)?;
        let previous_hash = previous.stats().content_hash;
        let (updated, report) = previous
            .apply_deltas(
                &[SceneDelta::ReplaceMesh {
                    mesh_id: MeshId::new(mesh_id),
                    mesh,
                }],
                SceneUpdatePolicy {
                    maximum_refit_quality_ratio: policy.maximum_refit_quality_ratio,
                    maximum_consecutive_refits: policy.maximum_consecutive_refits,
                },
            )
            .map_err(scene_error_status)?;
        let updated = Arc::new(updated);
        {
            let mut registry = write_registry();
            let Some(SceneSlot::Ready(current)) = registry.get(&handle) else {
                return Err(XvStatus::InvalidState);
            };
            if current.stats().content_hash != previous_hash {
                return Err(XvStatus::InvalidState);
            }
            registry.insert(handle, SceneSlot::Ready(Arc::clone(&updated)));
        }
        // SAFETY: Both caller-owned outputs were checked for null.
        unsafe {
            output_report.write(update_report(report));
            output_stats.write(stats_from_scene(updated.stats()));
        }
        Ok(())
    })
}

/// Traces an ordered closest-hit ray batch against a compiled scene.
///
/// # Safety
///
/// `rays` references `ray_count` readable [`XvRay`] values and `output_hits`
/// references `ray_count` writable [`XvHit`] values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_trace_closest(
    handle: u64,
    rays: *const XvRay,
    ray_count: usize,
    output_hits: *mut XvHit,
    hit_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if hit_capacity < ray_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Pointer and slice lengths are validated by the slice helpers.
        let rays = unsafe { input_slice(rays, ray_count)? };
        // SAFETY: Pointer, capacity, and slice length are validated above and by the helper.
        let output = unsafe { output_slice(output_hits, ray_count)? };
        let scene = get_ready_scene(handle)?;
        let queries = convert_rays(rays)?;
        for (destination, hit) in output.iter_mut().zip(scene.trace_closest_batch(&queries)) {
            *destination = hit_from_scene(hit);
        }
        Ok(())
    })
}

/// Traces an ordered visibility/any-hit ray batch against a compiled scene.
///
/// # Safety
///
/// `rays` references `ray_count` readable [`XvRay`] values and `output_hits`
/// references `ray_count` writable bytes for this call. Each output is zero or one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_scene_trace_any(
    handle: u64,
    rays: *const XvRay,
    ray_count: usize,
    output_hits: *mut u8,
    hit_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if hit_capacity < ray_count {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Pointer and slice lengths are validated by the slice helpers.
        let rays = unsafe { input_slice(rays, ray_count)? };
        // SAFETY: Pointer, capacity, and slice length are validated above and by the helper.
        let output = unsafe { output_slice(output_hits, ray_count)? };
        let scene = get_ready_scene(handle)?;
        let queries = convert_rays(rays)?;
        for (destination, hit) in output.iter_mut().zip(scene.trace_any_batch(&queries)) {
            *destination = u8::from(hit);
        }
        Ok(())
    })
}

/// Releases a scene builder or compiled scene handle.
#[unsafe(no_mangle)]
pub extern "C" fn xv_scene_release(handle: u64) -> i32 {
    ffi_status(|| {
        write_registry()
            .remove(&handle)
            .map_or(Err(XvStatus::InvalidHandle), |_| Ok(()))
    })
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

fn registry() -> &'static RwLock<HashMap<u64, SceneSlot>> {
    SCENES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn write_registry() -> std::sync::RwLockWriteGuard<'static, HashMap<u64, SceneSlot>> {
    registry()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn next_handle() -> Result<u64, XvStatus> {
    let handle = NEXT_SCENE_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 || handle == u64::MAX {
        return Err(XvStatus::InvalidState);
    }
    Ok(handle)
}

pub fn get_ready_scene(handle: u64) -> Result<Arc<Scene>, XvStatus> {
    let registry = registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match registry.get(&handle) {
        Some(SceneSlot::Ready(scene)) => Ok(Arc::clone(scene)),
        Some(SceneSlot::Building(_)) => Err(XvStatus::InvalidState),
        None => Err(XvStatus::InvalidHandle),
    }
}

pub fn convert_rays(rays: &[XvRay]) -> Result<Vec<QueryRay>, XvStatus> {
    rays.iter()
        .map(|ray| {
            QueryRay::try_new(
                Vec3::new(ray.origin_x, ray.origin_y, ray.origin_z),
                Vec3::new(ray.direction_x, ray.direction_y, ray.direction_z),
                ray.t_min,
                ray.t_max,
                ray.category_mask,
            )
            .map_err(scene_error_status)
        })
        .collect()
}

fn take_builder(handle: u64) -> Result<SceneBuilder, XvStatus> {
    let mut registry = write_registry();
    match registry.get(&handle) {
        Some(SceneSlot::Ready(_)) => return Err(XvStatus::InvalidState),
        None => return Err(XvStatus::InvalidHandle),
        Some(SceneSlot::Building(_)) => {}
    }
    let Some(SceneSlot::Building(builder)) = registry.remove(&handle) else {
        unreachable!("scene state was validated while holding the write lock");
    };
    drop(registry);
    Ok(builder)
}

const fn scene_layer(value: u32) -> Result<SceneLayer, XvStatus> {
    match value {
        0 => Ok(SceneLayer::Static),
        1 => Ok(SceneLayer::Dynamic),
        _ => Err(XvStatus::InvalidArgument),
    }
}

fn convert_instance_delta(value: &XvSceneInstanceDelta) -> Result<SceneDelta, XvStatus> {
    const UPDATE_TRANSFORM: u32 = 1 << 0;
    const UPDATE_OBJECT: u32 = 1 << 1;
    const UPDATE_CATEGORY: u32 = 1 << 2;
    const UPDATE_LAYER: u32 = 1 << 3;
    const ALL_UPDATE_FLAGS: u32 = UPDATE_TRANSFORM | UPDATE_OBJECT | UPDATE_CATEGORY | UPDATE_LAYER;
    if value.structure_size
        != u32::try_from(mem::size_of::<XvSceneInstanceDelta>()).expect("ABI size fits u32")
        || value.flags & !ALL_UPDATE_FLAGS != 0
    {
        return Err(XvStatus::InvalidArgument);
    }
    let transform =
        || Transform::try_from_row_major(value.transform).map_err(|_| XvStatus::InvalidArgument);
    match value.operation {
        0 => Ok(SceneDelta::UpdateInstance {
            instance_id: InstanceId::new(value.instance_id),
            transform: (value.flags & UPDATE_TRANSFORM != 0)
                .then(transform)
                .transpose()?,
            object_id: (value.flags & UPDATE_OBJECT != 0).then_some(ObjectId::new(value.object_id)),
            category_mask: (value.flags & UPDATE_CATEGORY != 0).then_some(value.category_mask),
            layer: (value.flags & UPDATE_LAYER != 0)
                .then(|| scene_layer(value.layer))
                .transpose()?,
        }),
        1 if value.flags == 0 => Ok(SceneDelta::AddInstance {
            mesh_id: MeshId::new(value.mesh_id),
            transform: transform()?,
            object_id: ObjectId::new(value.object_id),
            instance_id: InstanceId::new(value.instance_id),
            category_mask: value.category_mask,
            layer: scene_layer(value.layer)?,
        }),
        2 if value.flags == 0 => Ok(SceneDelta::RemoveInstance {
            instance_id: InstanceId::new(value.instance_id),
        }),
        _ => Err(XvStatus::InvalidArgument),
    }
}

const fn hierarchy_code(value: HierarchyUpdateKind) -> u32 {
    value as u32
}

fn update_report(report: SceneUpdateReport) -> XvSceneUpdateReport {
    XvSceneUpdateReport {
        structure_size: u32::try_from(mem::size_of::<XvSceneUpdateReport>())
            .expect("ABI update report size fits u32"),
        flags: hierarchy_code(report.static_tlas_update)
            | (hierarchy_code(report.dynamic_tlas_update) << 2),
        delta_count: u64::try_from(report.delta_count).unwrap_or(u64::MAX),
        static_change_count: u64::try_from(report.static_change_count).unwrap_or(u64::MAX),
        dynamic_change_count: u64::try_from(report.dynamic_change_count).unwrap_or(u64::MAX),
        reused_blas_count: u64::try_from(report.reused_blas_count).unwrap_or(u64::MAX),
        rebuilt_blas_count: u64::try_from(report.rebuilt_blas_count).unwrap_or(u64::MAX),
        update_time_microseconds: report.update_time_microseconds,
        maximum_refit_quality_ratio: report.maximum_refit_quality_ratio,
        previous_hash: [
            hash_word(&report.previous_hash, 0),
            hash_word(&report.previous_hash, 1),
            hash_word(&report.previous_hash, 2),
            hash_word(&report.previous_hash, 3),
        ],
        current_hash: [
            hash_word(&report.current_hash, 0),
            hash_word(&report.current_hash, 1),
            hash_word(&report.current_hash, 2),
            hash_word(&report.current_hash, 3),
        ],
    }
}

#[allow(clippy::needless_pass_by_value)]
fn scene_error_status(error: SceneError) -> XvStatus {
    match error {
        SceneError::ThreadPool(_) => XvStatus::InvalidState,
        SceneError::UnknownMesh(_)
        | SceneError::InvalidUnitScale
        | SceneError::InvalidTolerance
        | SceneError::InvalidLeafSize
        | SceneError::MeshRequiresRepair { .. }
        | SceneError::EmptyResources
        | SceneError::EmptyInstances
        | SceneError::InvalidRay
        | SceneError::InvalidTransform
        | SceneError::DuplicateInstance(_)
        | SceneError::UnknownInstance(_)
        | SceneError::InvalidUpdatePolicy => XvStatus::InvalidArgument,
    }
}

pub fn hit_from_scene(hit: Hit) -> XvHit {
    let mut flags = 0;
    if hit.hit {
        flags |= HIT_FLAG_HIT;
    }
    if hit.front_face {
        flags |= HIT_FLAG_FRONT_FACE;
    }
    XvHit {
        structure_size: u32::try_from(mem::size_of::<XvHit>()).expect("ABI hit size fits u32"),
        flags,
        distance: hit.distance,
        barycentric_u: hit.barycentric_u,
        barycentric_v: hit.barycentric_v,
        object_id: hit.object_id.get(),
        instance_id: hit.instance_id.get(),
        mesh_id: hit.mesh_id.get(),
        triangle_id: hit.triangle_id,
        reserved: 0,
    }
}

fn stats_from_scene(stats: SceneStats) -> XvSceneStats {
    XvSceneStats {
        structure_size: u32::try_from(mem::size_of::<XvSceneStats>())
            .expect("ABI scene statistics size fits u32"),
        thread_count: u32::try_from(stats.thread_count).unwrap_or(u32::MAX),
        mesh_resource_count: u64::try_from(stats.mesh_resource_count).unwrap_or(u64::MAX),
        instance_count: u64::try_from(stats.instance_count).unwrap_or(u64::MAX),
        unique_triangle_count: u64::try_from(stats.unique_triangle_count).unwrap_or(u64::MAX),
        instanced_triangle_count: u64::try_from(stats.instanced_triangle_count).unwrap_or(u64::MAX),
        blas_node_count: u64::try_from(stats.blas_node_count).unwrap_or(u64::MAX),
        tlas_node_count: u64::try_from(stats.tlas_node_count).unwrap_or(u64::MAX),
        maximum_bvh_depth: u64::try_from(stats.maximum_bvh_depth).unwrap_or(u64::MAX),
        approximate_memory_bytes: u64::try_from(stats.approximate_memory_bytes).unwrap_or(u64::MAX),
        build_time_microseconds: stats.build_time_microseconds,
        rebase_origin_x: stats.rebase_origin_meters.x,
        rebase_origin_y: stats.rebase_origin_meters.y,
        rebase_origin_z: stats.rebase_origin_meters.z,
        content_hash_0: hash_word(&stats.content_hash, 0),
        content_hash_1: hash_word(&stats.content_hash, 1),
        content_hash_2: hash_word(&stats.content_hash, 2),
        content_hash_3: hash_word(&stats.content_hash, 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> ([f64; 9], [u32; 3]) {
        ([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], [0, 1, 2])
    }

    #[test]
    fn layouts_are_stable() {
        assert_eq!(mem::size_of::<XvSceneOptions>(), 32);
        assert_eq!(mem::size_of::<XvRay>(), 72);
        assert_eq!(mem::size_of::<XvHit>(), 64);
        assert_eq!(mem::size_of::<XvSceneStats>(), 136);
        assert_eq!(mem::size_of::<XvSceneInstanceDelta>(), 176);
        assert_eq!(mem::size_of::<XvSceneUpdateOptions>(), 16);
        assert_eq!(mem::size_of::<XvSceneUpdateReport>(), 128);
    }

    #[test]
    fn scene_lifecycle_deduplicates_and_traces() {
        let options = XvSceneOptions::default();
        let mut handle = 0;
        // SAFETY: All pointers reference correctly sized live test values.
        assert_eq!(
            unsafe { xv_scene_create(&raw const options, &raw mut handle) },
            0
        );
        let (positions, triangles) = triangle();
        let mut first_mesh = 0;
        let mut second_mesh = 0;
        // SAFETY: Buffers and outputs are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    handle,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut first_mesh,
                )
            },
            0
        );
        // SAFETY: Buffers and outputs are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    handle,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut second_mesh,
                )
            },
            0
        );
        assert_eq!(first_mesh, second_mesh);
        let transform = Transform::IDENTITY.to_row_major();
        // SAFETY: The matrix contains sixteen readable values.
        assert_eq!(
            unsafe { xv_scene_add_instance(handle, first_mesh, transform.as_ptr(), 7, 9, 1) },
            0
        );
        let mut stats = XvSceneStats {
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
        // SAFETY: Statistics output is live and writable.
        assert_eq!(unsafe { xv_scene_build(handle, &raw mut stats) }, 0);
        assert_eq!(stats.mesh_resource_count, 1);
        let rays = [XvRay {
            origin_x: 0.25,
            origin_y: 0.25,
            origin_z: 1.0,
            direction_x: 0.0,
            direction_y: 0.0,
            direction_z: -1.0,
            t_min: 0.0,
            t_max: 10.0,
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
        // SAFETY: Input and output arrays are live and correctly sized.
        assert_eq!(
            unsafe { xv_scene_trace_closest(handle, rays.as_ptr(), 1, hits.as_mut_ptr(), 1) },
            0
        );
        assert_eq!(hits[0].flags & HIT_FLAG_HIT, HIT_FLAG_HIT);
        assert_eq!(hits[0].object_id, 7);
        assert_eq!(hits[0].instance_id, 9);
        assert!((hits[0].distance - 1.0).abs() < 1.0e-12);
        assert_eq!(xv_scene_release(handle), 0);
        assert_eq!(xv_scene_release(handle), XvStatus::InvalidHandle as i32);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn layered_delta_abi_refits_only_dynamic_tlas() {
        let options = XvSceneOptions::default();
        let mut handle = 0;
        // SAFETY: Test pointers are live and correctly sized.
        assert_eq!(
            unsafe { xv_scene_create(&raw const options, &raw mut handle) },
            0
        );
        let (positions, triangles) = triangle();
        let mut mesh_id = 0;
        // SAFETY: Test arrays and output are live.
        assert_eq!(
            unsafe {
                xv_scene_add_mesh(
                    handle,
                    positions.as_ptr(),
                    3,
                    triangles.as_ptr(),
                    1,
                    &raw mut mesh_id,
                )
            },
            0
        );
        let identity = Transform::IDENTITY.to_row_major();
        let dynamic = [
            1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        // SAFETY: Matrices are live for both calls.
        assert_eq!(
            unsafe {
                xv_scene_add_instance_layered(handle, mesh_id, identity.as_ptr(), 1, 1, 1, 0)
            },
            0
        );
        // SAFETY: Matrix is live for the call.
        assert_eq!(
            unsafe { xv_scene_add_instance_layered(handle, mesh_id, dynamic.as_ptr(), 2, 2, 2, 1) },
            0
        );
        let mut stats = XvSceneStats {
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
        assert_eq!(unsafe { xv_scene_build(handle, &raw mut stats) }, 0);
        let moved = [
            1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let delta = XvSceneInstanceDelta {
            structure_size: 176,
            operation: 0,
            flags: 1,
            layer: 0,
            instance_id: 2,
            mesh_id: 0,
            object_id: 0,
            category_mask: 0,
            transform: moved,
        };
        let policy = XvSceneUpdateOptions::default();
        let mut report = XvSceneUpdateReport {
            structure_size: 0,
            flags: 0,
            delta_count: 0,
            static_change_count: 0,
            dynamic_change_count: 0,
            reused_blas_count: 0,
            rebuilt_blas_count: 0,
            update_time_microseconds: 0,
            maximum_refit_quality_ratio: 0.0,
            previous_hash: [0; 4],
            current_hash: [0; 4],
        };
        // SAFETY: All fixed-layout records are live and correctly sized.
        assert_eq!(
            unsafe {
                xv_scene_apply_instance_deltas(
                    handle,
                    &raw const delta,
                    1,
                    &raw const policy,
                    &raw mut report,
                    &raw mut stats,
                )
            },
            0
        );
        assert_eq!(report.flags & 3, HierarchyUpdateKind::Reused as u32);
        assert_eq!((report.flags >> 2) & 3, HierarchyUpdateKind::Refit as u32);
        assert_eq!(report.reused_blas_count, 1);
        assert_ne!(report.previous_hash, report.current_hash);
        assert_eq!(xv_scene_release(handle), 0);
    }
}
