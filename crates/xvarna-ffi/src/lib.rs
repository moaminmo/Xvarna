//! Stable C ABI for the XVARNA native engine.

mod compute;
mod daena;
mod daena_advanced;
mod daena_compute;
mod daena_intelligence;
mod daylight;
mod irradiance;
mod job;
mod scene;
mod sky;
mod solar;
mod solar_intelligence;
mod study;
mod study_evidence;
mod study_platform;
mod surface;

pub use compute::{
    XvAdapterInfo, XvCacheOptions, XvCacheStats, XvComputeBatchStats, XvComputeInfo,
    XvComputeOptions, XvComputeRuntimeStats,
};
pub use daena::{
    XvIntervisibilityEntry, XvIntervisibilityMetadata, XvIntervisibilityOptions, XvIsovistMetadata,
    XvIsovistOptions, XvIsovistRay, XvIsovistSummary, XvObserverVisibilitySummary,
    XvSparseVisibilityGraphOptions, XvTargetVisibilitySummary, XvViewpoint,
    XvVisibilityGraphMetadata, XvVisibilityGraphOptions, XvVisibilityGraphPair, XvVisibilityNode,
    XvVisibilityNodeMetrics, XvVisibilityObserver, XvVisibilityTarget,
};
pub use daena_advanced::{
    XvObserverPath, XvObserverPathMetadata, XvObserverPathOptions, XvObserverPathSample,
    XvObserverPathSummary, XvPoint3, XvTargetViewEntry, XvTargetViewMetadata, XvTargetViewOptions,
    XvTargetViewSummary, XvViewCorridor, XvViewCorridorMetadata, XvViewCorridorOptions,
    XvViewCorridorSample, XvViewCorridorSummary, XvViewObserver, XvViewTargetPatch,
};
pub use daena_compute::XvDaenaExecutionInfo;
pub use daena_intelligence::{
    XvAnalysisMaterial, XvAttributionEntry, XvCategoryBreakdown, XvIsovist3dMetadata,
    XvIsovist3dOptions, XvIsovist3dRay, XvIsovist3dSummary, XvLandmark, XvLandmarkMetadata,
    XvLandmarkObserver, XvLandmarkOptions, XvLandmarkVisibilityEntry, XvMaterialAssignment,
    XvSpatialViewpoint,
};
pub use daylight::{
    XvAnnualDaylightMetadata, XvAnnualDaylightOptions, XvAnnualDaylightSummary,
    XvAnnualDaylightTimelineEntry, XvDaylightFactorEntry, XvDaylightMatrixOptions,
    XvDaylightMetadata, XvDaylightMoment, XvDaylightSensor, XvDaylightValidationReport,
    XvOpticalMaterial, XvPointIlluminanceEntry, XvRadianceExportMetadata,
};
pub use irradiance::{
    XvAnnualIrradianceMetadata, XvAnnualIrradianceOptions, XvEpwMetadata, XvIrradianceSummary,
    XvIrradianceTimelineEntry,
};
pub use job::XvJobProgress;
pub use scene::{
    XvHit, XvRay, XvSceneInstanceDelta, XvSceneOptions, XvSceneStats, XvSceneUpdateOptions,
    XvSceneUpdateReport,
};
pub use sky::{XvSkyRayEntry, XvSkyViewMetadata, XvSkyViewOptions, XvSkyViewSummary};
pub use solar::{
    XvDirectSunMetadata, XvDirectSunOptions, XvDirectSunSummary, XvSolarOptions, XvSolarSensor,
    XvSunSample, XvSunSetMetadata, XvSunTimelineEntry, XvTimeSample,
};
pub use solar_intelligence::{
    XvEnvelopeCandidate, XvEnvelopeControl, XvSolarAttributionEntry, XvSolarCategoryBreakdown,
    XvSolarEnvelopeCell, XvSolarEnvelopeMetadata, XvSolarEnvelopeOptions, XvSolarScenario,
    XvSolarScenarioDelta, XvSolarScenarioMetadata, XvSolarScenarioOptions, XvSolarScenarioSummary,
    XvSolarScenarioTimelineEntry,
};
pub use study::{
    XvOptimizerConfig, XvOptimizerMetadata, XvRankedSolution, XvSensitivityCoefficient,
    XvStudyMetadata, XvVariableSpec,
};
pub use study_platform::XvStudyReportOptions;
pub use surface::{
    XvPvPotentialCell, XvPvPotentialMetadata, XvPvPotentialOptions, XvPvPotentialRegion,
    XvSurfaceCell, XvSurfaceGridMetadata, XvSurfaceGridOptions,
};

