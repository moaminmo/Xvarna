# XVARNA engineering CLI 0.19

## Scientific evidence and multi-fidelity

The RASHNU/VAHMAN commands consume strict camel-case JSON and write portable pretty JSON. Checked-in input examples live in `validation/evidence`.

```powershell
./xvarna.exe evidence passport passport.json sealed-passport.json

./xvarna.exe evidence sensitivity-design variables.json design.json `
  --method saltelli --samples 4096 --seed 42
./xvarna.exe evidence sensitivity-analyze design.json aligned-outputs.json sensitivity.json

./xvarna.exe evidence robust robust-scenarios.json robust-summary.json
./xvarna.exe evidence rank uncertain-evaluations.json interval-pareto.json
./xvarna.exe evidence fidelity multifidelity.json selected-reference-jobs.json
```

`sensitivity-analyze` rejects any changed design hash or row mismatch. `fidelity` requires at least two candidate IDs with both Fast and Reference evidence; candidates already evaluated at Reference fidelity are never recommended. Two-objective selection uses deterministic Monte-Carlo expected hypervolume improvement divided by observed Reference cost. More than two objectives use a transparent uncertainty proxy and do not claim qNEHVI or MF-HVKG. See [the validation contract](validation/evidence-multifidelity.md).

## Daylight and Radiance

All daylight CLI coordinates and areas use metres and square metres. A sensor is `id,x,y,z,nx,ny,nz,area`; an optical material is `id,kind,Rr,Rg,Rb,Tr,Tg,Tb,specularity,roughness,ior` and an assignment is `object-id,material-id`.

```powershell
./xvarna.exe daylight-point context.obj `
  --sensor 1,0,0,0.8,0,0,1,1 `
  --moment 1718966400,0.4,0.2,0.89,80000,12000 `
  --patches 2304 --json

./xvarna.exe daylight-factor context.obj `
  --sensor 1,0,0,0.8,0,0,1,1 --exterior-lux 10000 --json

./xvarna.exe annual-daylight context.obj weather.epw `
  --sensor 1,0,0,0.8,0,0,1,1 --patches 2304 `
  --timeline-csv annual-daylight.csv --json

./xvarna.exe radiance-export context.obj radiance-bundle `
  --sensor 1,0,0,0.8,0,0,1,1

./xvarna.exe radiance-point context.obj `
  --sensor 1,0,0,0.8,0,0,1,1 `
  --moment 1718966400,0.4,0.2,0.89,80000,12000 `
  --radiance-bin C:\Radiance\bin --json

./xvarna.exe radiance-annual context.obj weather.epw `
  --sensor 1,0,0,0.8,0,0,1,1 `
  --radiance-bin C:\Radiance\bin --matrix-cache D:\xvarna-radiance-cache

./xvarna.exe daylight-compare fast-lux.txt radiance-lux.txt `
  --absolute-lux 50 --relative 0.10 --json
```

The point runner requires `oconv` and `rtrace`. The annual runner additionally requires `rfluxmtx`, `gendaymtx`, and `dctimestep`. No Radiance binaries are bundled. Matrix cache keys exclude weather but include scene, materials, sensors, and ambient controls. See [the scientific validation contract](validation/daylight-radiance.md).

The standalone CLI audits OBJ meshes without Rhino. It supports vertices, triangle faces, quad faces, positive indices, relative-negative indices, and `v/vt/vn` face syntax. Quads use a deterministic diagonal. N-gons above four vertices fail explicitly because silent fan triangulation can corrupt concave polygons.

## VAYU portable compute

Inspect, clear, or atomically replace the process-wide checksummed scene-cache policy:

```powershell
./xvarna.exe cache status --json
./xvarna.exe cache configure D:\xvarna-cache 268435456 4294967296
./xvarna.exe cache clear
```

Budgets are bytes and may be zero to disable that tier. `clear` removes only XVARNA `.xvc` entries from the configured directory; it does not recursively delete the directory or unrelated files. Status includes memory/disk hits, misses, writes, evictions, corruption recovery, occupancy, budgets, and the last event.

Discover every adapter visible through wgpu without creating a scene:

