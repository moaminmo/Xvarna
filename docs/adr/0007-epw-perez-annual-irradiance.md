# ADR 0007: EPW midpoint semantics and component-shaded Perez irradiance

Status: Accepted for XVARNA 0.6.

## Context

An annual surface-radiation feature must be fast enough for interactive computational-design studies, traceable to a published model, explicit about EPW interval semantics, and capable of explaining which geometry removed energy. A uniform-isotropic sky is too weak for a flagship workflow. A full angular all-weather luminance implementation or multi-bounce renderer would materially expand the validation and runtime scope.

## Decision

- Parse EPW inside the Rust ZURVAN domain and retain radiation as interval Wh/m².
- Interpret EPW timestamps as local-standard interval ends and evaluate solar position at the UTC interval midpoint.
- Use the documented EnergyPlus/Perez 1990 tilted-surface decomposition: isotropic dome, circumsolar, and horizon terms.
- Precompute diffuse obstruction per sensor using 144 projected sky-dome rays and 24 projected horizon rays.
- Reuse one attributed closest-hit solar ray for direct beam and circumsolar visibility per eligible interval.
- Use isotropic unshaded ground reflection with explicit EPW/fallback albedo precedence.
- Preserve component energy, state, first hit, lost direct/circumsolar energy, diagnostics, and deterministic hashes through Rust, C, .NET, Grasshopper, and CLI.

## Consequences

The engine gains a scientifically traceable and explainable plane-of-array annual metric at bounded ray cost. Horizontal open-sky diffuse and vertical ground reflection have exact analytic checks. The result is not a Radiance daylight simulation, multiple-reflection solver, PV yield model, or whole-building EnergyPlus replacement. A later reference-validation milestone can add full-year comparative cases without changing the 0.6 result schema.
