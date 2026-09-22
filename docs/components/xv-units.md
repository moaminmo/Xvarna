# XV Units

**Purpose.** Establishes the authoritative model-to-metre scale and numeric tolerance used by XVARNA components.

**Inputs.** unit system, optional scale override and absolute tolerance. **Outputs.** immutable unit context and summary. Length inputs follow Rhino model units unless explicitly labelled metres; areas are squared model units.

**Method and assumptions.** Validates finite positive scale/tolerance and records them in provenance. Changing units after a result is computed makes that result stale.

**Example.** Connect the context at project setup and keep it unchanged within one Study.
