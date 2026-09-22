# XV Commit

Atomically commits one completed row for every candidate emitted by `XV Workspace`. Candidate IDs and objective branches must align exactly. `Objective Widths` describes scalar/vector groups; for example `[2,1]` means one two-component vector objective followed by one scalar objective. Constraint branches use values `<= 0` for feasibility.

Optional Metric Fields JSON carries indexed or spatial fields used for baseline delta maps. Pulse Commit only after all rows are complete. Repeating an identical commit is safe; changing a completed batch is rejected.
