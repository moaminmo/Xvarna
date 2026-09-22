using Xvarna.Native;
using Xunit;

namespace Xvarna.Tests;

public sealed class MeshAuditTests
{
    private const double Tolerance = 1.0e-9;

    [Fact]
    public void ClosedTetrahedronCrossesNativeBoundaryAsWatertight()
    {
        double[] positions =
        [
            0.0, 0.0, 0.0,
            1.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 0.0, 1.0,
        ];
        uint[] faces =
        [
            0, 2, 1,
            0, 1, 3,
            1, 2, 3,
            2, 0, 3,
        ];

        MeshAuditResult result = MeshAuditor.Audit(positions, faces, Tolerance);

        Assert.True(result.IsAnalysisReady);
        Assert.True(result.IsWatertight);
        Assert.Equal(4ul, result.VertexCount);
        Assert.Equal(4ul, result.AcceptedFaceCount);
        Assert.Equal(0ul, result.BoundaryEdgeCount);
        Assert.Equal(1ul, result.ConnectedComponentCount);
        Assert.Equal(1.0 / 6.0, result.SignedVolume, 12);
        Assert.Equal(64, result.ContentHash.Length);
    }

    [Fact]
    public void OpenQuadIsAnalysisReadyButNotWatertight()
    {
        MeshAuditResult result = MeshAuditor.Audit(OpenQuadPositions(), OpenQuadFaces(), Tolerance);

        Assert.True(result.IsAnalysisReady);
        Assert.False(result.IsWatertight);
        Assert.Equal(4ul, result.BoundaryEdgeCount);
        Assert.Equal(1.0, result.SurfaceArea, 12);
        Assert.All(result.FaceIssues, issue => Assert.Equal(MeshFaceIssue.None, issue));
    }

    [Fact]
    public void DuplicateDegenerateAndIsolatedGeometryIsPreciselyFlagged()
    {
        double[] positions = [.. OpenQuadPositions(), 9.0, 9.0, 9.0];
        uint[] faces = [.. OpenQuadFaces(), 2, 1, 0, 0, 0, 1];

        MeshAuditResult result = MeshAuditor.Audit(positions, faces, Tolerance);

        Assert.False(result.IsAnalysisReady);
        Assert.Equal(1ul, result.DuplicateFaceCount);
        Assert.Equal(1ul, result.DegenerateFaceCount);
        Assert.Equal(1ul, result.IsolatedVertexCount);
        Assert.Equal(MeshFaceIssue.Duplicate, result.FaceIssues[2]);
        Assert.Equal(MeshFaceIssue.Degenerate, result.FaceIssues[3]);
        Assert.Equal(MeshVertexIssue.Isolated, result.VertexIssues[4]);
    }

    [Fact]
    public void ContentHashIsDeterministicAcrossCalls()
    {
        MeshAuditResult first = MeshAuditor.Audit(OpenQuadPositions(), OpenQuadFaces(), Tolerance);
        MeshAuditResult second = MeshAuditor.Audit(OpenQuadPositions(), OpenQuadFaces(), Tolerance);

        Assert.Equal(first.ContentHash, second.ContentHash);
        Assert.Contains("Geometry hash", first.ToReport(), StringComparison.Ordinal);
    }

    [Theory]
    [InlineData(2, 3)]
    [InlineData(3, 2)]
    public void ManagedBoundaryRejectsMisalignedBuffers(int positionLength, int faceLength)
    {
        Assert.Throws<ArgumentException>(() =>
            MeshAuditor.Audit(new double[positionLength], new uint[faceLength], Tolerance));
    }

    [Fact]
    public void ManagedBoundaryRejectsInvalidTolerance()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            MeshAuditor.Audit(OpenQuadPositions(), OpenQuadFaces(), 0.0));
    }

    private static double[] OpenQuadPositions() =>
    [
        0.0, 0.0, 0.0,
        1.0, 0.0, 0.0,
        1.0, 1.0, 0.0,
        0.0, 1.0, 0.0,
    ];

    private static uint[] OpenQuadFaces() => [0, 1, 2, 0, 2, 3];
}
