# XV Scene

**Purpose.** Compiles Rhino meshes into the immutable ZAMYAD BLAS/TLAS scene shared by all analyses.

**Inputs.** static/dynamic meshes, stable object IDs, category masks, transforms and update policy. **Outputs.** scene, revision/hash, refit/rebuild/cache state and report. Geometry uses Rhino model units.

**Method and assumptions.** Identical geometry is deduplicated into BLAS; instances form the TLAS. Dynamic deltas refit when safe and rebuild under the recorded policy. Valid non-degenerate triangles and stable IDs are required.

**Example.** Keep urban context static, shade geometry dynamic, and verify revision plus refit/rebuild state after each edit.
