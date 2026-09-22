# Development setup

## Required

- Windows 10 or 11 x64
- .NET SDK 10.0.300 or a compatible feature band; the SDK builds both `net8.0` and `net10.0`
- Rust 1.96.0 stable with `rustfmt` and `clippy`
- Git

## Native linker

Install Visual Studio 2022/2026 Build Tools with the **Desktop development with C++** workload for official-compatible MSVC builds. A MinGW-w64 `gcc.exe` on `PATH` is accepted only for local engineering checks. Published binaries are always produced with `x86_64-pc-windows-msvc`.

## First run

```powershell
./eng/bootstrap.ps1
./eng/check.ps1
```

`bootstrap.ps1` restores the pinned Rust toolchain, the selected native target, NuGet packages, and the solution. `check.ps1` treats formatting and lint warnings as failures, executes Rust tests, builds the native library, compiles both Rhino connectors, and runs the managed/native ABI tests on .NET 8 and .NET 10.

## Local Rhino smoke test

1. Run `./eng/build.ps1 -Configuration Debug`.
2. In Rhino 8, load `dotnet/Xvarna.Grasshopper/bin/Debug/net8.0/Xvarna.Grasshopper.gha`.
3. In Rhino 9, load `dotnet/Xvarna.Grasshopper/bin/Debug/net10.0/Xvarna.Grasshopper.gha`.
4. Add `XVARNA > 00 Setup > XV Info` and confirm `Ready = True`.

Keep each host pointed at its matching folder. Loading the Rhino 9 assembly into Rhino 8 is intentionally unsupported.

When Rhino is installed in its default location, validate each compiled connector against the installed runtime assemblies and instantiate every component with:

```powershell
./eng/test-rhino8-compat.ps1 -Configuration Debug
./eng/test-rhino9-compat.ps1 -Configuration Debug
```

Each check locks all twenty-six expected component nicknames and GUID uniqueness. A missing host is reported as an explicit skipped runtime gate; both connectors still pass their compile-time/API gates.
