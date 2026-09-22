# XV Scene and XV Ray Query

## Purpose

`XV Scene` turns repeated Rhino meshes into one immutable query snapshot. Identical packed meshes are content-hashed and stored once; every occurrence retains its own affine transform, exact object/instance identifiers, category mask, and `Static` or `Dynamic` acceleration layer. Geometry is normalized to metres and internally rebased around the scene bounds before acceleration data is built.

`XV Ray Query` reuses that snapshot across ordered ray batches. Closest mode returns model-unit distance, point, object/instance/mesh/triangle IDs, barycentric U/V, and front-facing state. Any Hit mode stops on the first valid occluder and is intended for visibility predicates.

## Data matching

- Meshes: one or more Rhino meshes; quads are deterministically triangulated on a copy.
- Transforms: empty means identity, one broadcasts, otherwise count must equal mesh count.
- Object IDs: empty creates `1..N`; one value is used as a starting ID; otherwise count must equal mesh count.
- Category masks: exact unsigned 64-bit text; empty or `-1` means all categories; one broadcasts; otherwise count must equal mesh count.
- Layers: empty means `Static`; one broadcasts; otherwise count must equal mesh count. Use `Dynamic` for occurrences expected to move interactively.
- Ray directions: one broadcasts to every origin; otherwise count must equal origin count.

## Numerical contract

Grasshopper inputs and outputs use the active Rhino model unit. The managed connector converts positions, translations, ray origins, and ray bounds to canonical metres. The Rust engine keeps geometry, transforms, traversal, barycentrics, and distances in `f64`. An internal scene-centre rebase limits precision loss for large world coordinates without changing source IDs or returned model-space distance.

## Lifecycle and concurrency

The output is an opaque managed object owning a native handle. Building freezes the scene; updates publish a new snapshot and never mutate readers already holding the prior revision. Queries clone an internal shared reference and can execute concurrently. When geometry/build policy is unchanged, Grasshopper sends one atomic instance delta transaction: unchanged BLAS resources are shared and each layer independently chooses TLAS reuse, refit, or rebuild from the quality/periodic policy. An identical input reuses the snapshot directly; changed mesh geometry performs a full connector rebuild. `Revision` changes only after a committed transition and `Lifecycle` exposes the decision. `SafeHandle` provides final cleanup if a document or component is abandoned.

## Failure behavior

Invalid/degenerate/non-finite meshes must be repaired with `XV Mesh Check` first. Singular or projective transforms, invalid ray bounds, zero directions, unknown resource IDs, invalid handles, and incorrect ABI layouts fail explicitly. Rust panics are caught before crossing the C boundary.
