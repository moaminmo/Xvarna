# Executed Radiance reference — 2026-09-19

The existing production Radiance export/runner was exercised against the official **Radiance 6.0.2**, build `6.0.c1700d56cc`, Windows x64. This adds real external-solver evidence to the prior export and command-construction tests.

```powershell
# From this repository root; use your separately installed Radiance distribution.
$env:RAYPATH = 'C:/Radiance/lib'
cargo run -p xvarna-hvare --example radiance_acceptance --locked -- 'C:/Radiance/bin'
```

The fixture exports a 2,000 m square Lambertian ground plane with reflectance 0.5, two sensors one metre above it, and an isotropic sky of 10,000 lux diffuse horizontal illuminance. One sensor faces up and one down. At two ambient bounces, 8,192 divisions and zero ambient interpolation, the production pipeline returned:

| Quantity | Analytical target | Observed | Acceptance |
|---|---:|---:|---:|
| Upward illuminance | 10,000 lux | 9,998.81291 lux | ±200 lux |
| Downward reflected illuminance | Approximately 5,000 lux for the large plane | 4,999.40735 lux | ±250 lux |
| Downward value with ambient bounces disabled | 0 lux | 0 lux | ±1 lux |

The numerical assertions are executable and fail on an unacceptable result. The large finite plane approximates the analytical infinite plane; this is not a measured-room validation. The direct/diffuse **Fast Path still intentionally omits interreflection**. Use the Radiance reference path (`radiance-point` / `radiance-annual`) when reflected light matters; do not describe the Fast Path as a full transport solver. Annual matrix execution, complex glazing, measured buildings and certification require separate evidence.

Official binaries are obtained through [LBNL's linked distributions](https://www.radiance-online.org/download-install/installation-information); they are not bundled in the source archive.
