# XV Optimizer Step

**Purpose.** Commits aligned objective/constraint evaluations to a deterministic ask/tell optimizer and requests the next batch.

**Inputs.** optimizer handle and evaluations aligned by candidate ID. **Outputs.** snapshot, next candidate batch, hashes and portable JSON. Objective units are those declared by the Study.

**Method and assumptions.** Uses the bounded mixed-variable NSGA-II policy stored in the manifest. Missing, duplicate or non-finite evaluations are rejected; stochastic simulations require their own recorded seed.

**Example.** Evaluate every candidate from XV Optimizer before calling Step; checkpoint the returned snapshot.
