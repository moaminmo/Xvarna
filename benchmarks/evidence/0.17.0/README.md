# XVARNA 0.17 portable GPU evidence

This is a reproducible engineering microbenchmark, not a competitor benchmark and not a claim about whole-building speed. It uses the tracked two-triangle analytic mesh so parity and transfer/runtime behaviour remain auditable. Timings are warm p50 milliseconds and include upload plus execution plus readback; lower is better.

| Rays | NVIDIA GPU | CPU paired with NVIDIA run | Intel GPU | CPU paired with Intel run | NVIDIA state / identity mismatches | Intel state / identity mismatches |
|---:|---:|---:|---:|---:|---:|---:|
| 4096 | 0.259 | 0.308 | 0.887 | 0.187 | 0 / 0 | 0 / 0 |
| 65536 | 3.625 | 2.096 | 11.647 | 1.22 | 0 / 0 | 0 / 0 |
| 262144 | 13.947 | 4.3 | 30.58 | 3.42 | 0 / 0 | 0 / 0 |

Both adapters completed without fallback, device loss, state mismatch, or identity mismatch. On this tiny two-triangle corpus CPU is faster at the larger batches because transfer dominates; the evidence must not be presented as a GPU speedup claim. Representative architectural scenes, AMD hardware, thermal/power controls, and third-party competitor workflows remain separate launch gates.

Regenerate from the repository root with:

~~~powershell
.\eng\capture-public-evidence.ps1
~~~
