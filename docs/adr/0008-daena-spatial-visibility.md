# ADR 0008: DAENA spatial visibility on the canonical scene

## Status

Accepted for XVARNA 0.8.0.

## Decision

Implement planar isovists, directed intervisibility/privacy, and undirected visibility graphs in a new dependency-light Rust crate, `xvarna-daena`, over immutable `xvarna-scene::Scene` queries. Expose fixed caller-owned result arrays through additive C ABI functions, then keep .NET and Grasshopper connectors thin and unit-aware.

Use one exact first-hit source-identity contract for all three analyses. Preserve full pair states instead of exporting only successful edges. Keep the privacy equation transparent, bounded at pair level, configurable, and explicitly labeled as a screening proxy. Compute visibility-graph components and selected centralities in Rust so CLI, C, .NET, Rhino 8, and Rhino 9 cannot diverge.

## Rationale

ZAMYAD already provides deterministic, parallel, double-precision batch queries, category masks, instancing, origin rebasing, and source attribution. Reusing it avoids a second geometric truth. Rust is materially useful here for bounded allocation, stable layouts, fast batch traversal, and one cross-host implementation; it is not chosen as decoration.

The 0.8 milestone groups three features because together they form a complete design workflow: local spatial field, directed view/privacy question, and network-scale topology. Each remains independently callable.

## Consequences

- Planar isovists are sampled approximations with a mandatory finite range and visible convergence diagnostic.
- Exact graph construction remains quadratic and is protected by explicit caps.
- Opaque zero-radius ray semantics exclude material transmission and finite target apertures.
- The ABI grows additively to 0.8 without changing existing structure layouts or symbols.
