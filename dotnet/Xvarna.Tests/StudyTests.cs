using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class StudyTests
{
    [Fact]
    public void StudyRankingSensitivityAndHypervolumeCrossNativeBoundary()
    {
        StudyProblem problem = Problem();
        StudyCandidate[] candidates = Candidates();
        StudyEvaluation[] evaluations =
        [
            Evaluation(1, 1.0, 1.0, -1.0),
            Evaluation(2, 2.0, 3.0, -1.0),
            Evaluation(3, 0.0, 10.0, 0.5),
            Evaluation(4, 4.0, 0.0, -1.0),
        ];

        RankedStudyResult ranked = StudyAnalyzer.Rank(problem, candidates, evaluations);
        Assert.Equal([1ul, 2ul], ranked.ParetoCandidateIds);
        Assert.False(ranked.Solutions[2].Feasible);
        Assert.Equal(64, ranked.ContentHash.Length);

        IReadOnlyList<StudySensitivity> sensitivity = StudyAnalyzer.SpearmanSensitivity(problem, candidates, evaluations);
        Assert.Equal(6, sensitivity.Count);
        Assert.All(sensitivity, value => Assert.InRange(value.SpearmanRho, -1.0, 1.0));

        double hypervolume = StudyAnalyzer.Hypervolume2d(problem, candidates, evaluations, 5.0, -1.0);
        Assert.True(hypervolume > 0.0);
    }

    [Fact]
    public void AskTellOptimizerIsDeterministicMixedAndConstraintAware()
    {
        static OptimizerSnapshot Run()
        {
            using XvarnaOptimizer optimizer = new(Problem(), new MultiObjectiveOptimizerOptions(
                PopulationSize: 16, OffspringSize: 16, ArchiveCapacity: 32, Seed: 77));
            OptimizerSnapshot? snapshot = null;
            for (int generation = 0; generation < 4; generation++)
            {
                OptimizerBatch batch = optimizer.Ask();
                Assert.Equal(16, batch.Candidates.Count);
                StudyEvaluation[] evaluations = batch.Candidates.Select(candidate =>
                {
                    double x = candidate.Parameters[0], integer = candidate.Parameters[1];
                    return new StudyEvaluation
                    {
                        CandidateId = candidate.CandidateId,
                        Objectives = [x * x + integer * 0.01, 1.0 - Math.Abs(x - 0.8)],
                        ConstraintResiduals = [x + integer / 20.0 - 1.2],
                    };
                }).ToArray();
                snapshot = optimizer.Tell(evaluations);
            }
            return snapshot!;
        }

        OptimizerSnapshot first = Run(), second = Run();
        Assert.Equal(first.ContentHash, second.ContentHash);
        Assert.Equal(4ul, first.Generation);
        Assert.Equal(64ul, first.EvaluationCount);
        Assert.NotEmpty(first.ArchiveCandidateIds);
        Assert.Equal(16, first.Population.Solutions.Count);
    }

    private static StudyProblem Problem() => new()
    {
        Variables =
        [
            new(1, DesignVariableKind.Continuous, 0.0, 1.0),
            new(2, DesignVariableKind.Integer, 1.0, 10.0),
            new(3, DesignVariableKind.Categorical, 0.0, 2.0),
        ],
        ObjectiveDirections = [ObjectiveDirection.Minimize, ObjectiveDirection.Maximize],
        ConstraintCount = 1,
    };

    private static StudyCandidate[] Candidates() =>
    [
        new() { CandidateId = 1, Parameters = [0.1, 1, 0] },
        new() { CandidateId = 2, Parameters = [0.5, 5, 1] },
        new() { CandidateId = 3, Parameters = [0.9, 9, 2] },
        new() { CandidateId = 4, Parameters = [0.3, 3, 1] },
    ];

    private static StudyEvaluation Evaluation(ulong id, double first, double second, double residual) => new()
    {
        CandidateId = id,
        Objectives = [first, second],
        ConstraintResiduals = [residual],
    };
}
