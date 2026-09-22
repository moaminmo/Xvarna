using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class IntelligenceTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void Isovist3dCrossesAbiWithMaterialsAttributionAndCounterfactuals()
    {
        using XvarnaScene scene = BuildWall();
        AnalysisMaterialLibrary materials = new(
            [new AnalysisMaterial(4, 0.5, 0.4, 0.1)],
            [new MaterialAssignment(42, 4)]);

        Isovist3dResult result = scene.AnalyzeIsovists3d(
            [new SpatialViewpoint(7, -1000.0, 0.0, 0.0)],
            materials,
            new Isovist3dOptions(256, 10_000.0, 0.1, 1, 4, 1));

        Assert.Single(result.Summaries);
        Assert.Equal(256, result.Rays.Count);
        ObstructionAttribution blocker = Assert.Single(result.Attribution, value => value.ObjectId == 42);
        Assert.Equal(1ul, blocker.CategoryMask);
        Assert.True(blocker.CounterfactualVolumeDelta > 0.0);
        Assert.Contains(result.Categories, value => value.CategoryMask == 1);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void LandmarkCrossesAbiWithPartialMaterialVisibility()
    {
        using XvarnaScene scene = BuildWall();
        AnalysisMaterialLibrary materials = new(
            [new AnalysisMaterial(4, 0.5, 0.4, 0.1)],
            [new MaterialAssignment(42, 4)]);

        LandmarkVisibilityResult result = scene.AnalyzeLandmarkVisibility(
            [new LandmarkObserver(1, -1000.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0)],
            [new Landmark(9, 1000.0, 0.0, 0.0, 100.0)],
            materials,
            new LandmarkVisibilityOptions(64, 90.0, 90.0, 10_000.0, 0.1, 1, 4));

        LandmarkVisibilityEntry entry = Assert.Single(result.Entries);
        Assert.True(entry.InsideFieldOfView);
        Assert.InRange(entry.VisibleFraction, 0.45, 0.55);
        Assert.Equal(42ul, entry.DominantBlockerObjectId);
        Assert.Contains(result.Attribution, value => value.ObjectId == 42);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void SolarScenarioAndEnvelopeCrossAbiWithDeltasAndControls()
    {
        XvarnaSunSet sunSet = SolarEngine.Calculate(
            new SolarLocation(39.742476, -105.1786, 1830.14),
            [new SolarTimeSample(DateTimeOffset.Parse("2003-10-17T19:30:30Z"), 1.0)],
            new SolarPositionOptions(67.0, 820.0, 11.0));
        SunSample sun = Assert.Single(sunSet.Samples);
        using XvarnaScene blockedScene = BuildHorizontalRoof();
        SolarSensor sensor = new(1, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        AnalysisMaterialLibrary glass = new(
            [new AnalysisMaterial(5, 0.5, 0.4, 0.1)],
            [new MaterialAssignment(99, 5)]);

        SolarScenarioComparison comparison = blockedScene.CompareSolarScenarios(
            [sensor], sunSet,
            [
                new SolarMaterialScenario { ScenarioId = 1, Name = "opaque", Materials = AnalysisMaterialLibrary.Opaque },
                new SolarMaterialScenario { ScenarioId = 2, Name = "glass", Materials = glass },
            ],
            new SolarScenarioOptions(0.1, double.PositiveInfinity, 0.0, 4, 8, 0.0001, 4));

        Assert.Equal(2, comparison.Summaries.Count);
        Assert.True(comparison.Summaries[1].ReceivedSunHours > comparison.Summaries[0].ReceivedSunHours);
        Assert.True(Assert.Single(comparison.Deltas).ReceivedSunHoursDelta > 0.0);
        Assert.Contains(comparison.Attribution, value => value.ObjectId == 99);

        using XvarnaScene emptyContext = BuildFarTriangle();
        double horizontal = Math.Sqrt(sun.DirectionX * sun.DirectionX + sun.DirectionY * sun.DirectionY);
        EnvelopeCandidate candidate = new(3, sun.DirectionX / horizontal * 1000.0,
            sun.DirectionY / horizontal * 1000.0, 0.0);
        SolarEnvelopeResult envelope = emptyContext.AnalyzeSolarEnvelope(
            [sensor], [candidate], sunSet, AnalysisMaterialLibrary.Opaque,
            new SolarEnvelopeOptions(50.0, 0.8, 0.5, 0.0, 0.1));

        SolarEnvelopeCell cell = Assert.Single(envelope.Cells);
        Assert.Equal(1ul, cell.ConstraintCount);
        Assert.True(double.IsFinite(cell.MaximumSolarAccessElevation));
        Assert.Equal(1ul, cell.AccessControl.SensorId);
        Assert.Equal(64, envelope.ContentHash.Length);

        SolarEnvelopeResult explicitlyOriented = emptyContext.AnalyzeOrientedSolarEnvelope(
            [sensor],
            [new OrientedEnvelopeCandidate(candidate.CandidateId, candidate.X, candidate.Y, candidate.Z, 0.0, 0.0, 5.0)],
            sunSet, AnalysisMaterialLibrary.Opaque,
            new SolarEnvelopeOptions(50.0, 0.8, 0.5, 0.0, 0.1));
        SolarEnvelopeCell orientedCell = Assert.Single(explicitlyOriented.Cells);
        Assert.Equal(cell.MaximumSolarAccessHeight, orientedCell.MaximumSolarAccessHeight, 9);
        Assert.Equal(envelope.ContentHash, explicitlyOriented.ContentHash);
    }

    private static XvarnaScene BuildWall()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [0.0, -2000.0, -2000.0, 0.0, 2000.0, -2000.0, 0.0, 2000.0, 2000.0, 0.0, -2000.0, 2000.0],
            [0, 1, 2, 0, 2, 3]);
        builder.AddInstance(mesh, Identity, 42, 9, 1);
        return builder.Build();
    }

    private static XvarnaScene BuildHorizontalRoof()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [-100_000.0, -100_000.0, 1000.0, 100_000.0, -100_000.0, 1000.0, -100_000.0, 100_000.0, 1000.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 99, 7, 4);
        return builder.Build();
    }

    private static XvarnaScene BuildFarTriangle()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [100_000.0, 100_000.0, 100_000.0, 101_000.0, 100_000.0, 100_000.0, 100_000.0, 101_000.0, 100_000.0],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1, 1);
        return builder.Build();
    }
}
