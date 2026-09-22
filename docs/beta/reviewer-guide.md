# XVARNA review-candidate guide

## Purpose

This package is for structured external review before public release. It is not a public `1.0` release and makes no universal speed or certification claim.

## Setup

1. Record Windows, Rhino, Grasshopper, GPU and driver versions in `test-session-template.md`.
2. Verify the archive against `SHA256SUMS.txt`.
3. Extract the Rhino 8 or Rhino 9 ZIP to a new folder. Follow its `README.md`; do not load binaries from the source tree.
4. Open one packaged tutorial at a time. Start with `01-zurvan-sun-hours`, then `03-daena-view-analysis`, `07-hvare-daylight-radiance`, and `04-vahman-study-report`.
5. Generate `XV Diagnostics` after the session and remove project/client identifiers before sharing.

## Review tasks

- Install/uninstall without developer assistance.
- Complete one solar, one visibility, one daylight and one Study/report workflow.
- Change geometry and confirm that the result is recomputed or visibly marked stale.
- Cancel one long operation and confirm Rhino remains responsive.
- Inspect backend, adapter, cache and fallback diagnostics.
- Save, close, reopen and recompute the document.
- Open the generated HTML report on a computer without Rhino if available.

## Reporting

Attach the completed session form, diagnostic bundle and minimal reproducible model. Never include confidential geometry without permission. Classify observations with `issue-severity.md`. A crash, data loss, wrong result, silent fallback, stale export or security/privacy issue is never a cosmetic defect.

