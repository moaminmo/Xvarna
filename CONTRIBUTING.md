# Contributing to XVARNA

XVARNA is in its bootstrap phase. Contributions are welcome when they preserve
the contracts in `XVARNA_MASTER_SPEC.md`.

## Development flow

1. Open or select an issue with a requirement ID.
2. For a material architectural change, add an ADR or RFC first.
3. Keep the change focused and include tests and documentation.
4. Run `./eng/check.ps1` on Windows or the equivalent Cargo/.NET commands.
5. Submit a pull request describing correctness, performance, compatibility,
   and user-visible effects.

## Definition of done

A feature is not complete with code alone. It needs relevant tests, error and
cancellation behavior, documentation, an example, and validation or benchmark
evidence when it affects a metric or hot path.

## Compatibility

- Rust stable 1.96 is the current minimum toolchain.
- Rhino 8 connector: .NET 8 and Rhino/Grasshopper 8.x SDK.
- Rhino 9 connector: .NET 10 and Rhino/Grasshopper 9.x SDK.
- Connector assemblies are AnyCPU; native artifacts are platform-specific.

## Sign-off

By contributing, you certify that you have the right to submit the work under
the Apache-2.0 license. A formal Developer Certificate of Origin workflow will
be enabled before accepting external code contributions.