```powershell
./xvarna.exe devices --json
```

Run the same ordered ray batch through canonical f64 CPU traversal and the selected VAYU backend, then compare hit state, exact source identity, and distance:

```powershell
./xvarna.exe backend-benchmark context.obj `
  --backend auto --adapter "NVIDIA" `
  --rays 1048576 --max-rays-per-dispatch 262144 `
  --geometry-chunk-mb 256 --gpu-memory-mb 1024 `
  --warmup 3 --iterations 10 `
  --max-error 0.00025 --parity-tolerance 0.000001 --json

./xvarna.exe backend-benchmark context.obj `
  --backend cpu --rays 262144 --threads 0 --json
```

`auto` attempts portable GPU and reports any creation/runtime fallback; `gpu` makes GPU availability and dispatch success mandatory; `cpu` runs the reference path. Adapter matching is case-insensitive against the reported device name. `--geometry-chunk-mb` bounds each two-level scene chunk, `--gpu-memory-mb` bounds the geometry working set, and `--no-cache` bypasses scene-plan cache lookup/write for reproducible cold runs. The report separates one cold arena-allocation request from configurable warm-up and measured iterations, including BLAS/instance counts, chunking/streaming, cache hit, upload bytes, min/p50/p95/mean totals, transfer phases, scratch generations/capacity/bytes, reuse, fallback, and device health. A parity violation fails the command. See [the VAYU validation contract](validation/portable-gpu.md), [production-throughput protocol](validation/vayu-production-throughput.md), and [0.13 lifecycle contract](validation/zamyad-vayu-lifecycle.md).

## DAENA spatial visibility

All OBJ and endpoint coordinates below use metres. A viewpoint contains position, plane normal, and forward direction. Boundary CSV retains every radial endpoint and first-hit object/instance/mesh/triangle.

```powershell
./xvarna.exe isovist context.obj `
  --viewpoint 0,0,1.6,0,0,1,1,0,0 `
  --viewpoint 5,0,1.6,0,0,1,0,1,0 `
  --samples 1440 --fov 360 --max-distance 50 `
  --boundary-csv isovists.csv --json
```

Directed observers use `x,y,z,weight`. Targets use `x,y,z,sensitivity` for omnidirectional exposure or append `fx,fy,fz` for directional exposure.

```powershell
./xvarna.exe intervisibility context.obj `
  --observer 0,0,1.6,1 --observer 5,0,1.6,0.75 `
  --target 10,0,1.6,0.9,-1,0,0 `
  --privacy-reference 8 --facing-exponent 2 `
  --matrix-csv privacy-matrix.csv --json

./xvarna.exe visibility-graph context.obj `
  --node 0,0,1.6 --node 2,0,1.6 --node 4,0,1.6 `
  --max-distance 25 --pairs-csv graph-pairs.csv `
  --metrics-csv graph-metrics.csv --json

./xvarna.exe visibility-graph campus.obj `
  --node 0,0,1.6 --node 2,0,1.6 --node 4,0,1.6 `
  --sparse --max-distance 12 --max-neighbors 16 --json
```

All three commands accept `--category-mask` and `--threads`. Exact graph distance is unbounded unless `--max-distance` is supplied; it materializes all pairs and is capped at 4,096 nodes. `--sparse` requires a finite distance, enables the spatial 27-cell search, defaults to 16 maximum neighbors, and supports up to 100,000 nodes without emitting out-of-range pairs. Sparse centrality is disabled by default; pass options through the API for opt-in graphs of at most 4,096 nodes. See [the method contract](validation/product-experience-release-quality.md).

## DAENA/HVARE intelligence 0.14

Material values use `id,visible-transmittance,solar-transmittance,reflectance`; assignments use `object-id,material-id`. Unassigned objects remain opaque.

```powershell
./xvarna.exe isovist-3d context.obj `
  --viewpoint 0,0,1.6 --samples 4096 --max-distance 50 `
  --top-k 10 --counterfactual 5 `
  --material 1,0.7,0.5,0.2 --assign 17,1 --json

./xvarna.exe landmark-visibility context.obj `
  --observer 1,0,0,1.6,1,0,0,0,0,1 `
  --landmark 7,20,0,8,2,1 --samples 128 --json

./xvarna.exe solar-scenarios context.obj `
  --sensor 1,0,0,1,0,0,1 --sun 0,0.5,0,0.866,1,1 `
  --scenario 1 --scenario 2 `
  --material 1,0.7,0.5,0.2 --assign 17,1 --json

