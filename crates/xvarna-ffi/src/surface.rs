//! Fixed-layout surface-grid, hotspot-region, and PV proxy ABI.

use super::{XvStatus, hash_word, input_slice, output_slice};
use core::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use xvarna_geometry::{
    Mesh, SurfaceCell, SurfaceGridError, SurfaceGridOptions, Vec3, generate_surface_grid,
};
use xvarna_hvare::{PvPotentialError, PvPotentialOptions, analyze_pv_potential};
use xvarna_types::SensorId;

const GRID_HAS_SKIPPED_FACES_FLAG: u32 = 1 << 0;
const POTENTIAL_PROXY_MODEL_FLAG: u32 = 1 << 0;
const POTENTIAL_HAS_ELIGIBLE_CELLS_FLAG: u32 = 1 << 1;

/// Fixed-layout deterministic surface-grid policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSurfaceGridOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Maximum accepted analysis-cell edge length in input model units.
    pub target_edge_length: f64,
    /// Normal offset of sensor centroids in input model units.
    pub sensor_offset: f64,
    /// Stable first sensor identifier; zero is invalid.
    pub first_sensor_id: u64,
    /// Hard ceiling on generated cells.
    pub maximum_cell_count: u64,
}

/// One fixed-layout area-aware analysis triangle.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSurfaceCell {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Source triangle index.
    pub source_face_index: u32,
    /// Stable sensor identifier.
    pub sensor_id: u64,
    /// Offset sensor position X.
    pub position_x: f64,
    /// Offset sensor position Y.
    pub position_y: f64,
    /// Offset sensor position Z.
    pub position_z: f64,
    /// Unit normal X.
    pub normal_x: f64,
    /// Unit normal Y.
    pub normal_y: f64,
    /// Unit normal Z.
    pub normal_z: f64,
    /// Cell area in squared input model units.
    pub area: f64,
    /// First corner X.
    pub a_x: f64,
    /// First corner Y.
    pub a_y: f64,
    /// First corner Z.
    pub a_z: f64,
    /// Second corner X.
    pub b_x: f64,
    /// Second corner Y.
    pub b_y: f64,
    /// Second corner Z.
    pub b_z: f64,
    /// Third corner X.
    pub c_x: f64,
    /// Third corner Y.
    pub c_y: f64,
    /// Third corner Z.
    pub c_z: f64,
    /// Longest-edge subdivision depth.
    pub subdivision_depth: u32,
    /// Reserved; always zero.
    pub reserved: u32,
}

/// Surface-grid dimensions, diagnostics, metrics, and identity.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvSurfaceGridMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Diagnostic flags.
    pub flags: u32,
    /// Source triangle count.
    pub source_face_count: u64,
    /// Generated cell count.
    pub cell_count: u64,
    /// Invalid or degenerate source faces skipped.
    pub skipped_face_count: u64,
    /// Greatest subdivision depth.
    pub maximum_subdivision_depth: u64,
    /// Valid source area in squared model units.
    pub source_area: f64,
    /// Generated area in squared model units.
    pub sampled_area: f64,
    /// Longest generated edge in model units.
    pub maximum_cell_edge_length: f64,
    /// First little-endian word of the result hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Fixed-layout assumptions for the transparent PV potential proxy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPvPotentialOptions {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Input model-unit length expressed in metres.
    pub unit_scale_to_meters: f64,
    /// Eligibility threshold in Wh/m².
    pub minimum_irradiance_wh_m2: f64,
    /// Module efficiency from zero through one.
    pub module_efficiency: f64,
    /// Geometric module coverage from zero through one.
    pub coverage_ratio: f64,
    /// Aggregate downstream system loss from zero through one.
    pub system_loss_fraction: f64,
}

/// Per-cell PV proxy output.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPvPotentialCell {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Bit zero is set when the cell meets the threshold.
    pub flags: u32,
    /// Stable source sensor identifier.
    pub sensor_id: u64,
    /// One-based connected region identifier, or zero.
    pub region_id: u64,
    /// Area in m².
    pub area_m2: f64,
    /// Annual plane-of-array energy density in Wh/m².
    pub irradiance_wh_m2: f64,
    /// Incident energy over the complete cell in kWh.
    pub incident_energy_kwh: f64,
    /// Proxy module output in kWh.
    pub proxy_yield_kwh: f64,
}

