# XV View Path

Evaluates DAENA Target/Weighted/Green View along one or more ordered Rhino polyline curves. Connect `XV Scene` for canonical f64 execution or a cached `XV Backend` for VAYU acceleration. Cameras face the local non-zero segment tangent and use the supplied Up vector. The maximum sample spacing controls path resolution.

Outputs provide distance-weighted means, sample points and tangent vectors, complete metric series, best/worst hotspots, immutable result, hash, and assumptions report including backend/adapter/batching/precision/fallback provenance. Rays for every sample of one path are aggregated into a domain batch. Target setup follows `XV Target View`. Curves must be polylines in 0.11; no speed or time weighting is inferred.

See [the scientific contract](../validation/advanced-view-and-optimization.md).
