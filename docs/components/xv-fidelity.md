# XV Fidelity

**Purpose.** Selects which Fast Path candidates should receive a costly reference evaluation.

**Inputs.** objective directions, candidates, paired Fast/Reference observations, costs, reference point and budget. **Outputs.** calibration, ranked recommendations, scores and hash.

**Method and assumptions.** Uses transparent linear discrepancy calibration and deterministic Monte-Carlo expected hypervolume improvement per reference cost. It is not a Gaussian-process or qNEHVI implementation; sparse calibration data widens uncertainty.

**Example.** Feed completed Radiance comparisons, then run only the recommended candidate IDs within the declared budget.
