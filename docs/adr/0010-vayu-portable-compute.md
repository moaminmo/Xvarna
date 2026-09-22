# ADR 0010: VAYU Portable Compute and Explicit Fallback

## Status

Accepted for XVARNA 0.10.0.

## Context

The canonical ZAMYAD scene is f64, origin-rebased, instance-aware, and source-attributed. A portable GPU path must accelerate large ray batches without changing IDs, hiding reduced precision, relying on vendor-specific ray-tracing extensions, or making Rhino unusable when an adapter/driver fails.

## Decision

- Name the subsystem `VAYU`, after the ancient Iranian name associated with wind, atmosphere, and space. One subsystem name covers device discovery, portable traversal, and fallback; individual shader operations do not receive mythological names.
- Use Rust `wgpu 30.0.1` and WGSL compute. DX12, Vulkan, Metal, GL, and WebGPU availability follows wgpu's supported backends; XVARNA does not promise every API on every host.
- Keep the canonical f64 CPU scene authoritative. Build a separate immutable portable snapshot by flattening effective instances into rebased world-space triangles and a deterministic global 16-bin SAH BVH.
- Preserve object, instance, mesh, triangle, front-face, and 64-bit category identity in shader-visible buffers using explicit low/high u32 words.
- Measure actual coordinate quantization error while building the snapshot and for every ray batch. Reject required-GPU work that exceeds the caller's budget. `Auto` may fall back to CPU, but must report when and why.
- Use fixed resource ceilings for expanded triangles, BVH depth, storage binding size, workgroups, and rays per dispatch. Chunk batches deterministically when necessary.
- Report upload/encode, execution/wait, readback/decode, dispatch count, approximate static payload, adapter/driver, and precision error separately. Do not fold transfer time into a misleading shader-only performance number.
- Expose case-insensitive adapter-name filtering consistently in Rust, C, .NET, CLI, and Grasshopper. A required-GPU mismatch is an error; Auto may fall back only with recorded provenance.
- Keep C ABI additions append-only and expose safe-handle managed sessions for .NET 8 and .NET 10.

## Consequences

- GPU payload currently expands instances. The canonical CPU scene still deduplicates mesh resources, but highly instanced scenes can reach the portable triangle cap earlier. A two-level shader BLAS/TLAS layout remains a measured 1.x optimization, not a hidden claim in 0.10.
- f32 GPU results may differ slightly from f64 CPU distances/barycentrics. State and stable identity must match away from declared boundary/tolerance cases; numeric acceptance is tied to the explicit budget.
- Small scenes or small ray batches may be slower on GPU because transfer and dispatch overhead dominate. VAYU is retained for high-workload scaling and portable research, not marketed as universally faster.
- The 0.10 public session accelerates the raw ray-query workflow. Migration of every domain metric to a backend-neutral execution interface remains separate work and must retain each metric's error semantics.
