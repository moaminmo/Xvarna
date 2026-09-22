//! UTF-8 C ABI for the transactional XVARNA 0.16 Study platform.

use super::{XvStatus, input_slice, output_slice};
use core::mem;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};
use xvarna_study::platform::{
    BatchEvaluation, PlatformError, SensitivityOptions, StudyManifest, StudyWorkspace,
    study_manifest_schema, write_report_package,
};

/// Fixed-layout statistical report export policy.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvStudyReportOptions {
    /// Byte size.
    pub structure_size: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Bootstrap resamples.
    pub bootstrap_samples: u64,
    /// Permutation samples.
    pub permutation_samples: u64,
    /// Confidence interval level.
    pub confidence_level: f64,
    /// Family-wise significance level before Holm correction.
    pub alpha: f64,
    /// Deterministic statistical seed.
    pub seed: u64,
}

/// Copies the Study Manifest JSON Schema through a sizing-pass UTF-8 contract.
///
/// # Safety
///
/// `required_bytes` is writable. Output is null with zero capacity or writable to capacity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_study_manifest_schema_json(
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        let value = serde_json::to_string_pretty(&study_manifest_schema())
            .map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&value, output_utf8, output_capacity, required_bytes) }
    })
}

/// Creates a new transactional Study workspace from a manifest JSON document.
///
/// # Safety
///
/// UTF-8 inputs reference their declared lengths; output follows the sizing-pass contract.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_study_workspace_create(
    root_utf8: *const u8,
    root_length: usize,
    manifest_utf8: *const u8,
    manifest_length: usize,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        // SAFETY: Input pointer/count pairs are validated by shared helpers.
        let root = unsafe { read_utf8(root_utf8, root_length)? };
        // SAFETY: Input pointer/count pairs are validated by shared helpers.
        let manifest_text = unsafe { read_utf8(manifest_utf8, manifest_length)? };
        let manifest: StudyManifest =
            serde_json::from_str(manifest_text).map_err(|_| XvStatus::InvalidArgument)?;
        let workspace = match StudyWorkspace::create(root, manifest.clone()) {
            Ok(value) => value,
            Err(PlatformError::WorkspaceExists) => {
                let existing = StudyWorkspace::open(root).map_err(platform_status)?;
                if existing.manifest() != &manifest {
                    return Err(XvStatus::InvalidState);
                }
                existing
            }
            Err(error) => return Err(platform_status(error)),
        };
        let value =
            serde_json::to_string(&workspace.resume_state()).map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&value, output_utf8, output_capacity, required_bytes) }
    })
}

/// Reads and validates the current resume/checkpoint state.
///
/// # Safety
///
/// UTF-8 input and sizing-pass output follow their declared lengths/capacities.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_study_workspace_status(
    root_utf8: *const u8,
    root_length: usize,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        // SAFETY: Input pointer/count pair is validated by shared helper.
        let root = unsafe { read_utf8(root_utf8, root_length)? };
        let workspace = StudyWorkspace::open(root).map_err(platform_status)?;
        let value =
            serde_json::to_string(&workspace.resume_state()).map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&value, output_utf8, output_capacity, required_bytes) }
    })
}

/// Resumes an active external batch or transactionally begins the next batch.
///
/// # Safety
///
/// UTF-8 input and sizing-pass output follow their declared lengths/capacities.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_study_workspace_begin_batch(
    root_utf8: *const u8,
    root_length: usize,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        // SAFETY: Input pointer/count pair is validated by shared helper.
        let root = unsafe { read_utf8(root_utf8, root_length)? };
        let mut workspace = StudyWorkspace::open(root).map_err(platform_status)?;
        let batch = workspace.begin_batch().map_err(platform_status)?;
        let value = serde_json::to_string(&batch).map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&value, output_utf8, output_capacity, required_bytes) }
    })
}

/// Atomically commits a complete batch-evaluation JSON envelope.
///
/// Input JSON is `{ "batchId": N, "evaluations": [...] }`.
///
/// # Safety
///
/// UTF-8 inputs and sizing-pass output follow their declared lengths/capacities.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_study_workspace_commit_batch(
    root_utf8: *const u8,
    root_length: usize,
    evaluations_utf8: *const u8,
    evaluations_length: usize,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        // SAFETY: Input pointer/count pairs are validated by shared helpers.
        let root = unsafe { read_utf8(root_utf8, root_length)? };
        // SAFETY: Input pointer/count pairs are validated by shared helpers.
        let evaluation_text = unsafe { read_utf8(evaluations_utf8, evaluations_length)? };
        let value: serde_json::Value =
            serde_json::from_str(evaluation_text).map_err(|_| XvStatus::InvalidArgument)?;
        let batch_id = value
            .get("batchId")
            .and_then(serde_json::Value::as_u64)
            .ok_or(XvStatus::InvalidArgument)?;
        let evaluations: Vec<BatchEvaluation> = serde_json::from_value(
            value
                .get("evaluations")
                .cloned()
                .ok_or(XvStatus::InvalidArgument)?,
        )
        .map_err(|_| XvStatus::InvalidArgument)?;
        let mut workspace = StudyWorkspace::open(root).map_err(platform_status)?;
        let state = workspace
            .commit_batch(batch_id, evaluations)
            .map_err(platform_status)?;
        let result = serde_json::to_string(&state).map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&result, output_utf8, output_capacity, required_bytes) }
    })
}

