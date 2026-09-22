# ADR 0018: Evidence passports and transparent multi-fidelity selection

Status: accepted for 0.18.0 on 2026-09-04.

## Context

XVARNA can evaluate many design variants quickly, while Radiance and other reference solvers are substantially more expensive. A fast result without immutable provenance is difficult to audit; running every variant at reference fidelity wastes computation; and descriptive rank correlation is not a substitute for a generated global-sensitivity experiment.

## Decision

RASHNU owns a versioned Evidence Passport containing result/scene/input identities, method, fidelity, backend/device/tool version, deterministic seed, effective sample count, runtime, assumptions, limitations, citations, and parity/reference diagnostics. A domain-separated BLAKE3 seal rejects later mutation.

VAHMAN adds three distinct inference contracts:

1. Saltelli cross-sampled `A`, `B`, and `A_B(i)` designs analyzed with a Saltelli first-order and Jansen total-order estimator; Morris uses even-level randomized one-at-a-time trajectories and reports μ, μ*, and σ. Results are accepted only when row count, row identity, dimensions, method layout, and design hash match.
2. Robust scenario aggregation reports weighted mean, population deviation, direction-aware worst value and upper-loss-tail CVaR, plus probability of any residual-constraint violation. Interval ranking requires non-overlapping confidence bounds and orders proven, possible, and infeasible candidates conservatively.
3. Multi-fidelity selection fits a transparent per-objective autoregressive linear discrepancy model from aligned Fast/Reference pairs. It propagates fitted residual and scaled Fast observation noise. For exactly two objectives it estimates expected hypervolume improvement by deterministic Monte Carlo and divides by observed Reference cost. Above two objectives it returns an explicitly labelled uncertainty-per-cost proxy.

## Research boundary

The selector is inspired by noisy and cost-aware Bayesian optimization, but it is not qNEHVI, MF-HVKG, a Gaussian process, or a joint-batch fantasy model. Those names are never emitted as implemented methods. This transparent baseline is independently testable and can later serve as the policy interface for a version-pinned surrogate backend.

## Consequences

- Rust, C, .NET 8/10, CLI, and three Grasshopper components share one native implementation.
- Every generated sensitivity design and result has deterministic identity.
- Reference jobs can be prioritized without hiding the calibration slope, residual error, R², expected improvement, or cost.
- External project evidence, AMD hardware evidence, and independent reviewer sign-off remain empirical launch gates rather than software claims.