/// Per-region PV proxy output.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPvPotentialRegion {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Reserved; always zero.
    pub reserved: u32,
    /// One-based region identifier.
    pub region_id: u64,
    /// Eligible cell count.
    pub cell_count: u64,
    /// Geometric region area in m².
    pub area_m2: f64,
    /// Area-weighted irradiance in Wh/m².
    pub mean_irradiance_wh_m2: f64,
    /// Minimum cell irradiance in Wh/m².
    pub minimum_irradiance_wh_m2: f64,
    /// Maximum cell irradiance in Wh/m².
    pub maximum_irradiance_wh_m2: f64,
    /// Incident energy over the complete region in kWh.
    pub incident_energy_kwh: f64,
    /// Proxy module output in kWh.
    pub proxy_yield_kwh: f64,
}

/// Area-weighted solar distribution and transparent PV proxy summary.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvPvPotentialMetadata {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Model and eligibility flags.
    pub flags: u32,
    /// Input cell count.
    pub cell_count: u64,
    /// Eligible cell count.
    pub eligible_cell_count: u64,
    /// Connected eligible-region count.
    pub region_count: u64,
    /// Total geometric area in m².
    pub total_area_m2: f64,
    /// Eligible geometric area in m².
    pub eligible_area_m2: f64,
    /// Area-weighted mean annual irradiance in Wh/m².
    pub mean_irradiance_wh_m2: f64,
    /// Area-weighted tenth percentile in Wh/m².
    pub p10_irradiance_wh_m2: f64,
    /// Area-weighted median in Wh/m².
    pub p50_irradiance_wh_m2: f64,
    /// Area-weighted ninetieth percentile in Wh/m².
    pub p90_irradiance_wh_m2: f64,
    /// Incident energy on all complete cells in kWh.
    pub total_incident_energy_kwh: f64,
    /// Incident energy on eligible complete cells in kWh.
    pub eligible_incident_energy_kwh: f64,
    /// Proxy installed capacity in kWp.
    pub capacity_kwp: f64,
    /// Annual output proxy in kWh.
    pub proxy_yield_kwh: f64,
    /// Annual output proxy divided by capacity in kWh/kWp.
    pub specific_yield_kwh_kwp: f64,
    /// First little-endian word of the result hash.
    pub content_hash_0: u64,
    /// Second hash word.
    pub content_hash_1: u64,
    /// Third hash word.
    pub content_hash_2: u64,
    /// Fourth hash word.
    pub content_hash_3: u64,
}

/// Generates an area-preserving surface sensor grid from packed triangle geometry.
///
/// A null output-cell pointer with zero capacity performs a metadata-only sizing pass.
///
/// # Safety
///
/// Input and output pointers reference their declared counts and remain valid,
/// non-overlapping storage for the duration of the call.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_surface_grid(
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    face_count: usize,
    options: *const XvSurfaceGridOptions,
    output_metadata: *mut XvSurfaceGridMetadata,
    output_cells: *mut XvSurfaceCell,
    cell_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required option pointer was validated non-null.
        let native_options = unsafe { options.read() };
        if native_options.structure_size != structure_size::<XvSurfaceGridOptions>()
            || native_options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let maximum_cell_count = usize::try_from(native_options.maximum_cell_count)
            .map_err(|_| XvStatus::InvalidLength)?;
        let policy = SurfaceGridOptions::try_new(
            native_options.target_edge_length,
            native_options.sensor_offset,
            native_options.first_sensor_id,
            maximum_cell_count,
        )
        .map_err(map_grid_error)?;
        let position_length = vertex_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        let triangle_length = face_count.checked_mul(3).ok_or(XvStatus::InvalidLength)?;
        // SAFETY: Shared helpers validate the declared input ranges.
        let position_values = unsafe { input_slice(positions, position_length)? };
        // SAFETY: Shared helpers validate the declared input ranges.
        let triangle_values = unsafe { input_slice(triangles, triangle_length)? };
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
        let result = generate_surface_grid(&mesh, policy).map_err(map_grid_error)?;
        let metadata = grid_metadata(&result);
        // SAFETY: Metadata pointer is non-null and writable by contract.
        unsafe { output_metadata.write(metadata) };
        if output_cells.is_null() && cell_capacity == 0 {
            return Ok(());
        }
        if cell_capacity < result.cells.len() {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Capacity was validated above.
        let outputs = unsafe { output_slice(output_cells, result.cells.len())? };
        for (output, cell) in outputs.iter_mut().zip(&result.cells) {
            *output = surface_cell(*cell);
        }
        Ok(())
    })
}

