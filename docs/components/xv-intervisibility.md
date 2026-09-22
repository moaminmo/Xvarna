# XV Intervisibility + Privacy

`XV Intervisibility` evaluates every observer against every target using the native DAENA/ZAMYAD line-of-sight path. Optional target facings make exposure directional; observer weights and target sensitivities are bounded inputs for the transparent privacy-screening formula.

The component returns visible and blocked Rhino lines, row-major Data Trees for state, distance, risk, and blocker object ID, observer visible fractions, target exposure fractions, target combined risk, a reusable immutable result, hash, and a report containing the exact formula and policy.

State values are `0 Visible`, `1 Blocked`, `2 OutOfRange`, and `3 Coincident`. Empty facing inputs make all targets omnidirectional. A zero maximum distance means unbounded. A zero endpoint clearance resolves to the Rhino document tolerance.

Combined privacy risk is a complement-product screening proxy, not an empirical probability or compliance claim. See [the DAENA scientific contract](../validation/spatial-visibility-and-privacy.md).
