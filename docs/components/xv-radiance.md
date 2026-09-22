# XV Radiance

Exports `materials.rad`, `geometry.rad`, `sensors.pts`, and `manifest.json` from the exact canonical scene. It maps materials by Object ID, expands instancing only at export, preserves RGB properties, and writes metres. Use the packaged CLI `radiance-point` or `radiance-annual` runner with your installed Radiance `bin` directory for cancellable reference execution and provenance.
