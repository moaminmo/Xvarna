# VAYU Production Throughput Validation

## Contract

XVARNA 0.12 validates a retained synchronous GPU pipeline, not a universal performance claim. Every benchmark case must preserve canonical hit/miss state, complete object/instance/mesh/triangle identity, declared distance tolerance, input ordering, bounded memory, and explicit backend/fallback provenance.

## Benchmark phases

`xvarna backend-benchmark <mesh.obj>` performs four separate phases:

1. canonical CPU repetitions for the reference output and p50 throughput;
2. one cold selected-backend request, including first lazy arena allocation;
3. configurable unmeasured warm-up requests;
4. configurable measured warm requests producing min, p50, p95, and mean total time plus upload, execution, and readback p50.

The JSON report also records arena generation/capacity/bytes, reusable batches, cumulative upload/readback bytes, runtime fallbacks, device health, device-loss count, execution-error count, and the last diagnostic.

## Reproducible runner

Build Release artifacts, then run:

```powershell
./eng/build-native.ps1 -Configuration Release
./eng/build-cli.ps1 -Configuration Release
./eng/benchmark-vayu-throughput.ps1 -Backend gpu -Adapter "NVIDIA" -EvidenceLabel nvidia
./eng/benchmark-vayu-throughput.ps1 -Backend gpu -Adapter "Intel" -EvidenceLabel intel
```

The default corpus executes 4,096, 65,536, and 262,144 rays with two warm-up and five measured iterations. Evidence is written under `artifacts/validation/`. Required-GPU cases fail unless:

- the selected backend is portable GPU;
- parity is accepted with zero state and identity mismatch;
- device health remains true with zero device loss;
- every cold/warm request reports arena reuse;
- the arena has exactly one generation for a fixed-size case;
- the reusable-batch counter equals cold + warm-up + measured calls.

## Fault containment

Unit tests prove device-loss state is idempotent, the first loss increments the loss counter once, subsequent diagnostics remain visible, and lost health rejects GPU work. ABI and dual-runtime managed tests prove telemetry layout and CPU-session behavior. Real driver fault injection is not claimed by this milestone and remains a release gate before 1.0.

## 0.12 release smoke evidence

The Release runner passed on 2026-09-01 with Vulkan adapters NVIDIA RTX 5060 Laptop GPU (driver 591.91) and Intel Raptor Lake-S Mobile Graphics Controller (driver 101.7076). Every row used two warm-ups and five measured iterations, produced exactly one arena generation and eight reused batches, retained healthy device state with zero execution errors, and had zero state/identity mismatch. Values below are end-to-end warm p50 for the two-triangle analytic smoke scene:

| Adapter | Rays | Cold ms | Warm p50 ms | Warm p95 ms | Arena MiB |
|---|---:|---:|---:|---:|---:|
| NVIDIA RTX 5060 | 4,096 | 7.180 | 0.260 | 0.300 | 1.125 |
| NVIDIA RTX 5060 | 65,536 | 8.190 | 3.359 | 3.448 | 18.000 |
| NVIDIA RTX 5060 | 262,144 | 60.664 | 11.057 | 11.650 | 72.000 |
| Intel Raptor Lake | 4,096 | 11.129 | 2.663 | 3.101 | 1.125 |
| Intel Raptor Lake | 65,536 | 16.027 | 7.296 | 13.458 | 18.000 |
| Intel Raptor Lake | 262,144 | 39.796 | 25.760 | 27.439 | 72.000 |

The smoke geometry is too simple for a meaningful CPU/GPU traversal comparison. Its purpose here is to expose allocation/synchronization behavior and verify telemetry invariants on real devices.

## Interpretation boundary

`open_quad.obj` is an analytic correctness and pipeline-overhead smoke case. Results from it must not be advertised as representative architectural-model acceleration. A public speedup claim requires saved evidence from the future A/B/C corpus on at least one discrete and one integrated adapter, with scene complexity, driver, cold/warm settings, memory, CPU baseline, and p50/p95 disclosed.
