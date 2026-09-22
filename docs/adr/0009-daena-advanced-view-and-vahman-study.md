# ADR 0009: DAENA advanced view and VAHMAN Study

## Status

Accepted for XVARNA 0.9.0.

## Decision

DAENA owns view evidence: solid-angle Target View, explicit Weighted View, Green View Index, protected View Corridors, and Dynamic Observer Paths. VAHMAN—named for *Vohu Manah*, “good mind”—owns completed-study ranking and mixed-variable constrained optimization. Individual metrics do not receive separate mythological names; one stable name per coherent subsystem keeps the API understandable.

All geometry tracing remains in the reusable double-precision ZAMYAD scene. DAENA returns raw, weighted, green, convergence, and blocker-attribution channels separately. It never hides desirability assumptions inside raw visibility. VAHMAN consumes arbitrary externally evaluated objectives, including DAENA results, without depending on Rhino or Grasshopper.

The optimizer uses an explicit `ask`/`tell` boundary. `ask` produces bounded continuous, integer, and categorical candidates. The host evaluates geometry and analysis. `tell` accepts every objective and residual constraint, where a residual is feasible at `<= 0`. This avoids embedding a brittle geometry evaluator in the optimizer and makes expensive Grasshopper studies reproducible.

The initial production algorithm is constraint-domination plus NSGA-II environmental selection, stratified initialization, tournament selection, simulated-binary crossover, polynomial mutation, discrete mutation, crowding preservation, and a bounded historical Pareto archive. It is deterministic for the same problem, seed, evaluation values, and evaluation order. Parallel work is limited to independent dominance comparisons; ordering and hashing remain deterministic.

## Consequences

- `xvarna-daena` remains independent of the connector and exposes all five view workflows in Rust.
- New `xvarna-study` provides ranking, Spearman diagnostics, exact two-objective hypervolume, and stateful optimization.
- The C ABI grows additively to 0.9; fixed layouts are asserted in Rust, C, and .NET.
- .NET 8 and .NET 10 expose the same source-unit-aware API.
- Rhino 8 and Rhino 9 receive identical component GUIDs and semantics.
- Grasshopper optimization is split into explicit Start and Step components so external objectives can be evaluated between generations without hiding state transitions.
- No result is described as a global optimum, causal sensitivity, confidence interval, legal view-right determination, health outcome, or regulatory compliance result.

## Rejected alternatives

- Pixel counting from rendered screenshots: dependent on display resolution, clipping, materials, and renderer state; it also loses first-blocker identity.
- Projected target area only: not a view metric and cannot represent partial occlusion.
- A single opaque “view quality” score: prevents audit of weighting assumptions.
- Python-only evolutionary loops in the connector: easy to prototype but weaker for packaging, deterministic state, ABI reuse, and Rhino 8/9 parity.
- Claiming a globally optimal solution from a heuristic finite-budget search: scientifically indefensible.
