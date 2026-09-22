# XVARNA Grasshopper 2 examples

XVARNA 1.0 RC exposes all 49 analysis and product capabilities through the independent
Grasshopper 2 API. Geometry-first components use native GH2 mesh, point, vector,
transform, number, boolean and twig parameters. The six Basic workflows do not require
hand-written JSON: Target/Weighted/Green View, Intervisibility/Privacy, point and annual
daylight, Radiance export, and the scalar Study Manifest expose direct inputs. Optional
versionable JSON remains an Expert/import override for high-dimensional records.

Start with `XVARNA Info`, then connect `XVARNA Scene` to an analysis component. Result
objects remain typed for downstream XVARNA components; complete JSON and BLAKE3 hashes
are also emitted for notebooks, reports and reproducibility. The request files below
demonstrate the optional automation/import interface, not a Basic-mode prerequisite.

- `isovist-request.json`: planar DAENA visibility.
- `target-view-request.json`: solid-angle Target/Weighted/Green View.
- `annual-daylight-request.json`: sDA, ASE and UDI using an EPW path supplied separately.
- `study-rank-request.json`: scalar/vector objective ranking and sensitivity.

The connector targets Rhino 9 + Grasshopper 2 and .NET 10. Rhino 8 remains supported
through the 49-component Grasshopper 1 connector; Rhino 8 + Grasshopper 2 is not claimed
because the current official GH2 package depends on RhinoCommon 9.
