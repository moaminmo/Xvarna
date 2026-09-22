using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class SolarTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    [Fact]
    public void NrelSpaGoldenCaseCrossesNativeAndManagedBoundaries()
    {
        XvarnaSunSet sunSet = GoldenSunSet();

        SunSample sun = Assert.Single(sunSet.Samples);
        Assert.Equal(194.34024, sun.AzimuthDegrees, 4);
        Assert.Equal(39.88838, sun.AltitudeDegrees, 4);
        Assert.True(sun.IsActive);
        Assert.Equal(1ul, sunSet.ActiveSampleCount);
        Assert.True(sunSet.IncludesAtmosphericRefraction);
        Assert.Equal(64, sunSet.ContentHash.Length);
    }

    [Fact]
    public void DirectSunReturnsHoursTimelineAndDominantBlockerInModelUnits()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong meshId = builder.AddMesh(
            [
                -100_000.0, -100_000.0, 1000.0,
                100_000.0, -100_000.0, 1000.0,
                -100_000.0, 100_000.0, 1000.0,
            ],
            [0, 1, 2]);
        builder.AddInstance(meshId, Identity, objectId: 99, instanceId: 7, categoryMask: 4);
        using XvarnaScene scene = builder.Build();
        XvarnaSunSet sunSet = GoldenSunSet();
        SolarSensor sensor = new(5, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);

        DirectSunResult blocked = scene.AnalyzeDirectSun(
            [sensor],
            sunSet,
            new DirectSunOptions(0.1, double.PositiveInfinity, CategoryMask: 4));
        SensorSunSummary summary = Assert.Single(blocked.Summaries);
        SunTimelineEntry entry = Assert.Single(blocked.Timeline);
        Assert.Equal(SunState.Blocked, entry.State);
        Assert.Equal(99ul, entry.ObjectId);
        Assert.Equal(7ul, entry.InstanceId);
        Assert.Equal(1.0, summary.ShadowHours, 12);
        Assert.Equal(0.0, summary.DirectSunHours, 12);
        Assert.Equal(99ul, summary.DominantOccluderObjectId);
        Assert.InRange(entry.Distance, 1000.0, 2000.0);
        Assert.Equal(64, blocked.ContentHash.Length);

        DirectSunResult filtered = scene.AnalyzeDirectSun(
            [sensor],
            sunSet,
            new DirectSunOptions(0.1, double.PositiveInfinity, CategoryMask: 2));
        Assert.Equal(SunState.Visible, Assert.Single(filtered.Timeline).State);
        Assert.Equal(1.0, Assert.Single(filtered.Summaries).DirectSunHours, 12);
    }

    [Fact]
    public void SolarEngineRejectsInvalidScheduleAndLocation()
    {
        SolarTimeSample sample = new(DateTimeOffset.UtcNow, 1.0);
        Assert.Throws<XvarnaNativeException>(() =>
            SolarEngine.Calculate(new SolarLocation(95.0, 0.0), [sample]));
        Assert.Throws<XvarnaNativeException>(() =>
            SolarEngine.Calculate(new SolarLocation(0.0, 0.0), [sample with { DurationHours = 0.0 }]));
    }

    private static XvarnaSunSet GoldenSunSet() =>
        SolarEngine.Calculate(
            new SolarLocation(39.742476, -105.1786, 1830.14),
            [new SolarTimeSample(DateTimeOffset.Parse("2003-10-17T19:30:30Z"), 1.0)],
            new SolarPositionOptions(
                DeltaTSeconds: 67.0,
                PressureMillibars: 820.0,
                TemperatureCelsius: 11.0));
}
