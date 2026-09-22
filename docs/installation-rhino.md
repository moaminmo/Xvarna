# XVARNA 0.19 for Rhino and Grasshopper

This package is host-specific. Use the `rh8`/`rhino8` build with Rhino 8.20 or later for Grasshopper 1. The `rh9`/`rhino9` package contains both the 49-component Grasshopper 1 `.gha` and the independent 49-component Grasshopper 2 `.rhp`.

## Grasshopper 2 release-candidate workflow

1. Install the Rhino 9 package and restart Rhino.
2. Open Grasshopper 2 and confirm that `XVARNA Info` reports a compatible native ABI.
3. Use native mesh/point/vector/twig inputs for the geometry-first components.
4. For components with `Request JSON`, paste and edit the tracked examples from `examples/grasshopper2`; the output remains a typed XVARNA object and also provides portable JSON/hash provenance.
5. Verify at least `Scene → Ray Query`, `Scene → Sky View`, and one domain-specific workflow in your installed WIP before relying on a file. GH2 is prerelease software and the exact SDK is pinned deliberately.

The engineering gate verifies 49/49 registration, exact GH1/GH2 semantic identity, installed-runtime loading, and numerical contracts for scene rays, surface grids, sky view, daylight, Target View, Study and Evidence. Interactive canvas save/reopen, previews and cancellation still require the release-candidate acceptance checklist. Rhino 8 + Grasshopper 2 is not claimed because the current official GH2 SDK depends on RhinoCommon 9.

## Recommended Yak install

From the repository root, install the already-built local package without publishing it:

```powershell
.\eng\install-local.ps1 -RhinoVersion 8
.\eng\install-local.ps1 -RhinoVersion 9
```

The installer verifies the package checksum when `SHA256SUMS.txt` is present and delegates installation to the matching official Rhino `Yak.exe`. Use `-Action Uninstall` to remove the package. Restart Rhino after an install or uninstall.

## Manual archive install

1. Extract the entire archive to a stable folder. Keep `Xvarna.Grasshopper.gha`, `Xvarna.Native.dll`, and `xvarna_core.dll` together.
2. In Windows file properties, select **Unblock** for downloaded files if Windows shows that option.
3. Open Grasshopper and run `GrasshopperDeveloperSettings` in Rhino.
4. Add the extracted folder to Grasshopper's library folders, then restart Rhino.
5. Add `XVARNA > 00 Setup > XV Info`; `Ready` must be `True`.
6. Add `XVARNA > 01 Scene > XV Mesh Check` and connect a Rhino mesh.
7. Add `XV Scene` to compile one or more checked meshes. Connect it directly to `XV Ray Query` for canonical CPU tracing.
8. Add `XV Devices` to inspect available portable adapters. Connect `XV Scene` to `XV Backend`, choose Auto/GPU/CPU and memory/chunk/cache policy, then connect that session to `XV Ray Query` for backend-aware tracing and `XV Runtime` for live reuse/memory/device telemetry.
9. Add `XV Cache` to configure persistent scene-plan reuse and `XV Scheduler` to observe or tune the shared analysis queue.
10. For solar studies, connect `XV Period` → `XV Sun Vectors`, then connect both `XV Scene` and the Sun Set to `XV Sun Hours`.
11. For sky studies, connect `XV Scene` to `XV Sky View`, then connect its Result to `XV Shadow Mask` for per-direction inspection.
12. For annual radiation, connect `XV Scene`, an EPW file path, sensor points, and normals to `XV Irradiance`.
13. For surface analysis, connect Mesh/Brep/Surface/Extrusion geometry to `XV Sensor Grid`; connect its points, normals, and IDs to `XV Irradiance`.
14. Connect `Sensor Grid` and the immutable irradiance `Result` to `XV Solar Potential` for a bakeable solar map, area-weighted statistics, eligible regions, and PV screening proxy.
15. For spatial visibility, connect the same `XV Scene` to `XV Isovist`, `XV Intervisibility`, or `XV Visibility Graph` under `XVARNA > 04 Visibility`.
16. For 0.14 spatial intelligence, build `XV Materials`, then connect it and `XV Scene` to `XV Isovist 3D` or `XV Landmark`; connect ranked IDs to `XV Highlight`.
17. For 0.15 daylight, build `XV Optical`, connect `XV Scene` plus workplane sensors to `XV Daylight` or `XV Annual Daylight`, and inspect the matrix cache-hit/hash outputs.
18. Use `XV Radiance` to export the exact reference bundle; run the packaged CLI against an installed Radiance toolchain and feed aligned lux values into `XV Daylight Δ`.
19. Feed two aligned `XV Isovist 3D` results to `XV Compare`. Feed multiple named material catalogs to `XV Solar Scenarios`, or protected sensors/candidates plus optional freeform axes to `XV Solar Envelope`.
20. For a persistent 0.16 design study, create registries with `XV Manifest`; initialize or resume the directory with `XV Workspace`; evaluate every candidate branch through the definition; group scalar/vector objectives in `XV Commit`; then use `XV Report` to create the portable report directory.

