using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class IrradianceTests
{
    private static readonly double[] Identity =
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    private const string Epw =
        "LOCATION,Tehran,Tehran,IRN,IWEC,407540,35.68,51.32,3.5,1191\n" +
        "DESIGN CONDITIONS,0\n" +
        "TYPICAL/EXTREME PERIODS,0\n" +
        "GROUND TEMPERATURES,0\n" +
        "HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0\n" +
        "COMMENTS 1,XVARNA managed fixture\n" +
        "COMMENTS 2,interval-ending radiation\n" +
        "DATA PERIODS,1,1,Data,Monday,1/1,12/31\n" +
        "2024,6,21,12,60,A,25,0,0,90000,0,0,333,800,700,100,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.2,0,0\n";

    [Fact]
    public void EpwInspectionExposesTimingLocationDiagnosticsAndIdentity()
    {
        EpwWeatherFile weather = EpwWeatherFile.Parse(Epw);

        Assert.Equal(1ul, weather.WeatherCount);
        Assert.Equal(1u, weather.RecordsPerHour);
        Assert.Equal(35.68, weather.Location.LatitudeDegrees, 12);
        Assert.Equal(3.5, weather.TimeZoneHours, 12);
        Assert.Equal(0ul, weather.MissingDirectNormalCount);
        Assert.Equal(64, weather.ContentHash.Length);
    }

    [Fact]
    public async Task AnnualIrradianceCrossesManagedNativeAsyncAndProgressBoundaries()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        EpwWeatherFile weather = EpwWeatherFile.Parse(Epw);
        using AnnualIrradianceAnalysisJob job = scene.BeginAnnualIrradiance(
            [new SolarSensor(5, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0)],
            weather,
            AnnualIrradianceOptions.Default);

        AnnualIrradianceResult result = await job.Completion;
        AnnualIrradianceProgress progress = job.Progress;
        SensorIrradianceSummary summary = Assert.Single(result.Summaries);
        IrradianceTimelineEntry entry = Assert.Single(result.Timeline);

        Assert.Equal(SunState.Visible, entry.State);
        Assert.True(entry.DirectWhM2 > 0.0);
        Assert.Equal(100.0, entry.DiffuseSkyWhM2, 8);
        Assert.Equal(0.0, entry.GroundReflectedWhM2, 12);
        Assert.Equal(entry.GlobalWhM2, summary.GlobalWhM2, 12);
        Assert.Equal(1.0, summary.DomeVisibilityRatio, 12);
        Assert.Equal(169ul, progress.TotalWorkUnits);
        Assert.Equal(169ul, progress.CompletedWorkUnits);
        Assert.Equal(1.0, progress.Fraction);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public async Task AnnualIrradianceObservesPreCancelledToken()
    {
        using XvarnaScene scene = BuildOpenMillimetreScene();
        using CancellationTokenSource cancellation = new();
        cancellation.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            scene.AnalyzeAnnualIrradianceAsync(
                [new SolarSensor(1, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0)],
                EpwWeatherFile.Parse(Epw),
                AnnualIrradianceOptions.Default,
                cancellation.Token));
    }

    private static XvarnaScene BuildOpenMillimetreScene()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1.0e-6, ThreadCount: 2));
        ulong meshId = builder.AddMesh(
            [
                -1000.0, -1000.0, -1000.0,
                1000.0, -1000.0, -1000.0,
                0.0, 1000.0, -1000.0,
            ],
            [0, 1, 2]);
        builder.AddInstance(meshId, Identity, objectId: 1, instanceId: 1);
        return builder.Build();
    }
}