use core::{mem, slice};
use std::panic::{AssertUnwindSafe, catch_unwind};
use xvarna_geometry::{Mesh, MeshAuditOptions, MeshAuditReport, Vec3, audit_mesh};
use xvarna_types::ABI_VERSION;

const AUDIT_FLAG_ANALYSIS_READY: u32 = 1 << 0;
const AUDIT_FLAG_WATERTIGHT: u32 = 1 << 1;
const AUDIT_FLAG_HAS_BOUNDS: u32 = 1 << 2;
const AUDIT_FLAG_F32_PRECISION_RISK: u32 = 1 << 3;

/// Status returned by native ABI operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum XvStatus {
    /// Operation completed successfully.
    Success = 0,
    /// A required pointer was null.
    NullPointer = 1,
    /// A length overflowed or an output buffer was too small.
    InvalidLength = 2,
    /// A numeric option was invalid.
    InvalidArgument = 3,
    /// A scene handle does not exist or was already released.
    InvalidHandle = 4,
    /// The scene handle is valid but not in the required build state.
    InvalidState = 5,
    /// A cooperative job cancellation was observed.
    Cancelled = 6,
    /// A Rust panic was contained at the ABI boundary.
    Panic = 255,
}

/// Fixed-layout summary produced by [`xv_mesh_audit`].
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvMeshAuditSummary {
    /// Byte size of this structure for layout diagnostics.
    pub structure_size: u32,
    /// Stable summary flags.
    pub flags: u32,
    /// Number of source vertices.
    pub vertex_count: u64,
    /// Number of source triangle faces.
    pub face_count: u64,
    /// Number of accepted faces.
    pub accepted_face_count: u64,
    /// Number of non-finite vertices.
    pub non_finite_vertex_count: u64,
    /// Number of faces containing invalid indices.
    pub invalid_index_face_count: u64,
    /// Number of faces referencing non-finite vertices.
    pub non_finite_face_count: u64,
    /// Number of degenerate faces.
    pub degenerate_face_count: u64,
    /// Number of duplicate faces.
    pub duplicate_face_count: u64,
    /// Number of isolated vertices.
    pub isolated_vertex_count: u64,
    /// Number of boundary edges.
    pub boundary_edge_count: u64,
    /// Number of non-manifold edges.
    pub non_manifold_edge_count: u64,
    /// Number of inconsistent-winding edges.
    pub inconsistent_winding_edge_count: u64,
    /// Number of connected components.
    pub connected_component_count: u64,
    /// Surface area in squared model units.
    pub surface_area: f64,
    /// Signed volume in cubed model units.
    pub signed_volume: f64,
    /// Minimum X bound when the bounds flag is set.
    pub bounds_min_x: f64,
    /// Minimum Y bound when the bounds flag is set.
    pub bounds_min_y: f64,
    /// Minimum Z bound when the bounds flag is set.
    pub bounds_min_z: f64,
    /// Maximum X bound when the bounds flag is set.
    pub bounds_max_x: f64,
    /// Maximum Y bound when the bounds flag is set.
    pub bounds_max_y: f64,
    /// Maximum Z bound when the bounds flag is set.
    pub bounds_max_z: f64,
    /// Maximum absolute source coordinate.
    pub maximum_coordinate_magnitude: f64,
    /// Conservative absolute f32 rounding estimate.
    pub estimated_f32_error: f64,
    /// First little-endian word of the 256-bit content hash.
    pub content_hash_0: u64,
    /// Second little-endian word of the 256-bit content hash.
    pub content_hash_1: u64,
    /// Third little-endian word of the 256-bit content hash.
    pub content_hash_2: u64,
    /// Fourth little-endian word of the 256-bit content hash.
    pub content_hash_3: u64,
}

