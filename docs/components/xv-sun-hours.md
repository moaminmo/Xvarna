# XV Sun Hours

Category: `XVARNA > 03 Solar`

`XV Sun Hours` evaluates oriented sensor points against an immutable `XV Scene` and `XV Sun Vectors` result. For each active, front-facing sensor/time pair it traces a ZAMYAD closest-hit ray and classifies the pair as visible or blocked.

Metrics are weighted sums:

```text
Direct Sun Hours = Σ(duration × weight) for visible eligible pairs
Shadow Hours     = Σ(duration × weight) for blocked eligible pairs
Eligible Hours   = Direct Sun Hours + Shadow Hours
Solar Access     = Direct Sun Hours / Eligible Hours
```

The normal-offset is expressed in Rhino document units and defaults to document tolerance when the input is zero. Maximum Distance zero means unbounded. Maximum Incidence Angle defaults to 90 degrees. Category Mask filters scene instances without rebuilding the scene.

The full sensor-major timeline preserves inactive, back-facing, visible, and blocked state. Every blocked entry carries its first-hit object, instance, mesh, triangle, and distance internally; the Grasshopper occluder tree exposes the exact object ID. Dominant Occluder aggregates weighted blocked hours by object with a deterministic ID tie-break.
