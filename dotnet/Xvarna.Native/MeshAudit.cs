using System.Buffers.Binary;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;

namespace Xvarna.Native;

/// <summary>Stable issue bits for one source triangle.</summary>
[Flags]
public enum MeshFaceIssue : uint
{
    /// <summary>No face issue was detected.</summary>
    None = 0,
    /// <summary>The face contains an index outside the vertex buffer.</summary>
    InvalidIndex = 1 << 0,
    /// <summary>The face references a NaN or infinite vertex.</summary>
    NonFiniteVertex = 1 << 1,
    /// <summary>The face repeats an index or has negligible area.</summary>
    Degenerate = 1 << 2,
    /// <summary>The face duplicates an earlier face independent of winding.</summary>
    Duplicate = 1 << 3,
}

/// <summary>Stable issue bits for one source vertex.</summary>
[Flags]
public enum MeshVertexIssue : uint
{
    /// <summary>No vertex issue was detected.</summary>
    None = 0,
    /// <summary>The vertex contains NaN or infinity.</summary>
    NonFinite = 1 << 0,
    /// <summary>The vertex is unused by every accepted face.</summary>
    Isolated = 1 << 1,
}

/// <summary>Double-precision axis-aligned mesh bounds.</summary>
public readonly record struct MeshBounds(
    double MinX,
    double MinY,
    double MinZ,
    double MaxX,
    double MaxY,
    double MaxZ);

/// <summary>Deterministic mesh-health result produced by the native engine.</summary>
public sealed record MeshAuditResult
{
    /// <summary>Source vertex count.</summary>
    public required ulong VertexCount { get; init; }
    /// <summary>Source triangle count.</summary>
    public required ulong FaceCount { get; init; }
    /// <summary>Accepted triangle count.</summary>
    public required ulong AcceptedFaceCount { get; init; }
    /// <summary>Non-finite vertex count.</summary>
    public required ulong NonFiniteVertexCount { get; init; }
    /// <summary>Invalid-index face count.</summary>
    public required ulong InvalidIndexFaceCount { get; init; }
    /// <summary>Face count referencing non-finite vertices.</summary>
    public required ulong NonFiniteFaceCount { get; init; }
    /// <summary>Degenerate face count.</summary>
    public required ulong DegenerateFaceCount { get; init; }
    /// <summary>Duplicate face count.</summary>
    public required ulong DuplicateFaceCount { get; init; }
    /// <summary>Isolated vertex count.</summary>
    public required ulong IsolatedVertexCount { get; init; }
    /// <summary>Boundary edge count.</summary>
    public required ulong BoundaryEdgeCount { get; init; }
    /// <summary>Non-manifold edge count.</summary>
    public required ulong NonManifoldEdgeCount { get; init; }
    /// <summary>Inconsistent-winding edge count.</summary>
    public required ulong InconsistentWindingEdgeCount { get; init; }
    /// <summary>Connected component count.</summary>
    public required ulong ConnectedComponentCount { get; init; }
    /// <summary>Surface area in squared model units.</summary>
    public required double SurfaceArea { get; init; }
    /// <summary>Signed volume in cubed model units.</summary>
    public required double SignedVolume { get; init; }
    /// <summary>Bounds, or null when there are no finite vertices.</summary>
    public required MeshBounds? Bounds { get; init; }
    /// <summary>Maximum absolute source coordinate.</summary>
    public required double MaximumCoordinateMagnitude { get; init; }
    /// <summary>Conservative absolute f32 rounding estimate.</summary>
    public required double EstimatedF32Error { get; init; }
    /// <summary>True when the f32 precision budget is exceeded.</summary>
    public required bool ExceedsF32PrecisionBudget { get; init; }
    /// <summary>True when no face requires conservative removal.</summary>
    public required bool IsAnalysisReady { get; init; }
    /// <summary>True for clean, closed, manifold, consistently wound topology.</summary>
    public required bool IsWatertight { get; init; }
    /// <summary>Lowercase BLAKE3 geometry digest.</summary>
    public required string ContentHash { get; init; }
    /// <summary>One issue mask per source triangle.</summary>
    public required IReadOnlyList<MeshFaceIssue> FaceIssues { get; init; }
    /// <summary>One issue mask per source vertex.</summary>
    public required IReadOnlyList<MeshVertexIssue> VertexIssues { get; init; }

