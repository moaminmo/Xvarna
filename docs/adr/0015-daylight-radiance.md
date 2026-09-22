# ADR 0015 — HVARE photometric daylight and Radiance reference boundary

- Status: Accepted
- Date: 2026-09-02
- Milestone: 0.15.0

## Decision

XVARNA exposes two deliberately separate daylight paths over one canonical scene, sensor set, and RGB optical-material catalog.

The **HVARE Fast Path** is the interactive deterministic path. It builds a sensor-by-upper-hemisphere coefficient matrix with equal-solid-angle Fibonacci directions, projected-solid-angle weighting, exact ZAMYAD obstruction queries, per-interaction photopic transmission, and a declared isotropic or CIE-overcast sky distribution. Point-in-time direct illuminance is evaluated separately with one material-aware sun ray. Annual analysis reuses the static coefficient matrix across every EPW interval and calculates occupied sDA, direct-only ASE, four-bin UDI, and area-weighted project metrics.

The **Radiance Reference Path** exports the exact instanced scene as canonical world-space metre polygons, maps materials by exact Object ID to Radiance `plastic`, `glass`, `metal`, `trans`, or `mirror` primitives, and preserves RGB values. Point reference execution uses `oconv` and `rtrace -I+`. Annual reference execution uses a weather-independent `rfluxmtx` daylight-coefficient matrix plus `gendaymtx` and `dctimestep`. External processes are cancellable and record executable version, ordered commands, elapsed time, matrix reuse, and a content hash.

The Fast Path is not presented as a multi-bounce radiosity engine or an automatic LM-83 compliance result. Its reflectance values are retained for Radiance but do not generate interreflection in the Fast Path. A validation report compares aligned Fast Path and Radiance arrays using signed bias, MAE, RMSE, MAPE, maximum absolute error, R², and an explicit absolute/relative acceptance envelope.

## Consequences

- Interactive design gets stable, cacheable response without pretending to be Radiance.
- The same geometry, sensors, assignments, and export identity can be audited outside Rhino.
- Coefficient reuse is exact-keyed by scene, sensors, materials, and ray policy; changing weather does not invalidate the Radiance geometry matrix.
- Unassigned scene objects are intentionally opaque.
- Optical validation is channel-wise: reflectance and transmittance are finite fractions and may not create energy.
- Point and annual native jobs support cooperative cancellation and progress; Radiance children are killed and reaped when cancellation is observed.
- Radiance remains an optional external dependency and is not redistributed by XVARNA.

## Alternatives rejected

- Calling every Fast Path result “Radiance-equivalent” was rejected because it would hide the missing multi-bounce and sky-model differences.
- Embedding Radiance binaries was rejected because licensing, platform maintenance, and user-selected versions should remain explicit.
- Rebuilding annual coefficients for every weather file was rejected because geometry/material/sensor transport is weather-independent.
- Scalar-only optical materials were rejected because they cannot round-trip faithfully to Radiance or represent spectral colour differences.
