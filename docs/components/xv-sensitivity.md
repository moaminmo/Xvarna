# XV Sensitivity+

**Purpose.** Generates estimator-valid global sensitivity experiments and analyses aligned outputs.

**Inputs.** variable bounds/distributions, method (Saltelli/Jansen or Morris), sample count, seed and optional evaluated responses. **Outputs.** experiment rows, first/total-order or Morris effects, convergence/evidence and hash. Indices are dimensionless.

**Method and assumptions.** Sampling layout is deterministic for a seed and must remain intact; reordered or missing response rows are rejected. Sensitivity describes the declared ranges and model only, not causal effects beyond them.

**Example.** Generate rows, evaluate them without filtering, return responses in the same order, and seal the result with XV Evidence.
