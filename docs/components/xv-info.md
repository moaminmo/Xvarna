# XV Info

**Purpose.** Confirms that the managed connector can load the native XVARNA engine and that the ABI is compatible.

**Inputs.** None. **Outputs.** readiness, engine/ABI versions, native path and a diagnostic report. Values are dimensionless strings/booleans.

**Method and assumptions.** Loads the packaged native library through the same boundary used by analyses. `Ready` is true only after version and ABI checks. It does not validate a particular model, GPU, EPW or Radiance installation.

**Example.** Place this first in `01-xvarna-preflight.ghx`; do not continue after a false readiness result.
