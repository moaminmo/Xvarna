# XV Sensor Grid

`XV Sensor Grid` converts design geometry into the common area-aware sensor contract used by XVARNA solar and sky analyses on Rhino 8 and Rhino 9.

## Inputs

- **Geometry** accepts Mesh, Brep, Surface, and Extrusion items. Inputs are copied and never mutated.
- **Cell Size** is the maximum accepted analysis-triangle edge in active Rhino model units.
- **Offset** moves each centroid along its unit normal. Zero resolves to the active document tolerance.
- **First Sensor ID** starts a deterministic contiguous unsigned 64-bit ID range. Zero is reserved.
- **Maximum Cells** is a hard safety ceiling. Exceeding it fails explicitly instead of allocating an unbounded grid.

Brep-like geometry is first meshed with Rhino at the requested scale. The native Rust core then applies deterministic longest-edge bisection to every canonical triangle until the edge contract is met.

## Outputs

`Sensor Grid` retains the immutable native result. `Analysis Mesh`, `Sensors`, `Normals`, `Areas`, and `Sensor IDs` align one-to-one. The points/normals/IDs connect directly to `XV Irradiance`, `XV Sun Hours`, or `XV Sky View`; retain `Sensor Grid` for `XV Solar Potential`.

`Source Faces` and `Depth` provide provenance and local resolution diagnostics. `Report` verifies source versus sampled area, achieved maximum edge, skipped invalid faces, resource policy, and BLAKE3 identity.

Run `XV Mesh Check` first when skipped faces are reported. A smaller Cell Size increases both annual analysis time and the sensor-major timeline size.
