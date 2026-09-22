# Evidence and multi-fidelity .NET API

`EvidenceEngine` is the typed .NET 8/10 façade over the Rust RASHNU/VAHMAN engine. Enums serialize as camel-case strings and all operations cross the native boundary through sizing-pass UTF-8 JSON, so contracts are portable to non-.NET clients.

```csharp
EvidencePassport sealedPassport = EvidenceEngine.Seal(new EvidencePassport
{
    ResultHash = resultHash,
    SceneHash = sceneHash,
    Method = "HVARE annual daylight",
    Fidelity = EvidenceFidelity.Reference,
    Backend = "Radiance 6.1a",
    SampleCount = 8760,
    Assumptions = ["Declared occupancy schedule"],
    Limitations = ["Project-specific validation"],
    Citations = ["Radiance 6.1a"],
});
```

Generate parameter rows, evaluate them through Grasshopper or a headless pipeline, then pass the aligned output rows back without reordering:

```csharp
GlobalSensitivityDesign design = EvidenceEngine.CreateSensitivityDesign(
    variables, GlobalSensitivityMethod.SaltelliJansen, baseSamples: 4096, seed: 42);
GlobalSensitivityResult result = EvidenceEngine.AnalyzeSensitivity(design, outputs);
```

`AggregateScenarios` produces weighted mean/deviation, direction-aware worst and CVaR, and chance-constraint failure probability. `RankUncertainty` performs conservative interval Pareto sorting. `RecommendReferenceEvaluations` consumes aligned Fast and Reference observations plus Evidence hashes and returns calibration diagnostics and an ordered expensive-run batch.

For two objectives, `ExpectedHypervolumeImprovement` and `AcquisitionPerCost` are Monte-Carlo estimates. For more than two objectives the acquisition is an explicitly labelled uncertainty proxy; it is not qNEHVI or MF-HVKG.
