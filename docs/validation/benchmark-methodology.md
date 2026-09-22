# Phase 2 benchmark methodology

`eng/run-phase2-evidence.ps1` builds the release CLI, deterministically generates the CC0 S/M/L corpus, enumerates adapters, then records cold and warm CPU/GPU runs without discarding unfavorable results. S/M/L contain approximately 100k/1M/5M triangles and 10k/100k/250k declared sensors. Generated geometry is excluded from Git; the generator, definition manifest, hashes and exact outputs provide reproducibility without repository bloat.

Performance rays use a deterministic rotated low-discrepancy sequence. The rotation avoids exact shared-edge hits where two triangle identities are equally valid. Thin, coplanar, corrupt, scale and far-origin cases remain correctness fixtures and are not silently mixed into throughput numbers. Canonical f64 CPU is the reference. Reported fields include requested and actual backend, cold, median, P95, host/VRAM estimates, adapter, parity, fallback and device-loss count. Identity acceptance requires at least 99.9% agreement and closest-distance error no greater than the declared 0.25 mm portable precision budget, while preserving exact mismatch/error values in raw JSON.

The monolithic L OBJ is executed through GPU-preferred `Auto`: adapters unable to bind one 5M-triangle mesh must fall back explicitly to canonical CPU and record that fallback. Streaming of multi-resource architectural scenes is tested separately because an OBJ imported as one mesh is an indivisible geometry unit in the current two-level plan.

`Smoke` proves the pipeline using S. `Full` is the release-evidence profile and runs S/M/L. A detected adapter is not equivalent to vendor sign-off. Missing AMD hardware, external device-loss injection, long-run L soak, 20 reviewers and 10 projects remain explicit external collection gates in `hardware-matrix.json` and `beta-kpis.json`.

Fast/Radiance tables preserve paired sensor values and absolute/relative differences. The analytic equality fixture verifies the evidence pipeline; project-specific validation must use the pinned Radiance runner and must report bias, MAE, RMSE, MAPE, maximum error and spatial limitations. XVARNA Fast Path is not an LM-83 certification engine.
