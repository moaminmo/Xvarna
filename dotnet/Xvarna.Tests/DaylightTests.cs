using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class DaylightTests
{
    private static readonly double[] Identity =
    [
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
    ];

    private const string Epw =
        "LOCATION,Tehran,Tehran,IRN,IWEC,407540,35.68,51.32,3.5,1191\n" +
        "DESIGN CONDITIONS,0\nTYPICAL/EXTREME PERIODS,0\nGROUND TEMPERATURES,0\n" +
        "HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0\nCOMMENTS 1,XVARNA daylight fixture\n" +
        "COMMENTS 2,interval ending\nDATA PERIODS,1,1,Data,Monday,1/1,12/31\n" +
        "2024,6,21,12,60,A,25,0,0,90000,0,0,333,800,700,100,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.2,0,0\n";

    [Fact]
    public void PointFactorMatrixReuseAndRadianceExportCrossTheCompleteAbi()
    {
        using XvarnaScene scene = BuildOpenScene();
        DaylightSensor[] sensors = [new(7, 0, 0, 0, 0, 0, 1, 1)];
        DaylightMatrixOptions options = new(SkyPatchCount: 64);
        DaylightMoment moment = new(DateTimeOffset.FromUnixTimeSeconds(1_718_966_400),
            0, 0, 1, 50000, 10000);

        PointIlluminanceResult first = scene.AnalyzePointIlluminance(
            sensors, OpticalMaterialLibrary.Opaque, moment, options);
        PointIlluminanceResult second = scene.AnalyzePointIlluminance(
            sensors, OpticalMaterialLibrary.Opaque, moment, options);
        DaylightFactorResult factor = scene.AnalyzeDaylightFactor(
            sensors, OpticalMaterialLibrary.Opaque, 10000, options);
        RadianceExportBundle export = scene.ExportRadiance(sensors, OpticalMaterialLibrary.Opaque);

        PointIlluminanceEntry point = Assert.Single(first.Entries);
        Assert.Equal(60000, point.TotalLux, 7);
        Assert.False(first.MatrixReused);
        Assert.True(second.MatrixReused);
        Assert.Equal(first.MatrixHash, second.MatrixHash);
        Assert.Equal(100, Assert.Single(factor.Entries).DaylightFactorPercent, 7);
        Assert.Contains("polygon", export.GeometryRad);
        Assert.Contains("xv_implicit_opaque", export.MaterialsRad);
        Assert.Contains("\"schemaVersion\":\"0.16.0\"", export.ManifestJson);
        Assert.Equal(64, export.ContentHash.Length);
    }

    [Fact]
    public async Task AnnualMetricsProgressCancellationAndValidationArePublicContracts()
    {
        using XvarnaScene scene = BuildOpenScene();
        DaylightSensor[] sensors = [new(1, 0, 0, 0, 0, 0, 1, 2)];
        AnnualDaylightOptions options = AnnualDaylightOptions.Default with
        {
            Matrix = new DaylightMatrixOptions(SkyPatchCount: 64),
        };
        using DaylightAnalysisJob<AnnualDaylightResult> job = scene.BeginAnnualDaylight(
            sensors, EpwWeatherFile.Parse(Epw), OpticalMaterialLibrary.Opaque, options);

        AnnualDaylightResult result = await job.Completion;
        AnnualDaylightSensorSummary summary = Assert.Single(result.Summaries);
        Assert.Single(result.Timeline);
        Assert.Equal(2, result.Project.TotalSensorArea, 10);
        Assert.True(summary.MeanOccupiedLux > 0);
        Assert.Equal(1, job.Progress.Fraction);
        Assert.Equal(64, result.MatrixHash.Length);

        DaylightValidationReport identical = DaylightValidation.Compare([100, 500], [100, 500]);
        Assert.True(identical.Accepted);
        Assert.Equal(0, identical.RootMeanSquareErrorLux, 12);
        Assert.Equal(1, identical.RSquared, 12);

        using CancellationTokenSource cancellation = new();
        cancellation.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => scene.AnalyzeAnnualDaylightAsync(
            sensors, EpwWeatherFile.Parse(Epw), OpticalMaterialLibrary.Opaque, options, cancellation.Token));
    }

    [Fact]
    public void OpticalLibraryRejectsEnergyCreationAndPreservesRgbMapping()
    {
        Assert.Throws<ArgumentException>(() => new OpticalMaterialLibrary(
            [new OpticalMaterial(1, OpticalMaterialKind.Glass, 0.6, 0.1, 0.1, 0.6, 0.1, 0.1)], []));

        OpticalMaterialLibrary valid = new(
            [new OpticalMaterial(2, OpticalMaterialKind.Glass, 0.1, 0.1, 0.1, 0.6, 0.65, 0.7)],
            [new MaterialAssignment(1, 2)]);
        using XvarnaScene scene = BuildOpenScene();
        RadianceExportBundle export = scene.ExportRadiance(
            [new DaylightSensor(1, 0, 0, 0, 0, 0, 1, 1)], valid);
        Assert.Contains("void glass xv_mat_2", export.MaterialsRad);
        Assert.Contains("xv_mat_2 polygon xv_o1_i1_t0", export.GeometryRad);
    }

    private static XvarnaScene BuildOpenScene()
    {
        using XvarnaSceneBuilder builder = new(new SceneBuildOptions(0.001, 1e-6, ThreadCount: 2));
        ulong mesh = builder.AddMesh(
            [-1000, -1000, -1000, 1000, -1000, -1000, 0, 1000, -1000],
            [0, 1, 2]);
        builder.AddInstance(mesh, Identity, 1, 1);
        return builder.Build();
    }
}
