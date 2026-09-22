# ADR 0003: Windows native toolchains

- Status: Accepted
- Date: 2026-08-30

## Context

Rhino is a native Windows application and official binaries need the normal Windows ABI. Some contributor machines may temporarily lack Visual C++ Build Tools.

## Decision

Official Windows artifacts use the Rust `x86_64-pc-windows-msvc` target. The build scripts may select `x86_64-pc-windows-gnu` for local engineering checks when Visual C++ Build Tools are unavailable and MinGW is installed.

GNU-built binaries are never published as official XVARNA releases. CI and release provenance record the Rust compiler, target triple, and .NET SDK.

## Consequences

- Published binaries use the most compatible Windows toolchain.
- New contributors can still execute early local tests before installing MSVC.
- Release workflows must reject non-MSVC native artifacts.
