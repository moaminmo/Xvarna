# XV Diagnostics

**Purpose.** Aggregates engine, host, cache, scheduler and compute health into a support record.

**Inputs.** Optional backend, bundle directory, write toggle and error chain. **Outputs.** health, severity, issues, active jobs, recent events, preflight report, diagnostic ZIP path and SHA-256 manifest hash. No physical units.

**Method and assumptions.** Reports detected state without repairing files or changing configuration. The optional ZIP contains a versioned JSON manifest and README only: OS/runtime architecture, engine/ABI, compute adapters, cache/scheduler telemetry and supplied event/error context. Absolute paths and user/profile names are redacted; environment variables, model geometry, weather data and user documents are excluded.

**Example.** Run in the preflight canvas and attach its bundle when reporting native-load, cache or GPU failures.
