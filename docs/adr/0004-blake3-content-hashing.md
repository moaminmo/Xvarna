# ADR 0004: BLAKE3 for canonical geometry identity

- Status: Accepted
- Date: 2026-08-30

## Context

Scene caching, reproducibility, mesh deduplication, and repair provenance require a deterministic content identity. A platform-dependent standard-library hash is unsuitable, while a slow cryptographic hash would add unnecessary ingest cost.

## Decision

Canonical geometry uses BLAKE3 1.8.7. The v1 mesh preimage begins with the domain separator `XVARNA_MESH_V1\0`, followed by little-endian counts, canonical f64 coordinate bits, and u32 triangle indices. Negative zero is normalized to positive zero. Face order and winding remain semantic and therefore change the hash.

Schema/domain changes require a new separator and migration policy. Hashes identify canonical bytes; they do not claim two geometrically equivalent but differently indexed meshes are identical.

## Consequences

- Hashes are deterministic across supported hosts and suitable for cache keys.
- Repair reports can prove that output geometry differs from input.
- Semantic canonicalization remains a separate explicit step.
- BLAKE3 becomes a small audited native dependency.
