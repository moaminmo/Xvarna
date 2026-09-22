# ADR 0016 — Persistent Study, Optimization and Report platform

- Status: Accepted
- Milestone: 0.16.0
- Date: 2026-09-02

## Context

The original VAHMAN API could rank completed tables and run a deterministic in-memory ask/tell optimizer. A real architectural design study also needs durable hand-off to external Grasshopper definitions, crash-safe resume, named scalar/vector objectives, comparable spatial fields, and reports that remain useful without Rhino.

## Decision

XVARNA 0.16 adopts a versioned `StudyManifest` as the authoritative registry for parameters, objectives, constraints, baseline identity, optimizer policy, metadata, and provenance. Draft 2020-12 JSON Schema is emitted by the same Rust contract and tracked in `schemas/xvarna-study-0.16.schema.json`.

Each workspace stores an immutable manifest, schema, hash-protected ledger, exact optimizer checkpoint, and monotonic batch envelopes. A batch transaction registers candidate variants and the complete optimizer state—including PRNG state and pending candidate IDs—before evaluation leaves the engine. A commit accepts exactly one aligned result for every pending candidate. Journal, temporary file, file synchronization, backup recovery, and content hashes prevent a partial write from being mistaken for a valid generation. Repeating begin or an identical commit is idempotent.

An objective with no components is scalar. A vector objective contributes one named scalar column per component to constraint-aware non-dominated sorting, crowding, Parquet, glTF, and statistical analysis. Constraints retain the `g(x) <= 0` residual convention.

Sensitivity is inferential association, not causal attribution. The platform reports Spearman rank correlation, deterministic bootstrap percentile intervals, a two-sided permutation p-value, and Holm correction across the reported family. Constant columns and non-significant relationships are explicitly marked. Report generation rejects datasets too small for the method.

The portable report package contains:

- complete schema-versioned JSON;
- an Apache Parquet table written through Arrow with typed parameter, objective, constraint, rank, and provenance columns;
- an embedded-buffer glTF 2.0 point cloud of normalized objective space;
- a self-contained narrative HTML report; and
- a compact self-contained interactive viewer.

The HTML files embed their data and require no CDN, web server, Rhino, or XVARNA installation. The glTF and Parquet files remain separate standards-based research artifacts.

## Consequences

The same state machine is exposed through Rust, an additive UTF-8 C ABI, .NET 8/10, CLI, and four Grasshopper components. External evaluators can stop between ask and commit and recover the exact pending work. Scenario comparison uses the declared baseline and only constructs a delta map when field identity, units, values, and optional positions align exactly.

Version 0.16 does not claim causal sensitivity, uncertainty-aware optimization, a hosted collaboration service, or browser-side optimization. The viewer is a portable local artifact; multi-user storage and authorization remain outside this milestone.
