# Surface Intelligence and PV Proxy Validation Contract

## Scope

XVARNA 0.7 maps MEHR annual plane-of-array irradiation onto real surface area. The validated scope is deterministic triangle sampling, area preservation, stable mapping, area-weighted distribution statistics, threshold regions, and a transparent first-order PV screening proxy.

## Surface grid

Every finite non-degenerate canonical source triangle is processed independently. While its longest edge exceeds the requested length, the engine splits that edge at its exact midpoint and creates two winding-preserving child triangles. Ties use the stable order AB, BC, CA. A depth ceiling and caller-selected maximum cell count bound pathological inputs.

For cell `i`:

- the sensor point is its centroid plus `offset × unit normal`;
- its weight is the exact triangle area in squared source model units;
- its ID is `first_sensor_id + deterministic_cell_index`;
- source triangle and subdivision depth are retained.

Acceptance tests require sampled-area conservation within floating-point tolerance, every cell edge at or below the requested limit, deterministic cells/hash for identical input, explicit invalid-face counts, and explicit failure at the resource ceiling.

## Area-weighted distribution

For annual irradiation `G_i` in Wh/m² and cell area `A_i` in m²:

`mean(G) = Σ(G_i A_i) / Σ(A_i)`

P10, P50, and P90 are weighted empirical quantiles using `A_i`, not unweighted sensor counts. This prevents small high-resolution triangles from dominating statistics.

The public heatmap uses the full uncropped `[minimum, maximum]` domain. It does not default to percentile clipping. Viridis is monotonic and colourblind-friendly; numeric values and the exact domain remain outputs.

## Eligibility and regions

A cell is eligible when `G_i >= threshold`. Eligible triangles sharing the same complete edge are joined with a deterministic union-find pass; region IDs are assigned by first cell order.

Current limitation: exact complete-edge adjacency intentionally avoids tolerance-dependent false joins. A T-junction created where neighboring source triangles subdivide their shared boundary differently can split what appears visually continuous into multiple conservative regions. Region area and energy remain correct; only grouping may fragment.

## PV screening equations

Let `S` be eligible cells, `η` module efficiency, `c` geometric coverage, and `L` aggregate downstream loss:

- eligible incident energy: `E_inc = Σ(i∈S) G_i A_i / 1000` kWh;
- capacity proxy: `P_dc = Σ(i∈S) A_i × c × η × 1 kW/m²` kWp;
- annual output proxy: `E_proxy = E_inc × c × η × (1 - L)` kWh;
- specific yield proxy: `E_proxy / P_dc` kWh/kWp when capacity is positive.

When Grasshopper receives individual loss fractions `L_j`, it computes `L = 1 - Π(1 - L_j)`, consistent with the multiplicative loss aggregation described by NREL PVWatts. The engine stores the resulting aggregate assumption explicitly.

## Non-claims

The proxy is not NREL-certified and is not a replacement for PVWatts, SAM, detailed module-layout software, or a bankable engineering study. It does not independently model cell temperature, inverter efficiency curves, DC/AC ratio, clipping, mismatch topology, bypass diodes, degradation, snow, soiling time series, availability, module layout, structural setbacks, curtailment, storage, tariffs, or financing. MEHR obstruction and EPW uncertainty still apply.

The baseline comparison is Ladybug's published Incident Radiation / Spatial Heatmap workflow: XVARNA adds native stable IDs, exact area-weighted distribution, resource ceilings, attribution-compatible result joining, and connected threshold regions. This is a workflow comparison, not a claim of universal numerical superiority.

## Automated evidence

- Rust unit tests cover conservation, resolution, determinism, invalid geometry, resource limits, weighted statistics, regions, thresholds, and proxy arithmetic.
- C ABI tests cover all fixed layouts, two-pass sizing, cross-boundary cells/regions, and proxy totals.
- .NET tests execute the real native DLL on .NET 8 and .NET 10, including centimetre-to-metre area conversion.
- Rhino 8 runtime loading instantiates all components; Rhino 9 compiles against the official SDK target.
