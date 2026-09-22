# XV Isovist

`XV Isovist` computes one or more oriented planar isovists through the native DAENA engine. Supply an immutable `XV Scene`, eye points, and optional per-eye plane normals and forward vectors. The defaults are world Z, world X, 360°, 720 rays, and the document tolerance as eye offset.

The component returns an immutable result, closed Rhino polylines, branch-aligned radial lines and distances, area, perimeter, compactness, mean depth, full-versus-half-sample area convergence, dominant blocker object, open-at-limit fraction, hash, and method report. A partial FOV closes through the eye; a full FOV closes the ordered radial boundary directly.

Increase `Samples` when thin geometry or a large convergence delta matters. `Maximum Distance` is required and finite: open rays end there, so it is part of the scientific question rather than a display-only setting. Use `Category Mask` to compare selected context classes without rebuilding geometry.

See [the DAENA scientific contract](../validation/spatial-visibility-and-privacy.md) for equations, approximation labels, validation, and non-claims.