./xvarna.exe solar-envelope context.obj `
  --sensor 1,0,0,1,0,0,1 --candidate 1,5,0,0,0.707,0,0.707 `
  --sun 0,0.5,0,0.866,1,1 --radius 0.5 `
  --preserve 0.8 --shade 0.5 --json
```

`--sun` values are `unix-seconds,dx,dy,dz,duration-hours,weight`. Candidate values are `id,x,y,z` for backward-compatible World Z or `id,x,y,z,axis-x,axis-y,axis-z` for freeform screening. The first solar scenario is the baseline; material/assignment options belong to the most recent `--scenario`. Solar Envelope is an early-massing proxy, not code compliance. See [the component contract](components/xv-solar-envelope.md).

## DAENA advanced view

Observer values are `id,x,y,z,fx,fy,fz,ux,uy,uz,weight`. Each target triangle is `id,ax,ay,az,bx,by,bz,cx,cy,cz,category,weight`; repeat triangles with the same ID to form a logical target.

```powershell
./xvarna.exe target-view context.obj `
  --observer "1,0,0,1.6,1,0,0,0,0,1,1" `
  --target "7,15,-2,0,15,0,4,15,2,0,2,0.9" `
  --green-mask 2 --samples 64 --two-sided `
  --backend auto --adapter "NVIDIA" --verify-cpu `
  --parity-tolerance 0.000001 --json

./xvarna.exe view-corridor context.obj `
  --corridor "1,0,0,1.6,50,0,10,0,0,1,3" `
  --samples 4096 --backend auto --verify-cpu --json

./xvarna.exe observer-path context.obj `
  --path "1,0,0,1,1;0,0,1.6;10,0,1.6;20,5,1.6" `
  --target "7,30,-2,0,30,0,4,30,2,0,2,1" `
  --spacing 0.5 --green-mask 2 --two-sided `
  --backend auto --verify-cpu --json
```

Advanced inputs and OBJ coordinates are metres. Controls include FOV, maximum distance, target/occluder/green masks, distance reference/exponents, clearance, sidedness, spacing, sample count, and worker threads. All three commands also accept `--backend auto|gpu|cpu`, `--adapter`, `--low-power`, `--max-error`, `--max-triangles`, `--max-rays-per-dispatch`, `--geometry-chunk-mb`, `--gpu-memory-mb`, and `--no-cache`. `--verify-cpu` compares complete domain metrics and attribution against canonical f64, uses `--parity-tolerance` as an absolute numeric ceiling, and fails the command on a mismatch. JSON contains `execution` and `cpuParity` objects. See [the method contract](validation/advanced-view-and-optimization.md) and [VAYU domain validation](validation/vayu-domain-acceleration.md).

## VAHMAN Study and optimization

Study rows use `id,generation;parameters;objectives;constraints`. The final constraints group may be empty but the final semicolon remains.

```powershell
./xvarna.exe study-rank `
  --variable "1,continuous,0,1" `
  --variable "2,integer,1,20" `
  --direction min --direction max `
  --candidate "1,0;0.2,4;0.31,0.76;-0.1" `
  --candidate "2,0;0.8,12;0.55,0.93;0.03" `
  --reference "1,0" --json

./xvarna.exe optimizer-benchmark --population 64 --generations 50 --seed 42 --json
```

`study-rank` returns constraint-aware fronts, crowding, feasible Pareto IDs, descriptive Spearman coefficients, and optional exact two-objective hypervolume. `optimizer-benchmark` is a deterministic end-to-end reference workload; application-specific external evaluation uses the Rust, C, .NET, or Grasshopper ask/tell API.

### Persistent Study workspace and portable report

The 0.16 platform uses a manifest and durable external-evaluation batches:

```powershell
./xvarna.exe study-schema study-manifest.schema.json
./xvarna.exe study-init study-workspace manifest.json --json
./xvarna.exe study-batch study-workspace --output batch.json --json

