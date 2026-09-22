# Repository map

```text
crates/
  xvarna-types/       Stable IDs, result states, ABI versions
  xvarna-geometry/    Canonical geometry and reference algorithms
  xvarna-io/          Auditable OBJ import/export and conversion reports
  xvarna-scene/       Immutable BLAS/TLAS scene plus backend-neutral ray-executor contract
  xvarna-cache/       Checksummed bounded memory/disk LRU and corruption recovery
  xvarna-vayu/        Portable wgpu/WGSL compute, bounded dispatch and explicit CPU fallback
  xvarna-daena/       Spatial visibility, solid-angle views, corridors and observer paths
  xvarna-study/       VAHMAN Pareto analysis and mixed-variable ask/tell optimization
  xvarna-ffi/         Stable C ABI exported as xvarna_core
  xvarna-cli/         Host-independent diagnostics and validation
dotnet/
  Xvarna.Native/      Managed loader, safe scene/compute/optimizer handles, and ABI checks
  Xvarna.Grasshopper/ Rhino 8/9 Grasshopper connector
  Xvarna.Tests/       Managed contract tests
eng/                  Reproducible developer and packaging scripts
docs/adr/             Accepted architecture decisions
```

The dependency direction is always host connector → C ABI → Rust engine. DAENA depends on the backend-neutral executor in `xvarna-scene`; VAYU implements that contract without DAENA depending on wgpu. The core must never reference RhinoCommon, Grasshopper, UI code, or document state.
