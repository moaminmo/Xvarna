# Solar position and Direct Sun Hours validation contract

## Method boundary

ZURVAN/HVARE 0.4 uses the Reda–Andreas Solar Position Algorithm through the MIT-licensed Rust `solar-positioning 0.5.3` crate. That crate states that it independently implements the published method; XVARNA does not claim that its code or binaries are produced, endorsed, or certified by NREL.

NREL's published SPA material states an uncertainty of ±0.0003 degrees for solar zenith and azimuth over years −2000 through 6000. XVARNA's native, C ABI, and managed tests use the published 2003-10-17 example:

| Input | Value |
|---|---:|
| UTC timestamp | `2003-10-17T19:30:30Z` |
| Latitude | `39.742476°` |
| Longitude | `−105.1786°` |
| Elevation | `1830.14 m` |
| Delta T | `67 s` |
| Pressure | `820 mbar` |
| Temperature | `11 °C` |
| Expected azimuth | `194.34024°` |
| Expected apparent altitude | `39.88838°` |

The acceptance tolerance is `0.0003°` for each angle. This verifies one published reference case across three API layers; broader cross-engine and multi-epoch validation remains a release gate for XVARNA 1.0.

## Direct-sun analytic cases

The HVARE unit and managed integration suite includes:

- a horizontal occluder whose known first hit must produce one blocked hour and object/instance/category attribution;
- an unobstructed ray selected by category mask;
- a back-facing sensor whose interval is excluded from Eligible Hours;
- millimetre Rhino model-unit conversion through scene ingest, sensor offset, hit distance, and managed output;
- deterministic timeline and result identity.

Direct Sun Hours is a geometric line-of-sight metric. It is not irradiance, illuminance, thermal load, glare, energy use, or a compliance result.

## Primary references

- [NREL Solar Position Algorithm landing page](https://midcdmz.nrel.gov/spa/)
- [Reda and Andreas, Solar Position Algorithm for Solar Radiation Applications](https://docs.nrel.gov/docs/fy08osti/34302.pdf)
- [`solar-positioning` crate source and method notes](https://crates.io/crates/solar-positioning/0.5.3)
