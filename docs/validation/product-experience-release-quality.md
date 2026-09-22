# Product experience and release-quality contract — 0.17

## Document-wide policy

`XV Quality` owns one weakly referenced policy per Grasshopper document. The four presets scale declared baseline samples within each component's scientific safety bounds. Experience mode is Basic or Expert; execution is Live with preset-specific debounce or Manual with an explicit run generation. A Manual solution never silently launches because upstream geometry changed. Scheduled components retain the last successful result, visibly mark it stale, and replace it only when the active generation completes.

`XV Units` records the active Rhino unit name, metres per model unit, model tolerance, and converted SI tolerance. Scene compilation and backend creation use this authoritative scale. `XV Diagnostics` combines these policies with scheduler, cache, backend, and recent document events into a shareable preflight report. Missing explicit setup is a notice with named defaults, not hidden state.

## Legend and evidence

`XV Legend` is a pure deterministic contract shared by viewport consumers. It rejects invalid fixed domains, ignores non-finite values when deriving a domain, makes non-finite samples transparent, and reports low/high clipping counts. Full range, percentile clipping, zero-symmetric, and fixed domains are supported with Viridis, Cividis, and diverging palettes.

Publication quality requires an evidence label. Current sampled flagship reports state their estimator and interleaved convergence diagnostic. These diagnostics measure numerical resolution stability; they are not statistical confidence intervals or guarantees of model validity.

## Freeform Solar/Shading Envelope

Every candidate is a base point and non-zero axis. The engine normalizes the axis and solves the closest points between it and each eligible protected solar ray. A constraint is retained only when the ray parameter and axis distance are positive, perpendicular separation is within Candidate Radius, and the context query is within Maximum Distance. Transmission through the assigned optical materials multiplies the interval weight.

Access and shading distances are transmission-weighted quantiles shifted by Axis Clearance. Endpoint previews use `base + normalized axis × distance`. Absolute endpoint Z remains in the ABI result for compatibility. A missing candidate axis is exactly World Z, so older files preserve their behavior and hash-equivalent explicit-Z inputs.

This remains early-massing guidance. It does not encode zoning law, arbitrary buildable solids, diffuse light, interreflection, structure, or construction tolerances.

## Automated release gate

`eng/release-quality.ps1` enforces three independent properties:

1. repeated parse/repair/serialize/unpack cycles remain byte deterministic;
2. repeated daylight analyses retain an identical scientific content hash;
3. deterministic checksum mutations and multiple truncations fail closed without producing a valid scene.

The same gate collects Coverlet Cobertura reports for .NET 8 and .NET 10 and rejects the build when the lower line rate is below 85%. `eng/check.ps1` and Windows CI invoke the gate. Rust formatting, warning-free Clippy, the complete workspace test corpus, C ABI layouts, CPU/GPU parity, and Rhino 8/9 isolated component loading remain separate mandatory checks.

Passing these gates is evidence about the tested corpus and machines. It is not a universal performance claim; multi-vendor GPU benchmarks and external architectural projects remain launch evidence requirements.

## Scalable visibility

Exact Visibility Graph retains one record for every unordered pair and is capped at 4,096 nodes. Sparse mode is a different, explicitly labelled contract. It requires a finite radius, assigns nodes to radius-sized 3D cells, searches the 27 adjacent cells, sorts local candidates by distance and stable index, then applies a global deterministic pass that caps both endpoint degrees before tracing.

Only retained candidate pairs appear in a sparse result. Degree and components describe that sparse visible graph; degree centrality retains the full `n−1` denominator. Full Brandes/harmonic centrality is opt-in and rejected above 4,096 nodes. Sparse input is capped at 100,000 nodes, candidate degree at 1,024, and output at 16 million pairs. A pathological point cloud packed into one cell can still require substantial neighbor discovery time; spatial indexing is not a claim of constant-time behavior.
