# ADR 0001: Rust core with thin managed connectors

- Status: Accepted
- Date: 2026-08-30

## Context

XVARNA must support multiple design hosts without duplicating its numerical algorithms. The core also needs predictable memory ownership, deterministic data contracts, explicit concurrency, and performance that can be measured independently of Rhino or Grasshopper.

## Decision

The canonical engine is a Rust workspace. Host integrations are thin connectors and communicate with the engine through a versioned C ABI. The first connector is C# for Rhino/Grasshopper.

The ABI exposes opaque handles and plain fixed-layout values only. Rust allocations never cross the boundary without a matching Rust-owned release function. Panics must be contained inside the native library before production functions are added.

## Consequences

- One engine can serve Rhino 8, Rhino 9, a CLI, and future Revit or web services.
- Numerical tests and benchmarks run without loading a CAD host.
- ABI compatibility becomes a release gate.
- FFI and deployment require explicit engineering and integration tests.
