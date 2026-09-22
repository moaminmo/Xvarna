# Core, time, and interoperability validation contract

This milestone removes the former OBJ/EPW-only bottleneck while keeping transformations auditable.

## Automated acceptance

- Scene JSON and `.xvscene` binary round-trip mesh resources, stable identities, affine transforms, categories, and static/dynamic layers.
- A one-byte binary payload mutation must fail BLAKE3 verification.
- Version-zero scene JSON migrates to schema version one with documented defaults.
- Welding is deterministic, tolerance-bounded, and followed by a second degeneracy/duplicate pass.
- Shared manifold edges receive opposite directions after winding repair; closed components may be oriented by signed volume.
- ROI clipping creates plane intersections and triangulates the clipped polygon; it is not centroid filtering.
- ASCII/binary STL and ASCII/little-endian PLY round-trip the analytic triangle fixture.
- glTF and GLB round-trip the analytic fixture and apply scene-node transforms.
- WEA parses into analysis weather, reconstructs GHI with NREL-SPA, and round-trips through lossless XVARNA CSV.
- Missing-radiation interpolation and occupancy-aware selection retain exact counts, weighted hours, and content hashes.
- CLI smoke covers OBJ → GLB → PLY → XVSCN → OBJ and EPW → WEA → CSV.
- Two independent XVSCN builds must have identical SHA-256 hashes.
- The complete engineering gate must load every component in isolated Rhino 8 and Rhino 9/WIP processes and execute installed Radiance when available.

## Explicit limits

The glTF analysis path consumes geometry and scene transforms, not rendering assets. It does not claim support for animation, skins, morph targets, texture/image pipelines, Draco/meshopt compression, or sparse accessors. External buffer URIs are rejected by the in-memory API; GLB or embedded glTF is required. PLY accepts vertex scalar properties and a vertex-index face list; polygons above four vertices fail rather than receiving unsafe fan triangulation.

WEA contains DNI and DHI but not measured GHI. XVARNA reconstructs GHI as `DHI + DNI × max(sun_z, 0)` using NREL-SPA at the WEA midpoint. This is sufficient for interoperability but remains labeled as reconstructed data in provenance.

## Runtime smoke commands

```powershell
xvarna mesh-convert validation/meshes/tetrahedron.obj tetra.glb --repair-winding --json
xvarna mesh-convert tetra.glb tetra.ply --json
xvarna scene-pack tetra.ply tetra.xvscene --repair-winding --json
xvarna scene-unpack tetra.xvscene tetra-unpacked.obj --json
xvarna weather-convert validation/weather/tehran-mini.epw tehran.wea --missing zero --json
xvarna weather-convert tehran.wea tehran.csv --missing reject --json
```
