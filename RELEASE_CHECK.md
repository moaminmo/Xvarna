# Release check - 22 September 2026

Source distribution for research and supervised evaluation. The checks below were executed locally on Windows against this source snapshot. Passing tests do not certify all host workflows or production suitability. Original validation documents retain their historical dates.

- `Xvarna_clippy`: PASS (exit 0); [execution log](release-evidence/Xvarna_clippy.log).
- `Xvarna_managed_tests`: PASS (exit 0); [execution log](release-evidence/Xvarna_managed_tests.log).
- `Xvarna_native_build`: PASS (exit 0); [execution log](release-evidence/Xvarna_native_build.log).
- `Xvarna_tests`: PASS (exit 0); [execution log](release-evidence/Xvarna_tests.log).

Current release includes the portfolio EPW column/timestamp, Radiance command and OBJ source-index corrections. Release lint review also made bounded integer conversions explicit, formatted the working-directory provenance with Display, and completed a CLI assignment statement. Managed daylight/irradiance EPW fixtures now use the same official column positions as the corrected parser (35 fields; radiation at zero-based 13/14/15 and albedo at 32). Full Rust tests, clippy with warnings denied, native build, and managed tests on net8.0 and net10.0 pass. Earlier portfolio runs retain their original binary hashes; this later build is not presented as the binary used for those runs.

Packaging excludes caches, generated build outputs, environments and compiled binaries. Build prerequisites and scope are described in README, CLAIMS and validation documents where present. No remote repository URL or publication status is invented.