    /// <summary>Builds a deterministic human-readable diagnostic report.</summary>
    public string ToReport(string lengthUnit = "model unit")
    {
        StringBuilder report = new();
        report.AppendLine(IsAnalysisReady ? "Status: analysis-ready" : "Status: repair required");
        report.AppendLine(IsWatertight ? "Topology: watertight" : "Topology: open or inconsistent");
        report.AppendLine(CultureInfo.InvariantCulture, $"Vertices: {VertexCount:N0}; triangles: {FaceCount:N0}; accepted: {AcceptedFaceCount:N0}");
        report.AppendLine(CultureInfo.InvariantCulture, $"Area: {SurfaceArea:G10} {lengthUnit}²");
        report.AppendLine(CultureInfo.InvariantCulture, $"Signed volume: {SignedVolume:G10} {lengthUnit}³");
        report.AppendLine(CultureInfo.InvariantCulture, $"Boundary edges: {BoundaryEdgeCount:N0}; non-manifold: {NonManifoldEdgeCount:N0}; winding conflicts: {InconsistentWindingEdgeCount:N0}");
        report.AppendLine(CultureInfo.InvariantCulture, $"Degenerate: {DegenerateFaceCount:N0}; duplicate: {DuplicateFaceCount:N0}; invalid index: {InvalidIndexFaceCount:N0}; non-finite faces: {NonFiniteFaceCount:N0}");
        report.AppendLine(CultureInfo.InvariantCulture, $"Isolated vertices: {IsolatedVertexCount:N0}; components: {ConnectedComponentCount:N0}");
        report.AppendLine(CultureInfo.InvariantCulture, $"Estimated f32 error: {EstimatedF32Error:G6} {lengthUnit}{(ExceedsF32PrecisionBudget ? " — exceeds budget" : string.Empty)}");
        report.Append("Geometry hash: ").Append(ContentHash);
        return report.ToString();
    }
}

/// <summary>Executes native deterministic mesh-health analysis.</summary>
public static class MeshAuditor
{
    private const uint AnalysisReadyFlag = 1 << 0;
    private const uint WatertightFlag = 1 << 1;
    private const uint HasBoundsFlag = 1 << 2;
    private const uint PrecisionRiskFlag = 1 << 3;
    private const uint ExpectedSummarySize = 224;

