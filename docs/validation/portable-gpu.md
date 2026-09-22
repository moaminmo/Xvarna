# VAYU Portable GPU Validation Contract

## Scope

XVARNA 0.10 introduces portable closest-hit and early-exit any-hit traversal. The contract covers adapter discovery, origin-rebased f32 snapshot conversion, global BVH upload, WGSL traversal, source attribution, deterministic chunk ordering, precision/resource rejection, and explicit CPU fallback.

It does not yet claim that every DAENA/HVARE high-level metric is dispatched through VAYU. In 0.10 the public accelerated product workflow is `XV Backend` → `XV Ray Query`, the equivalent C/.NET compute session, and the CLI parity/benchmark path.

## Numeric contract

The canonical ZAMYAD scene and CPU reference remain f64. Before GPU upload:

1. the canonical scene rebase origin is retained;
2. each effective triangle is transformed into rebased world coordinates;
3. every coordinate is converted to f32;
4. the maximum observed absolute conversion error is measured in metres;
5. snapshot construction fails if that error exceeds `maximum_precision_error_meters`;
6. BVH bounds are conservatively expanded before their own f32 conversion.

Every query origin is rebased and its origin, direction, `t_min`, and `t_max` conversion errors are measured. Required-GPU mode rejects a batch above budget. Auto mode reruns the complete ordered batch on canonical CPU and marks runtime fallback.

GPU triangle intersection uses two-sided Möller–Trumbore arithmetic in f32. CPU remains f64. Therefore equality of barycentric/distance bit patterns is not required. Acceptance requires:

- equal hit/miss state outside declared geometric tolerance cases;
- equal object, instance, mesh, and triangle identity for mutual hits;
- absolute distance error no larger than the configured parity tolerance;
- full output order equal to input order across chunks.

## Resource and failure policy

- effective portable triangles default cap: 20,000,000;
- portable BVH leaf size: 4;
- shader traversal stack: 96 u32 node indices;
- snapshot construction rejects hierarchy depth above the stack contract;
- requested rays per dispatch are clamped to adapter storage-binding and workgroup limits;
- buffer-size arithmetic is checked before allocation;
- required GPU returns errors for adapter, device, precision, or execution failure;
- Auto records creation fallback and may record per-batch runtime fallback;
- CPU mode performs no adapter/device initialization.
- optional adapter-name matching is case-insensitive and identical across Rust, C, .NET, CLI, and Grasshopper; mismatch is an error in required-GPU mode and a recorded fallback in Auto, while CPU rejects the meaningless filter.

The portable snapshot expands instances in 0.10. This is disclosed in triangle/memory metadata and protected by the hard cap.

## Differential evidence on the development host

The 2026-09-01 development-host smoke runs used `validation/meshes/open_quad.obj` with vertical low-discrepancy rays:

| Adapter | API selected | Rays | Dispatches | State mismatches | Identity mismatches | Max distance error |
|---|---|---:|---:|---:|---:|---:|
| NVIDIA GeForce RTX 5060 Laptop GPU | Vulkan | 262,144 | 4 | 0 | 0 | `1.0876911e-8 m` |
| Intel Raptor Lake-S Mobile Graphics Controller | Vulkan | 131,072 | 1 | 0 | 0 | `1.0876911e-8 m` |

The observed maximum scene/ray conversion error was `2.1753821e-8 m`. These are real-device smoke results, not a claim of universal driver parity. The automated Rust test executes 4,096 differential rays when a portable adapter exists and otherwise keeps CPU/fallback coverage runnable on headless CI.

## Performance reporting

`backend-benchmark` reports CPU throughput and end-to-end backend throughput together with separate upload, execution, and readback durations. Shader-only timing is not substituted for user-visible latency. The two-triangle validation mesh is intentionally a correctness case and shows transfer-dominated GPU behavior; it is not used to claim speedup.

Performance claims require Release builds, warmed runs, the master-spec reference workloads, fixed power state where practical, adapter/driver disclosure, and raw machine-readable output.

## Primary technical references

- wgpu device/queue and adapter contracts: <https://docs.rs/wgpu/30.0.1/wgpu/>
- WebGPU specification: <https://www.w3.org/TR/webgpu/>
- XVARNA canonical CPU/reference contracts in `docs/validation/` and `XVARNA_MASTER_SPEC.md`
