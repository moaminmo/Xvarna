# XV Mesh Check

Category: `XVARNA > 01 Scene`

`XV Mesh Check` is the first component of RASHNU, XVARNA's Model Doctor and validation subsystem. It sends packed double-precision geometry through the same fixed C ABI used by the standalone engine and returns a deterministic audit.

## Inputs

| Input | Meaning |
|---|---|
| Mesh | Rhino mesh. It is duplicated and never mutated. |
| Tolerance | Positive model-space length tolerance; `0` uses the active Rhino document tolerance. |
| Repair | Enables conservative face removal and unused-vertex compaction on the output copy. |

## Important outputs

- **Analysis Ready** permits ray-based XVARNA ingest; open occluder meshes can be ready.
- **Watertight** additionally requires closed, manifold, consistently wound topology.
- **Bad Faces/Vertices** preserve indices from the triangulated pre-repair copy.
- **Volume** is emitted only for watertight geometry; otherwise it is `NaN`.
- **Hash** is a BLAKE3 digest over canonical positions and triangle indices.
- **Precision Error** estimates f32 rounding risk for future GPU execution; a warning means rebase, chunk, or CPU/f64 fallback is required.

## Repair policy

Repair removes invalid-index, non-finite, degenerate, and duplicate faces and culls vertices unused afterward. It does not weld, fill holes, resolve self-intersections, repair non-manifold topology, or reorient components. The report records face/vertex counts and before/after hashes.
