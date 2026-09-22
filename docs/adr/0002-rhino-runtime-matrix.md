# ADR 0002: Rhino 8 and Rhino 9 runtime matrix

- Status: Accepted
- Date: 2026-08-30

## Context

Rhino 8 and Rhino 9 do not share the same preferred managed runtime. A single ambiguous assembly would make runtime selection and dependency loading difficult to diagnose.

## Decision

XVARNA builds two managed artifacts from one C# source tree:

| Host | Minimum supported host | Target framework | Native architecture |
|---|---:|---:|---:|
| Rhino 8 | 8.20 | `net8.0` | Windows x64 |
| Rhino 9 | 9.0 | `net10.0` | Windows x64 |

The native ABI and component GUIDs are identical in both distributions. Each Rhino package includes only its matching managed target plus the same compatible `xvarna_core.dll`.

The target choice follows McNeel's current guidance: Rhino 8.20 and later default to .NET 8, while Rhino 9 defaults to .NET 10. The upstream reference is [Moving to .NET Core](https://developer.rhino3d.com/en/guides/rhinocommon/moving-to-dotnet-core/).

## Consequences

- Users install an unambiguous host-specific package.
- Both builds share behavior and Grasshopper definitions remain portable.
- CI must compile and test both target frameworks.
- Rhino 8 versions older than 8.20 are outside the supported matrix.
