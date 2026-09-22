# XV Study

Ranks completed design variants. Parameters, Objectives, and optional residual Constraints are rectangular Grasshopper Data Trees with one candidate per branch. Supply explicit variable bounds/kinds and one minimize/maximize direction per objective.

Outputs preserve input order and report feasibility, summed violation, non-dominated rank, crowding, feasible Pareto IDs, descriptive parameter/objective Spearman trees, optional exact two-objective hypervolume, deterministic hash, and method report. Constraints are feasible at `<= 0`. A hypervolume reference point must be worse than intended solutions in each original objective direction.

Spearman values are descriptive associations; the component does not claim causality. See [the scientific contract](../validation/advanced-view-and-optimization.md).
