# XV Landmark

Evaluates each oriented observer against each spherical landmark proxy. The component separates camera-FOV inclusion, analytic apparent solid angle, deterministic partial visibility, material transmission, and the declared landmark importance weight. Outputs are observer/landmark Data Trees with dominant and top blocking Object IDs.

Radii describe analysis proxies, not automatically generated scene geometry. Increase disk Samples for partially occluded landmarks and connect top IDs to `XV Highlight` for source inspection.
