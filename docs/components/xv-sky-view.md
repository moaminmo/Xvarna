# XV Sky View

Category: `XVARNA > 03 Solar`

`XV Sky View` is the task-based ASMAN analysis component. It evaluates oriented points against an immutable `XV Scene` and returns aggregate sky metrics plus a complete sensor-major Shadow Mask retained in Result.

## Inputs

- **Scene**: immutable `XV Scene` output.
- **Sensors**: one or more model-space points.
- **Normals**: one broadcast vector or one per sensor.
- **Sensor IDs**: empty for `1..N`, one unsigned 64-bit start ID, or one exact ID per sensor.
- **Samples**: 16–262,144; default 2,048.
- **Seed**: exact unsigned 64-bit azimuth-phase seed.
- **Offset**: model-space normal offset; zero resolves to the active document tolerance.
- **Maximum Distance**: model-space obstruction range; zero is unbounded.
- **Category Mask**: exact unsigned 64-bit instance filter; `-1` selects all categories.
- **Run**: pauses or executes analysis.

## Outputs

- **Result**: complete ASMAN result for extraction and downstream APIs.
- **Sky View Factor**: Lambert cosine-weighted fraction `[0,1]`.
- **Visible Hemisphere**: unweighted visible solid-angle fraction `[0,1]`.
- **Visible Solid Angle**: steradians in `[0,2π]`.
- **Convergence Delta**: interleaved half-sample absolute SVF difference; not a confidence interval.
- **Dominant Occluder / Dominant Blocked Fraction**: object ID and its projected-cosine contribution.
- **Visible Count / Blocked Count**: exact state counts.
- **Heatmap**: Viridis colours matching the viewport points.
- **Hash / Report**: deterministic BLAKE3 identity and an auditable policy/runtime summary.

The component derives from Grasshopper's task-capable component contract. Cancelling or superseding a solution propagates a cancellation token through .NET to the native job flag; Rust observes it between bounded ray chunks. Completed results are emitted only on the Grasshopper solution thread.

Managed clients that need live progress can call `XvarnaScene.BeginSkyView`, read `SkyViewAnalysisJob.Progress`, request `Cancel`, and await `Completion`. Simpler clients can use `AnalyzeSkyView` or `AnalyzeSkyViewAsync`.

For the equations, convergence meaning, analytic acceptance cases, and limits, see [the ASMAN validation contract](../validation/sky-view-and-shadow-mask.md).
