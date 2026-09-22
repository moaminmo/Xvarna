# ADR 0019: independent Grasshopper 2 connector

Status: accepted for 0.19.0 on 2026-09-04.

## Context

Grasshopper 2 is a separate prerelease API and document model, not a binary-compatibility
mode for a Grasshopper 1 `.gha`. Reusing the GH1 assembly would couple the engine to two
incompatible host object models and would make Rhino 8/9 support fragile.

## Decision

XVARNA ships `Xvarna.Grasshopper2.rhp`, targeting .NET 10 and the exact official
`Grasshopper2 9.0.26237.15343-beta` package used by the installed Rhino 9 WIP. The
connector contains host adaptation only. Rust remains the source of analysis truth;
`Xvarna.Native` owns typed managed records, cancellation, cache, scheduler and ABI checks.

All 49 GH2 capabilities reuse the corresponding GH1 `ComponentGuid` as their GH2 `IoId`.
The automated gate compares the complete identity sets. Geometry-first workflows expose
typed GH2 mesh, point, vector, transform and twig parameters. High-dimensional scientific
records use versioned JSON request inputs and typed result outputs during the prerelease
SDK period; tracked examples make these contracts inspectable and copyable.

The Rhino 9 Yak/ZIP contains both GH1 and GH2 connectors with one managed/native engine.
Rhino 8 remains GH1-only because the current GH2 package references RhinoCommon 9.

## Consequences

- Scientific behavior cannot drift between GH1 and GH2 without changing the shared engine.
- Semantic IDs, full records, object/category attribution and hashes survive host adaptation.
- GH2 SDK changes are isolated to one project and one exact package pin.
- JSON is intentionally used where premature custom GH2 goo/tree types would risk data loss;
  friendlier native parameter wrappers can replace it without changing the engine contract.
- Installed-runtime load and representative numerical execution are automated. Actual canvas
  placement, save/reopen, preview and cancellation are explicit release-candidate acceptance
  tasks, not hidden behind the phrase “runtime-tested.”