## VAYU portable compute workflow

`XV Devices` lists adapter, driver, graphics API, device class, vendor, and device identifiers. It is diagnostic and does not allocate a scene or persistent compute session.

`XV Backend` converts the immutable scene into a portable, origin-rebased f32 two-level BLAS/instance/TLAS plan with a measured precision budget. Its optional Adapter Filter is a case-insensitive substring from `XV Devices`, so multi-GPU selection is repeatable. Auto prefers GPU and reports any fallback; GPU treats a missing filtered device, unavailability, precision rejection, or dispatch failure as an error; CPU remains the canonical f64 path. Triangle, per-dispatch ray, geometry-chunk, and GPU-memory caps are explicit inputs rather than hidden allocation limits. Over-budget geometry is deterministically streamed. Cache Hit and Streamed outputs make both decisions visible.

`XV Ray Query`, `XV Target View`, `XV Corridor`, and `XV View Path` accept either the original `XV Scene` or an `XV Backend` session. Existing scientific outputs preserve order and exact object/instance/mesh/triangle identity. Reports expose adapter choice, logical batch/device dispatch counts, upload, execution, readback, precision error, and fallback. A small batch can be slower on GPU because transfer and synchronization are included; use representative repeated workloads before making a performance decision.

`XV Runtime` reads cumulative session counters. For a stable repeated workload, Arena Generations should stop increasing after the cold request while Reused Batches continues to rise. Arena Memory is dynamic scratch memory and is reported separately from host snapshot and uploaded geometry. Geometry upload counts, cache hit, chunking, device-loss/error, and fallback remain visible.

`XV Cache` configures bounded process-memory and checksummed disk tiers. Corrupt entries are rejected, removed, and rebuilt; eviction enforces both budgets. Clear removes only XVARNA `.xvc` entries. `XV Scheduler` exposes the shared bounded queue, active job IDs/names/phases/progress, cancellations, stale-result suppression, failures, and latency. Scheduler limits can change only while idle.

Read the [VAYU validation and non-claim contract](validation/portable-gpu.md), [0.13 lifecycle contract](validation/zamyad-vayu-lifecycle.md), [DAENA domain-acceleration contract](validation/vayu-domain-acceleration.md), [0.14 spatial/solar contract](validation/daena-spatial-solar-intelligence.md), [0.15 daylight/Radiance contract](validation/daylight-radiance.md), [0.16 Study platform contract](validation/study-platform.md), [0.17 product/release-quality contract](validation/product-experience-release-quality.md), and [0.18 Evidence/multi-fidelity contract](validation/evidence-multifidelity.md) before publishing results.

## VAHMAN Study platform 0.16

`XV Manifest` is the immutable registry contract. Category labels and vector-component branches are part of that contract; changing their order after workspace creation is invalid. `XV Workspace` Action 2 either returns a new optimizer batch or the exact pending batch already stored on disk. Candidate IDs and parameter Data Tree branches must remain aligned through every analysis component.

`XV Commit` expects one objective branch per candidate and an `Objective Widths` list that restores the manifest groups—for example `[2,1]` for a two-component view objective plus scalar energy. Constraint values are feasible at `<= 0`. Optional metric-field JSON carries aligned values and positions for baseline delta maps. `XV Report` runs off the UI thread and produces JSON, Parquet, glTF, narrative HTML, and an independent viewer. Only one Rhino/CLI process should write a workspace at a time.

