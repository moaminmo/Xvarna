# XV Optimizer and XV Opt Step

`XV Optimizer` creates a deterministic VAHMAN ask/tell session and an initial stratified candidate Data Tree. It accepts continuous, integer, and categorical variables, objective directions, residual-constraint count, population/offspring/archive sizes, and seed.

Evaluate every candidate branch through any Grasshopper workflow—including DAENA results—then pass the session and aligned objective/constraint trees to `XV Opt Step`. Pulse Advance once with a Button. The step performs feasibility-first NSGA-II selection, updates the bounded historical Pareto archive, and outputs the next batch. Chain explicit Step components for a fixed workflow or control pulses from automation; do not leave Advance permanently true.

The session is stateful and native. Reset changes the candidate sequence. Save evaluated tables separately for provenance. The optimizer searches a finite budget and does not certify a global optimum.

See [the scientific contract](../validation/advanced-view-and-optimization.md).
