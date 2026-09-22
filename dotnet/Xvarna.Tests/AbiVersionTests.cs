using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class AbiVersionTests
{
    [Fact]
    public void ExpectedAbiConsumesSameVersion()
    {
        Assert.True(AbiVersion.Expected.IsSatisfiedBy(new(0, 19, 0)));
    }

    [Theory]
    [InlineData(1u, 2u, false)]
    [InlineData(0u, 1u, false)]
    [InlineData(0u, 2u, false)]
    [InlineData(0u, 3u, false)]
    [InlineData(0u, 4u, false)]
    [InlineData(0u, 5u, false)]
    [InlineData(0u, 6u, false)]
    [InlineData(0u, 7u, false)]
    [InlineData(0u, 8u, false)]
    [InlineData(0u, 9u, false)]
    [InlineData(0u, 10u, false)]
    [InlineData(0u, 11u, false)]
    [InlineData(0u, 12u, false)]
    [InlineData(0u, 13u, false)]
    [InlineData(0u, 14u, false)]
    [InlineData(0u, 15u, false)]
    [InlineData(0u, 16u, false)]
    [InlineData(0u, 17u, false)]
    [InlineData(0u, 18u, false)]
    [InlineData(0u, 19u, true)]
    public void CompatibilityRequiresSameMajorAndSupportedMinor(
        uint major,
        uint minor,
        bool expected)
    {
        Assert.Equal(expected, AbiVersion.Expected.IsSatisfiedBy(new(major, minor, 0)));
    }

    [Fact]
    public void VersionUsesStableInvariantRepresentation()
    {
        Assert.Equal("0.19.0", AbiVersion.Expected.ToString());
    }

    [Fact]
    public void ManagedConnectorLoadsAndValidatesNativeEngine()
    {
        EngineProbeResult probe = EngineProbe.TryLoad();

        Assert.True(probe.IsAvailable, probe.Status);
        Assert.Equal(new Version(0, 19, 0), probe.EngineVersion);
        Assert.Equal(AbiVersion.Expected, probe.NativeAbi);
        Assert.Equal("Ready", probe.Status);
    }
}