/// Computes area-weighted hotspots, connected regions, and a transparent PV proxy.
///
/// Null cell/region outputs with zero capacities perform a metadata-only sizing pass.
///
/// # Safety
///
/// Every pointer references its declared count and remains valid and non-overlapping
/// for the duration of the call.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_surface_pv_potential(
    cells: *const XvSurfaceCell,
    irradiance_wh_m2: *const f64,
    cell_count: usize,
    options: *const XvPvPotentialOptions,
    output_metadata: *mut XvPvPotentialMetadata,
    output_cells: *mut XvPvPotentialCell,
    cell_capacity: usize,
    output_regions: *mut XvPvPotentialRegion,
    region_capacity: usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() || output_metadata.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Required option pointer was validated non-null.
        let native_options = unsafe { options.read() };
        if native_options.structure_size != structure_size::<XvPvPotentialOptions>()
            || native_options.reserved != 0
            || !native_options.unit_scale_to_meters.is_finite()
            || native_options.unit_scale_to_meters <= 0.0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let policy = PvPotentialOptions::try_new(
            native_options.minimum_irradiance_wh_m2,
            native_options.module_efficiency,
            native_options.coverage_ratio,
            native_options.system_loss_fraction,
        )
        .map_err(map_potential_error)?;
        // SAFETY: Shared helpers validate declared input ranges.
        let inputs = unsafe { input_slice(cells, cell_count)? };
        // SAFETY: Shared helpers validate declared input ranges.
        let values = unsafe { input_slice(irradiance_wh_m2, cell_count)? };
        let scale = native_options.unit_scale_to_meters;
        let canonical_cells = inputs
            .iter()
            .map(|cell| canonical_surface_cell(*cell, scale))
            .collect::<Result<Vec<_>, _>>()?;
        let result =
            analyze_pv_potential(&canonical_cells, values, policy).map_err(map_potential_error)?;
        let metadata = potential_metadata(&result);
        // SAFETY: Metadata pointer is non-null and writable by contract.
        unsafe { output_metadata.write(metadata) };
        if output_cells.is_null()
            && cell_capacity == 0
            && output_regions.is_null()
            && region_capacity == 0
        {
            return Ok(());
        }
        if cell_capacity < result.cells.len() || region_capacity < result.regions.len() {
            return Err(XvStatus::InvalidLength);
        }
        // SAFETY: Both capacities were validated above.
        let cell_outputs = unsafe { output_slice(output_cells, result.cells.len())? };
        // SAFETY: Both capacities were validated above.
        let region_outputs = unsafe { output_slice(output_regions, result.regions.len())? };
        for (output, cell) in cell_outputs.iter_mut().zip(&result.cells) {
            *output = XvPvPotentialCell {
                structure_size: structure_size::<XvPvPotentialCell>(),
                flags: u32::from(cell.is_eligible),
                sensor_id: cell.sensor_id.get(),
                region_id: cell.region_id,
                area_m2: cell.area_m2,
                irradiance_wh_m2: cell.irradiance_wh_m2,
                incident_energy_kwh: cell.incident_energy_kwh,
                proxy_yield_kwh: cell.proxy_yield_kwh,
            };
        }
        for (output, region) in region_outputs.iter_mut().zip(&result.regions) {
            *output = XvPvPotentialRegion {
                structure_size: structure_size::<XvPvPotentialRegion>(),
                reserved: 0,
                region_id: region.region_id,
                cell_count: region.cell_count as u64,
                area_m2: region.area_m2,
                mean_irradiance_wh_m2: region.mean_irradiance_wh_m2,
                minimum_irradiance_wh_m2: region.minimum_irradiance_wh_m2,
                maximum_irradiance_wh_m2: region.maximum_irradiance_wh_m2,
                incident_energy_kwh: region.incident_energy_kwh,
                proxy_yield_kwh: region.proxy_yield_kwh,
            };
        }
        Ok(())
    })
}

