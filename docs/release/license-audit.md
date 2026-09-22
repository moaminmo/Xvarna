# Dependency and content license audit

## Product content

- Source code: Apache-2.0 (`LICENSE`).
- Notices: `NOTICE` is included in Rhino, CLI and SDK archives.
- Generated benchmark corpus: CC0-1.0; definitions and generator are committed.
- User/project case-study models: not bundled until each manifest contains an explicit source, license and SHA-256.
- Radiance: external executable, not redistributed by XVARNA.
- Rhino/Grasshopper SDK assemblies: build/runtime dependencies, not redistributed unless their terms explicitly permit it.

The review-candidate build generates a CycloneDX SBOM from locked Cargo metadata and resolved .NET assets. Before public upload, a maintainer must inspect every component whose license is missing or non-permissive, verify binary redistribution terms, and record the audit date/name below. Automated metadata is evidence, not legal advice.

- Auditor:
- Date:
- SBOM SHA-256:
- Missing/ambiguous license components:
- Exceptions and rationale:
- Decision: PENDING

