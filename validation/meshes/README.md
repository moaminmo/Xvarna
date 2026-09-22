# Analytic mesh corpus v0.2

These small CC0 fixtures provide exact topology and metric expectations for the Rust engine, C ABI, .NET wrapper, CLI, and Grasshopper connector. They are deliberately human-inspectable and are not performance benchmarks.

Run the standalone checker:

```powershell
cargo run -p xvarna-cli -- mesh-check validation/meshes/tetrahedron.obj --json
cargo run -p xvarna-cli -- mesh-check validation/meshes/broken_mesh.obj
cargo run -p xvarna-cli -- mesh-repair validation/meshes/broken_mesh.obj artifacts/repaired.obj
```

The second command intentionally returns exit code `1` because repair is required.
