//! Reusable opaque cooperative job-control handles.

use super::XvStatus;
use core::mem;
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const JOB_FLAG_CANCEL_REQUESTED: u32 = 1 << 0;

static NEXT_JOB_HANDLE: AtomicU64 = AtomicU64::new(1);
static JOBS: OnceLock<RwLock<HashMap<u64, Arc<JobControl>>>> = OnceLock::new();

pub struct JobControl {
    pub cancelled: AtomicBool,
    pub completed_units: AtomicU64,
    pub total_units: AtomicU64,
}

impl JobControl {
    const fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            completed_units: AtomicU64::new(0),
            total_units: AtomicU64::new(0),
        }
    }
}

/// Fixed-layout cooperative job progress snapshot.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct XvJobProgress {
    /// Byte size of this structure.
    pub structure_size: u32,
    /// Cancellation and future job-state flags.
    pub flags: u32,
    /// Total work units declared by the running operation.
    pub total_units: u64,
    /// Work units completed at the last safe cancellation boundary.
    pub completed_units: u64,
    /// Reserved; always zero.
    pub reserved: u64,
}

/// Creates an independent cooperative job-control handle.
///
/// # Safety
///
/// `output_handle` must point to writable `u64` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_job_create(output_handle: *mut u64) -> i32 {
    ffi_status(|| {
        if output_handle.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let handle = next_handle()?;
        write_registry().insert(handle, Arc::new(JobControl::new()));
        // SAFETY: Output pointer is non-null and caller-owned writable storage.
        unsafe { output_handle.write(handle) };
        Ok(())
    })
}

/// Requests cancellation. Running operations observe it between bounded chunks.
#[unsafe(no_mangle)]
pub extern "C" fn xv_job_cancel(handle: u64) -> i32 {
    ffi_status(|| {
        let job = get_job(handle)?;
        job.cancelled.store(true, Ordering::Relaxed);
        Ok(())
    })
}

/// Reads a thread-safe progress snapshot.
///
/// # Safety
///
/// `output_progress` must point to writable [`XvJobProgress`] storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xv_job_progress(handle: u64, output_progress: *mut XvJobProgress) -> i32 {
    ffi_status(|| {
        if output_progress.is_null() {
            return Err(XvStatus::NullPointer);
        }
        let job = get_job(handle)?;
        let progress = XvJobProgress {
            structure_size: structure_size::<XvJobProgress>(),
            flags: if job.cancelled.load(Ordering::Relaxed) {
                JOB_FLAG_CANCEL_REQUESTED
            } else {
                0
            },
            total_units: job.total_units.load(Ordering::Relaxed),
            completed_units: job.completed_units.load(Ordering::Relaxed),
            reserved: 0,
        };
        // SAFETY: Output pointer is non-null and caller-owned writable storage.
        unsafe { output_progress.write(progress) };
        Ok(())
    })
}

/// Releases a job-control handle. A running operation retains its own reference.
#[unsafe(no_mangle)]
pub extern "C" fn xv_job_release(handle: u64) -> i32 {
    ffi_status(|| {
        write_registry()
            .remove(&handle)
            .map_or(Err(XvStatus::InvalidHandle), |_| Ok(()))
    })
}

pub fn get_job(handle: u64) -> Result<Arc<JobControl>, XvStatus> {
    registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .cloned()
        .ok_or(XvStatus::InvalidHandle)
}

fn registry() -> &'static RwLock<HashMap<u64, Arc<JobControl>>> {
    JOBS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn write_registry() -> std::sync::RwLockWriteGuard<'static, HashMap<u64, Arc<JobControl>>> {
    registry()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn next_handle() -> Result<u64, XvStatus> {
    let handle = NEXT_JOB_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 || handle == u64::MAX {
        return Err(XvStatus::InvalidState);
    }
    Ok(handle)
}

fn structure_size<T>() -> u32 {
    u32::try_from(mem::size_of::<T>()).expect("ABI structure size fits u32")
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

    #[test]
    fn job_control_reports_progress_cancellation_and_release() {
        let mut handle = 0;
        // SAFETY: Output handle points to live writable storage.
        assert_eq!(unsafe { xv_job_create(&raw mut handle) }, 0);
        let job = get_job(handle).expect("job exists");
        job.total_units.store(100, Ordering::Relaxed);
        job.completed_units.store(25, Ordering::Relaxed);
        let mut progress = XvJobProgress {
            structure_size: 0,
            flags: 0,
            total_units: 0,
            completed_units: 0,
            reserved: 0,
        };
        // SAFETY: Progress output points to live writable storage.
        assert_eq!(unsafe { xv_job_progress(handle, &raw mut progress) }, 0);
        assert_eq!(progress.total_units, 100);
        assert_eq!(progress.completed_units, 25);
        assert_eq!(xv_job_cancel(handle), 0);
        // SAFETY: Progress output points to live writable storage.
        assert_eq!(unsafe { xv_job_progress(handle, &raw mut progress) }, 0);
        assert_ne!(progress.flags & JOB_FLAG_CANCEL_REQUESTED, 0);
        assert_eq!(xv_job_release(handle), 0);
        assert_eq!(xv_job_cancel(handle), XvStatus::InvalidHandle as i32);
    }

    #[test]
    fn job_progress_layout_is_stable() {
        assert_eq!(mem::size_of::<XvJobProgress>(), 32);
    }
}
