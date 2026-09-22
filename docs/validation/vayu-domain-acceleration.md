# VAYU DAENA Domain Acceleration Validation Contract

## Scope

XVARNA 0.11 accelerates three complete DAENA workflows through the same backend-neutral ray contract:

- Target View, Weighted View, and Green View Index;
- protected View Corridor;
- Dynamic Observer Path.

Sampling, solid-angle calculation, weighting, convergence, aggregation, source attribution, and content hashing remain canonical Rust DAENA logic. VAYU replaces only the ordered closest-hit traversal batch. Direct `Scene` methods remain the f64 reference.

## Execution contract

`RayQueryExecutor` must provide:

1. the canonical immutable scene used by domain validation and hashing;
2. an immutable backend context containing adapter and creation-fallback provenance;
3. an ordered closest-hit result of exactly the requested length;
4. per-batch backend, runtime fallback, ray/dispatch count, stage timings, and precision error.

DAENA aggregates those values without discarding fallback evidence. Aggregate backend is `CPU`, `PortableGpu`, or `Mixed`. A required-GPU session fails instead of returning a hidden CPU result. Auto may fall back only with a recorded reason and count.

## Batching contract

| Workflow | Logical domain batches | Rays before device chunking |
|---|---:|---:|
| Target/Weighted/Green View | one per call | every eligible observer × target-patch sample |
| View Corridor | one per corridor | aperture sample count |
| Dynamic Observer Path | one per source path | every eligible camera-sample × target-patch sample |

VAYU splits a logical batch at the configured rays-per-dispatch/device limit. Readback order must equal ray generation order across every split. Zero-ray valid studies still produce a valid CPU/GPU provenance record without fabricated dispatches.

## CPU parity contract

The CLI options below are common to all three workflows:

```text
--backend auto|gpu|cpu
--adapter <case-insensitive name substring>
--low-power
--max-error <metres>
--max-triangles <count>
--max-rays-per-dispatch <count>
--verify-cpu
--parity-tolerance <absolute value>
```

With `--verify-cpu`, the CLI first evaluates the identical domain request through canonical f64 CPU, then through the selected session. It compares:

- every scalar summary and per-target/per-sample metric;
- ordered counts and stable source IDs;
- visible/blocked state, dominant target/blocker, and triangle attribution where applicable;
- content-hash equality as additional evidence, not as the sole numeric acceptance rule.

Acceptance requires zero discrete identity/state mismatch and maximum absolute numeric delta at or below `--parity-tolerance`. Failure returns a non-zero process code. Non-finite diagnostic values are emitted as JSON `null`, never invalid JSON tokens.

`eng/validate-vayu-domain.ps1` runs all three cases, parses their JSON, asserts schema/parity/provenance, and writes machine-readable evidence under `artifacts/validation`. Required-device matrix runs can use `-Backend gpu -Adapter <name> -EvidenceLabel <label>`.

## Cross-language contract

- Rust: generic DAENA `*_with_executor` entry points and canonical scene wrappers.
- C: `xv_compute_target_view`, `xv_compute_view_corridor`, `xv_compute_observer_path`, and fixed-layout `xv_daena_execution_info`.
- .NET 8/10: matching `XvarnaComputeSession.Analyze*` methods and `DaenaExecutionReport`.
- Rhino 8/9: unchanged component GUIDs/output indices; the first input accepts either `XV Scene` or `XV Backend`; reports disclose execution provenance.
- CLI: schema 0.11 `execution` and optional `cpuParity` objects.

## Development-host evidence

The 2026-09-01 release smoke cases used `validation/meshes/open_quad.obj` and forced multiple 64-ray dispatches:

| Workflow | Adapter | Rays | Dispatches | Metric max delta | Identity/state mismatch | Hash match |
|---|---|---:|---:|---:|---:|---:|
| Target/Weighted/Green View | NVIDIA RTX 5060 / Vulkan | 256 | 4 | `0` | 0 | yes |
| Target/Weighted/Green View | Intel Raptor Lake-S / Vulkan | 256 | 4 | `0` | 0 | yes |
| View Corridor | NVIDIA RTX 5060 / Vulkan | 256 | 4 | `0` | 0 | yes |
| View Corridor | Intel Raptor Lake-S / Vulkan | 256 | 4 | `0` | 0 | yes |
| Dynamic Observer Path | NVIDIA RTX 5060 / Vulkan | 320 | 5 | `0` | 0 | yes |
| Dynamic Observer Path | Intel Raptor Lake-S / Vulkan | 320 | 5 | `0` | 0 | yes |

Maximum measured conversion error in these domain runs was below `1.20e-7 m`. These are correctness smoke results for the named adapters and driver state, not universal performance or driver claims.

## Non-claims and remaining gates

- No universal GPU speedup is claimed from the small analytic corpus.
- No perceptual validity, legal view-right determination, or causal health claim is added.
- DAENA remains a deterministic geometric evidence engine, not a renderer.
- Large production-scene performance corpus, warm/cold distributions, device-loss fault injection, persistent/overlapped buffers, and two-level portable instancing remain open before the flagship performance claim.
