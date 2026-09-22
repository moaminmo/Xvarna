# XVARNA threat model

## Assets and trust boundaries

XVARNA protects user geometry, weather data, study parameters/results, local filesystem integrity, Rhino process stability and provenance. Inputs cross from untrusted OBJ/EPW/JSON/cache/Grasshopper documents into Rust parsers, the C ABI, managed connectors, GPU buffers, child Radiance processes and HTML/glTF/Parquet exports. Review packages are local and contain no product telemetry uploader.

## Principal threats and controls

| Threat | Control | Residual risk |
|---|---|---|
| Malformed or huge input exhausts memory/time | finite checks, typed schemas, triangle/ray/sensor budgets, chunking, cancellation | deliberate worst cases can still consume the declared budget |
| Unsafe FFI lengths/pointers | narrow C ABI, validation before slices, negative tests | native callers remain responsible for valid ownership/lifetime |
| Cache poisoning/corruption | versioned schema, BLAKE3 identity/checksum, atomic writes, corrupt-entry removal, bounded eviction | local attacker with user privileges can replace binaries/data |
| Path traversal/overwrite | caller-visible output roots and guarded packaging scripts | user-selected external paths remain trusted decisions |
| GPU/device failure | bounded allocations, device diagnostics, explicit Auto fallback or Require-GPU failure | driver defects are outside project control |
| Radiance command injection/process leak | argument-list execution, explicit executable discovery, cancellation of process tree, provenance | third-party Radiance binaries must be trusted and patched |
| Stale or misleading scientific output | scene/result hashes, generations, stale suppression, method/fidelity/limitations in Evidence Passport | downstream screenshots can omit provenance |
| HTML report injection | structured serialization and self-contained viewer; no remote CDN | future free-text fields require continued escaping review |
| Supply-chain substitution | lockfiles, checksums, SBOM, release manifest; signing before public release | review candidate is checksummed but not publicly signed |

## Response

Security reports follow `SECURITY.md`. S0 distribution is stopped immediately; affected hashes are added to the revocation record, fixed artifacts receive a new immutable version, and users are told whether results must be recomputed.

