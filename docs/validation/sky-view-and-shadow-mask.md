# ASMAN Sky View and Shadow Mask validation contract

## Metric definitions

For an oriented sensor with unit normal **n**, visibility function `V(ω) ∈ {0,1}`, and upper hemisphere `Ω+`, ASMAN reports the Lambert cosine-weighted Sky View Factor

```text
SVF = (1 / π) ∫Ω+ V(ω) max(0, n·ω) dω
```

This is the fraction of isotropic diffuse irradiance from the hemisphere that reaches the oriented surface. The definition and projected-cosine weighting follow the formulation summarized by the peer-reviewed [HORAYZON horizon and sky-view-factor paper](https://gmd.copernicus.org/articles/15/6817/2022/gmd-15-6817-2022.html).

ASMAN also reports a separate unweighted visible-hemisphere fraction

```text
VisibleHemisphere = visible sample count / total sample count
VisibleSolidAngle = 2π × VisibleHemisphere
```

These outputs must not be relabelled as cosine-weighted SVF. Keeping both prevents a common ambiguity in architectural tools.

## Sampling and finite estimator

Each sensor uses a deterministic Fibonacci lattice over its oriented upper hemisphere. Sample centres use `z = (i + 0.5) / N`, the golden angle for azimuth, and a seed-derived azimuth phase. Every sample represents the same solid angle `2π/N`. Fibonacci lattices are used because their near-uniform distribution and equal-area weighting avoid pole clusters; the construction follows the family described in [González, *Measurement of Areas on a Sphere Using Fibonacci and Latitude–Longitude Lattices*](https://arxiv.org/abs/0912.4540).

The implemented finite estimator is

```text
SVF_N = Σ V_i max(0, n·ω_i) / Σ max(0, n·ω_i)
```

Normalizing by the sampled cosine sum makes the fully open reference exactly one for every supported sample count while converging to the integral definition. Results are deterministic for identical scene identity, sensors, options, engine version, and platform math behavior. The BLAKE3 result identity includes the scene hash, sensors, sampling policy, every direction, state, first-hit identity, and distance.

## Convergence diagnostic

Samples are split into even and odd interleaved subsets. ASMAN reports the absolute difference between their independently normalized estimates, plus the corresponding unweighted difference in the managed/native result. This is a resolution diagnostic only. It is not a confidence interval, probability, certified error bound, or proof that thin obstructions were sampled.

Users should increase the sample count and check decision stability when:

- the convergence delta is material relative to the design threshold;
- thin or distant obstructions matter;
- two options rank closely;
- the result will support a publication or compliance-adjacent claim.

## Analytic and integration acceptance cases

The automated suite includes:

| Case | Samples | Acceptance |
|---|---:|---|
| Open upper hemisphere | 1,024 | cosine SVF = 1; visible fraction = 1 |
| Effectively infinite horizontal blocker above sensor | 1,024 | cosine SVF = 0; dominant object is exact |
| Effectively infinite vertical half-screen | 8,192 | cosine SVF and visible fraction within 0.01 of 0.5; convergence delta below 0.02 |
| Sampling repeatability | 2,048 | same seed is bitwise equal; different seed changes directions; all directions are unit and upper-hemisphere |
| Managed millimetre scene | 1,024 | native/managed state and object/instance identity agree; distances return in model units |
| Cancellation before dispatch | default | zero completed rays and explicit cancelled status/exception |
| Cancellation after first chunk | 262,144 | at least one 8,192-ray chunk completed, remaining chunks stopped, explicit cancelled result |
| Category and maximum-distance exclusion | 128 | an otherwise full blocker is ignored when filtered or outside range |
| Arbitrary oriented normal | 2,048 | every world direction is unit and strictly inside the sensor's oriented hemisphere |

The C ABI separately locks structure sizes for options, summaries, directions, metadata, and job progress. Managed tests run under both .NET 8 and .NET 10.

## Attribution policy

Every blocked direction stores the first ZAMYAD hit: object ID, instance ID, deduplicated mesh ID, local triangle ID, and distance. The dominant occluder is the object with the largest sum of blocked projected-cosine contribution. Equal contributions resolve to the lower object ID, making the result deterministic. Category masks and maximum distance are part of the analysis and result identity.

## Scope and known limits

- Geometry is binary opaque for this release; porous vegetation and partial transmittance are not modelled.
- This metric does not apply an anisotropic sky luminance model and is not illuminance, irradiance, daylight factor, sDA, ASE, or a Radiance replacement.
- The convergence delta cannot detect every missed thin feature.
- Sensor offset prevents ordinary self-intersection but must remain appropriate to model tolerance and geometric scale.
- Fibonacci sampling is numerical integration, not an exact analytic solid-angle solver.

These limits are product boundaries, not hidden assumptions. Later ASMAN diffuse-sky and validated daylight paths will consume the same attributed Shadow Mask without changing the meaning of the 0.5 metrics.