fn grid_metadata(result: &xvarna_geometry::SurfaceGridResult) -> XvSurfaceGridMetadata {
    XvSurfaceGridMetadata {
        structure_size: structure_size::<XvSurfaceGridMetadata>(),
        flags: if result.skipped_face_count > 0 {
            GRID_HAS_SKIPPED_FACES_FLAG
        } else {
            0
        },
        source_face_count: result.source_face_count as u64,
        cell_count: result.cells.len() as u64,
        skipped_face_count: result.skipped_face_count as u64,
        maximum_subdivision_depth: u64::from(result.maximum_subdivision_depth),
        source_area: result.source_area,
        sampled_area: result.sampled_area,
        maximum_cell_edge_length: result.maximum_cell_edge_length,
        content_hash_0: hash_word(&result.content_hash, 0),
        content_hash_1: hash_word(&result.content_hash, 1),
        content_hash_2: hash_word(&result.content_hash, 2),
        content_hash_3: hash_word(&result.content_hash, 3),
    }
}

fn potential_metadata(result: &xvarna_hvare::PvPotentialResult) -> XvPvPotentialMetadata {
    XvPvPotentialMetadata {
        structure_size: structure_size::<XvPvPotentialMetadata>(),
        flags: POTENTIAL_PROXY_MODEL_FLAG
            | if result.eligible_area_m2 > 0.0 {
                POTENTIAL_HAS_ELIGIBLE_CELLS_FLAG
            } else {
                0
            },
        cell_count: result.cells.len() as u64,
        eligible_cell_count: result.cells.iter().filter(|cell| cell.is_eligible).count() as u64,
        region_count: result.regions.len() as u64,
        total_area_m2: result.total_area_m2,
        eligible_area_m2: result.eligible_area_m2,
        mean_irradiance_wh_m2: result.mean_irradiance_wh_m2,
        p10_irradiance_wh_m2: result.p10_irradiance_wh_m2,
        p50_irradiance_wh_m2: result.p50_irradiance_wh_m2,
        p90_irradiance_wh_m2: result.p90_irradiance_wh_m2,
        total_incident_energy_kwh: result.total_incident_energy_kwh,
        eligible_incident_energy_kwh: result.eligible_incident_energy_kwh,
        capacity_kwp: result.capacity_kwp,
        proxy_yield_kwh: result.proxy_yield_kwh,
        specific_yield_kwh_kwp: result.specific_yield_kwh_kwp,
        content_hash_0: hash_word(&result.content_hash, 0),
        content_hash_1: hash_word(&result.content_hash, 1),
        content_hash_2: hash_word(&result.content_hash, 2),
        content_hash_3: hash_word(&result.content_hash, 3),
    }
}

fn surface_cell(cell: SurfaceCell) -> XvSurfaceCell {
    XvSurfaceCell {
        structure_size: structure_size::<XvSurfaceCell>(),
        source_face_index: cell.source_face_index,
        sensor_id: cell.sensor_id.get(),
        position_x: cell.position.x,
        position_y: cell.position.y,
        position_z: cell.position.z,
        normal_x: cell.normal.x,
        normal_y: cell.normal.y,
        normal_z: cell.normal.z,
        area: cell.area,
        a_x: cell.a.x,
        a_y: cell.a.y,
        a_z: cell.a.z,
        b_x: cell.b.x,
        b_y: cell.b.y,
        b_z: cell.b.z,
        c_x: cell.c.x,
        c_y: cell.c.y,
        c_z: cell.c.z,
        subdivision_depth: cell.subdivision_depth,
        reserved: 0,
    }
}

fn canonical_surface_cell(cell: XvSurfaceCell, scale: f64) -> Result<SurfaceCell, XvStatus> {
    if cell.structure_size != structure_size::<XvSurfaceCell>() || cell.reserved != 0 {
        return Err(XvStatus::InvalidArgument);
    }
    let point = |x: f64, y: f64, z: f64| Vec3::new(x * scale, y * scale, z * scale);
    let area_scale = scale * scale;
    Ok(SurfaceCell {
        sensor_id: SensorId::new(cell.sensor_id),
        source_face_index: cell.source_face_index,
        subdivision_depth: cell.subdivision_depth,
        a: point(cell.a_x, cell.a_y, cell.a_z),
        b: point(cell.b_x, cell.b_y, cell.b_z),
        c: point(cell.c_x, cell.c_y, cell.c_z),
        position: point(cell.position_x, cell.position_y, cell.position_z),
        normal: Vec3::new(cell.normal_x, cell.normal_y, cell.normal_z),
        area: cell.area * area_scale,
    })
}

