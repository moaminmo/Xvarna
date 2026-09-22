# XV Target View

Computes raw Target View, explicit Weighted View, and Green View Index for oriented cameras. Connect an immutable `XV Scene` for canonical f64 execution or a cached `XV Backend` for VAYU acceleration, plus eye points, camera Forward/Up vectors, and one or more target meshes. Target IDs aggregate multiple meshes/triangles into one logical object; category masks select green targets and weights declare desirability.

The component returns immutable result data, observer summaries, per-observer/per-target visibility and solid-angle trees, dominant targets, first blockers, convergence, BLAKE3 identity, and a complete method report. A backend result also records adapter, batch/ray/dispatch counts, transfer stages, precision error, and fallback. Target meshes are analysis targets, not automatically occluders; add physical geometry to `XV Scene` when it should block rays.

Use more Samples per Patch until convergence is acceptable for the decision. FOV must be below 180° in each dimension. Maximum Distance, `D50`, sidedness, and weights are assumptions—not universal defaults.

See [the scientific contract](../validation/advanced-view-and-optimization.md).
