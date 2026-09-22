# ZAMYAD/VAYU 0.13 Lifecycle Validation Contract

This document defines what XVARNA 0.13 means by incremental scene lifecycle, two-level GPU instancing, bounded streaming, recoverable caching, and shared scheduling. It is a correctness contract, not a universal performance claim.

## Scene lifecycle invariants

- Every successful delta transaction is atomic and produces a deterministic new content hash.
- A failed delta leaves the published registry snapshot unchanged.
- Existing readers and compute sessions retain the prior immutable snapshot.
- Transform-only edits reuse every BLAS and refit only affected static/dynamic TLAS layers.
- Add/remove/layer-transfer operations rebuild only layers whose topology changed.
- Replacing a mesh preserves its `MeshId`, rebuilds exactly that resource BLAS, and updates layers containing its instances.
- Refit is replaced by rebuild when the declared quality ratio or consecutive-refit ceiling is crossed.

Analytic Rust tests cover unchanged reuse, dynamic-only refit, topology rebuild, single-resource replacement, old-snapshot stability, refit bounds, and ray attribution after updates. C ABI and dual-runtime .NET tests exercise the same transition and optimistic registry publication.

## GPU two-level and chunking invariants

- Portable snapshots store unique triangles and BLAS nodes once per resident chunk rather than expanding every occurrence.
- TLAS leaves reference instances; instances reference BLAS roots and carry inverse transforms, layer, category, mirror state, and exact identity words.
- WGSL performs bounded iterative TLAS then BLAS traversal and matches canonical CPU hit state and object/instance/mesh/triangle identity.
- Chunk order and packing are deterministic for a scene hash and policy.
- No resident scene payload exceeds `MaximumGeometryChunkBytes`.
- Geometry plus retained ray scratch cannot exceed `MaximumGpuMemoryBytes`; invalid budgets fail before dispatch.
- Device loss marks the session unhealthy. Auto falls back to canonical CPU; required GPU reports failure.

The GPU parity test forces more than one geometry chunk on the available adapter, traces a deterministic corpus, and requires zero state and identity mismatches within the declared distance tolerance.

## Cache invariants

- Keys include the scene content hash, portable precision policy, hierarchy/chunk schema, and execution-plan version.
- Memory and disk budgets are independent and may be zero.
- Memory eviction is LRU. Disk eviction is deterministic oldest-entry removal under the configured byte budget.
- Disk writes use a same-directory temporary file and atomic rename.
- Header magic, schema, key, declared length, and BLAKE3 checksum are validated before decode.
- Truncated, modified, unknown-schema, or semantically invalid plans are removed and recomputed as misses.
- Clear removes only XVARNA `.xvc` entries under the configured directory.

Rust tests cover memory hit, disk hit after memory eviction, eviction budgets, checksum corruption, recovery, and exact portable-plan round trip. The public C/.NET layouts and Grasshopper `XV Cache` expose the same counters and active budgets.

## Scheduler invariants

- One bounded process-wide queue governs compute-heavy Grasshopper analyses.
- Maximum concurrency and queue capacity are explicit and can change only while idle.
- Input data is captured on the Grasshopper solution thread; native work does not access live Rhino geometry.
- Cancellation is linked to Grasshopper task cancellation. Queued work stops immediately; running work observes safe phase or native chunk boundaries.
- A newer generation for the same component scope suppresses publication of the older result.
- Live job ID, name, state, phase, fraction, cancellation state, queue latency, and terminal counters are observable from `XV Scheduler`.

Managed tests cover completion/progress, cooperative cancellation, stale-generation suppression, and dual .NET 8/.NET 10 behavior.

## Release gates

The milestone can be labeled complete only when all of the following pass:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. C11 public-header layout assertions
5. .NET 8 and .NET 10 managed test suites
6. Rhino 8 and Rhino 9 connector builds with identical component GUIDs
7. Multi-chunk GPU/CPU parity on at least one available adapter
8. Cache cold/write, memory-hit, disk-hit, eviction, and corruption-recovery tests
9. Package creation with ABI 0.13.0 and SHA-256 manifests

Vendor coverage beyond locally available adapters remains evidence to publish separately; absence of an AMD or Intel/NVIDIA machine is never represented as a passing hardware result.
