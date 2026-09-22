# MEHR annual irradiance scientific and validation contract

This document defines what XVARNA 0.6 calculates, how it treats EPW time and missing data, which obstruction terms are ray traced, what is validated, and what the result must not be called.

## Inputs and units

MEHR consumes a compiled ZAMYAD scene, one or more finite oriented sensors, an EnergyPlus Weather (`.epw`) file, and explicit analysis options. Native scene positions are canonical metres; managed/Rhino positions and distances are transparently converted from source model units. Radiation fields are EPW interval energy in Wh/m². Timeline `*WhM2` values remain interval energy; `GlobalWM2` is the interval-average power density.

EPW parsing follows the official [EnergyPlus weather-file data dictionary](https://bigladdersoftware.com/epx/docs/23-1/auxiliary-programs/energyplus-weather-file-epw-data-dictionary.html):

- the eight mandatory header records and one declared data period are required;
- `records per hour` must be a positive divisor of 60, so hourly and subhourly intervals are supported;
- hour is 1–24 and minute is 1–60; the record timestamp is the local-standard **end** of the accumulation interval;
- the solar calculation timestamp is the UTC midpoint: `local end − time-zone offset − interval/2`;
- GHI, DNI, and DHI are fields 13, 14, and 15 (one-based) and are accumulated Wh/m² for the preceding interval;
- negative, non-finite, unparseable, or `≥ 9999` radiation is set to zero and counted independently. Counts stay in metadata and reports;
- valid EPW albedo is accepted from 0 through 1. Missing/invalid albedo uses the configured fallback when EPW-albedo preference is enabled.

No Windows timezone, daylight-saving rule, or current locale is consulted. EPW local standard time and the header's fixed UTC offset are authoritative.

## Solar position and direct beam

ZURVAN calculates apparent solar position at every interval midpoint using the Reda–Andreas NREL SPA implementation and explicit Delta T, pressure, temperature, true-north rotation, and minimum-altitude options. Before model-north rotation, +X is east, +Y north, and +Z up.

For unit surface normal **n**, unit sensor-to-sun direction **s**, direct-normal irradiance `DNI`, and binary closest-hit visibility `Vsun`, received beam power is

```text
Ibeam = DNI · max(0, n·s) · Vsun
```

One ZAMYAD closest-hit ray supplies both visibility and object/instance/mesh/triangle/distance attribution. A blocked interval's attributed loss includes unshaded direct beam plus non-negative Perez circumsolar energy. This is a causal first-hit allocation, not a multi-object counterfactual decomposition.

## Perez anisotropic sky

MEHR implements the EnergyPlus-style Perez 1990 three-component tilted-surface model documented in the [EnergyPlus Engineering Reference](https://bigladdersoftware.com/epx/docs/23-1/engineering-reference/sky-radiance-model.html):

```text
Isky = Idome + Icircumsolar + Ihorizon

Idome        = DHI · (1 − F1) · (1 + cos S) / 2
Icircumsolar = DHI · F1 · a / b
Ihorizon     = DHI · F2 · sin S

a = max(0, n·s)
b = max(0.087, cos Z)
Δ = DHI · m / 1353
ε = [((DHI + DNI) / DHI) + 1.041 Z³] / [1 + 1.041 Z³]
```

`S` is surface tilt from +Z, `Z` is solar zenith in radians, `m` is elevation-adjusted relative air mass, `ε` selects one of eight Perez clearness bins, and `F1/F2` use the published EnergyPlus coefficient table. As in EnergyPlus, `F1` is clamped to zero or above. The final received sky total is clamped non-negative; the horizon correction itself may be negative and is retained in the component timeline.

When DHI is zero or the sun is below the geometric horizon, MEHR uses only the isotropic dome term. A horizontal upward surface in unobstructed sky therefore receives exactly DHI, which is an analytic acceptance test.

## Diffuse obstruction

Static visibility is evaluated once per sensor:

- dome ratio: 24 azimuth sectors × 6 altitude bands = 144 patch-centre rays. Each ray is weighted by patch solid angle and `max(0, n·ω)`;
- horizon ratio: 24 azimuth-sector centre rays at altitude zero, weighted by `max(0, n·ω)`;
- circumsolar ratio: the same closest-hit solar visibility used by direct beam.

Received sky power is

```text
Ireceived = Rdome · Idome + Vsun · Icircumsolar + Rhorizon · Ihorizon
Isky,received = max(0, Ireceived)
```

The dominant static-sky occluder is the first-hit object with the largest combined projected dome/horizon sampling weight. Static sky attribution is a directional quadrature diagnostic, while annual solar attribution is measured in lost Wh/m².

## Ground-reflected term

For global-horizontal irradiance `GHI`, albedo `ρ`, and tilt `S`, MEHR uses the standard isotropic-ground view factor

```text
Iground = GHI · ρ · (1 − cos S) / 2
```

This term is not ray shaded. It does not include terrain slope, local ground masks, specular reflection, interreflection, radiosity, or multiple bounces. The limitation is visible in every component/CLI report.

## Aggregation, peak, and state

Every power component is multiplied by the source interval duration to obtain Wh/m². Annual summaries use direct sums and do not assume exactly 8,760 records. Peak is the maximum interval-average global W/m²; ties retain the first UTC midpoint. State codes are `0 inactive`, `1 back-facing`, `2 visible`, and `3 blocked`.

The BLAKE3 result identity includes scene hash, parsed-weather hash, calculated sun-set hash, sensors, options, visibility ratios, summaries, complete energy timeline, and first-hit attribution. Identical semantic inputs on the same engine contract must produce the same hash.

## Cancellation and progress

Native work units are `sensorCount × (168 + weatherCount)`. Cancellation is checked before each sensor and between weather chunks of at most 512 intervals. Static sky work is one bounded 168-ray batch per sensor. Cancellation returns a contained status and never publishes a partial result as complete.

## Acceptance tests in 0.6

- official published NREL SPA golden solar angles within `0.0003°`;
- EPW hourly and subhourly end-time to UTC-midpoint conversion;
- independent missing GHI/DNI/DHI sanitization and counts;
- deterministic semantic weather hash;
- exact horizontal-open-sky Perez identity `Isky = DHI`;
- exact isotropic ground reflection on a vertical surface;
- open-sky annual component totals and first peak time;
- full 8,760-interval sensor-major alignment and finite component outputs;
- full-blocker direct/circumsolar first-hit object attribution;
- static dome obstruction and dominant-sky object attribution;
- Rust cancellation before tracing;
- fixed C ABI layouts and UTF-8 EPW inspection across the ABI;
- .NET 8 and .NET 10 EPW, annual async, progress, cancellation, unit-conversion, and timeline integration tests;
- Rhino 8 and Rhino 9 Grasshopper compile gates.

## Explicit non-claims

MEHR 0.6 reports weather-driven plane-of-array irradiation. It is not:

- illuminance, daylight autonomy, UDI, ASE, glare, or a Radiance daylight simulation;
- photovoltaic electrical yield, module temperature, inverter output, or financial return;
- building thermal load, HVAC energy, comfort, CFD, or an EnergyPlus whole-building simulation;
- measured local microclimate, forecast weather, stochastic uncertainty, or a certified code-compliance result;
- spectral, polarization, specular, terrain-reflected, or multi-bounce radiative transfer.

The Perez formulation is traceable to the documented EnergyPlus model, but XVARNA is not EnergyPlus-certified. Real-project validation should compare representative geometries and climates against a trusted reference workflow before making high-stakes decisions.
