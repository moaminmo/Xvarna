# XV Legend

**Purpose.** Produces a consistent scientific colour scale, labels and mapped colours for analysis values.

**Inputs.** values, palette, range and clamp/log options. **Outputs.** colours, ticks, labels, range and report. Units are inherited from the supplied metric.

**Method and assumptions.** Non-finite values are classified explicitly; automatic ranges are data-dependent and must not be used for visual comparison unless locked.

**Example.** Lock one range before comparing baseline and candidate heatmaps.
