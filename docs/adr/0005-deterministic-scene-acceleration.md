# ADR 0005: Deterministic CPU scene acceleration

## Status

Accepted — 2026-08-31.

## Decision

The baseline query engine uses immutable scene snapshots. Unique meshes own flattened bottom-level BVHs; affine occurrences are indexed by a top-level BVH. Both levels use deterministic 16-bin surface-area-heuristic splitting and a configurable leaf size. Traversal and triangle intersection use double precision. A scene-owned Rayon pool executes sufficiently large ordered batches; small batches stay serial to avoid scheduling overhead.

Meshes are deduplicated only when both the BLAKE3 digest and canonical geometry match. Instances carry stable object, occurrence, resource, category, and triangle attribution. Source coordinates are converted to metres and world transforms are rebased around scene bounds before TLAS construction.

## Rationale

This provides a portable, independently testable CPU baseline for Rhino 8, Rhino 9, CLI, and future connectors. It avoids coupling core correctness to Rhino document state or a GPU vendor. Deterministic layout and hashes make caching, benchmark comparison, bug reproduction, and research reporting possible. A future GPU backend must preserve the same public result contract and be validated against this path.

## Consequences

- Scene compilation has an explicit cost and produces immutable state.
- Repeated queries amortize compilation and reuse acceleration structures.
- Transform-only incremental refit is deferred; version 0.3 performs a full deterministic rebuild.
- Any-hit uses early exit; closest-hit preserves full attribution.
- Benchmarks must report build and query time separately and include differential mismatch counts.
