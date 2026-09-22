# XV View Corridor

Tests the open fraction of a circular target aperture from each origin. Supply `XV Scene` for canonical f64 execution or `XV Backend` for VAYU acceleration, origin/target pairs, aperture radius, optional up vectors, deterministic sample count, clearance, and scene category mask.

Outputs include open fraction and solid angle, convergence, dominant blocker/share, nearest conflict, aperture sample points, per-sample state, exact first-blocker IDs, result hash, and a report containing backend/adapter/batching/precision/fallback provenance. The aperture is perpendicular to the origin-to-target axis; the component evaluates geometric obstruction and does not decide legal protection.

See [the scientific contract](../validation/advanced-view-and-optimization.md).
