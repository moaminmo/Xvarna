# XV Runtime

**Purpose.** Reads live VAYU backend, transfer, fallback, buffer-reuse and device-health telemetry.

**Inputs.** compute session. **Outputs.** adapter/backend, upload/execute/readback timing, dispatches, memory and fallback diagnostics. Time is microseconds or milliseconds as labelled; memory is bytes.

**Method and assumptions.** Telemetry describes completed work on this session, not a synthetic benchmark. CPU fallback is explicit and must be retained in reports.

**Example.** Capture Runtime beside every performance-sensitive result and after a device-loss test.
