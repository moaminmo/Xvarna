# DAENA advanced view and VAHMAN optimization contract

## Scope

XVARNA 0.9 introduced five connected DAENA workflows and one independent study engine. XVARNA 0.11 executes the three ray-intensive advanced-view workflows through either canonical CPU or VAYU without changing these definitions:

- Target View: how much target solid angle is directly visible inside an oriented rectangular camera field of view.
- Weighted View: the same visible solid angle multiplied by declared target desirability, distance, view-centre direction, and observer weights.
- Green View Index: visible solid angle from target categories selected by an explicit green mask.
- View Corridor: the unblocked fraction of a protected circular target aperture plus first-conflict attribution.
- Dynamic Observer Path: distance-ordered view metrics along tangent-facing polyline motion.
- VAHMAN Study: constraint-aware Pareto ranking, diversity, descriptive sensitivity, hypervolume, and mixed-variable ask/tell optimization.

All distances are canonical metres in Rust/C. The .NET and Grasshopper connectors convert active Rhino model units at the boundary.

## Solid-angle model

For an eye and triangle vectors `a`, `b`, and `c`, DAENA uses the exact unsigned triangular solid angle:

```text
Ωtriangle = 2 atan2(
  |a · (b × c)|,
  |a||b||c| + (a·b)|c| + (b·c)|a| + (c·a)|b|
)
```

For horizontal and vertical rectangular camera angles `h` and `v`, with `x = tan(h/2)` and `y = tan(v/2)`:

```text
ΩFOV = 4 atan2(xy, sqrt(1 + x² + y²))
```

The engine samples each target triangle with a deterministic Hammersley sequence. Sample contributions are normalized so their sum equals the exact triangle solid angle. This allows partial occlusion while preserving the exact unoccluded angular measure. A closest-hit ZAMYAD ray records the blocking object. The full result is compared with the even interleaved half-sample estimate; their absolute solid-angle difference is exposed as convergence, not hidden.

For one observer:

```text
TargetView        = Ωvisible targets / ΩFOV
TargetVisibility  = Ωvisible targets / Ωpotential targets in FOV
GreenViewIndex    = Ωvisible green targets / ΩFOV
GreenShare        = Ωvisible green targets / Ωvisible targets
```

Weighted View keeps the raw metric intact and applies only declared factors:

```text
wdistance(d) = 1 / (1 + (d / d50)^p)
wdirection   = max(0, forward · direction)^q

WeightedView = Σ(Ωvisible sample × categoryWeight × wdistance
                 × wdirection × observerWeight) / ΩFOV
```

`d50`, `p`, `q`, every category weight, category mask, FOV, maximum distance, endpoint clearance, sidedness, and sample count are result inputs and participate in deterministic identity.

## View Corridor

A corridor traces deterministic golden-angle, uniform-area samples over a circular aperture perpendicular to the origin-to-target axis. For axis distance `d` and aperture radius `r`:

```text
Ωaperture = 2π (1 - d / sqrt(d² + r²))
OpenFraction = open samples / all samples
OpenSolidAngle = Ωaperture × OpenFraction
```

The result includes open/blocked state per sample, aperture point, ray direction, first-hit distance, object/instance/mesh/triangle identity, dominant blocker share, nearest blocker, and full-versus-interleaved-half convergence. It is an evidence layer for design review, not a legal determination of a protected view corridor.

## Dynamic Observer Path

Each non-zero polyline segment supplies the tangent camera direction. DAENA creates uniform accumulated-distance samples with maximum spacing chosen by the caller, including both endpoints. The target-view engine is evaluated at every sample. Path means use trapezoidal distance weights, so changing vertex density without changing path geometry does not arbitrarily reweight the result. Minimum/maximum Weighted View and their sample indices identify hotspots.

No time model is inferred. If time weighting is required, the caller must supply a speed/time mapping outside this version.

## VAHMAN Study and optimization

Every objective declares minimize or maximize. Every constraint is a residual with feasibility at `g(x) <= 0`. Ranking follows Deb-style constraint domination:

1. A feasible solution dominates an infeasible solution.
2. Between infeasible solutions, smaller summed positive residual violation is preferred.
3. Between feasible solutions, ordinary multi-objective Pareto dominance is used.

Non-dominated sorting assigns zero-based fronts. NSGA-II crowding preserves objective-space diversity; boundary solutions have infinite crowding. The historical archive retains feasible rank-zero solutions under a declared capacity.

The optimizer supports bounded continuous, integer, and zero-based categorical variables. Initial candidates use deterministic stratification. Later candidates use crowding-aware tournament selection, simulated-binary crossover for numeric variables, polynomial mutation, and discrete mutation. The same problem, seed, evaluation values, and call order produce the same candidates and BLAKE3 state hash.

Exact dominated hypervolume is available only for two objectives and a caller-supplied reference point that is worse in the original objective directions. Spearman `ρ` is descriptive rank association only; it is not causal sensitivity and carries no confidence interval in 0.9.

## Resource limits and failure behavior

- Target samples per triangle: 1–4,096.
- Total target rays: hard bounded in the core; oversized products fail before tracing.
- Corridor samples: 1–1,048,576 per corridor, subject to global result limits.
- Observer-path samples: hard capped at 1,000,000.
- Study variants: at most 4,096; variables 256; objectives 32; constraints 256.
- Invalid frames, non-finite values, degenerate triangles, inconsistent dimensions, duplicate IDs, incomplete `tell`, and capacity errors fail explicitly.

## Backend-neutral execution in 0.11

Target/Weighted/Green View, View Corridor, and Dynamic Observer Path generate ordered domain-scale closest-hit batches through a shared `RayQueryExecutor`. Direct scene calls use canonical f64 ZAMYAD. Compute-session calls use VAYU portable f32 traversal when its measured precision/resource policy succeeds, with explicit Auto fallback or required-GPU failure.

The metric engine remains shared: sampling, solid angle, weighting, convergence, path aggregation, blocker attribution, and hashes are not reimplemented in WGSL. Every accelerated result includes backend, adapter, batch/ray/dispatch totals, transfer/execution/readback timing, maximum precision error, and fallback provenance. The detailed parity and batching contract is [VAYU DAENA Domain Acceleration](vayu-domain-acceleration.md).

## Verification delivered in 0.9

- Analytic solid-angle, weighting separation, green category, partial corridor, dynamic path, ranking, hypervolume, sensitivity, mixed-variable reproducibility, and ask/tell lifecycle tests.
- Fixed-layout tests in Rust plus C `_Static_assert` coverage and .NET layout guards.
- .NET 8 and .NET 10 boundary tests using millimetre source models.
- CLI parser and deterministic optimizer smoke tests.
- Rhino 8 runtime loading checks for all twenty-one components and duplicate GUID rejection.

## Verification added in 0.11

- Generic executor and aggregate-provenance unit tests in Rust.
- High-level C ABI fixed-layout coverage and dual-runtime .NET tests for CPU plus conditional real GPU execution.
- CLI CPU-oracle parity for all three domain workflows with numeric tolerance, exact state/identity comparison, and fail-closed exit status.
- Rhino 8/9 compile coverage for dual Scene/Backend inputs without GUID or output-index changes.
- Real-device multi-dispatch smoke runs on NVIDIA RTX 5060 and Intel Raptor Lake/Vulkan with zero observed metric and identity/state mismatch.

## Non-claims

The release does not claim validated human visual perception, preference prediction, causal design importance, global optimality, legal view rights, universal green-view health thresholds, confidence intervals, universal GPU speedup, or equivalence to a renderer. Results are deterministic geometric and optimization evidence under declared inputs.