# Evaluate every candidate from batch.json in Grasshopper or another program,
# then write one aligned result per candidate to evaluations.json.
./xvarna.exe study-commit study-workspace evaluations.json --json
./xvarna.exe study-status study-workspace --json
./xvarna.exe study-report study-workspace --output study-report `
  --bootstrap 1000 --permutations 1000 `
  --confidence 0.95 --alpha 0.05 --seed 1701 --json
```

`study-batch` is resumable: if a pending batch exists it returns the exact same IDs, values, and content hash. `study-commit` requires a complete batch but permits an identical retry. `objectiveValues` contains one array per registered objective, so a scalar is `[value]` and a vector retains all declared components. `metricFields` can carry aligned values/positions for baseline delta maps.

The report directory contains `study-data.json`, `variants.parquet`, `objective-space.gltf`, `report.html`, and `viewer.html`. The HTML files are self-contained and network-free. Statistical output is Spearman association with bootstrap intervals, permutation p-values, and Holm correction—not causal sensitivity. The executable fixture and expected JSON shape are in `validation/study/platform-manifest.json` and `eng/smoke-study-platform.ps1`.

## Deterministic surface grid

```powershell
./xvarna.exe surface-grid roof.obj sensors.csv `
  --cell-size 0.5 --offset 0.001 --max-cells 1000000 `
  --mesh-obj analysis-cells.obj

./xvarna.exe surface-grid roof.obj sensors.csv --cell-size 1.0 --json
```

OBJ coordinates, cell size, offset, CSV coordinates, and optional output OBJ use metres. The CSV retains every cell corner, offset sensor, normal, exact area, stable Sensor ID, source face, and subdivision depth. The report verifies area conservation, achieved maximum edge, skipped faces, elapsed time, and deterministic hash. `--first-id` selects the first non-zero ID; exceeding `--max-cells` fails explicitly.

## MEHR annual irradiance

OBJ and sensor coordinates are metres. EPW coordinates and fixed local-standard timezone come from the file header. Repeat `--sensor x,y,z,nx,ny,nz` for any number of oriented surfaces.

```powershell
./xvarna.exe annual-irradiance context.obj weather.epw `
  --sensor 12.5,4.0,1.2,0,0,1 `
  --sensor 12.5,4.0,1.2,0,1,0 `
  --north 15 --threads 8

./xvarna.exe annual-irradiance context.obj weather.epw `
  --sensor 12.5,4.0,1.2,0,0,1 `
  --timeline-csv irradiance.csv --json
```

The human and JSON summaries report direct, sky-diffuse, ground-reflected and global kWh/m², peak W/m²/time, dome and horizon visibility, dominant solar/sky occluders, attributed lost energy, EPW diagnostics, runtime, and deterministic hashes. `--timeline-csv` writes every component and first-hit identity for every sensor/interval. Controls include `--albedo`, `--no-weather-albedo`, `--offset`, `--max-distance`, `--category-mask`, `--north`, `--delta-t`, `--pressure`, `--temperature`, `--min-altitude`, and `--threads`.

## ASMAN Sky View and Shadow Mask

OBJ coordinates and sensor coordinates are interpreted as metres. Repeat `--sensor` for batch analysis; each value is `x,y,z,nx,ny,nz`. The first triplet is position and the second is an oriented normal.

```powershell
./xvarna.exe sky-view context.obj `
  --sensor 12.5,4.0,1.2,0,0,1 `
  --sensor 13.5,4.0,1.2,0,0,1 `
  --samples 8192 --seed 42 --threads 8

./xvarna.exe sky-view context.obj `
  --sensor 12.5,4.0,1.2,0,0,1 `
  --samples 4096 --mask-csv shadow-mask.csv --json
