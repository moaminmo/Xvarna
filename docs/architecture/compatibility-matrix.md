# Compatibility matrix

## Supported hosts

| Distribution | Rhino | Host API | Package used at compile time | .NET | Status |
|---|---|---|---|---|---|
| `xvarna-rhino8-win-x64` | 8.20 or newer | Grasshopper 1 | `Grasshopper 8.21.25188.17001` | 8.0 | implemented and runtime-tested |
| `xvarna-rhino9-win-x64` | 9.x | Grasshopper 1 | `Grasshopper 9.0.26237.15343-beta` | 10.0 | implemented and runtime-tested |
| `xvarna-rhino9-win-x64` | 9.x | Grasshopper 2 | `Grasshopper2 9.0.26237.15343-beta` | 10.0 | 49/49 capability adapters implemented; isolated installed-runtime and representative numerical contracts pass |

The Rhino NuGet versions are compile-time contracts, not a requirement for the user to install that exact service release. We compile Rhino 8 against an early compatible SDK to avoid accidentally relying on APIs introduced late in the Rhino 8 lifecycle. Rhino 9 and Grasshopper 2 remain pinned while their SDKs are pre-release and are updated deliberately after API review. The native target for all current Windows distributions is `x86_64-pc-windows-msvc`.

Grasshopper 2 is a separate host API, not a compatibility mode of a Grasshopper 1 `.gha`. `Xvarna.Grasshopper2.rhp` therefore owns only host adaptation—component registration, parameters, twig conversion, progress and diagnostics—while `Xvarna.Native`, the C ABI, result schemas and the Rust compute core remain shared. Geometry-first components use typed GH2 parameters. High-dimensional immutable records use JSON requests plus typed result objects to avoid lossy or unstable prerelease data coercion. The current official `Grasshopper2` package depends on RhinoCommon 9 prerelease, so Rhino 8 + Grasshopper 2 is not a supported combination unless McNeel publishes a compatible contract.

Upstream contracts: [McNeel's .NET runtime guidance](https://developer.rhino3d.com/en/guides/rhinocommon/moving-to-dotnet-core/), the [official Grasshopper2 prerelease package](https://www.nuget.org/packages/Grasshopper2/9.0.26216.12303-beta), and McNeel's [RhinoMCP dual GH1/GH2 project layout](https://github.com/mcneel/RhinoMCP/blob/main/rhino/plugin/RhMcp.csproj).

## Compatibility rules

1. The GH2 `IoId` set must exactly equal the 49 GH1 `ComponentGuid` values, so one identity always denotes one semantic capability even though document formats differ.
2. A native ABI major-version mismatch is a hard failure with a diagnostic, never a crash.
3. RhinoCommon objects do not cross the native boundary; connectors convert them into canonical XVARNA buffers.
4. Official Windows releases are x64 MSVC builds.
5. Every release is smoke-tested in both Rhino versions before publication.
6. All forty-nine GH1 component GUIDs and semantics, including VAYU cache/runtime telemetry, the shared scheduler, advanced DAENA, and XVARNA Study/Optimizer workflows, are identical in the `net8.0` and `net10.0` connectors.
7. VAYU discovers adapters through the same wgpu/WebGPU portability layer in both hosts. Actual graphics API and driver are recorded per session; GPU availability is not inferred from Rhino version.
8. GH2 types never leak into the native or managed analysis contracts; GH1 and GH2 connectors are replaceable adapters over the same engine.
9. The 0.19 engineering claim covers compile, isolated installed-runtime load, constructor/IoId checks, exact GH1/GH2 identity-set parity, twig parameter construction, and representative native numerical contracts. Interactive canvas placement, save/reopen, preview and user-cancellation acceptance remain a separate release-candidate gate and are not implied by the automated result.
