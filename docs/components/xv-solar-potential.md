# XV Solar Potential

`XV Solar Potential` joins an immutable `XV Sensor Grid` and `XV Irradiance` result by stable Sensor ID, maps annual plane-of-array energy back to every analysis cell, and calculates area-aware opportunity regions and a transparent PV screening proxy.

## Inputs

- **Minimum Irradiance** is the eligibility threshold in kWh/m²/year for the analyzed EPW period.
- **Module Efficiency** and **Coverage** are percentages.
- **System Losses** accepts individual percentages and combines them multiplicatively. An empty input uses one explicit aggregate 14% assumption.

The component fails if a grid Sensor ID is absent from the annual result; it never silently aligns mismatched lists by index.

## Outputs

`Solar Map` is a bakeable mesh with one uniform vertex colour per analysis triangle. It uses the complete uncropped minimum-to-maximum data domain and a monotonic colourblind-friendly Viridis palette. `Eligible Regions` contains one mesh per exact-edge-connected threshold region.

Mean, P10, median, and P90 are weighted by physical cell area. Total/eligible area are m², incident and proxy yield are kWh, capacity is kWp, and specific yield is kWh/kWp. The immutable result retains aligned per-cell values, region summaries, assumptions, and a deterministic hash.

This is a screening proxy, not a bankable PV simulation. Read [the scientific contract](../validation/surface-intelligence-and-pv-proxy.md) before publication or design decisions.