```

The summary contains cosine-weighted Sky View Factor, unweighted visible hemisphere, visible solid angle, convergence delta, visible/blocked counts, dominant object and projected contribution, analysis time, and BLAKE3 result hash. `--mask-csv` writes every direction plus first-hit object/instance/mesh/triangle/distance attribution. Optional controls are `--offset`, `--max-distance`, `--category-mask`, `--seed`, and `--threads`; sample count is limited to 16–262,144 per sensor.

## HVARE solar position

```powershell
./xvarna.exe sun-position --latitude 39.742476 --longitude -105.1786 `
  --elevation 1830.14 --time 2003-10-17T19:30:30Z `
  --delta-t 67 --pressure 820 --temperature 11

./xvarna.exe sun-position --latitude 35.6892 --longitude 51.3890 `
  --time 2026-06-21T09:00:00+03:30 `
  --time 2026-06-21T12:00:00+03:30 --north 15 --json
```

Every `--time` must be RFC 3339 with an explicit UTC offset. Repeat it for an ordered set. Common interval provenance can be set with `--duration-hours` and `--weight`; atmosphere/orientation controls are `--delta-t`, `--pressure`, `--temperature`, `--north`, and `--min-altitude`. The vector points from the observer toward the sun; before north rotation its axes are +X east, +Y true north, and +Z up.

## Audit

```powershell
./xvarna.exe mesh-check model.obj
./xvarna.exe mesh-check model.obj --tolerance 0.001 --json
```

Exit code `0` means the geometry is analysis-ready. Exit code `1` means the model was read successfully but contains faces requiring repair, or the file could not be read. Invalid command options return `2`.

## Conservative repair

```powershell
./xvarna.exe mesh-repair model.obj model.repaired.obj --tolerance 0.001
```

The command writes a new OBJ and reports exact face/vertex removals plus BLAKE3 hashes before and after. It performs no welding, hole filling, or hidden source mutation.

## Core/time interoperability — 0.17 development line

```text
xvarna mesh-convert <INPUT> <OUTPUT> [--weld <METRES>] [--repair-winding] [--clip <minx,miny,minz,maxx,maxy,maxz>] [--json]
xvarna scene-pack <INPUT> <OUTPUT.xvscene|json> [--weld <METRES>] [--repair-winding] [--dynamic] [--json]
xvarna scene-unpack <INPUT.xvscene|json> <OUTPUT.obj|stl|ply|gltf|glb> [--json]
xvarna weather-convert <INPUT.epw|wea|csv> <OUTPUT.wea|csv> [--missing reject|zero|interpolate] [--drop-leap] [--preserve-gaps] [--json]
```

`mesh-convert` reads OBJ, ASCII/binary STL, ASCII/little-endian PLY, embedded glTF 2.0, or GLB 2.0. Output format follows the file extension. Mutation remains opt-in: `--weld` merges vertices only within the declared Euclidean tolerance, `--repair-winding` propagates manifold normal consistency and orients closed components, and `--clip` performs exact six-plane triangle clipping.

`scene-pack` compiles a canonical-metre scene and writes either tracked JSON or the checksummed XVSCN binary envelope. `scene-unpack` verifies schema/version/checksum, rebuilds the scene, expands instances, and writes a standard mesh format. Backend-specific BVHs and GPU buffers are never serialized as authoritative scene data.

`weather-convert` imports EPW, Radiance WEA, or versioned XVARNA weather CSV. Missing radiation, leap days, and timeline gaps require explicit policy. WEA DNI/DHI values are integrated over inferred interval duration; GHI is reconstructed from the NREL-SPA sun direction and remains labeled as reconstructed provenance.

## ZAMYAD scene benchmark and validation

```powershell
./xvarna.exe scene-bench model.obj
./xvarna.exe scene-bench model.obj --rays 1000000 --threads 8 --json
```

The command audits the mesh, compiles a deterministic BLAS/TLAS scene, traces a top-down ray grid, and compares the first 2,048 results against the independent analytic triangle reference kernel. Output includes build time, native memory, hierarchy size/depth, worker count, query throughput, hit count, mismatch count, and deterministic scene hash. Exit code `0` requires zero differential mismatches; benchmark numbers must be collected from a Release build on an otherwise idle machine.

## Diagnostics

```powershell
./xvarna.exe info --json
./xvarna.exe self-test
```
