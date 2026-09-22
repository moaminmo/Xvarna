using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class SurfaceTests
{
    [Fact]
    public void SurfaceGridPreservesAreaIdsAndResolution()
    {
        SurfaceGridResult result = SurfaceGridGenerator.Generate(
            UnitQuadPositions(1.0),
            [0, 1, 2, 0, 2, 3],
            new(0.36, 0.01, 100, 10_000));

        Assert.True(result.Cells.Count > 2);
        Assert.Equal(1.0, result.SourceArea, 12);
        Assert.Equal(1.0, result.SampledArea, 12);
        Assert.True(result.MaximumCellEdgeLength <= 0.36 + 1.0e-12);
        Assert.Equal(100UL, result.Cells[0].SensorId);
        Assert.Equal(99UL + (ulong)result.Cells.Count, result.Cells[^1].SensorId);
        Assert.All(result.Cells, cell => Assert.Equal(0.01, cell.PositionZ, 12));
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void PotentialIsUnitSafeAreaWeightedAndRegionAware()
    {
        // A 100 cm square is one square metre. This exercises explicit unit scaling.
        SurfaceGridResult grid = SurfaceGridGenerator.Generate(
            UnitQuadPositions(100.0),
            [0, 1, 2, 0, 2, 3],
            new(200.0, 0.1, 1, 10));

        PvPotentialResult result = SurfacePotentialAnalyzer.Analyze(
            grid,
            [1_000_000.0, 500_000.0],
            new(800.0, 0.20, 1.0, 0.0),
            0.01);

        Assert.Equal(1.0, result.TotalAreaM2, 12);
        Assert.Equal(0.5, result.EligibleAreaM2, 12);
        Assert.Equal(750_000.0, result.MeanIrradianceWhM2, 8);
        Assert.Equal(100.0, result.ProxyYieldKWh, 12);
        Assert.Equal(0.1, result.CapacityKwp, 12);
        Assert.Equal(1_000.0, result.SpecificYieldKWhKwp, 12);
        PvPotentialRegion region = Assert.Single(result.Regions);
        Assert.Equal(1UL, region.CellCount);
        Assert.Equal(1UL, result.Cells[0].RegionId);
        Assert.Equal(0UL, result.Cells[1].RegionId);
    }

    [Fact]
    public void SurfaceContractsRejectUnsafeResolutionAndMisalignedValues()
    {
        Assert.Throws<XvarnaNativeException>(() => SurfaceGridGenerator.Generate(
            UnitQuadPositions(1.0),
            [0, 1, 2, 0, 2, 3],
            new(0.01, 0.0, 1, 4)));

        SurfaceGridResult grid = SurfaceGridGenerator.Generate(
            UnitQuadPositions(1.0),
            [0, 1, 2, 0, 2, 3],
            new(2.0));
        Assert.Throws<ArgumentException>(() => SurfacePotentialAnalyzer.Analyze(
            grid,
            [1.0],
            new(),
            1.0));
    }

    private static double[] UnitQuadPositions(double length) =>
    [
        0.0, 0.0, 0.0,
        length, 0.0, 0.0,
        length, length, 0.0,
        0.0, length, 0.0,
    ];
}
