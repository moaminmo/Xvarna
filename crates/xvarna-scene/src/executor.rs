//! Backend-neutral ordered ray execution contracts shared by domain engines.

use crate::{Hit, QueryRay, Scene};
use core::fmt;
use std::time::Instant;

/// Backend that produced a batch or aggregate analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RayQueryBackend {
    /// Canonical f64 CPU traversal.
    Cpu = 0,
    /// Portable f32 GPU traversal.
    PortableGpu = 1,
    /// More than one backend produced batches in the aggregate.
    Mixed = 2,
}

/// Immutable session-level execution context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RayQueryContext {
    /// Backend selected when the executor was created.
    pub backend: RayQueryBackend,
    /// Selected adapter name, absent for CPU.
    pub adapter_name: Option<String>,
    /// True when session creation fell back from portable GPU to CPU.
    pub used_creation_fallback: bool,
    /// Exact creation fallback reason, when present.
    pub fallback_reason: Option<String>,
}

/// Per-batch backend, transfer, timing, precision, and fallback provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct RayQueryBatchStats {
    /// Backend that produced the ordered output.
    pub backend: RayQueryBackend,
    /// Ordered ray count.
    pub ray_count: usize,
    /// Portable dispatch count; zero for canonical CPU.
    pub dispatch_count: usize,
    /// Upload and command-encoding time in microseconds.
    pub upload_microseconds: u64,
    /// Device/CPU execution and wait time in microseconds.
    pub execution_microseconds: u64,
    /// Readback and decoding time in microseconds.
    pub readback_microseconds: u64,
    /// Largest measured f32 scene/ray conversion error in metres.
    pub precision_error_meters: f64,
    /// True when this batch retried on CPU.
    pub used_runtime_fallback: bool,
    /// True when the backend served the batch from persistent GPU scratch resources.
    pub used_reusable_buffers: bool,
    /// Exact runtime fallback reason, when present.
    pub fallback_reason: Option<String>,
}

/// Ordered closest-hit batch returned by a backend-neutral executor.
#[derive(Clone, Debug, PartialEq)]
pub struct RayQueryBatch {
    /// One hit per input ray in identical order.
    pub hits: Vec<Hit>,
    /// Complete batch provenance.
    pub stats: RayQueryBatchStats,
}

/// Aggregated execution provenance embedded in a domain result.
#[derive(Clone, Debug, PartialEq)]
pub struct RayQueryExecutionSummary {
    /// Backend used by all batches, or Mixed when runtime fallback changed it.
    pub backend: RayQueryBackend,
    /// Selected adapter, absent for canonical CPU or creation fallback.
    pub adapter_name: Option<String>,
    /// True when executor creation fell back to CPU.
    pub used_creation_fallback: bool,
    /// Number of batches that retried on CPU.
    pub runtime_fallback_batch_count: usize,
    /// Number of batches served by persistent GPU scratch resources.
    pub reusable_buffer_batch_count: usize,
    /// Number of ordered query batches.
    pub batch_count: usize,
    /// Total rays submitted across batches.
    pub ray_count: usize,
    /// Total portable dispatches.
    pub dispatch_count: usize,
    /// Total upload/encoding microseconds.
    pub upload_microseconds: u64,
    /// Total execution/wait microseconds.
    pub execution_microseconds: u64,
    /// Total readback/decode microseconds.
    pub readback_microseconds: u64,
    /// Maximum measured precision error across batches.
    pub maximum_precision_error_meters: f64,
    /// First creation/runtime fallback reason, when present.
    pub fallback_reason: Option<String>,
}

impl RayQueryExecutionSummary {
    /// Starts an empty aggregate from immutable executor context.
    #[must_use]
    pub fn from_context(context: RayQueryContext) -> Self {
        Self {
            backend: context.backend,
            adapter_name: context.adapter_name,
            used_creation_fallback: context.used_creation_fallback,
            runtime_fallback_batch_count: 0,
            reusable_buffer_batch_count: 0,
            batch_count: 0,
            ray_count: 0,
            dispatch_count: 0,
            upload_microseconds: 0,
            execution_microseconds: 0,
            readback_microseconds: 0,
            maximum_precision_error_meters: 0.0,
            fallback_reason: context.fallback_reason,
        }
    }

    /// Accumulates one completed ordered batch.
    pub fn record(&mut self, stats: &RayQueryBatchStats) {
        if self.batch_count > 0 && self.backend != stats.backend {
            self.backend = RayQueryBackend::Mixed;
        } else if self.batch_count == 0 && !self.used_creation_fallback {
            self.backend = stats.backend;
        }
        self.batch_count = self.batch_count.saturating_add(1);
        self.ray_count = self.ray_count.saturating_add(stats.ray_count);
        self.dispatch_count = self.dispatch_count.saturating_add(stats.dispatch_count);
        self.upload_microseconds = self
            .upload_microseconds
            .saturating_add(stats.upload_microseconds);
        self.execution_microseconds = self
            .execution_microseconds
            .saturating_add(stats.execution_microseconds);
        self.readback_microseconds = self
            .readback_microseconds
            .saturating_add(stats.readback_microseconds);
        self.maximum_precision_error_meters = self
            .maximum_precision_error_meters
            .max(stats.precision_error_meters);
        if stats.used_runtime_fallback {
            self.runtime_fallback_batch_count = self.runtime_fallback_batch_count.saturating_add(1);
        }
        if stats.used_reusable_buffers {
            self.reusable_buffer_batch_count = self.reusable_buffer_batch_count.saturating_add(1);
        }
        if self.fallback_reason.is_none() {
            self.fallback_reason.clone_from(&stats.fallback_reason);
        }
    }