## HVARE daylight 0.15

`XV Optical` creates the RGB shared material catalog. `XV Daylight` returns point illuminance plus CIE-overcast Daylight Factor. `XV Annual Daylight` calculates area-weighted sDA, ASE, and UDI from EPW with async cancellation and exact matrix reuse. `XV Radiance` writes the canonical interchange bundle, and `XV Daylight Δ` reports numerical agreement. Radiance executables are not bundled; install Radiance separately and use the CLI runner for reference execution.

## DAENA spatial visibility workflow

`XV Isovist` accepts one or more eye points plus optional plane normals and forward directions. It returns Rhino isovist polylines, branch-aligned radials/distances, area, perimeter, compactness, radial depth, convergence, dominant blocker, and the immutable DAENA result. The finite maximum distance clips open space and is part of the study definition.

`XV Intervisibility` evaluates every observer against every target. Visible and blocked line outputs are immediately previewable; state, distance, privacy risk, and blocker IDs remain aligned as one Data Tree branch per observer. Target facings are optional. The report prints the exact risk formula and its assumptions.

`XV Visibility Graph` has two explicit contracts. Exact mode traces every unordered pair and returns blockers, degree, components, harmonic closeness, and Brandes betweenness; it is capped at 4,096 nodes. Sparse mode requires a finite radius, uses a 3D uniform spatial index, proposes local nearest neighbors, enforces a deterministic hard degree cap, and traces only retained candidates. Sparse mode supports up to 100,000 nodes and avoids the all-pairs matrix; centrality is disabled by default because Brandes remains expensive on large graphs. Sparse density is reported against the complete theoretical pair count so it cannot be confused with density only among sampled edges.

## Advanced view and optimization

`XV Target View` accepts either `XV Scene` or `XV Backend`, cameras, and target meshes, then returns raw Target View, explicit Weighted View, Green View Index, per-target solid-angle/visibility trees, convergence, dominant source IDs, and complete backend provenance. Target meshes are not automatically part of the occluding scene.

`XV Corridor` tests protected circular target apertures. `XV View Path` evaluates the same target-view model along tangent-facing polyline motion and returns ordered series and best/worst hotspots. Connecting `XV Backend` accelerates their closest-hit batches without changing the DAENA metric implementation.

`XV Study` ranks completed candidate/objective/constraint trees. `XV Optimizer` returns an initial candidate tree and a stateful session. Evaluate that tree through the Grasshopper definition and send aligned objective/constraint trees plus the session to `XV Opt Step`; pulse Advance exactly once per generation. Chain explicit steps or automate pulses. Resetting the start component creates a new deterministic sequence.

Read the [advanced scientific contract](validation/advanced-view-and-optimization.md) before interpreting weighted or optimized results.

## DAENA/HVARE intelligence 0.14

`XV Materials` maps visible/direct-solar transmittance and reflectance to exact scene Object IDs; unassigned objects are opaque. `XV Isovist 3D` returns equal-solid-angle volumetric metrics, top-K/category loss, and exact remove-one recovery. `XV Landmark` evaluates partial material-aware visibility inside an oriented camera. `XV Compare` requires aligned viewpoints and returns candidate-minus-baseline deltas. `XV Highlight` previews attributed source meshes directly.

`XV Solar Scenarios` holds geometry and schedule fixed while comparing material alternatives. `XV Solar Envelope` produces access-preserving and target-shading distances along freeform circular candidate axes; no axes means World Z. Treat the resulting endpoints as deterministic early-massing guidance, not a legal or physics-complete construction envelope.

## Annual irradiance workflow

`XV Irradiance` validates the EPW header, location, timezone, temporal resolution, radiation fields, and missing-value policy before dispatch. It runs asynchronously and returns annual direct, Perez sky-diffuse, ground-reflected and global kWh/m²; peak W/m²/time; dome/horizon visibility; dominant solar blocker/lost energy; dominant static-sky blocker; and complete sensor-major global/direct/diffuse/state/occluder timeline trees.

