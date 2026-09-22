# Scientific evidence and multi-fidelity validation

Version: 0.18.0. Scope: RASHNU Evidence Passport, global sensitivity, robust uncertainty, and VAHMAN Fast/Reference selection.

## Enforced invariants

`EvidencePassport::seal` validates required identities, finite non-negative errors, consistent parity/reference flags, and non-empty assumptions/limitations/citations before computing a domain-separated BLAKE3 content hash. `validate` recomputes that identity, so any later field mutation fails closed.

Saltelli/Jansen generation uses domain-separated deterministic streams for independent `A` and `B` matrices and emits `N × (k + 2)` rows. Morris emits `r × (k + 1)` even-level OAT trajectories with explicit trajectory and changed-parameter identity. Analysis rejects a changed hash, malformed method layout, bad variable types/bounds, missing rows, non-finite outputs, or zero-variance normalization.

The analytic additive model `y = x1 + 2*x2`, with independent uniform inputs, has first/total Sobol indices 0.2 and 0.8. Rust tests and `eng/smoke-evidence.ps1` require the generated estimator to recover these within declared Monte-Carlo tolerances. Morris tests recover exact physical-unit gradients +3 and −2 for a linear model.

Robust aggregation is checked against a two-scenario weighted example with mean 1.8, upper 20% loss-tail CVaR 5.0, and violation probability 0.2. Interval-Pareto tests require non-overlapping objective confidence bounds and conservative feasibility tiering.

Multi-fidelity tests use an exact Reference relationship `reference = 1 + 2 × fast`. The calibration must recover slope 2, omit candidates that already have Reference evidence, propagate Fast noise through the fitted slope, and return the requested deterministic batch with an inspectable acquisition-per-cost score.

## Surfaces and automation

- Rust: `xvarna_study::evidence` unit and contract tests.
- C: six sizing-pass UTF-8 JSON functions in `include/xvarna.h`, including a direct ABI test.
- .NET: `EvidenceEngine` typed records and end-to-end tests on net8.0 and net10.0.
- CLI: `xvarna evidence passport|sensitivity-design|sensitivity-analyze|robust|rank|fidelity`.
- Rhino/Grasshopper: `XV Evidence`, `XV Sensitivity+`, and `XV Fidelity`, included in isolated Rhino 8 and Rhino 9 runtime component enumeration.
- Validation fixtures: `validation/evidence`; generated evidence: `artifacts/validation/evidence`.

Run the focused gate after building the CLI:

```powershell
eng\smoke-evidence.ps1 -Cli artifacts\cli\win-x64\xvarna.exe
```

The full `eng/check.ps1` gate also executes this workflow.

## Scientific non-claims

The two-objective selector is cost-aware Monte-Carlo EHVI over a linear discrepancy calibration. It does not implement or claim qNEHVI, MF-HVKG, Gaussian-process posterior learning, or joint-batch optimization. Sobol/Morris indices quantify the declared sampling distribution and model outputs; they do not by themselves establish causality. Analytic fixtures prove estimator correctness, not accuracy on a real building. Public-release claims still require representative architectural corpora, independent Radiance comparisons, AMD execution, external beta projects, and reviewer sign-off.

## Primary references

- Saltelli et al., *Variance based sensitivity analysis of model output. Design and estimator for the total sensitivity index*, Computer Physics Communications 181 (2010).
- Morris, *Factorial Sampling Plans for Preliminary Computational Experiments*, Technometrics 33 (1991), as operationalized by the JRC sensitivity-analysis handbook.
- Daulton et al., *Parallel Bayesian Optimization of Multiple Noisy Objectives with Expected Hypervolume Improvement*, NeurIPS 2021.
- BoTorch multi-objective multi-fidelity and cost-aware acquisition documentation, used as a design comparison rather than an implementation claim.