    /// Merges another domain sub-analysis into this aggregate.
    pub fn merge(&mut self, other: &Self) {
        if (self.batch_count > 0 || self.used_creation_fallback)
            && (other.batch_count > 0 || other.used_creation_fallback)
            && self.backend != other.backend
        {
            self.backend = RayQueryBackend::Mixed;
        } else if self.batch_count == 0 && !self.used_creation_fallback {
            self.backend = other.backend;
        }
        self.used_creation_fallback |= other.used_creation_fallback;
        self.runtime_fallback_batch_count = self
            .runtime_fallback_batch_count
            .saturating_add(other.runtime_fallback_batch_count);
        self.reusable_buffer_batch_count = self
            .reusable_buffer_batch_count
            .saturating_add(other.reusable_buffer_batch_count);
        self.batch_count = self.batch_count.saturating_add(other.batch_count);
        self.ray_count = self.ray_count.saturating_add(other.ray_count);
        self.dispatch_count = self.dispatch_count.saturating_add(other.dispatch_count);
        self.upload_microseconds = self
            .upload_microseconds
            .saturating_add(other.upload_microseconds);
        self.execution_microseconds = self
            .execution_microseconds
            .saturating_add(other.execution_microseconds);
        self.readback_microseconds = self
            .readback_microseconds
            .saturating_add(other.readback_microseconds);
        self.maximum_precision_error_meters = self
            .maximum_precision_error_meters
            .max(other.maximum_precision_error_meters);
        if self.adapter_name.is_none() {
            self.adapter_name.clone_from(&other.adapter_name);
        }
        if self.fallback_reason.is_none() {
            self.fallback_reason.clone_from(&other.fallback_reason);
        }
    }
}

/// Failure returned by a backend implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RayQueryExecutionError {
    message: String,
}

impl RayQueryExecutionError {
    /// Creates a contained execution error with a diagnostic message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RayQueryExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RayQueryExecutionError {}

/// Backend-neutral closest-hit service used by domain-analysis engines.
pub trait RayQueryExecutor: Sync {
    /// Returns the authoritative canonical scene and its scientific identity.
    fn canonical_scene(&self) -> &Scene;

    /// Returns immutable backend/adapter/fallback creation provenance.
    fn context(&self) -> RayQueryContext;

    /// Executes one ordered closest-hit batch.
    fn trace_closest(&self, rays: &[QueryRay]) -> Result<RayQueryBatch, RayQueryExecutionError>;
}

impl RayQueryExecutor for Scene {
    fn canonical_scene(&self) -> &Scene {
        self
    }

    fn context(&self) -> RayQueryContext {
        RayQueryContext {
            backend: RayQueryBackend::Cpu,
            adapter_name: None,
            used_creation_fallback: false,
            fallback_reason: None,
        }
    }

    fn trace_closest(&self, rays: &[QueryRay]) -> Result<RayQueryBatch, RayQueryExecutionError> {
        let started = Instant::now();
        let hits = self.trace_closest_batch(rays);
        Ok(RayQueryBatch {
            hits,
            stats: RayQueryBatchStats {
                backend: RayQueryBackend::Cpu,
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
        })
    }
}

fn elapsed_microseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_detects_mixed_backends_and_saturates_counters() {
        let mut summary = RayQueryExecutionSummary::from_context(RayQueryContext {
            backend: RayQueryBackend::PortableGpu,
            adapter_name: Some("test adapter".to_owned()),
            used_creation_fallback: false,
            fallback_reason: None,
        });
        summary.record(&RayQueryBatchStats {
            backend: RayQueryBackend::PortableGpu,
            ray_count: 100,
            dispatch_count: 2,
            upload_microseconds: 3,
            execution_microseconds: 4,
            readback_microseconds: 5,
            precision_error_meters: 1.0e-8,
            used_runtime_fallback: false,
            used_reusable_buffers: true,
            fallback_reason: None,
        });
        summary.record(&RayQueryBatchStats {
            backend: RayQueryBackend::Cpu,
            ray_count: 10,
            dispatch_count: 0,
            upload_microseconds: 0,
            execution_microseconds: 7,
            readback_microseconds: 0,
            precision_error_meters: 0.0,
            used_runtime_fallback: true,
            used_reusable_buffers: false,
            fallback_reason: Some("device lost".to_owned()),
        });
        assert_eq!(summary.backend, RayQueryBackend::Mixed);
        assert_eq!(summary.batch_count, 2);
        assert_eq!(summary.ray_count, 110);
        assert_eq!(summary.dispatch_count, 2);
        assert_eq!(summary.runtime_fallback_batch_count, 1);
        assert_eq!(summary.reusable_buffer_batch_count, 1);
        assert_eq!(summary.fallback_reason.as_deref(), Some("device lost"));
    }
}
