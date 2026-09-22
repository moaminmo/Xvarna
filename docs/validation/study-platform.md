# Study/Optimization/Report validation contract — 0.16.0

## Validated scope

The 0.16 gate exercises one complete external-evaluation lifecycle instead of validating isolated serializers. It creates a workspace from the published manifest fixture, starts a deterministic four-candidate batch, reopens the workspace and proves that begin returns the identical pending batch, commits aligned scalar/vector objectives plus constraints and spatial metric fields, repeats the same commit to verify idempotence, and generates the complete report package.

Automated Rust tests additionally cover schema meta-validation, invalid registries/dimensions, checkpoint restoration of PRNG and pending state, journal recovery, ledger/checkpoint hashes, constraint-aware Pareto ranking, sensitivity statistics, baseline delta maps, and standards-based exports. Native ABI and .NET tests cross the UTF-8 sizing boundary and repeat the managed create/batch/reopen/commit/report flow on .NET 8 and .NET 10.

`eng/smoke-study-platform.ps1` requires all of the following:

- tracked and runtime schema identities are `0.16.0`;
- a reopened workspace returns the exact batch ID and content hash;
- four candidate IDs and parameter rows remain aligned;
- vector objective group widths and scalar objectives survive commit;
- an identical second commit returns the same checkpoint hash;
- the analysis contains sensitivity rows and three baseline comparisons;
- Parquet begins and ends with the `PAR1` magic bytes;
- glTF declares asset version `2.0` and contains four objective-space points; and
- both HTML files embed the study payload and the full report links its research artifacts.

The Rust export test also reopens the Parquet file through the official Parquet reader and verifies its row count. JSON Schema is checked against its Draft 2020-12 meta-schema and against the typed manifest instance.

## Statistical semantics

For every parameter/objective scalar-column pair, XVARNA computes rank-based Spearman association. Bootstrap resampling estimates a percentile confidence interval. A deterministic label permutation estimates a two-sided null p-value. Holm's step-down procedure controls the declared family-wise alpha across the generated family. Seed, sample counts, confidence level, alpha, raw/corrected p-values, interval, sample count, constant-column status, and significance status are included in portable JSON/HTML.

These values describe association in the evaluated design sample. They are not Sobol indices, causal effects, confidence intervals for a physical law, or proof that the optimizer sampled the entire feasible space. Four rows are sufficient for the automated integration fixture but not for a publishable architectural conclusion; real studies need an appropriate sampling design and materially larger sample.

## Persistence and corruption boundary

The ledger and checkpoint are content-addressed independently. A transaction journal contains the complete intended next ledger, checkpoint, and batch. Open completes a valid interrupted journal before exposing state. JSON reads can recover the last synchronized backup. Invalid hashes, mismatched pending IDs, altered manifests, incompatible delta maps, and incomplete result rows fail visibly.

This protects XVARNA-owned workspace state against partial writes and accidental corruption. It is not a distributed database, hostile-filesystem security boundary, or multi-writer lock service. One authoritative writer per workspace is required.

## Reproduction

```powershell
./eng/build-cli.ps1 -Configuration Debug
./eng/smoke-study-platform.ps1 -Cli ./artifacts/cli/win-x64/xvarna.exe
```

The generated evidence is under `artifacts/validation/study-platform-workspace/report`. Run `./eng/check.ps1` for the full Rust, C ABI, .NET, CLI, and installed-Rhino gates.

## Claim boundary

The implementation supports persistent batch/resume, deterministic NSGA-II ask/tell, constraint-aware scalar/vector Pareto ranking, statistically qualified association, baseline comparison, delta maps, and portable standards-based reports. It does not establish that VAHMAN dominates every optimizer, that four fixture variants provide valid design insight, or that a report has undergone independent scientific review. Public superiority claims still require external datasets, real projects, reviewer sign-off, and comparative benchmarks.
