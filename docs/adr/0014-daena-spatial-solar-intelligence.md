# ADR 0014: DAENA Spatial and Solar Intelligence

Accepted for XVARNA 0.14.0.

## Context

The existing DAENA view metrics identified a dominant first blocker, but designers also need a volumetric spatial field, partial landmark visibility, reusable ranked attribution, exact design deltas, optical materials, and early solar-envelope constraints. These results must preserve the same stable ZAMYAD object identity across Rust, C, .NET, Rhino 8/9, and CLI workflows.

## Decision

XVARNA uses a validated immutable material library keyed by stable Object ID. Unassigned objects are opaque. Visible and direct-solar transmittance are independent fractions; diffuse reflectance is retained for reporting and future daylight/Radiance mapping. Energy conservation is enforced for both channels. Transmission is multiplied per geometric interaction and every removed fraction is assigned to the object at that interaction.

The 3D isovist uses a deterministic Fibonacci sphere with equal solid angle `4π/n`. It reports the radial volume integral `Σ(ωr³/3)`, radial surface measure `Σ(ωr²)`, transmission-weighted visible solid angle, openness, and an interleaved-half convergence delta. The radial surface measure is not represented as triangulated envelope area. Ranked obstruction and exact, non-overlapping category-mask partitions use attributed optical loss. Remove-one counterfactuals re-trace the same ray set while excluding one complete source object; they are not a subtraction estimate.

Landmarks are declared spherical proxies. DAENA applies camera FOV and analytic apparent solid angle, samples the apparent disk deterministically, and traces material-aware partial visibility. Spatial scenario comparison requires aligned viewpoint IDs and returns candidate-minus-baseline metric and object-attribution deltas.

HVARE solar scenario comparison keeps geometry, sensors, and schedule fixed while changing material libraries. It returns the full scenario/sensor/time transmission matrix, received/lost hours, ranked object and exact-category loss, and deltas from the first scenario.

The solar/shading envelope projects each protected sensor/sun ray through a vertical circular candidate column. Transmission-weighted quantiles produce the highest access-preserving elevation and lowest target-shading elevation, with the controlling sensor and UTC interval. This is a deterministic early-massing proxy, not a statutory construction envelope, daylight/radiosity calculation, or proof of code compliance.

## Consequences

- Stable source IDs connect numerical attribution directly to Rhino viewport highlighting.
- Top-K truncates only retained reporting rows; totals and category partitions are calculated before truncation.
- Exact category rows represent distinct bit-mask combinations and therefore do not double-count overlapping semantic labels.
- A closed mesh may create multiple optical interactions. Users must model thin glazing consistently or choose transmittance values that match the geometric representation.
- Scenario results are comparable only when their documented alignment requirements are satisfied.
- BLAKE3 identities include scene, material, policy, input, and result data.

## Rejected alternatives

- Axis-aligned voxel isovists: resolution and orientation bias obscure the angular sampling contract.
- First-hit-only transparency: cannot represent multiple glazing/context layers or allocate optical loss correctly.
- Approximate remove-one by deleting an attribution row: misses newly exposed downstream blockers.
- Unioned category totals: a multi-category occurrence would be counted more than once.
- An unrestricted buildable-envelope claim: the vertical-column method does not encode zoning law, façade thickness, reflections, or construction constraints.
