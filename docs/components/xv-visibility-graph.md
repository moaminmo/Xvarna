# XV Visibility Graph

`XV Visibility Graph` traces every unordered node pair exactly once and converts directly visible pairs into an undirected graph. It returns visible edges, blocked pair lines and source object IDs, degree, normalized degree centrality, deterministic component labels, normalized harmonic closeness, normalized Brandes betweenness, result hash, and a complete policy/runtime report.

Use `Maximum Distance` to construct a local rather than global graph and `Category Mask` to isolate context classes. Disable `Centrality` when only exact adjacency and components are needed. The production safety cap is 4,096 nodes because an exact all-pairs graph has quadratic pair count; the component fails explicitly rather than exhausting memory.

Metrics describe the supplied sample nodes, not continuous walkable space. See [the DAENA scientific contract](../validation/spatial-visibility-and-privacy.md).
