# Source release review — 19 September 2026

## Corrections in this review

Concurrent Grasshopper iterations have separate scheduler scopes, preventing one branch from superseding another. A two-job regression exercises the production scheduler.

## Intended use

**Environmental and spatial analysis for design.** This is a source distribution for reproducible research and supervised evaluation. A public source release does not certify a machine, material, building or autonomous workflow.

| Contract | Current implementation |
|---|---|
| Input | Scene, units, material assumptions, sensor grids, location and weather |
| Output | Environmental indicators, views, studies and evidence records |
| Executed local checks | 173 Rust tests and 88 managed tests on each of net8/net10 |
| Implementation | [Source](crates) |
| Reproducible evidence | [Tests](dotnet/Xvarna.Tests) |

## Workshop or laboratory workflow

Record weather and geometry provenance, conduct mesh/grid sensitivity checks, and compare daylight proxies with a calibrated reference model before making design decisions.

Record the input checksum, source revision, dependency versions, host version, units, tolerances, settings and output checksum for every evaluated specimen. The example fixtures establish specific behaviours; representative project data must be evaluated separately.

## Acceptance still required

Fast daylight omits full interreflection; the separate Radiance point path now passes [real analytical reference checks](docs/validation/radiance-executed.md). Interactive acceptance, annual external runs, measured-building validation and broader device coverage remain gates.

No comparison in this review establishes universal optimality or state-of-the-art superiority. Algorithm choice is justified by the task and tested numerical behaviour. Independent benchmarks should compare the same inputs, correctness criteria and hardware, retaining raw timings and failure cases.

## Publication package

- README, license, source, build metadata and regression tests are included.
- Publish each project as a separate repository, preserving third-party attribution.
- Build from the documented dependencies; proprietary SDKs and compiled outputs are excluded from the source archive.
- Run the provided CI on the actual repository before tagging a release. Local results are not GitHub-hosted CI results.
- Cite the exact version or commit used; no DOI, paper acceptance or external certification is asserted.

## Retained publication evidence

The [executed example](publication/README.md) includes raw results, a reproducible figure, source links and a SHA-256 manifest. The [introduction draft](publication/linkedin.md) describes this evidence within its tested scope.
