using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class VisibilityTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void IsovistCrossesNativeBoundaryWithModelUnitsAndAttribution()
    {
        using XvarnaScene scene = BuildSquareRoomMillimeters();
        PlanarViewpoint viewpoint = new(7, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0);

        IsovistResult result = scene.AnalyzeIsovists(
            [viewpoint],
            new IsovistOptions(1440, 360.0, 10_000.0, 0.0, 1));

        IsovistSummary summary = Assert.Single(result.Summaries);
        Assert.InRange(summary.Area, 3_980_000.0, 4_020_000.0);
        Assert.InRange(summary.Perimeter, 7_970.0, 8_030.0);
        Assert.Equal(1440ul, summary.OccludedCount);
        Assert.Equal(77ul, summary.DominantOccluderObjectId);
        Assert.Equal(1440, result.Rays.Count);
        Assert.All(result.Rays, ray => Assert.Equal(IsovistRayState.Occluded, ray.State));
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void IntervisibilityReturnsBlockersVisibilityAndPrivacyRisk()
    {
        using XvarnaScene scene = BuildDividerMillimeters();
        VisibilityObserverPoint[] observers =
        [
            new(1, -1000.0, 0.0, 0.0, 1.0),
            new(2, -1000.0, 5000.0, 0.0, 0.5),
        ];
        VisibilityTargetPoint target = new(10, 1000.0, 0.0, 0.0, 0.8, true, -1.0, 0.0, 0.0);

        IntervisibilityResult result = scene.AnalyzeIntervisibility(
            observers,
            [target],
            new IntervisibilityOptions(0.1, double.PositiveInfinity, 2000.0, 1.0, 1));

        Assert.Equal(VisibilityState.Blocked, result.Entries[0].State);
        Assert.Equal(42ul, result.Entries[0].BlockerObjectId);
        Assert.Equal(VisibilityState.Visible, result.Entries[1].State);
        Assert.True(result.Entries[1].PrivacyRisk > 0.0);
        Assert.InRange(result.TargetSummaries[0].CombinedPrivacyRisk, 0.0, 1.0);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void VisibilityGraphComputesTopologyAndExactPairStates()
    {
        using XvarnaScene scene = BuildDividerMillimeters();
        VisibilityNodePoint[] nodes =
        [
            new(1, -2000.0, 0.0, 0.0),
            new(2, -1000.0, 0.0, 0.0),
            new(3, 1000.0, 0.0, 0.0),
            new(4, 2000.0, 0.0, 0.0),
        ];

        VisibilityGraphResult result = scene.AnalyzeVisibilityGraph(
            nodes,
            new VisibilityGraphOptions(0.1, double.PositiveInfinity, 1, true));

        Assert.Equal(6, result.Pairs.Count);
        Assert.Equal(2ul, result.VisibleEdgeCount);
        Assert.Equal(2ul, result.ConnectedComponentCount);
        Assert.All(result.Metrics, value => Assert.Equal(1ul, value.Degree));
        Assert.Contains(result.Pairs, pair => pair.State == VisibilityState.Blocked && pair.BlockerObjectId == 42ul);
    }

    [Fact]
    public void SparseVisibilityGraphCrossesAbiWithoutAllPairsMaterialization()
    {
        using XvarnaScene scene = BuildDividerMillimeters();
        VisibilityNodePoint[] nodes = Enumerable.Range(0, 8)
            .Select(index => new VisibilityNodePoint(checked((ulong)index + 1), index * 1000.0, 5000.0, 0.0))
            .ToArray();

        VisibilityGraphResult result = scene.AnalyzeSparseVisibilityGraph(
            nodes,
            new SparseVisibilityGraphOptions(0.1, 1100.0, 2, 2, false));

        Assert.True(result.IsSparse);
        Assert.Equal(2u, result.MaximumNeighbors);
        Assert.Equal(7, result.Pairs.Count);
        Assert.Equal(7ul, result.VisibleEdgeCount);
        Assert.Equal(1ul, result.ConnectedComponentCount);
        Assert.All(result.Metrics, value => Assert.InRange(value.Degree, 1ul, 2ul));
        Assert.All(result.Pairs, value => Assert.InRange(value.Distance, 0.0, 1100.0));
    }

    [Fact]
    public void CategoryMaskCanExcludeAnOtherwiseBlockingDivider()
    {
        using XvarnaScene scene = BuildDividerMillimeters();
        IntervisibilityResult result = scene.AnalyzeIntervisibility(
            [new VisibilityObserverPoint(1, -1000.0, 0.0, 0.0)],
            [new VisibilityTargetPoint(1, 1000.0, 0.0, 0.0)],
            new IntervisibilityOptions(CategoryMask: 2));

        Assert.Equal(VisibilityState.Visible, Assert.Single(result.Entries).State);
    }

    private static XvarnaScene BuildSquareRoomMillimeters()
    {
        double[] positions =
        [
            -1000, -1000, -1000, 1000, -1000, -1000, 1000, -1000, 1000, -1000, -1000, 1000,
            1000, -1000, -1000, 1000, 1000, -1000, 1000, 1000, 1000, 1000, -1000, 1000,
            1000, 1000, -1000, -1000, 1000, -1000, -1000, 1000, 1000, 1000, 1000, 1000,
            -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, 1000, 1000,
        ];
        uint[] triangles =
        [
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7,
            8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15,
        ];
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(positions, triangles);
        builder.AddInstance(mesh, Identity, 77, 1, 1);
        return builder.Build();
    }

    private static XvarnaScene BuildDividerMillimeters()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [
                0.0, -2000.0, -2000.0,
                0.0, 2000.0, -2000.0,
                0.0, 2000.0, 2000.0,
                0.0, -2000.0, 2000.0,
            ],
            [0, 1, 2, 0, 2, 3]);
        builder.AddInstance(mesh, Identity, 42, 9, 1);
        return builder.Build();
    }
}