const fn map_grid_error(error: SurfaceGridError) -> XvStatus {
    match error {
        SurfaceGridError::CellLimitExceeded
        | SurfaceGridError::SensorIdOverflow
        | SurfaceGridError::SourceFaceIndexOverflow => XvStatus::InvalidLength,
        _ => XvStatus::InvalidArgument,
    }
}

const fn map_potential_error(_error: PvPotentialError) -> XvStatus {
    XvStatus::InvalidArgument
}

fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_layouts_are_stable() {
        assert_eq!(mem::size_of::<XvSurfaceGridOptions>(), 40);
        assert_eq!(mem::size_of::<XvSurfaceCell>(), 152);
        assert_eq!(mem::size_of::<XvSurfaceGridMetadata>(), 96);
        assert_eq!(mem::size_of::<XvPvPotentialOptions>(), 48);
        assert_eq!(mem::size_of::<XvPvPotentialCell>(), 56);
        assert_eq!(mem::size_of::<XvPvPotentialRegion>(), 72);
        assert_eq!(mem::size_of::<XvPvPotentialMetadata>(), 152);
    }

    #[test]
    fn grid_and_potential_cross_the_abi_with_sizing_passes() {
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let triangles = [0_u32, 1, 2, 0, 2, 3];
        let options = XvSurfaceGridOptions {
            structure_size: structure_size::<XvSurfaceGridOptions>(),
            reserved: 0,
            target_edge_length: 2.0,
            sensor_offset: 0.01,
            first_sensor_id: 10,
            maximum_cell_count: 100,
        };
        let mut metadata = XvSurfaceGridMetadata {
            structure_size: 0,
            flags: 0,
            source_face_count: 0,
            cell_count: 0,
            skipped_face_count: 0,
            maximum_subdivision_depth: 0,
            source_area: 0.0,
            sampled_area: 0.0,
            maximum_cell_edge_length: 0.0,
            content_hash_0: 0,
            content_hash_1: 0,
            content_hash_2: 0,
            content_hash_3: 0,
        };
        // SAFETY: Test buffers match every declared count.
        let sizing = unsafe {
            xv_surface_grid(
                positions.as_ptr(),
                4,
                triangles.as_ptr(),
                2,
                &raw const options,
                &raw mut metadata,
                core::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(sizing, XvStatus::Success as i32);
        assert_eq!(metadata.cell_count, 2);
        let mut cells = vec![unsafe { mem::zeroed::<XvSurfaceCell>() }; 2];
        // SAFETY: Test buffers match every declared count.
        let generated = unsafe {
            xv_surface_grid(
                positions.as_ptr(),
                4,
                triangles.as_ptr(),
                2,
                &raw const options,
                &raw mut metadata,
                cells.as_mut_ptr(),
                cells.len(),
            )
        };
        assert_eq!(generated, XvStatus::Success as i32);
        assert_eq!(cells[0].sensor_id, 10);

        let potential_options = XvPvPotentialOptions {
            structure_size: structure_size::<XvPvPotentialOptions>(),
            reserved: 0,
            unit_scale_to_meters: 1.0,
            minimum_irradiance_wh_m2: 800_000.0,
            module_efficiency: 0.2,
            coverage_ratio: 1.0,
            system_loss_fraction: 0.0,
        };
        let values = [1_000_000.0, 500_000.0];
        let mut potential_metadata = unsafe { mem::zeroed::<XvPvPotentialMetadata>() };
        // SAFETY: Test buffers match every declared count.
        let potential_sizing = unsafe {
            xv_surface_pv_potential(
                cells.as_ptr(),
                values.as_ptr(),
                cells.len(),
                &raw const potential_options,
                &raw mut potential_metadata,
                core::ptr::null_mut(),
                0,
                core::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(potential_sizing, XvStatus::Success as i32);
        assert_eq!(potential_metadata.eligible_cell_count, 1);
        assert_eq!(potential_metadata.region_count, 1);
        assert!((potential_metadata.proxy_yield_kwh - 100.0).abs() < 1.0e-12);
    }
}
