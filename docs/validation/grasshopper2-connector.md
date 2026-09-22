# Grasshopper 2 connector validation contract

Version: 0.19.0. Host: Rhino 9 WIP `9.0.26237.15343`; SDK:
`Grasshopper2 9.0.26237.15343-beta`; managed target: .NET 10; native target:
Windows x64.

`eng/test-grasshopper2-compat.ps1` runs in a fresh PowerShell process to avoid Rhino 8/9
assembly collisions. It loads the installed RhinoCommon, GrasshopperIO and Grasshopper2
assemblies, then loads `Xvarna.Native.dll` and `Xvarna.Grasshopper2.rhp`.

The gate requires:

1. exactly 49 concrete GH2 components and one GH2 plugin registration;
2. a present and unique `IoId` for every component;
3. successful construction of every component, including its declared parameters;
4. exact set equality between all GH2 `IoId` values and all 49 GH1 `ComponentGuid` values;
5. a real native scene build and attributed closest hit on an analytic unit quad;
6. area-preserving surface-grid generation;
7. analytic open-hemisphere Sky View Factor equal to one;
8. analytic point illuminance of 60,000 lux for the declared direct/diffuse case;
9. JSON request round-trip plus positive solid-angle Target View;
10. a two-objective Pareto trade-off retained by the Study engine; and
11. a sealed 64-character Evidence Passport hash.

The solution build treats warnings as errors. `eng/check.ps1` invokes this gate whenever an
installed Rhino 9 host contains Grasshopper 2. Packaging copies the GH2 `.rhp` only into the
Rhino 9 distribution and records the exact SDK in `release-manifest.json`.

## Non-claims and remaining acceptance

This gate proves connector registration, semantic identity and representative end-to-end
engine contracts. It does not prove that every possible component input combination has been
executed inside a live canvas. Before a public package upload, manually verify component
placement, wire/twig behavior, save/reopen, viewport previews, long-job progress/cancellation,
device loss/fallback and the packaged Yak on the target Rhino 9 build. Because GH2 is
prerelease, re-run this acceptance after every SDK pin change.