impl Default for XvMeshAuditSummary {
    fn default() -> Self {
        Self {
            structure_size: u32::try_from(mem::size_of::<Self>())
                .expect("ABI summary size fits u32"),
            flags: 0,
            vertex_count: 0,
            face_count: 0,
            accepted_face_count: 0,
            non_finite_vertex_count: 0,
            invalid_index_face_count: 0,
            non_finite_face_count: 0,
            degenerate_face_count: 0,
            duplicate_face_count: 0,
            isolated_vertex_count: 0,
            boundary_edge_count: 0,
            non_manifold_edge_count: 0,
            inconsistent_winding_edge_count: 0,
            connected_component_count: 0,
            surface_area: 0.0,
            signed_volume: 0.0,
            bounds_min_x: 0.0,
            bounds_min_y: 0.0,
            bounds_min_z: 0.0,
            bounds_max_x: 0.0,
            bounds_max_y: 0.0,
            bounds_max_z: 0.0,
            maximum_coordinate_magnitude: 0.0,
            estimated_f32_error: 0.0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        }
    }
}

/// Returns the native ABI major version.
#[unsafe(no_mangle)]
pub const extern "C" fn xv_abi_version_major() -> u32 {
    ABI_VERSION.major
}

/// Returns the native ABI minor version.
#[unsafe(no_mangle)]
pub const extern "C" fn xv_abi_version_minor() -> u32 {
    ABI_VERSION.minor
}

/// Returns the native ABI patch version.
#[unsafe(no_mangle)]
pub const extern "C" fn xv_abi_version_patch() -> u32 {
    ABI_VERSION.patch
}

/// Returns the engine package major version.
#[unsafe(no_mangle)]
pub extern "C" fn xv_engine_version_major() -> u32 {
    parse_version_component(0)
}

/// Returns the engine package minor version.
#[unsafe(no_mangle)]
pub extern "C" fn xv_engine_version_minor() -> u32 {
    parse_version_component(1)
}

/// Returns the engine package patch version.
#[unsafe(no_mangle)]
pub extern "C" fn xv_engine_version_patch() -> u32 {
    parse_version_component(2)
}