The component traces 144 sky-dome directions and 24 horizon sectors per sensor once, then uses one closest-hit solar ray per eligible interval for both direct beam and Perez circumsolar visibility. Purple-to-yellow viewport colours are normalized to the largest annual global value in that solution. `Result` retains all interval component energy, UTC timestamps, first-hit object/instance/mesh/triangle/distance identity, hashes, options, sensors, and weather diagnostics for custom scripts or downstream components.

`Ground Albedo` is the fallback. When `Use EPW Albedo` is true, each valid EPW albedo takes precedence. Ground reflection is an isotropic unshaded-ground view-factor estimate; it is not multi-bounce radiosity. North rotation is counter-clockwise about Rhino +Z. EPW timestamps are interpreted as local-standard end-of-interval values and evaluated at their UTC midpoints.

## Sky View and Shadow Mask workflow

`XV Sky View` accepts oriented sensor points, one or one-per-sensor normal, and a deterministic sample count. The default is 2,048 equal-solid-angle Fibonacci directions per sensor. It reports Lambert cosine-weighted Sky View Factor, unweighted visible-hemisphere fraction, visible solid angle, an interleaved-subset convergence delta, the dominant obstruction, and complete first-hit attribution retained in Result. The component uses Grasshopper's task scheduler and XVARNA's native cancellation checkpoints, so a superseded or cancelled solution does not have to finish the entire ray matrix.

The viewport colours use the Viridis scale from low SVF (purple) to high SVF (yellow). Connect Result to `XV Shadow Mask`, select a zero-based sensor index, and set a display radius to inspect every sampled world direction. Cyan rays reach open sky; magenta rays are blocked. The outputs include hit points and exact object, instance, mesh, and triangle IDs for downstream selection, reporting, or custom visualization.

`Convergence Delta` is the absolute difference between even/odd interleaved half-sample estimates. It is a practical resolution diagnostic, not a confidence interval. Increase Samples when small geometric obstructions materially affect the decision or the delta is too large for the study.

## Time and solar workflow

`XV Period` treats Start as inclusive and End as exclusive. Its timestamps are interval centres, and a final interval is truncated when the requested step does not divide the period exactly. Both boundaries require an explicit `Z` or `±HH:MM` offset and must share the same offset. This deliberately avoids silently using the Windows timezone or guessing a daylight-saving transition; split a DST transition into fixed-offset periods.

`XV Sun Vectors` accepts either that Period or manual timestamp/duration/weight lists. It reports apparent altitude and azimuth, a unit direction from sensor to sun, active samples, atmospheric inputs, true-north rotation, and a deterministic sun-set hash. Pressure `0` disables the refraction correction.

`XV Sun Hours` evaluates oriented sensors against the immutable scene. One normal may be broadcast to all points. Outputs include weighted sun, shadow, eligible hours, solar-access ratio, interval counts, dominant object ID/hours, and two sensor-major timeline trees. State codes are `0 inactive`, `1 back-facing`, `2 visible`, and `3 blocked`; the occluder tree contains the first-hit object ID for blocked states.

## Scene workflow

`XV Scene` accepts zero, one, or one-per-mesh transforms, object IDs, category masks, and `Static`/`Dynamic` layers. It converts active Rhino document units to canonical metres internally while all Grasshopper coordinates and hit distances remain in document units. An unchanged definition reuses its immutable snapshot. Transform/identity/category/layer changes use one atomic delta, share unchanged BLAS resources, and independently reuse/refit/rebuild each layer TLAS; geometry or build-policy changes rebuild. Revision and Lifecycle make this decision visible. The report exposes unique/effective triangle counts, BLAS/TLAS nodes, memory, build time, worker count, rebase origin, and BLAKE3 scene hash.

`XV Ray Query` accepts one direction for all origins or one direction per origin. Closest mode returns points, distances, barycentrics, facing, and exact IDs; Any Hit mode is the faster visibility path and returns booleans without attribution.

## Mesh Check semantics

An open mesh may be **analysis-ready** for ray occlusion while not **watertight** for volume. `Repair` only removes invalid, non-finite, degenerate, and duplicate triangles and then culls unused vertices. Source Rhino geometry is never modified; welding, hole filling, and topology reorientation are intentionally not automatic.

If `XV Info` reports that the native engine is unavailable, verify that all three binaries remain in the same folder and that the package matches the Rhino major version.
