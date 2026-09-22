# Rollback, hotfix and artifact revocation

1. Stop uploads/distribution when an S0/S1 correctness, security or data-loss issue is confirmed.
2. Identify affected versions and exact SHA-256 values from the release manifest. Never replace an artifact under an existing version/tag.
3. Publish a security/correctness advisory stating impact, detection, workaround and whether results must be recomputed. Avoid disclosing exploit detail before a fix is available.
4. Mark the release and Yak version withdrawn/deprecated where the platform allows; retain a revocation record containing hashes and reason.
5. Fix on a reviewed branch, add a regression test, run the full clean gate, rebuild on the release environment, generate a new SBOM/checksums and increment the version.
6. Re-run affected scientific validation and notify known reviewers/users. Close only after public download/install verification.

For an upload mistake without code impact, still create a new version. Immutable provenance is more valuable than silently replacing bytes.

