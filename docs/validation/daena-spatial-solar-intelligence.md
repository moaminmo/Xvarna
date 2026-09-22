# DAENA/HVARE 0.14 Spatial and Solar Intelligence Validation Contract

This document defines the implemented 0.14 scientific contract and its claim boundary. It is a correctness contract, not evidence that XVARNA universally outperforms another product.

## Material and attribution invariants

- Material IDs are unique and non-zero; assignments are unique by Object ID and must reference an existing material.
- Visible transmittance, solar transmittance, and reflectance are finite fractions. Visible-plus-reflectance and solar-plus-reflectance cannot exceed one.
- Unassigned objects are opaque. Transmission and attributed loss are evaluated per ordered geometric interaction.
- General top-K rows are deterministically ordered by descending attributed loss and then Object ID.
- Exact category-mask partitions are mutually exclusive and sum to total attributed loss within floating-point tolerance.
- A remove-one row re-traces the original rays while excluding every occurrence of that Object ID and can reveal downstream blockers.

## 3D isovist invariants

- Fibonacci samples have equal solid angle and cover the complete sphere.
- An unobstructed sphere clipped at radius `R` approaches `4πR³/3` volume, `4πR²` radial surface measure, `4π` visible solid angle, and openness one.
- A blocked ray ends at its first interaction while its visible-solid-angle contribution retains multi-layer optical transmission.
- The convergence value is the absolute difference between the full ray set and its deterministic interleaved-half estimate.
- Volume and distance are returned in source model units by .NET/Rhino and canonical metres by Rust/C.

## Landmark invariants

- Camera forward/up vectors are orthonormalized and invalid frames fail explicitly.
- Landmark-centre FOV inclusion, analytic spherical apparent solid angle, deterministic disk coverage, partial transmission, and importance weighting are separate reported quantities.
- Entries are observer-major and landmark-major; ranked source attribution retains both observer and target identity.

## Scenario invariants

- Spatial comparison rejects missing or differently ordered viewpoint IDs.
- Spatial metric deltas and attribution deltas are always candidate minus baseline.
- Solar comparison uses the first scenario as baseline and returns a complete scenario/sensor/time state matrix.
- Inactive and back-facing intervals are distinguished from evaluated zero-transmission intervals.
- Received plus attributed lost hours equals eligible scheduled hours within floating-point tolerance when the configured material-layer cap is not reached.

## Solar-envelope invariants and limits

- Only active, front-facing sun intervals whose horizontal ray projection crosses the declared candidate radius create constraints.
- Existing context transmission weights every constraint before deterministic elevation sorting.
- Access and shading thresholds are weighted quantiles with explicit vertical clearance, and retain their controlling sensor and UTC time.
- Infinite thresholds mean that no applicable projected constraint exists; they are not silently replaced with arbitrary heights.
- Results screen vertical circular columns at candidate XY locations. They do not model arbitrary solids, reflected radiation, diffuse sky, zoning rules, or legal rights-to-light compliance.

## Automated evidence

Rust analytic tests cover transparent multi-layer tracing, 3D open/blocked volume, exact remove-one recovery, category partitioning, partial landmark visibility, aligned scenario deltas, solar material alternatives, baseline deltas, and envelope control rays. Fixed-layout C assertions cover every new ABI record. .NET 8 and .NET 10 tests invoke the native ABI and verify source-unit conversion plus the same material, landmark, scenario, and envelope boundaries.

## Release gates

0.14 can be labeled complete only when all of the following pass:

1. Rust formatting, workspace Clippy with warnings denied, and all workspace tests.
2. C11 public-header compile and fixed-layout assertions.
3. .NET 8 and .NET 10 build and managed/native tests.
4. Rhino 8 and Rhino 9 connector builds with 33 unique stable component GUIDs.
5. Runtime instantiation on each locally installed Rhino host; an absent host must be reported as untested, never passed.
6. CLI smoke execution for `isovist-3d`, `landmark-visibility`, `solar-scenarios`, and `solar-envelope`.
7. Release packages with ABI 0.14.0 and SHA-256 manifests.

Real-project validation, independent user studies, competitor benchmarks, and Intel/NVIDIA/AMD coverage remain separate evidence required before broader superiority claims.
