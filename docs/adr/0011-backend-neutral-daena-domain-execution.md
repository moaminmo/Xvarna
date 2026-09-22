# ADR 0011: Backend-Neutral DAENA Domain Execution

## Status

Accepted for XVARNA 0.11.0.

## Context

VAYU 0.10 exposed portable GPU closest-hit and any-hit batches, while advanced DAENA metrics called `Scene` traversal directly. Copying Target View, Corridor, and Observer Path algorithms into shaders would duplicate scientific policy, make CPU/GPU drift likely, and fragment hashing, attribution, sampling, and validation. Passing millions of individual calls through language boundaries would also erase the value of GPU execution.

## Decision

- Define `RayQueryExecutor` in `xvarna-scene`, below every domain engine and above concrete traversal implementations.
- Require every executor to expose the same canonical `Scene`, immutable execution context, and ordered closest-hit batches with complete source attribution.
- Keep all DAENA sampling, solid-angle, weighting, aggregation, convergence, hashing, and blocker logic in one Rust implementation. Only the closest-hit batch changes backend.
- Implement the contract directly for canonical f64 `Scene` and for cached `xvarna-vayu::ComputeSession`.
- Batch every Target View study once, every Corridor definition once, and all camera samples of one Observer Path once. VAYU may split a domain batch into bounded device dispatches while preserving order.
- Aggregate backend, adapter, creation/runtime fallback, batch/ray/dispatch counts, upload/execute/readback timings, and maximum measured precision error into every high-level result.
- Keep existing scene APIs and Grasshopper output indices stable. Add compute-session C/.NET methods and allow the same Grasshopper input to accept `XV Scene` or `XV Backend`.
- Treat canonical CPU as the verification oracle. CLI `--verify-cpu` compares numeric domain outputs and exact discrete attribution, and fails closed at the caller's tolerance.

## Consequences

- Scientific semantics have one implementation, so backend selection cannot silently change weighting or convergence definitions.
- GPU execution still uses f32 traversal; a boundary ray can legitimately differ within the declared precision contract. Discrete state/identity mismatches are reported separately from numeric deltas.
- Domain results expose operational provenance without coupling DAENA to wgpu.
- The synchronous public API retains one device wait/readback per VAYU dispatch. Persistent mapped ray buffers, overlapped asynchronous domain execution, device-loss fault injection, and two-level GPU instancing remain later performance work.
- Backend-neutral migration of ASMAN, HVARE Direct Sun, and MEHR is possible through the same contract but is not implied by this decision.
