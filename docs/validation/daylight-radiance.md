# Daylight and Radiance validation contract — 0.15.0

Update 2026-09-19: [actual Radiance point-runner and interreflection acceptance](radiance-executed.md) passed. Earlier unexecuted-reference statements describe the 0.15 review.


## Implemented scope

The release implements:

- point-in-time direct, diffuse, and total workplane illuminance in lux;
- CIE-overcast Daylight Factor in percent;
- EPW-driven annual sensor timelines and area-weighted sDA, ASE, and four-bin UDI;
- RGB energy-conserving `plastic`, `glass`, `metal`, `trans`, and `mirror` materials;
- exact Object-ID assignment with opaque unassigned geometry;
- process-memory reuse of Fast Path daylight coefficients;
- exact world-space Radiance export with manifest and stable hash;
- cancellable `oconv`/`rtrace` point runner;
- weather-independent `rfluxmtx` coefficient reuse with `gendaymtx`/`dctimestep` annual execution;
- tool-version, command, runtime, cache-state, and content-hash provenance;
- aligned Fast Path/Radiance bias, MAE, RMSE, MAPE, maximum error, R², and tolerance reporting.

## Metric definitions

For sensor normal **n**, sky direction **ω**, visible throughput **τ**, and patch solid angle Δω, the Fast Path stores the projected coefficient `max(0, n·ω) τ Δω`. Diffuse illuminance scales these coefficients by the declared sky luminance distribution normalized to the supplied diffuse-horizontal illuminance. Direct illuminance is `DNI_lux × max(0, n·s) × τ_sun`.

Daylight Factor is `100 × interior illuminance / exterior horizontal illuminance` under the CIE-overcast relative distribution and no direct sun.

Annual sDA is the represented sensor area whose occupied fraction at or above the declared threshold meets the declared required fraction. Defaults are 300 lux and 50%. ASE is represented area whose direct-only illuminance exceeds 1000 lux for more than 250 occupied hours by default. UDI defaults to boundaries 100, 300, and 3000 lux and reports below, supplemental, preferred/useful, and exceeded bins. EPW interval durations, not row counts, weight every time metric.

## Analytic verification

Automated tests establish these invariants:

1. An unobstructed horizontal sensor receives exactly the declared direct-normal plus diffuse-horizontal illuminance for a zenith sun.
2. Repeating an identical scene/sensor/material/options analysis reuses the identical coefficient hash.
3. A 50% transmitting glazing layer produces 50% direct/diffuse transmission and a 50% Daylight Factor in the analytic fixture.
4. Channel-wise reflectance plus transmittance above one is rejected.
5. Annual timelines, sDA/ASE/UDI aggregation, and hashes are deterministic.
6. C and .NET layouts are checked for every 0.15 record; point, DF, validation, and Radiance export cross the actual C ABI.
7. Managed point, annual, cancellation, progress, material, export, and comparison contracts run on both .NET 8 and .NET 10.
8. CLI smoke cases run against `validation/meshes/open_quad.obj` and `validation/weather/tehran-mini.epw`.

The release fixture produced, for an unobstructed horizontal sensor, 50,000 direct + 10,000 diffuse = 60,000 lux and a 100% open-sky Daylight Factor. The mini-EPW annual smoke produced eight aligned intervals and a deterministic project summary. These are analytic/smoke checks, not a building-scale accuracy claim.

## Radiance status on the 0.15 release host

The exact export, material mapping, sky generation, command construction, output parsing, cancellation-before-launch, cache key, and provenance logic are automated and passing. The release host did **not** have `oconv`, `rtrace`, `rfluxmtx`, `gendaymtx`, or `dctimestep` installed. Consequently, this milestone does not claim that an external Radiance binary was executed on that host. The runner fails clearly when tools are unavailable and accepts an explicit Radiance `bin` directory.

Before a publication or universal accuracy claim, run the public reference commands with a pinned Radiance version, retain the generated bundle/matrices/log, and publish the comparison report. Required evidence includes multiple orientations, glazing types, room depths, sky conditions, real projects, and at least one independent reviewer.

## Non-claims

- Fast Path reflectance does not produce multi-bounce interreflection.
- Constant luminous efficacies are explicit approximations, not a spectral daylight model.
- sDA/ASE defaults mirror common LM-83 thresholds, but XVARNA Fast Path alone is not a compliance certification.
- A high R² can coexist with bias; no single metric establishes validity.
- Identical fixture values prove the comparison pipeline, not agreement with Radiance on arbitrary scenes.
- Performance or accuracy superiority over Honeybee/Radiance is not claimed by 0.15.

## Reproduction

```powershell
xvarna daylight-point validation/meshes/open_quad.obj `
  --sensor 1,0,0,1,0,0,1,1 `
  --moment 1718966400,0,0,1,50000,10000 --patches 64 --json

xvarna annual-daylight validation/meshes/open_quad.obj validation/weather/tehran-mini.epw `
  --sensor 1,0,0,1,0,0,1,1 --patches 64 --json

xvarna radiance-export validation/meshes/open_quad.obj radiance-bundle `
  --sensor 1,0,0,1,0,0,1,1

xvarna radiance-point validation/meshes/open_quad.obj `
  --sensor 1,0,0,1,0,0,1,1 `
  --moment 1718966400,0,0,1,50000,10000 --radiance-bin C:\Radiance\bin --json

xvarna daylight-compare validation/daylight/fast-lux.txt `
  validation/daylight/radiance-reference-lux.txt --absolute-lux 50 --relative 0.1 --json
```

All coordinates in CLI/Radiance files are metres. Managed APIs and Grasshopper use the scene's source model units and convert sensor area with the square of the declared unit scale.
