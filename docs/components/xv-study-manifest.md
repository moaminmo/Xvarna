# XV Manifest

Builds the normative 0.16 Study Manifest inside Grasshopper. Parameter names align with kinds, bounds, units, and category branches. Objective names align with directions, units, and component branches; an empty component branch is scalar and a populated branch is a vector. Optional constraints use residual feasibility `<= 0`. Outputs include typed manifest, schema-valid JSON, the Draft 2020-12 schema, and a registry summary.

IDs are assigned deterministically from registry order. Do not reorder registries after creating a workspace. Use the baseline ID only when that generated variant will be evaluated and all scenario field maps share exact identity, units, lengths, and optional positions.
