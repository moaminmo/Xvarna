# Issue severity

| Severity | Meaning | Release treatment |
|---|---|---|
| S0 | Security compromise, destructive data loss, or unsafe result propagation | Stop distribution; revoke affected artifact |
| S1 | Crash in a flagship workflow, materially wrong result, stale result exported as current, or silent scientific fallback | Blocks RC and public release |
| S2 | Workflow failure with a safe workaround or major usability/accessibility defect | Fix or document an owner-approved exception |
| S3 | Cosmetic, wording, minor discoverability or enhancement | Triage for current or 1.x roadmap |

Priority P0 is reserved for issues requiring immediate coordinated response. Severity describes impact; priority describes response order.

