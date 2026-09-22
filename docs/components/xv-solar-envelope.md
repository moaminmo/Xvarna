# XV Solar Envelope

Projects the baseline direct-sun rays of protected oriented sensors against freeform circular columns. Every candidate has its own normalized build axis; omitted axes default to World Z, preserving older definitions. The native engine solves the closest point between every eligible solar ray and candidate axis, rejects constraints outside the column radius or positive build direction, traces material transmission up to that point, and applies transmission-weighted quantiles.

`Maximum Access Height` and `Minimum Shading Height` are distances along the candidate axis. The point outputs are reconstructed as `base + normalized axis × distance`; absolute Z is retained in the immutable result for backward compatibility. Candidate Radius is perpendicular to the axis, Preserve and Shade are fractions of considered baseline hours, and Axis Clearance shifts the limit conservatively along the axis. Each threshold retains its controlling sensor and UTC interval.

Infinite outputs mean no applicable constraint. This is deterministic early-massing guidance—not arbitrary-solid optimization, diffuse/radiosity simulation, zoning analysis, or code compliance.
