# ADR 0012: Retained VAYU Throughput Pipeline

Accepted for XVARNA 0.12.0.

## Context

VAYU 0.10/0.11 retained the scene buffers and compiled compute pipeline, but created ray, hit, readback, parameter, and bind-group resources for every dispatch. It also waited once for execution and again for mapping. That design was correct and bounded, but repeated Grasshopper studies paid resource-creation and synchronization overhead on every solution.

The public API is synchronous and thread-safe. Results must remain input ordered, Auto fallback must remain explicit, and the crate forbids unsafe Rust. A single query may contain more rays than a prudent persistent allocation on an integrated GPU.

## Decision

Each portable session owns one mutex-protected, lazily allocated scratch arena with two slots. Each slot retains:

- a storage/COPY_DST ray buffer;
- a storage/COPY_SRC hit buffer;
- a MAP_READ/COPY_DST readback buffer;
- a COPY_DST uniform buffer;
- one pipeline-compatible bind group;
- a reusable host `Vec<GpuRay>`.

Capacity grows to the next power of two required by a request and is capped at 262,144 rays per slot and the caller/hardware dispatch limit. Growth replaces the complete arena atomically. Equal or smaller later requests allocate no dynamic GPU resource. Two sequential chunks are encoded into separate slots, submitted together, mapped, completed through one device poll, and decoded slot A then B to preserve order.

The session installs official wgpu device-loss and uncaptured-error callbacks. A lost device remains marked unusable with its exact last diagnostic. Auto mode retries the affected batch on canonical CPU; required-GPU mode returns failure. Lock-free cumulative counters expose allocation generations, retained bytes, transfers, requests, dispatches, reuse, fallback, and health.

The public C ABI is extended with a new fixed 368-byte structure and getter. Existing batch and DAENA fixed layouts are not resized; unused flag bits identify persistent-arena use.

Disk pipeline-cache serialization is deferred. In wgpu 30 its creation API requires an unsafe call, conflicting with `#![forbid(unsafe_code)]`; the retained in-memory pipeline remains session scoped.

## Consequences

- Repeated workloads avoid per-dispatch buffer and bind-group creation after warm-up.
- Two chunks share submission and polling overhead while memory remains bounded.
- Calls on one session serialize around its two-slot arena; callers needing independent concurrent queues create independent sessions.
- Readback buffers are retained but mapped only for completion and immediately unmapped, as required before their next COPY_DST use.
- Cold and warm performance must be reported separately. Analytic smoke scenes establish correctness, not universal speedup.
- Driver-level device-loss injection, multi-machine shader CI, out-of-core geometry, and two-level GPU instancing remain separate gates.
