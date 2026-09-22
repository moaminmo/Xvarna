# ADR 0017: Canonical scene, geometry interoperability, and weather policy

- Status: accepted for the 0.17 development line
- Scope: XVARNA Core, ZAMYAD, ZURVAN, CLI

## Decision

XVARNA separates authoritative source data from derived acceleration data. A `SceneDocument` stores canonical-metre f64 meshes, stable mesh/object/instance identities, affine transforms, category masks, static/dynamic layer membership, and portable build policy. BVHs, GPU buffers, cache entries, thread pools, and device details are rebuilt rather than serialized as truth.

The JSON contract uses `https://xvarna.dev/schemas/scene`, integer schema version `1`, and a tracked JSON Schema. The binary `.xvscene` envelope contains magic, envelope version, payload length, BLAKE3 checksum, and canonical compact JSON. Corrupt, truncated, oversized, unknown-version, unknown-resource, non-affine, and invalid-unit documents fail before scene compilation. Version-zero JSON migrates deterministically by assigning canonical metres, static layers, and the all-category mask.

Mesh interchange supports OBJ, ASCII/binary STL, ASCII/little-endian PLY, embedded glTF 2.0, and GLB 2.0. glTF conversion consumes indexed/non-indexed TRIANGLES, multiple primitives, active scenes, node hierarchies, matrix or TRS transforms, buffer views, accessor offsets/strides, and unsigned index widths. The byte API never fetches external URIs; callers must embed buffers or use GLB, preventing hidden network/file access.

Mesh mutation is opt-in. The default importer preserves geometry. Healing first removes invalid/duplicate/degenerate faces, then optionally performs deterministic tolerance-grid welding, removes newly collapsed faces, propagates consistent winding over manifold adjacency, and orients closed components by signed volume. ROI extraction clips triangles against all six AABB planes rather than selecting only centroids or bounds.

ZURVAN treats missing values, leap days, gaps, and occupancy as declared policy. WEA is converted into the same analysis weather model used by EPW; direct/diffuse irradiance is integrated over the inferred interval and GHI is reconstructed with the NREL-SPA solar direction. The XVARNA weather CSV is a lossless, versioned time-series interchange. Filters and weekly occupancy produce source indices, weighted `TimeSample` values, weighted hours, and a deterministic content hash.

## Consequences

- Scene files survive backend, driver, BVH, and cache changes.
- Migration tests become mandatory before changing the scene schema.
- STL, PLY, glTF, and GLB no longer need Rhino conversion as an intermediate step.
- glTF images, materials, animation, skinning, morph targets, Draco, sparse accessors, and implicit external-file loading are outside this geometry-analysis path and fail or are ignored explicitly rather than silently interpreted.
- WEA-derived GHI is reconstructed evidence, not measured source data, and provenance must retain `Radiance WEA`.