    /// <summary>Audits packed XYZ coordinates and packed triangle indices.</summary>
    public static unsafe MeshAuditResult Audit(
        double[] positions,
        uint[] triangles,
        double absoluteTolerance,
        double maximumF32ErrorRatio = 0.25)
    {
        ArgumentNullException.ThrowIfNull(positions);
        ArgumentNullException.ThrowIfNull(triangles);
        if (positions.Length % 3 != 0)
        {
            throw new ArgumentException("Position buffer length must be divisible by three.", nameof(positions));
        }
        if (triangles.Length % 3 != 0)
        {
            throw new ArgumentException("Triangle buffer length must be divisible by three.", nameof(triangles));
        }
        if (!double.IsFinite(absoluteTolerance) || absoluteTolerance <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(absoluteTolerance), "Tolerance must be finite and positive.");
        }
        if (!double.IsFinite(maximumF32ErrorRatio) || maximumF32ErrorRatio <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumF32ErrorRatio), "Precision ratio must be finite and positive.");
        }

        NativeLibraryBootstrap.Initialize();
        MeshFaceIssue[] faceIssues = new MeshFaceIssue[triangles.Length / 3];
        MeshVertexIssue[] vertexIssues = new MeshVertexIssue[positions.Length / 3];
        NativeMeshAuditSummary summary = default;

        fixed (double* positionPointer = positions)
        fixed (uint* trianglePointer = triangles)
        fixed (MeshFaceIssue* faceIssuePointer = faceIssues)
        fixed (MeshVertexIssue* vertexIssuePointer = vertexIssues)
        {
            NativeStatus status = NativeMethods.MeshAudit(
                positionPointer,
                (nuint)vertexIssues.Length,
                trianglePointer,
                (nuint)faceIssues.Length,
                absoluteTolerance,
                maximumF32ErrorRatio,
                &summary,
                (uint*)faceIssuePointer,
                (nuint)faceIssues.Length,
                (uint*)vertexIssuePointer,
                (nuint)vertexIssues.Length);
            if (status != NativeStatus.Success)
            {
                throw new XvarnaNativeException(status);
            }
        }

        if (summary.StructureSize != ExpectedSummarySize || summary.StructureSize != Marshal.SizeOf<NativeMeshAuditSummary>())
        {
            throw new InvalidOperationException($"Native mesh-audit layout mismatch: native {summary.StructureSize}, managed {Marshal.SizeOf<NativeMeshAuditSummary>()}.");
        }

        return ConvertSummary(summary, faceIssues, vertexIssues);
    }

    private static MeshAuditResult ConvertSummary(
        NativeMeshAuditSummary summary,
        MeshFaceIssue[] faceIssues,
        MeshVertexIssue[] vertexIssues)
    {
        MeshBounds? bounds = (summary.Flags & HasBoundsFlag) != 0
            ? new(
                summary.BoundsMinX,
                summary.BoundsMinY,
                summary.BoundsMinZ,
                summary.BoundsMaxX,
                summary.BoundsMaxY,
                summary.BoundsMaxZ)
            : null;

        return new()
        {
            VertexCount = summary.VertexCount,
            FaceCount = summary.FaceCount,
            AcceptedFaceCount = summary.AcceptedFaceCount,
            NonFiniteVertexCount = summary.NonFiniteVertexCount,
            InvalidIndexFaceCount = summary.InvalidIndexFaceCount,
            NonFiniteFaceCount = summary.NonFiniteFaceCount,
            DegenerateFaceCount = summary.DegenerateFaceCount,
            DuplicateFaceCount = summary.DuplicateFaceCount,
            IsolatedVertexCount = summary.IsolatedVertexCount,
            BoundaryEdgeCount = summary.BoundaryEdgeCount,
            NonManifoldEdgeCount = summary.NonManifoldEdgeCount,
            InconsistentWindingEdgeCount = summary.InconsistentWindingEdgeCount,
            ConnectedComponentCount = summary.ConnectedComponentCount,
            SurfaceArea = summary.SurfaceArea,
            SignedVolume = summary.SignedVolume,
            Bounds = bounds,
            MaximumCoordinateMagnitude = summary.MaximumCoordinateMagnitude,
            EstimatedF32Error = summary.EstimatedF32Error,
            ExceedsF32PrecisionBudget = (summary.Flags & PrecisionRiskFlag) != 0,
            IsAnalysisReady = (summary.Flags & AnalysisReadyFlag) != 0,
            IsWatertight = (summary.Flags & WatertightFlag) != 0,
            ContentHash = FormatHash(summary),
            FaceIssues = faceIssues,
            VertexIssues = vertexIssues,
        };
    }

    private static string FormatHash(NativeMeshAuditSummary summary)
    {
        Span<byte> hash = stackalloc byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(hash[0..8], summary.ContentHash0);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[8..16], summary.ContentHash1);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[16..24], summary.ContentHash2);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[24..32], summary.ContentHash3);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }
}

/// <summary>Represents a contained failure reported by the native engine.</summary>
public sealed class XvarnaNativeException : Exception
{
    internal XvarnaNativeException(NativeStatus status)
        : base($"XVARNA native operation failed with status {status} ({(int)status}).")
    {
        StatusCode = (int)status;
    }

    /// <summary>Stable native status code.</summary>
    public int StatusCode { get; }
}
