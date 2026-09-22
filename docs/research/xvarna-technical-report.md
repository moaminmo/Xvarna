# XVARNA: an evidence-first computational design engine

**Status:** preprint-ready structure for `1.0.0-rc.1`; external corpus, beta and reviewer results must be inserted before submission.

## Abstract

XVARNA integrates deterministic spatial, solar, daylight, visibility and design-study workflows through a Rust core and thin Rhino/Grasshopper adapters. Its design emphasizes immutable scene identity, incremental two-level acceleration, portable GPU execution with a canonical CPU reference, source attribution, explicit Fast/Reference fidelity and portable evidence/report artifacts. This report defines the architecture, methods, reproducibility protocol and limitations without claiming universal superiority or whole-building energy simulation.

## Research questions

1. Can one scene lifecycle support interactive environmental and spatial analysis while retaining traceable source identity?
2. Can CPU/GPU and Fast/Reference paths remain explicit, testable and scientifically qualified?
3. Can optimization output preserve enough provenance for another researcher to reproduce a design decision?

## System and methods

ZAMYAD owns immutable f64 scenes, shared BLAS resources, independently updated static/dynamic TLAS layers and content-addressed cache entries. VAYU compiles origin-rebased f32 GPU snapshots, traverses TLAS/BLAS in WGSL, chunks geometry under declared budgets and either exposes failure or explicitly falls back to CPU. DAENA produces view, isovist, intervisibility, privacy, corridor, path and blocker/counterfactual measures. ZURVAN/HVARE/ASMAN/MEHR provide time, solar position, sky visibility, irradiation and daylight screening. VAHMAN/RASHNU provide versioned studies, constrained Pareto ranking, sensitivity/robust/multi-fidelity analysis and Evidence Passports.

## Validation protocol

Analytic fixtures test geometry, solar and metric invariants. Phase 2 uses deterministic S/M/L throughput geometry plus separately classified pathological cases. CPU/GPU comparison records state, source identity and distance. Daylight pairs Fast output with version-pinned Radiance and reports bias, MAE, RMSE, MAPE, extrema and spatial deltas. Closed-beta tasks measure install success, time to first result, crash-free sessions and unaided workflow completion. Raw tables are Parquet and all artifacts carry hashes/tool versions.

## Current evidence and limitations

Automated checks pass on the development host for Rhino 8/GH1 and Rhino 9/GH1/GH2 runtime contracts, Rust/.NET tests, NVIDIA and Intel enumeration/parity, and a real Radiance 6.1a execution. AMD hardware, external interactive sessions, multi-project Radiance error envelopes, independent review and longitudinal beta KPIs are pending. Fast daylight omits multi-bounce interreflection; privacy, PV and solar-envelope outputs are screening tools. EnergyPlus, HVAC, CFD, structure and compliance certification are outside 1.0 scope.

## Reproducibility and ethics

The repository includes generators, schemas, seeds, lockfiles, case protocols, claims register and a CycloneDX SBOM. User models remain local unless deliberately shared. Beta consent separates anonymous aggregate metrics from case-study publication permission. Future revisions must replace this section’s pending statements only with archived evidence.

