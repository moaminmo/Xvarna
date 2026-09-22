using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class EvidenceTests
{
    [Fact]
    public void EvidencePassportIsSealedAndMutationSensitiveContractIsReturned()
    {
        EvidencePassport result = EvidenceEngine.Seal(new()
        {
            ResultHash = "result-hash",
            SceneHash = "scene-hash",
            Method = "DAENA target view",
            Fidelity = EvidenceFidelity.Fast,
            Backend = "VAYU CPU",
            GeneratedUtc = new DateTimeOffset(2026, 9, 4, 0, 0, 0, TimeSpan.Zero),
            Seed = 42,
            SampleCount = 128,
            Assumptions = ["Opaque geometry"],
            Limitations = ["Not a regulatory certification"],
            Citations = ["xvarna:method:daena-target-view"],
        });

        Assert.Equal(64, result.ContentHash.Length);
        Assert.Equal("xvarna.evidence-passport/1", result.SchemaVersion);
    }

    [Fact]
    public void SaltelliMorrisRobustUncertaintyAndFidelityFlowsAreNativeAndTyped()
    {
        DesignVariable[] variables =
        [
            new(1, DesignVariableKind.Continuous, 0, 1),
            new(2, DesignVariableKind.Continuous, 0, 1),
        ];
        GlobalSensitivityDesign design = EvidenceEngine.CreateSensitivityDesign(
            variables, GlobalSensitivityMethod.SaltelliJansen, 4096, seed: 9);
        IReadOnlyList<IReadOnlyList<double>> outputs = design.Rows
            .Select(row => (IReadOnlyList<double>)new[] { row.Values[0] + 2 * row.Values[1] })
            .ToArray();
        GlobalSensitivityResult sensitivity = EvidenceEngine.AnalyzeSensitivity(design, outputs);
        Assert.Equal(2, sensitivity.Sobol.Count);
        Assert.InRange(sensitivity.Sobol[0].TotalOrder, 0.15, 0.25);
        Assert.InRange(sensitivity.Sobol[1].TotalOrder, 0.75, 0.85);

        IReadOnlyList<RobustScenarioSummary> robust = EvidenceEngine.AggregateScenarios(
            [ObjectiveDirection.Minimize], 1,
            [
                new() { CandidateId = 1, ScenarioId = 1, Probability = 0.8,
                    Objectives = [1], ConstraintResiduals = [-1] },
                new() { CandidateId = 1, ScenarioId = 2, Probability = 0.2,
                    Objectives = [5], ConstraintResiduals = [1] },
            ], 0.8);
        Assert.Equal(5, robust[0].CvarObjectives[0], 10);
        Assert.Equal(0.2, robust[0].ViolationProbability, 10);

        IReadOnlyList<UncertaintyRank> ranks = EvidenceEngine.RankUncertainty(
            [ObjectiveDirection.Minimize], 0,
            [
                new() { CandidateId = 1, ObjectiveMeans = [1], ObjectiveStandardDeviations = [0.1] },
                new() { CandidateId = 2, ObjectiveMeans = [2], ObjectiveStandardDeviations = [0.1] },
            ]);
        Assert.Equal(0, ranks[0].RobustParetoRank);
        Assert.Equal(1, ranks[1].RobustParetoRank);

        ScientificCandidate[] candidates = Enumerable.Range(1, 5)
            .Select(id => new ScientificCandidate { CandidateId = (ulong)id, Values = [id] })
            .ToArray();
        List<FidelityObservation> observations = [];
        foreach (ScientificCandidate candidate in candidates)
        {
            double id = candidate.CandidateId;
            observations.Add(new()
            {
                CandidateId = candidate.CandidateId,
                Fidelity = EvidenceFidelity.Fast,
                Objectives = [id, 6 - id],
                Cost = 1,
                NoiseStandardDeviations = [0.01, 0.01],
                EvidenceHash = $"fast-{id}"
            });
            if (candidate.CandidateId <= 3)
                observations.Add(new()
                {
                    CandidateId = candidate.CandidateId,
                    Fidelity = EvidenceFidelity.Reference,
                    Objectives = [2 * id + 1, 2 * (6 - id) + 1],
                    Cost = 50,
                    NoiseStandardDeviations = [0, 0],
                    EvidenceHash = $"reference-{id}"
                });
        }
        MultiFidelitySelection selection = EvidenceEngine.RecommendReferenceEvaluations(
            [ObjectiveDirection.Minimize, ObjectiveDirection.Minimize], candidates, observations,
            new()
            {
                BatchSize = 2,
                MonteCarloSamples = 128,
                Seed = 9,
                ReferencePoint = [20, 20],
                MinimumStandardDeviation = 0.01
            });
        Assert.Equal(2, selection.Recommendations.Count);
        Assert.All(selection.Recommendations, value => Assert.True(value.CandidateId > 3));
    }
}