/// Audits a triangle mesh and fills caller-owned fixed-size outputs.
///
/// # Safety
///
/// `positions` must reference `vertex_count * 3` readable `f64` values and
/// `triangles` must reference `face_count * 3` readable `u32` values. Empty
/// inputs may use null pointers. `output_summary` must be writable. Flag
/// buffers must be writable for their declared capacities, which must be at
/// least the corresponding source count. All buffers must remain valid and
/// non-overlapping for the duration of this call.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_mesh_audit(
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    face_count: usize,
    absolute_tolerance: f64,
    maximum_f32_error_ratio: f64,
    output_summary: *mut XvMeshAuditSummary,
    output_face_flags: *mut u32,
    face_flags_capacity: usize,
    output_vertex_flags: *mut u32,
    vertex_flags_capacity: usize,
) -> i32 {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: The public contract is checked for null pointers, lengths,
        // and output capacities before slices are created.
        unsafe {
            audit_mesh_impl(
                positions,
                vertex_count,
                triangles,
                face_count,
                absolute_tolerance,
                maximum_f32_error_ratio,
                output_summary,
                output_face_flags,
                face_flags_capacity,
                output_vertex_flags,
                vertex_flags_capacity,
            )
        }
    }));

    match outcome {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn audit_mesh_impl(
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    face_count: usize,
    absolute_tolerance: f64,
    maximum_f32_error_ratio: f64,
    output_summary: *mut XvMeshAuditSummary,
    output_face_flags: *mut u32,
    face_flags_capacity: usize,
    output_vertex_flags: *mut u32,
    vertex_flags_capacity: usize,
) -> Result<(), XvStatus> {
    if output_summary.is_null() {
        return Err(XvStatus::NullPointer);
    }
    if face_flags_capacity < face_count || vertex_flags_capacity < vertex_count {
        return Err(XvStatus::InvalidLength);
    }

    let position_length = vertex_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
    let triangle_length = face_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
    // SAFETY: Length arithmetic and nullability are validated by `input_slice`.
    let position_values = unsafe { input_slice(positions, position_length)? };
    // SAFETY: Length arithmetic and nullability are validated by `input_slice`.
    let triangle_values = unsafe { input_slice(triangles, triangle_length)? };
    // SAFETY: Output capacity and nullability are validated by `output_slice`.
    let face_flags = unsafe { output_slice(output_face_flags, face_count)? };
    // SAFETY: Output capacity and nullability are validated by `output_slice`.
    let vertex_flags = unsafe { output_slice(output_vertex_flags, vertex_count)? };

    let options = MeshAuditOptions::try_new(absolute_tolerance, maximum_f32_error_ratio)
        .map_err(|_| XvStatus::InvalidArgument)?;
    let mesh = Mesh {
        positions: position_values
            .chunks_exact(3)
            .map(|value| Vec3::new(value[0], value[1], value[2]))
            .collect(),
        triangles: triangle_values
            .chunks_exact(3)
            .map(|value| [value[0], value[1], value[2]])
            .collect(),
    };
    let report = audit_mesh(&mesh, options).map_err(|_| XvStatus::InvalidArgument)?;

    for (destination, source) in face_flags.iter_mut().zip(&report.face_flags) {
        *destination = source.bits();
    }
    for (destination, source) in vertex_flags.iter_mut().zip(&report.vertex_flags) {
        *destination = source.bits();
    }

    // SAFETY: `output_summary` was checked non-null and is caller-owned writable memory.
    unsafe { output_summary.write(summary_from_report(&report)) };
    Ok(())
}

pub(crate) const unsafe fn input_slice<'a, T>(
    pointer: *const T,
    length: usize,
) -> Result<&'a [T], XvStatus> {
    if length == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() || length > isize::MAX as usize / mem::size_of::<T>() {
        return Err(if pointer.is_null() {
            XvStatus::NullPointer
        } else {
            XvStatus::InvalidLength
        });
    }
    // SAFETY: The caller guarantees readable memory and the guard above validates
    // non-nullness plus Rust's maximum slice byte length.
    Ok(unsafe { slice::from_raw_parts(pointer, length) })
}

pub(crate) const unsafe fn output_slice<'a, T>(
    pointer: *mut T,
    length: usize,
) -> Result<&'a mut [T], XvStatus> {
    if length == 0 {
        return Ok(&mut []);
    }
    if pointer.is_null() || length > isize::MAX as usize / mem::size_of::<T>() {
        return Err(if pointer.is_null() {
            XvStatus::NullPointer
        } else {
            XvStatus::InvalidLength
        });
    }
    // SAFETY: The caller guarantees writable memory and the guard above validates
    // non-nullness plus Rust's maximum slice byte length.
    Ok(unsafe { slice::from_raw_parts_mut(pointer, length) })
}

fn summary_from_report(report: &MeshAuditReport) -> XvMeshAuditSummary {
    let mut flags = 0;
    if report.is_analysis_ready {
        flags |= AUDIT_FLAG_ANALYSIS_READY;
    }
    if report.is_watertight {
        flags |= AUDIT_FLAG_WATERTIGHT;
    }
    if report.bounds.is_some() {
        flags |= AUDIT_FLAG_HAS_BOUNDS;
    }
    if report.exceeds_f32_precision_budget {
        flags |= AUDIT_FLAG_F32_PRECISION_RISK;
    }
    let bounds = report.bounds.unwrap_or(xvarna_geometry::Aabb {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    });

    XvMeshAuditSummary {
        structure_size: u32::try_from(mem::size_of::<XvMeshAuditSummary>())
            .expect("ABI summary size fits u32"),
        flags,
        vertex_count: report.vertex_count as u64,
        face_count: report.face_count as u64,
        accepted_face_count: report.accepted_face_count as u64,
        non_finite_vertex_count: report.non_finite_vertex_count as u64,
        invalid_index_face_count: report.invalid_index_face_count as u64,
        non_finite_face_count: report.non_finite_face_count as u64,
        degenerate_face_count: report.degenerate_face_count as u64,
        duplicate_face_count: report.duplicate_face_count as u64,
        isolated_vertex_count: report.isolated_vertex_count as u64,
        boundary_edge_count: report.boundary_edge_count as u64,
        non_manifold_edge_count: report.non_manifold_edge_count as u64,
        inconsistent_winding_edge_count: report.inconsistent_winding_edge_count as u64,
        connected_component_count: report.connected_component_count as u64,
        surface_area: report.surface_area,
        signed_volume: report.signed_volume,
        bounds_min_x: bounds.min.x,
        bounds_min_y: bounds.min.y,
        bounds_min_z: bounds.min.z,
        bounds_max_x: bounds.max.x,
        bounds_max_y: bounds.max.y,
        bounds_max_z: bounds.max.z,
        maximum_coordinate_magnitude: report.maximum_coordinate_magnitude,
        estimated_f32_error: report.estimated_f32_error,
        content_hash_0: hash_word(&report.content_hash, 0),
        content_hash_1: hash_word(&report.content_hash, 1),
        content_hash_2: hash_word(&report.content_hash, 2),
        content_hash_3: hash_word(&report.content_hash, 3),
    }
}

