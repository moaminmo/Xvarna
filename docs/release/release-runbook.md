# Review candidate and public release runbook

## Build the external review candidate

From a clean Windows x64 checkout with Rust, .NET, Rhino 8/9 and Radiance installed:

```powershell
.\eng\build-review-candidate.ps1 -EvidenceProfile Smoke
```

Use `-EvidenceProfile Full` on the designated evidence machine. The command runs automated checks, builds ZIP packages, creates benchmark/validation tables, generates a CycloneDX SBOM, verifies archive hashes and produces one reviewer ZIP under `artifacts/review-candidate/1.0.0-rc.1`. Yak creation is intentionally deferred; pass `-IncludeYak` only for the publication candidate.

Send the ZIP and its adjacent `.sha256` file. Reviewers follow `docs/beta/reviewer-guide.md`. Record all sessions; do not turn protocol-ready templates into fabricated completed studies.

## Public release gate (deferred)

Public release remains blocked until external Rhino interaction, AMD/device-loss/soak evidence, 20 testers/10 projects, independent sign-offs, legal/name decisions, artifact signing, DOI/site/hero assets and owner go/no-go are complete. Then run the same build from the immutable candidate commit, verify `sourceDirty=false`, sign each artifact, create `v1.0.0`, upload GitHub/Yak assets, publish documentation/DOI, and perform clean external download/install verification.