/// Generates JSON, Parquet, glTF, report HTML, and standalone viewer artifacts.
///
/// # Safety
///
/// Inputs reference valid structures/UTF-8 lengths; output follows the sizing-pass contract.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn xv_study_workspace_report(
    root_utf8: *const u8,
    root_length: usize,
    output_directory_utf8: *const u8,
    output_directory_length: usize,
    options: *const XvStudyReportOptions,
    output_utf8: *mut u8,
    output_capacity: usize,
    required_bytes: *mut usize,
) -> i32 {
    ffi_status(|| {
        if options.is_null() {
            return Err(XvStatus::NullPointer);
        }
        // SAFETY: Input pointer/count pairs and options pointer follow the function contract.
        let root = unsafe { read_utf8(root_utf8, root_length)? };
        // SAFETY: Input pointer/count pair is validated by shared helper.
        let output_directory =
            unsafe { read_utf8(output_directory_utf8, output_directory_length)? };
        // SAFETY: Pointer is non-null and references one readable record.
        let options = unsafe { options.read() };
        if options.structure_size != structure_size::<XvStudyReportOptions>()
            || options.reserved != 0
        {
            return Err(XvStatus::InvalidArgument);
        }
        let sensitivity = SensitivityOptions {
            bootstrap_samples: usize::try_from(options.bootstrap_samples)
                .map_err(|_| XvStatus::InvalidLength)?,
            permutation_samples: usize::try_from(options.permutation_samples)
                .map_err(|_| XvStatus::InvalidLength)?,
            confidence_level: options.confidence_level,
            alpha: options.alpha,
            seed: options.seed,
        };
        let workspace = StudyWorkspace::open(root).map_err(platform_status)?;
        let analysis = workspace.analyze(sensitivity).map_err(platform_status)?;
        let artifacts = write_report_package(
            Path::new(output_directory),
            workspace.manifest(),
            workspace.ledger(),
            &analysis,
        )
        .map_err(platform_status)?;
        let result = serde_json::json!({
            "schemaVersion": "0.16.0",
            "analysisHash": analysis.content_hash,
            "variantCount": workspace.ledger().evaluations.len(),
            "paretoCount": analysis.pareto_variant_ids.len(),
            "report": artifacts.html_report,
            "viewer": artifacts.standalone_viewer,
            "parquet": artifacts.variants_parquet,
            "gltf": artifacts.objective_space_gltf,
            "data": artifacts.data_json,
        });
        let result = serde_json::to_string(&result).map_err(|_| XvStatus::InvalidState)?;
        // SAFETY: Function contract delegates to checked helper.
        unsafe { write_utf8(&result, output_utf8, output_capacity, required_bytes) }
    })
}

pub unsafe fn read_utf8<'a>(value: *const u8, length: usize) -> Result<&'a str, XvStatus> {
    // SAFETY: Caller forwards the function-specific input contract.
    let bytes = unsafe { input_slice(value, length)? };
    core::str::from_utf8(bytes).map_err(|_| XvStatus::InvalidArgument)
}

pub unsafe fn write_utf8(
    value: &str,
    output: *mut u8,
    capacity: usize,
    required_bytes: *mut usize,
) -> Result<(), XvStatus> {
    if required_bytes.is_null() {
        return Err(XvStatus::NullPointer);
    }
    let required = value.len().checked_add(1).ok_or(XvStatus::InvalidLength)?;
    // SAFETY: Non-null pointer is writable by the function contract.
    unsafe { required_bytes.write(required) };
    if capacity == 0 {
        return Ok(());
    }
    if capacity < required {
        return Err(XvStatus::InvalidLength);
    }
    // SAFETY: Capacity has been validated against the exact required byte count.
    let destination = unsafe { output_slice(output, required)? };
    destination[..value.len()].copy_from_slice(value.as_bytes());
    destination[value.len()] = 0;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn platform_status(error: PlatformError) -> XvStatus {
    match error {
        PlatformError::InvalidManifest
        | PlatformError::InvalidLedger
        | PlatformError::InvalidBaseline
        | PlatformError::IncompatibleDeltaMap => XvStatus::InvalidArgument,
        PlatformError::WorkspaceExists
        | PlatformError::WorkspaceMissing
        | PlatformError::InvalidBatchState
        | PlatformError::CorruptWorkspace
        | PlatformError::Io(_)
        | PlatformError::Json(_)
        | PlatformError::Parquet(_)
        | PlatformError::Study(_) => XvStatus::InvalidState,
    }
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
}

pub fn ffi_status(operation: impl FnOnce() -> Result<(), XvStatus>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => XvStatus::Success as i32,
        Ok(Err(status)) => status as i32,
        Err(_) => XvStatus::Panic as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_options_layout_and_schema_sizing_are_stable() {
        assert_eq!(mem::size_of::<XvStudyReportOptions>(), 48);
        let mut required = 0_usize;
        // SAFETY: Sizing pass permits null output with zero capacity.
        let status =
            unsafe { xv_study_manifest_schema_json(core::ptr::null_mut(), 0, &raw mut required) };
        assert_eq!(status, XvStatus::Success as i32);
        assert!(required > 1_000);
        let mut output = vec![0_u8; required];
        // SAFETY: Buffer has the exact required capacity.
        let status = unsafe {
            xv_study_manifest_schema_json(output.as_mut_ptr(), output.len(), &raw mut required)
        };
        assert_eq!(status, XvStatus::Success as i32);
        let schema: serde_json::Value =
            serde_json::from_slice(&output[..output.len() - 1]).expect("schema");
        assert_eq!(schema["properties"]["schemaVersion"]["const"], "0.16.0");
    }
}