pub(crate) fn hash_word(hash: &[u8; 32], index: usize) -> u64 {
    let offset = index * 8;
    u64::from_le_bytes(
        hash[offset..offset + 8]
            .try_into()
            .expect("hash word is exactly eight bytes"),
    )
}

pub(crate) fn hash_from_words(words: [u64; 4]) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    for (index, word) in words.into_iter().enumerate() {
        hash[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    hash
}

fn parse_version_component(index: usize) -> u32 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .nth(index)
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_versions_match_workspace_contract() {
        assert_eq!(xv_abi_version_major(), 0);
        assert_eq!(xv_abi_version_minor(), 19);
        assert_eq!(xv_engine_version_major(), 0);
        assert_eq!(xv_engine_version_minor(), 19);
    }

    #[test]
    fn mesh_audit_layout_is_stable() {
        assert_eq!(mem::size_of::<XvMeshAuditSummary>(), 224);
    }

    #[test]
    fn mesh_audit_crosses_abi_with_expected_metrics() {
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let triangles = [0_u32, 1, 2, 0, 2, 3];
        let mut summary = XvMeshAuditSummary::default();
        let mut face_flags = [0_u32; 2];
        let mut vertex_flags = [0_u32; 4];

        // SAFETY: Every pointer references a correctly sized live test buffer.
        let status = unsafe {
            xv_mesh_audit(
                positions.as_ptr(),
                4,
                triangles.as_ptr(),
                2,
                1.0e-9,
                0.25,
                &raw mut summary,
                face_flags.as_mut_ptr(),
                face_flags.len(),
                vertex_flags.as_mut_ptr(),
                vertex_flags.len(),
            )
        };

        assert_eq!(status, XvStatus::Success as i32);
        assert_eq!(summary.boundary_edge_count, 4);
        assert_eq!(summary.accepted_face_count, 2);
        assert!((summary.surface_area - 1.0).abs() < 1.0e-12);
        assert_eq!(face_flags, [0, 0]);
        assert_eq!(vertex_flags, [0, 0, 0, 0]);
    }

    #[test]
    fn mesh_audit_rejects_null_and_short_outputs() {
        let mut summary = XvMeshAuditSummary::default();
        // SAFETY: This intentionally supplies a null input to test validation;
        // no dereference occurs because the function rejects it first.
        let null_status = unsafe {
            xv_mesh_audit(
                core::ptr::null(),
                1,
                core::ptr::null(),
                0,
                1.0e-6,
                0.25,
                &raw mut summary,
                core::ptr::null_mut(),
                0,
                core::ptr::null_mut(),
                1,
            )
        };
        assert_eq!(null_status, XvStatus::NullPointer as i32);

        let positions = [0.0_f64; 3];
        // SAFETY: Input and summary are valid; the intentionally null vertex
        // output has insufficient capacity and is rejected before use.
        let short_status = unsafe {
            xv_mesh_audit(
                positions.as_ptr(),
                1,
                core::ptr::null(),
                0,
                1.0e-6,
                0.25,
                &raw mut summary,
                core::ptr::null_mut(),
                0,
                core::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(short_status, XvStatus::InvalidLength as i32);
    }
}
