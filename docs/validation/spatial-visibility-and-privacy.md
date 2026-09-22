# DAENA spatial visibility and privacy contract

DAENA is XVARNA's deterministic spatial-visibility subsystem. The Avestan name *daēnā* carries the senses of vision, conscience, and individuality; the name is used here for a subsystem whose outputs explain what can be seen, by whom, at what distance, and which source object prevents a view.

Version 0.8 implements three connected workflows over the immutable double-precision ZAMYAD scene: sampled planar isovists, a directed observer-target matrix with privacy-risk screening, and an undirected visibility graph. All three retain deterministic BLAKE3 identity and category-filtered first-hit attribution.

## Coordinate, identity, and query contract

- Rust and the C ABI use canonical metres. The .NET and Rhino APIs convert positions, clearances, distances, areas, and convergence deltas to and from the source model unit declared when the scene was built.
- Object, instance, mesh, sensor/viewpoint, target, and triangle identity are never inferred from output order.
- A zero category mask and an all-bits mask both select all categories at the native scene layer. Public tools use all bits as the default.
- Every line-of-sight segment removes the declared clearance at both endpoints. This prevents an observer or target surface from blocking itself without moving the reported source positions.
- Exact ray/triangle decisions use the same ZAMYAD BLAS/TLAS path as solar and sky analysis.

## Planar isovist

For viewpoint position `p`, plane normal `n`, projected forward vector `f`, and in-plane right vector `r = normalize(f × n)`, sample `i` has direction:

```text
d_i = normalize(f cos(theta_i) + r sin(theta_i))
```

A full 360° study samples `[0, 2π)` without duplicating the first ray. A partial field of view includes both angular endpoints and closes its output polygon through the viewpoint. Each ray terminates at the closest hit or at the required finite maximum distance. Open-at-limit rays therefore represent clipped unknown/open space, not infinity.

The ordered boundary yields:

- planar polygon area, perimeter, and eye-to-centroid distance;
- radial minimum, mean, maximum, population standard deviation, and standardized third moment (skewness);
- compactness `4πA/P²`, clamped to `[0,1]` for numeric safety;
- occluded/open counts and the object with the largest angular sample share;
- convergence diagnostic `|A_N - A_half|`, where `A_half` uses every second ordered sample.

This is a sampled polygonal approximation. A small convergence delta is evidence of angular stability for that input and range; it is not a universal error bound. Thin occluders can be missed when angular resolution is too low. Distance clipping can dominate area and compactness when rays remain open.

## Directed intervisibility and privacy screening

DAENA evaluates every ordered observer-target pair. Results are `Visible`, `Blocked`, `OutOfRange`, or `Coincident`; blocked entries retain the first object, instance, mesh, and triangle. Row and column summaries expose visible fraction, target exposure, and risk aggregation.

For a visible pair only:

```text
facing = 1                                      (omnidirectional target)
facing = max(0, n_target · direction_target_to_observer)^exponent
distance = 1 / (1 + (pair_distance / reference_distance)^2)
risk = observer_weight × target_sensitivity × facing × distance
```

Weights and sensitivity are restricted to `[0,1]`. Pair risk is therefore bounded by `[0,1]`. Target combined risk is the complement product `1 - Π(1-risk_i)`. This is a convenient multi-observer screening proxy under an independence-like aggregation assumption; it is not a measured probability of surveillance, discomfort, occupancy, or harm. Cumulative risk is also returned without clipping so optimization can preserve additive pressure.

## Visibility graph

For `N` input nodes, DAENA traces each of the `N(N-1)/2` unordered segments once. Directly visible pairs become undirected graph edges. Blocked and out-of-range pairs remain in the result for audit and blocker mapping.

The graph returns degree, normalized degree centrality, deterministic connected-component labels, normalized harmonic closeness over hop distance, and normalized undirected Brandes betweenness. Centrality can be disabled when only adjacency is needed. Version 0.8 applies a hard 4,096-node and 16,000,000-result-entry safety cap; the all-pairs method is intentionally exact rather than pretending to be sparse at large scale.

## Determinism and resource policy

Input order defines stable row, column, and pair order. BTree ordering resolves dominant-occluder ties by stable object ID. Hashes cover scene identity, endpoints and frames, all numeric policy, state, distance/risk where relevant, and blocker attribution. Native batch tracing may run in parallel, but ordered collection keeps results and hashes deterministic for identical binaries and inputs.

## Validation evidence

The automated suite includes:

- an analytic square room whose 1,440-ray isovist converges to area `4 m²` and perimeter `8 m`;
- full first-object attribution in that room;
- a divider wall that blocks a directed pair and separates a visibility graph into two components;
- category-mask exclusion that makes the same divider transparent;
- directional, distance-weighted privacy output bounded by one;
- normalized Brandes betweenness on a three-node path;
- C/Rust structure-layout checks and model-unit round trips in both .NET 8 and .NET 10;
- Rhino 8 runtime component construction and Rhino 9 SDK compilation.

## Non-claims and current boundaries

- DAENA 0.8 is not a regulatory privacy, safety, security, accessibility, or view-rights compliance tool.
- The isovist is 2D in an arbitrary oriented plane; it is not a 3D visibility volume.
- Meshes are opaque. Transparency, material transmission, vegetation porosity, refraction, and reflection are not modeled.
- Pair visibility tests a zero-radius segment, not a finite pupil, body, aperture, or target surface.
- Privacy results do not infer occupancy, identity, intent, camera optics, dwell time, or social context.
- Visibility-graph centrality describes the supplied sample nodes and range policy, not continuous navigable space.

## References

- Michael L. Benedikt, “To Take Hold of Space: Isovists and Isovist Fields,” *Environment and Planning B*, 1979: [author-hosted scan](https://mbenedikt.com/computational-models-of.pdf).
- [depthmapX About](https://github.com/SpaceGroupUCL/depthmapX/blob/master/docs/about.md) and [command-line isovist documentation](https://github.com/varoudis/depthmapX/blob/master/docs/commandline.md), used to compare established isovist/visibility-graph workflows and export expectations.
- [Ladybug Tools View Percent documentation](https://docs.ladybug.tools/ladybug-primer/components/3_analyzegeometry/view_percent), used to compare designer-facing radial view modes and documented scaling constraints.
- [Encyclopaedia Iranica: DAĒNĀ](https://www.iranicaonline.org/articles/den/), naming reference.
