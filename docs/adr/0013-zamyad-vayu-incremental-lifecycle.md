# ADR 0013: Incremental ZAMYAD/VAYU Scene Lifecycle

Accepted for XVARNA 0.13.0.

## Context

Rebuilding a flattened world-space GPU BVH after every Grasshopper transform edit makes interactive design scale with all scene geometry. It also prevents reliable reuse across documents and offers no bounded memory contract. A production design engine needs stable resource identity, immutable snapshots for concurrent readers, explicit update provenance, recoverable caching, and predictable CPU/GPU memory use.

## Decision

ZAMYAD stores deduplicated mesh resources as shared immutable BLAS objects and separates static and dynamic occurrences into independent TLAS layers. An ordered delta transaction creates a new immutable snapshot. Unchanged BLAS resources are shared; bounds-only changes refit the affected TLAS; topology changes rebuild only the affected layer. A quality ratio and consecutive-refit ceiling force deterministic rebuilds before traversal quality degrades.

VAYU consumes deterministic two-level snapshots. WGSL traverses TLAS instances and their referenced BLAS while applying inverse affine transforms and preserving exact 64-bit object, instance, mesh, category, triangle, and facing attribution. Geometry is partitioned into deterministic chunks under a caller-declared payload ceiling and streamed through one bounded resident working set under a hard GPU-memory budget.

Portable plans use a versioned, BLAKE3-keyed two-tier cache. Memory uses bounded LRU eviction. Disk entries carry magic, schema, key, length, and checksum; writes use temporary files plus atomic rename. Invalid entries are removed and treated as misses. Both tiers expose cumulative hit, miss, write, eviction, corruption, occupancy, and last-event telemetry.

Managed analyses use one bounded process-wide scheduler. Grasshopper captures Rhino inputs on the solution thread, performs native work off-thread, requests cancellation cooperatively, suppresses superseded generations, and exposes live queue, phase, fraction, cancellation, and outcome telemetry.

## Consequences

- Existing compute sessions safely retain their prior immutable scene when the source scene advances.
- Transform edits avoid BLAS reconstruction and can refit only the dynamic TLAS.
- GPU memory is bounded independently of total scene size, at the cost of repeated chunk uploads for streamed scenes.
- Cache data is an optimization only; schema or checksum failure never changes scientific results.
- CPU f64 remains canonical and Auto preserves explicit device-loss/runtime fallback behavior.
- Cancellation is immediate for queued work and at declared native/managed safe boundaries for running work; already-submitted GPU dispatches complete before their result is discarded.

## Rejected alternatives

- A single flattened GPU BVH: duplicates instanced geometry and turns transform edits into full rebuilds.
- Mutable scenes shared with readers: introduces race-prone identity and attribution changes.
- Unbounded cache directories or driver-specific pipeline blobs: violate reproducibility and recovery requirements.
- One `Task.Run` per component: provides no shared backpressure, stale-result policy, or observable queue.
