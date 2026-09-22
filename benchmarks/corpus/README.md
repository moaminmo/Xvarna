# XVARNA public benchmark corpus

The S/M/L corpus is deterministic and generated on demand so repository clones and review packages remain small. All generated geometry and sensor coordinates are dedicated under CC0-1.0. The generator writes hashes and exact counts beside the files; benchmark reports retain those identities.

```powershell
./eng/generate-benchmark-corpus.ps1 -Tier All
./eng/run-phase2-evidence.ps1 -Profile Full
```

`S`, `M`, and `L` target approximately 100k/1M/5M triangles and exactly 10k/100k/250k sensors. They are synthetic workload controls, not substitutes for the three external architectural case studies collected during review. Generated files live in ignored `artifacts/benchmark-corpus` and may be removed after evidence capture.

Analytic/pathological fixtures remain tracked under `validation/meshes`. Every published result must disclose OS, CPU/GPU, driver, backend, cold/warm policy, hashes, memory/VRAM and failures; slower or failed rows must not be removed.
