# XV Compare

Compares two `XV Isovist 3D` results whose viewpoint IDs and order are aligned. Every value is candidate minus baseline: volume, openness, mean radius, and per-object obstruction fraction. A positive attribution delta means that object owns a larger share of obstruction in the candidate.

The component fails closed on misalignment; it does not interpolate or guess correspondence between unrelated studies.
