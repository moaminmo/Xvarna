# ADR 0006: Deterministic Sky Sampling and Cancellable Native Jobs

## Status

Accepted for XVARNA 0.5.

## Context

Sky View analysis needs stable spatial coverage, oriented-surface physics, complete explanation of obstruction, and enough responsiveness for interactive Grasshopper use. A dense angular grid clusters samples near the pole, while a synchronous managed loop blocks the host and cannot reliably stop native work.

## Decision

- Use a seeded Fibonacci upper-hemisphere sequence with equal solid angle per sample.
- Return both unweighted visible hemisphere and normalized Lambert cosine-weighted SVF.
- Preserve the complete first-hit Shadow Mask and derive dominant obstruction from projected blocked contribution.
- Keep the canonical implementation in Rust and expose fixed-layout result buffers through the C ABI.
- Represent long native work with opaque job handles containing atomic cancellation, completed-unit, and total-unit state.
- Check cancellation before each sensor and each bounded 8,192-ray chunk.
- Use `GH_TaskCapableComponent` only as the host scheduler; cancellation must propagate through .NET into the native job rather than merely abandoning a managed task.

## Consequences

Results are deterministic and fully attributable, and the same engine serves Grasshopper, .NET, C, and CLI clients. Complete masks require `O(sensor count × sample count)` memory. Cancellation latency is bounded by one chunk's query time rather than one ray, balancing responsiveness against traversal throughput. The interleaved-subset delta is useful for resolution checks but is explicitly not a statistical confidence interval.
