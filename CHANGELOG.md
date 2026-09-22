# Changelog

## 2026-09-19 — numerical and retrieval follow-up

Added an executable real-Radiance acceptance fixture. Production export and runner reproduce uniform-sky illuminance and Lambertian ground interreflection within specified analytical tolerances on Radiance 6.0.2.


## Unreleased — source release review, 2026-09-18

Isolate Grasshopper scheduler scopes by solve iteration; added a concurrent-iteration regression. Clarified software citation metadata.

Added a practical source-release guide, reproducible issue form and citation metadata; refreshed README navigation and evidence. See [SOURCE-RELEASE.md](SOURCE-RELEASE.md) for remaining acceptance gates.

All notable changes to XVARNA will be documented in this file.

The project follows [Semantic Versioning](https://semver.org/). Internal `0.x`
builds are engineering milestones and are not public flagship releases.

## [Unreleased]

## [0.19.0] - 2026-09-04

### Added

- Independent Rhino 9 + Grasshopper 2 `.NET 10` connector with all 49 XVARNA capability adapters and exact official prerelease SDK pinning.
- Exact GH1 `ComponentGuid` to GH2 `IoId` semantic identity parity, typed geometry/twig inputs, versioned scientific JSON requests, and tracked GH2 examples.
- Installed-runtime compatibility gate that constructs all 49 components and crosses scene-ray, surface, sky, daylight, Target View, Study, and Evidence numerical contracts through the real native library.
- Combined Rhino 9 GH1/GH2 ZIP and Yak packaging with explicit SDK/host provenance.

### Changed

- Solar period becomes a host-neutral `Xvarna.Native` contract shared by both connectors.
- Native ABI, managed assemblies, CLI, citation, and packages advance additively to `0.19.0`; the independent Study Manifest schema remains `0.16.0`.

### Validation boundary

- Interactive GH2 canvas placement, save/reopen, viewport preview, long-job cancellation, and packaged-host acceptance remain release-candidate gates while the upstream API is prerelease. No Rhino 8 + GH2 compatibility or identical GH1/GH2 parameter-layout claim is made.

## [0.18.0] - 2026-09-04

### Added

- RASHNU hash-protected Evidence Passports with scientific method/fidelity, device/tool provenance, assumptions, limitations, citations, and parity/reference validation.
- Generated Saltelli/Jansen and Morris global-sensitivity designs with strict row/hash alignment and analytic reference tests.
- Direction-aware robust scenario mean, deviation, worst-case, CVaR, chance-constraint risk, and confidence-interval Pareto ranking.
- Transparent Fast/Reference discrepancy calibration, uncertainty propagation, deterministic two-objective Monte-Carlo EHVI, and reference value-per-cost selection.
- Six JSON CLI/C ABI operations, typed .NET 8/10 APIs, and three Rhino 8/9 Grasshopper components: `XV Evidence`, `XV Sensitivity+`, and `XV Fidelity`.
- Draft 2020-12 Evidence Passport and global-sensitivity schemas, fixtures, focused smoke automation, SDK guide, ADR, and scientific validation contract.

### Changed

- Native ABI, assemblies, CLI, citation, and local packages advance additively to `0.18.0`; the independent Study Manifest remains `0.16.0` for workspace compatibility.
- Rhino runtime enumeration advances from 46 to 49 components.

### Validation boundary

- qNEHVI, MF-HVKG, Gaussian-process posterior learning, and joint-batch fantasies are explicit non-claims. Real architectural corpora, independent Radiance comparisons, AMD hardware evidence, beta users, and external review remain empirical release gates.

## [0.17.0] - 2026-09-03

### Added

- Portable versioned scene JSON and checksummed `.xvscene` documents with deterministic migration, stable identities, static/dynamic layers, canonical-metre meshes, and tracked JSON Schema.
- Conservative geometry healing, tolerance welding, manifold winding repair, exact AABB clipping, and OBJ/STL/PLY/glTF/GLB conversion without Rhino as an intermediary.
- Radiance WEA and lossless XVARNA weather CSV interoperability with explicit missing-data, leap-day, timeline-gap, filtering, and occupancy policies.
- Headless `mesh-convert`, `scene-pack`, `scene-unpack`, and `weather-convert` workflows plus deterministic end-to-end validation in local engineering checks and Windows CI.
- Document-scoped Quality, Units, Diagnostics, Basic/Expert mode, Live/Manual execution, debounce, stale-result, and cancellation/progress policies shared by the flagship Grasshopper analyses.
- Robust Full/Percentile/Symmetric/Fixed legends with Viridis, Cividis, and diverging palettes, plus evidence and convergence labels for presentation-ready outputs.
- Freeform material-aware Solar/Shading Envelope candidates with one normalized axis per candidate, closest ray/axis constraints, axis-distance endpoints, and controlling sensor/time provenance.
- Spatially indexed sparse Visibility Graph construction with deterministic nearest-neighbour selection, a hard endpoint-degree cap, a 100,000-node safety limit, and additive Rust/C/.NET/CLI/Grasshopper APIs.
- Deterministic scientific-result and scene soak tests, malformed binary mutation/truncation rejection, and an enforced managed line-coverage floor of 85%.
- Native Rhino 8 and Rhino 9 Yak distribution, release manifests, checksums, local installation tooling, and packaged validation evidence.

### Changed

- Rhino compatibility tests run in isolated processes and discover both Rhino 9 release and Rhino 9 WIP installations, preventing Rhino 8 assembly collisions and false skips.
- The engineering gate now executes installed Radiance when available and resolves the matching MinGW runtime before the C ABI layout check.
- Native ABI, assemblies, CLI, citation, archives, and Yak packages advance additively to `0.17.0`; the independent Study Manifest schema remains `0.16.0` for workspace compatibility.
- The Rhino 8/9 runtime gate now loads all 46 Grasshopper components in their real host runtimes.

### Validation boundary

- Engine/API, product-experience, packaging, installed Radiance, NVIDIA GPU parity, and local Rhino 8/9 runtime gates are automated. Representative AMD execution, broader Intel execution, external architectural beta projects, competitor studies, and independent reviewer sign-off remain empirical launch gates and are not claimed here.

## [0.16.0] - 2026-09-02

### Added

- Versioned Draft 2020-12 Study Manifest and named variant/parameter/objective/constraint registries, including true scalar/componentwise-vector objectives and optional baseline identity.
- Persistent hash-protected ledger, complete optimizer/PRNG checkpoint, journaled batch begin/resume, backup recovery, and atomic complete/idempotent commit.
- Constraint-aware Pareto/report table plus Spearman rank association with bootstrap percentile intervals, two-sided permutation p-values, Holm family-wise correction, and explicit constant/non-significant states.
- Baseline scenario comparison with parameter/objective deltas and exact indexed/spatial metric-field delta maps.
- Real Apache Parquet via Arrow/Snappy, embedded-buffer glTF 2.0 objective-space data, complete portable JSON, self-contained HTML report, and network-free standalone viewer.
- Additive C ABI, .NET 8/10 `StudyPlatformWorkspace`, six CLI commands, and four Grasshopper components: `XV Manifest`, `XV Workspace`, `XV Commit`, and `XV Report`.

### Changed

- Native ABI, assemblies, CLI schemas, packages, and citation advanced additively to `0.16.0`; Rhino 8/9 runtime component gate advances from 38 to 42.
- The full engineering gate now executes a real create → batch → reopen/resume → commit/retry → analysis/report workflow and validates Parquet/glTF/HTML artifacts.

### Validation boundary

- Persistence, statistics, serialization, native/managed ABI, CLI flow, Parquet readback, and glTF/HTML structure are automated. Sensitivity remains association rather than causality; one-writer workspace semantics, independent user testing, real-project validation, and external reviewer sign-off remain explicit limits.

## [0.15.0] - 2026-09-02

### Added

- Deterministic material-aware Point-in-Time Illuminance and CIE-overcast Daylight Factor with reusable equal-solid-angle daylight coefficients.
- EPW climate-based annual sDA, direct-only ASE, four-bin UDI, sensor distributions, and area-weighted project summaries.
- Shared Radiance-compatible RGB optical materials with exact Object-ID assignment, energy-conservation validation, and opaque unassigned geometry.
- Exact canonical world-space Radiance export for materials, instanced geometry, sensors, and a stable manifest.
- Cancellable Radiance point runner (`oconv`/`rtrace`) and annual daylight-coefficient runner (`rfluxmtx`/`gendaymtx`/`dctimestep`) with weather-independent matrix reuse and full provenance.
- Fast Path/Radiance validation with bias, MAE, RMSE, MAPE, maximum absolute error, R², accepted fraction, and explicit tolerance envelope.
- Additive fixed-layout C ABI, source-unit-aware .NET 8/.NET 10 API, seven CLI workflows, and five Rhino 8/9 Grasshopper components.

### Changed

- Native ABI, assemblies, CLI schema, and package version advanced additively from `0.14.0` to `0.15.0`; Rhino component gate advances from 33 to 38.
- Annual and point daylight use the shared native job contract for cancellation and progress; matrix reuse is visible in result metadata and Grasshopper.

### Validation boundary

- Analytic, ABI, managed, CLI, export, cancellation, and provenance gates pass. External Radiance executables were unavailable on the release host, so external-tool runtime execution is explicitly not claimed as validated in this milestone.

## [0.14.0] - 2026-09-02

### Added

- Material-aware equal-solid-angle 3D Isovist with radial volume/surface metrics, visible solid angle, convergence, complete ray fields, general top-K attribution, exact category partitions, and exact remove-one re-tracing.
- Landmark Visibility with oriented camera FOV, spherical apparent solid angle, deterministic projected-disk sampling, partial optical transmission, weighting, and source attribution.
- Aligned DAENA scenario comparison with spatial metric and per-object attribution deltas.
- Immutable visible/direct-solar material catalogs assigned by exact Object ID, multi-layer transmission, energy-conservation validation, and deterministic provenance.
- Solar material-scenario comparison with complete scenario/sensor/time state, received/lost hours, object/category loss attribution, and first-scenario deltas.
- Vertical-column Solar/Shading Envelope for early massing, including material-aware baseline weights, access/shading quantiles, and controlling sensor/time provenance.
- Seven Rhino 8/9 components: `XV Materials`, `XV Isovist 3D`, `XV Landmark`, `XV Compare`, `XV Solar Scenarios`, `XV Solar Envelope`, and `XV Highlight`.
- Four standalone CLI workflows and additive fixed-layout C plus source-unit-aware .NET 8/.NET 10 APIs.

### Changed

- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.13.0` to `0.14.0`; Rhino runtime gate advanced from 26 to 33 components.
- DAENA/HVARE obstruction reporting now shares stable Object-ID attribution and category semantics suitable for direct Rhino highlighting.

## [0.13.0] - 2026-09-02

### Added

- Transactional immutable scene deltas, explicit static/dynamic acceleration layers, BLAS sharing/replacement, and deterministic TLAS refit/rebuild quality policy.
- True VAYU two-level TLAS/BLAS WGSL traversal with affine instancing, mirrored-facing correction, exact 64-bit attribution, and multi-chunk CPU/GPU parity coverage.
- Hard geometry-chunk and total GPU-memory budgets with deterministic streamed working sets and geometry-upload telemetry.
- Versioned BLAKE3 memory/disk portable-plan cache with atomic writes, memory/disk eviction, checksum/schema validation, corruption recovery, and process-wide configuration/telemetry APIs.
- Additive scene-lifecycle and cache C ABI, .NET 8/.NET 10 APIs, CLI cache commands, and revision-aware Rhino 8/9 `XV Scene`, `XV Backend`, `XV Runtime`, and new `XV Cache` components.
- Bounded shared managed scheduler with cancellation, progress phases, stale-generation suppression, queue/outcome telemetry, and new Rhino 8/9 `XV Scheduler` component.

### Changed

- Compute-heavy Grasshopper analyses now use the shared Task-capable scheduler instead of independent unbounded tasks or synchronous solution-thread execution.
- `XV Scene` reuses exact snapshots, applies transform/metadata/layer changes as deltas when mesh resources are unchanged, and reports the chosen hierarchy lifecycle.
- `XV Backend` invalidates by scene revision and exposes geometry chunk count, cache hit, streaming state, and explicit VRAM/chunk budgets.
- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.12.0` to `0.13.0`; Rhino runtime gate advanced from 24 to 26 components.

## [0.12.0] - 2026-09-01

### Added

- Lazy growth-bounded, double-slot VAYU scratch arena retaining ray, hit, readback, parameter, bind-group, and host staging resources across repeated queries.
- Cumulative `ComputeRuntimeStats` covering requests, rays, dispatches, reuse, fallback, scratch generations/capacity/bytes, transfers, device losses, execution errors, health, and the last exact diagnostic.
- Additive 368-byte `xv_compute_runtime_stats` data contract and `xv_compute_get_runtime_stats`, plus source-unit-independent .NET 8/.NET 10 `GetRuntimeStatistics()` support.
- Rhino 8/9 `XV Runtime` component for live session throughput, persistent-memory, fallback, and device-health inspection.
- Configurable cold/warm `backend-benchmark` distributions and `eng/benchmark-vayu-throughput.ps1` evidence generation across multiple ray counts.

### Changed

- VAYU dynamic uploads now use queue writes into persistent resources; up to two bounded chunks are encoded and submitted together and completed through one poll window while preserving ordered results.
- Batch and advanced DAENA provenance now identifies persistent-arena use without changing existing fixed-layout ABI sizes.
- Device-loss and uncaptured-error callbacks retain exact diagnostics; Auto mode falls back quickly after loss while required-GPU mode fails visibly.
- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.11.0` to `0.12.0`; Rhino runtime gate advanced from 23 to 24 components.

## [0.11.0] - 2026-09-01

### Added

- Backend-neutral `RayQueryExecutor` contract in `xvarna-scene`, implemented by canonical f64 CPU traversal and cached VAYU compute sessions with aggregate backend, adapter, batching, dispatch, transfer, precision, and fallback provenance.
- VAYU execution for DAENA Target/Weighted/Green View, protected View Corridor, and Dynamic Observer Path without changing their scientific definitions, ordering, attribution, hashes, or source-unit contracts.
- Additive high-level C ABI entry points `xv_compute_target_view`, `xv_compute_view_corridor`, and `xv_compute_observer_path`, plus a fixed 464-byte `xv_daena_execution_info` report.
- Source-unit-safe .NET 8/.NET 10 compute-session methods for all three advanced DAENA domains and managed `DaenaExecutionReport` provenance.
- `--backend`, adapter/resource/precision selection, and fail-closed `--verify-cpu` domain parity for all three advanced CLI workflows.
- Real-device end-to-end domain validation on NVIDIA RTX 5060 and Intel Raptor Lake/Vulkan, including bounded multi-dispatch execution and zero observed release-smoke metric/identity mismatch.

### Changed

- `XV Target View`, `XV Corridor`, and `XV View Path` now accept either `XV Scene` or `XV Backend` and surface the selected backend and complete execution report without changing component GUIDs or output indices.
- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.10.0` to `0.11.0`.

## [0.10.0] - 2026-09-01

### Added

- New `xvarna-vayu` portable compute engine built on wgpu 30/WGSL with a bounded iterative BVH traversal shader for ordered closest-hit and early-exit visibility batches.
- Precision-governed portable scene snapshots: canonical origin rebasing, measured f64-to-f32 coordinate error, conservative node bounds, explicit error rejection, effective-triangle caps, hierarchy-stack caps, and stable 64-bit object/instance/mesh/category attribution.
- Portable adapter enumeration across DX12, Vulkan, Metal, GL, and WebGPU-capable wgpu backends, including driver, device class, vendor, and device identifiers.
- First-class `Auto`, required `GPU`, and `CPU` compute sessions with creation-time and per-batch fallback provenance, bounded ray chunking, transfer/execution/readback timings, and memory/precision reporting.
- Additive compute-session C ABI and source-unit-safe .NET 8/.NET 10 API with safe handles, adapter discovery/filtering, closest-hit/any-hit batches, and fixed-layout validation.
- Rhino 8/9 `XV Devices` and adapter-filterable `XV Backend` components; `XV Ray Query` now accepts either a canonical scene or a cached VAYU session and reports the backend used for every batch.
- `devices` and `backend-benchmark` CLI workflows with adapter filtering, CPU/GPU parity, stable identity comparison, distance error, throughput, transfer timing, fallback state, and schema-versioned JSON.
- Real-device differential coverage on NVIDIA RTX 5060 Laptop GPU and Intel Raptor Lake integrated graphics, plus adapter-independent CPU fallback tests and dual-runtime managed tests.

### Changed

- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.9.0` to `0.10.0`.
- Rhino compatibility gates now require and instantiate twenty-three XVARNA components with unique stable GUIDs.
- ZAMYAD scene instances now retain their rebased forward transform as well as the inverse so portable snapshots can preserve world-space geometry and source identity.

## [0.9.0] - 2026-08-31

### Added

- DAENA solid-angle Target View, separately auditable Weighted View, category-mask Green View Index, protected View Corridor, and Dynamic Observer Path engines over ZAMYAD.
- Exact triangle and rectangular-FOV solid angles, deterministic Hammersley target sampling, uniform-area aperture sampling, convergence diagnostics, distance-weighted path aggregation, and complete first-blocker attribution.
- New `xvarna-study` / VAHMAN engine with mixed continuous/integer/categorical variables, residual constraints, parallel constraint-aware non-dominated sorting, NSGA-II crowding, exact 2D hypervolume, and descriptive Spearman diagnostics.
- Deterministic stateful ask/tell optimization with stratified initialization, tournament selection, simulated-binary crossover, polynomial/discrete mutation, bounded historical Pareto archive, and BLAKE3 state identity.
- Additive fixed-layout C ABI plus source-unit-aware .NET 8/.NET 10 APIs for advanced DAENA, study analysis, and safe-handle optimizer sessions.
- Six Rhino 8/9 components: `XV Target View`, `XV Corridor`, `XV View Path`, `XV Study`, `XV Optimizer`, and `XV Opt Step`.
- Five standalone CLI workflows: `target-view`, `view-corridor`, `observer-path`, `study-rank`, and `optimizer-benchmark` with schema-versioned JSON.
- Scientific/non-claim contract, ADR, component guides, release guide, cross-language layout checks, analytic tests, optimizer reproducibility tests, dual-runtime boundary tests, and CLI smoke gates.

### Changed

- Native ABI, managed assemblies, CLI schema, and package version advanced additively from `0.8.0` to `0.9.0`.
- Rhino compatibility gates now require and instantiate twenty-one XVARNA components with unique stable GUIDs.

## [0.8.0] - 2026-08-31

### Added

- New `xvarna-daena` Rust engine for deterministic planar isovists, directed intervisibility/privacy matrices, and exact all-pairs visibility graphs over ZAMYAD.
- Ordered isovist boundary rays, polygons, area/perimeter/centroid, radial distribution, compactness, convergence, open/occluded state, and dominant first-object attribution.
- Directed pair state/distance/direction, first object/instance/mesh/triangle blocker, bounded transparent privacy formula, observer summaries, and target exposure/combined-risk summaries.
- Exact graph pairs plus degree, degree centrality, deterministic connected components, harmonic closeness, and Brandes betweenness with explicit resource ceilings.
- Additive public C ABI and source-unit-aware .NET 8/.NET 10 APIs for all DAENA workflows.
- `XV Isovist`, `XV Intervisibility`, and `XV Visibility Graph` for Rhino 8 and Rhino 9 with native Rhino geometry, Data Trees, hashes, and assumption reports.
- Standalone `isovist`, `intervisibility`, and `visibility-graph` CLI commands with schema-versioned JSON and complete CSV export.
- Scientific contract, component guides, ADR, analytic room/divider/category/centrality tests, fixed ABI layouts, dual-runtime unit tests, and CLI smoke tests.

### Changed

- Native ABI and package version advanced additively from `0.7.0` to `0.8.0`.
- Rhino compatibility gates now require and instantiate fifteen XVARNA components.

## [0.7.0] - 2026-08-31

### Added

- Native deterministic longest-edge surface tessellation with target edge length, normal offset, stable sensor IDs, source-face provenance, exact cell areas, area-conservation metrics, resource ceilings, and BLAKE3 identity.
- Native area-weighted irradiance distribution statistics (mean/P10/P50/P90), threshold classification, exact-edge connected eligible regions, and transparent photovoltaic potential proxy.
- Unit-safe PV surface math across Rhino model units with explicit module efficiency, coverage, multiplicatively combined system loss, incident energy, capacity proxy, annual yield proxy, and specific yield.
- Additive two-pass C ABI plus public .NET 8/.NET 10 `SurfaceGridGenerator` and `SurfacePotentialAnalyzer` APIs.
- `XV Sensor Grid` for Mesh/Brep/Surface/Extrusion ingestion and direct points/normals/areas/IDs/analysis-mesh output.
- `XV Solar Potential` with bakeable full-domain Viridis mesh, connected eligible-region meshes, area-weighted distribution outputs, PV proxy outputs, deterministic mapping by Sensor ID, and an explicit non-bankable report.
- Standalone `surface-grid` CLI with complete cell CSV and optional analysis OBJ export.
- Cross-language area, determinism, resource-limit, ABI-layout, unit-scale, region, and PV arithmetic tests.

### Changed

- Native ABI and package version advanced additively from `0.6.0` to `0.7.0`.
- Rhino 8 and Rhino 9 runtime compatibility gates now require and instantiate twelve XVARNA components.
- Public heatmaps in the solar-potential workflow use the complete uncropped data domain and a monotonic colourblind-friendly Viridis palette.
- The previously uncompilable C typedef/function collision on `xv_job_progress` is repaired by naming the fixed-layout type `xv_job_progress_info`; the exported function symbol and binary layout are unchanged.

## [0.6.0] - 2026-08-31

### Added

- MEHR strict hourly/subhourly EPW parser with interval-end to UTC-midpoint conversion, radiation missing-value diagnostics, station metadata, and semantic BLAKE3 identity.
- EnergyPlus-style Perez 1990 dome, circumsolar, and horizon tilted-surface irradiance using ZURVAN NREL SPA solar positions.
- Ray-traced direct/circumsolar visibility, 144-patch diffuse-dome obstruction, 24-sector horizon obstruction, and isotropic ground reflection.
- Complete sensor/interval energy timeline, first-hit object/instance/mesh/triangle/distance identity, dominant solar blocker/lost Wh/m², and dominant static-sky blocker.
- Native cooperative cancellation and exact progress for annual work, additive EPW/annual C ABI, and synchronous/asynchronous/observable .NET 8/.NET 10 APIs.
- Task-based `XV Irradiance` with annual component outputs, Viridis viewport heatmap, timeline trees, weather diagnostics, and scientific report on Rhino 8 and Rhino 9.
- Standalone `annual-irradiance` CLI with repeated sensors, human/JSON reporting, and complete timeline CSV export.
- EPW, Perez, ground-reflection, open-sky, full-blocker, attribution, ABI-layout, async, cancellation, and dual-runtime tests.
- Scientific contract, component guide, CLI/SDK/installation updates, and release notes.

### Changed

- Native ABI and package version advanced additively from `0.5.0` to `0.6.0`.
- Annual irradiance is branded MEHR while ASMAN remains the sky-visibility subsystem consumed by it.

## [0.5.0] - 2026-08-31

### Added

- ASMAN deterministic equal-solid-angle Fibonacci hemisphere sampling for oriented sensors.
- Lambert cosine-weighted Sky View Factor, unweighted visible-hemisphere fraction, visible solid angle, and interleaved-subset convergence diagnostics.
- Complete sensor-major Shadow Mask directions with first-hit object, instance, mesh, triangle, and distance attribution.
- Deterministic dominant-occluder projected fraction and solid-angle contribution.
- Native cancellable job handles with atomic progress state and bounded 8,192-ray cancellation checkpoints.
- Additive fixed-layout Sky View/job C ABI plus synchronous, cancellable, and asynchronous .NET 8/.NET 10 APIs.
- Task-based `XV Sky View` with Viridis viewport heatmap and `XV Shadow Mask` with interactive state-coloured dome preview for Rhino 8 and Rhino 9.
- Standalone `sky-view` CLI with repeated sensors, human/JSON summaries, and full Shadow Mask CSV export.
- Analytic open-sky, full-blocker, half-screen, determinism, attribution, model-unit, ABI, async, and cancellation tests.
- Scientific method, component, CLI, installation, and release documentation for ASMAN.

### Changed

- Native ABI and package version advanced additively from `0.4.0` to `0.5.0`.
- Major engines use Iranian historical/mythic names while public components retain explicit functional names.

## [0.4.0] - 2026-08-31

### Added

- ZURVAN fixed-offset, interval-centred schedules with explicit durations and weights.
- HVARE apparent solar positions using a Rust Reda–Andreas SPA implementation and the published NREL golden case.
- Direct Sun Hours, Shadow Hours, Solar Access, full sensor/time state, first-hit attribution, and deterministic dominant-occluder aggregation.
- Additive fixed-layout C ABI and public .NET 8/.NET 10 APIs for solar positions and integrated scene/solar analysis.
- `XV Period`, `XV Sun Vectors`, and `XV Sun Hours` for Rhino 8 and Rhino 9.
- Standalone `sun-position` CLI with repeated RFC 3339 timestamps and human/JSON output.
- Scientific method, component, installation, CLI, and release documentation for the complete workflow.

### Changed

- Native ABI and package version advanced additively from `0.3.0` to `0.4.0`.
- Direct-sun result identity now includes first-hit distance in addition to state and source IDs.

## [0.3.0] - 2026-08-31

### Added

- ZAMYAD immutable scene snapshots with model-unit normalization and large-world origin rebasing.
- BLAKE3 mesh-resource deduplication, affine instancing, stable source IDs, and 64-bit category masks.
- Deterministic flattened BLAS/TLAS acceleration using 16-bin surface-area-heuristic construction.
- Parallel ordered closest-hit and early-exit any-hit batches with object, instance, mesh, triangle,
  barycentric, distance, and facing attribution.
- Opaque handle-based C ABI with fixed layouts, state validation, concurrent query safety,
  panic containment, and explicit release.
- Public managed `XvarnaSceneBuilder`/`XvarnaScene` API with safe handles and transparent
  source-model-unit conversion on .NET 8 and .NET 10.
- `XV Scene` and `XV Ray Query` Grasshopper components for Rhino 8 and Rhino 9.
- `scene-bench` CLI workflow with throughput metrics and differential reference validation.
- Native lifecycle/layout tests, managed unit/concurrency tests, and a 1,024-ray Rust reference test.

### Changed

- Native ABI and package version advanced additively from `0.2.0` to `0.3.0`.

## [0.2.0] - 2026-08-30

### Added

- Approved master product and engineering specification.
- Rhino 8 (`net8.0`) and Rhino 9 (`net10.0`) compatibility contract.
- Initial Rust workspace, FFI boundary, CLI, and .NET connector structure.
- Project governance, security, contribution, and CI foundations.
- Native `XV Mesh Check` engine with deterministic BLAKE3 content hashing.
- Detection of invalid indices, non-finite geometry, degenerate and duplicate faces,
  isolated vertices, naked/non-manifold edges, winding conflicts, components, area,
  signed volume, bounds, and f32 precision risk.
- Conservative non-mutating repair and Grasshopper component for Rhino 8 and Rhino 9.
- Fixed-layout C API and cross-runtime native integration tests.
- Standalone `mesh-check` and `mesh-repair` CLI workflows with explicit OBJ
  triangle/quad ingest, JSON diagnostics, analytic fixtures, and CI smoke tests.
