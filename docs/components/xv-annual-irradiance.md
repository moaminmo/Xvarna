# XV Irradiance

`XV Irradiance` is the task-based MEHR annual component for Rhino 8 and Rhino 9. It accepts an immutable `XV Scene`, EPW file path, sensor points, and one broadcast or one-per-sensor normal.

## Core inputs

- **Scene**: compiled context from `XV Scene`.
- **EPW Path**: validated EnergyPlus Weather file.
- **Sensors / Normals / Sensor IDs**: oriented analysis surfaces with stable optional IDs.
- **Ground Albedo / Use EPW Albedo**: fallback and per-interval precedence.
- **Offset / Maximum Distance / Category Mask**: ZAMYAD query policy in Rhino model units.
- **North Rotation / Delta T / Pressure / Temperature / Minimum Altitude**: explicit NREL SPA policy.
- **Run**: pauses or dispatches task-based computation.

## Summary outputs

`Global`, `Direct`, `Diffuse Sky`, and `Ground` are kWh/m². `Peak` is W/m² and `Peak Time` is UTC. Dome and horizon visibility are normalized 0–1 ratios. Dominant solar obstruction is paired with attributed lost kWh/m²; dominant sky obstruction is based on projected static quadrature weight.

`Heatmap` uses a Viridis scale normalized to the largest annual global value in the current solution. The component previews those colours at sensor points.

## Timeline outputs

The five trees have one branch per sensor and one item per EPW interval:

- interval-average global W/m²;
- received direct Wh/m²;
- received sky-diffuse Wh/m²;
- state `0 inactive`, `1 back-facing`, `2 visible`, `3 blocked`;
- first-hit object ID for blocked solar intervals.

`Result` retains additional dome/circumsolar/horizon components, ground energy, UTC timestamp, lost energy, instance/mesh/triangle/distance attribution, source weather, options, diagnostics, and hashes without forcing all data through Grasshopper wires.

See [the MEHR validation contract](../validation/annual-irradiance.md) before interpreting the values or comparing them with other software.
