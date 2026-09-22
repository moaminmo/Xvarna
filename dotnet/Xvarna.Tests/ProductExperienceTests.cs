using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class ProductExperienceTests
{
    [Fact]
    public void QualityPresetsScaleSamplesAndValidateExecutionPolicy()
    {
        AnalysisQualityProfile preview = AnalysisQualityProfile.Create(XvarnaQualityPreset.Preview);
        AnalysisQualityProfile publication = AnalysisQualityProfile.Create(
            XvarnaQualityPreset.Publication,
            XvarnaExperienceMode.Expert,
            XvarnaExecutionMode.Manual,
            runGeneration: 3);

        Assert.Equal(256, preview.ScaleSamples(1024, 16, 262_144));
        Assert.Equal(4096, publication.ScaleSamples(1024, 16, 262_144));
        Assert.Equal(3, publication.RunGeneration);
        Assert.Contains("Publication", publication.Report, StringComparison.Ordinal);
    }

    [Fact]
    public void UnitContextPreservesExplicitSiProvenance()
    {
        XvarnaUnitContext units = XvarnaUnitContext.Create("Millimeters", 0.001, 0.01);
        Assert.Equal(0.00001, units.AbsoluteToleranceMeters, 12);
        Assert.Contains("f64 metres", units.Report, StringComparison.Ordinal);
    }

    [Fact]
    public void EvidenceLabelDistinguishesConvergedAndUnresolvedResults()
    {
        XvarnaEvidenceLabel accepted = XvarnaEvidenceLabel.Convergence("nested rays", 0.5, 0.499, 4096, 0.01);
        XvarnaEvidenceLabel rejected = XvarnaEvidenceLabel.Convergence("nested rays", 0.5, 0.4, 128, 0.01);
        Assert.Equal(XvarnaEvidenceLevel.Converged, accepted.Level);
        Assert.Equal(XvarnaEvidenceLevel.MethodDeclared, rejected.Level);
    }

    [Fact]
    public void UnifiedLegendHandlesOutliersSymmetryAndMissingValues()
    {
        double[] values = [-100, -2, 0, 2, 100, double.NaN];
        XvarnaLegendResult percentile = XvarnaLegend.Build(values, new(
            XvarnaLegendDomainMode.Percentile, XvarnaPalette.Cividis,
            LowerPercentile: 0.2, UpperPercentile: 0.8));
        Assert.Equal(1, percentile.ClippedLowCount);
        Assert.Equal(1, percentile.ClippedHighCount);
        Assert.Equal(1, percentile.NonFiniteCount);
        Assert.Equal(0, percentile.Colors[^1].Alpha);

        XvarnaLegendResult symmetric = XvarnaLegend.Build(values, new(
            XvarnaLegendDomainMode.Symmetric, XvarnaPalette.Diverging));
        Assert.Equal(-symmetric.Maximum, symmetric.Minimum, 12);
        Assert.Equal(0.5, symmetric.NormalizedValues[2], 12);
    }
}
