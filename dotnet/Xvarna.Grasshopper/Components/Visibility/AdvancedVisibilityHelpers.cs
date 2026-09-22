using System.Globalization;
using Grasshopper.Kernel.Types;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

internal static class AdvancedVisibilityHelpers
{
    internal static XvarnaScene? UnwrapScene(object? value) => value switch
    {
        XvarnaScene scene => scene,
        GH_ObjectWrapper { Value: XvarnaScene scene } => scene,
        _ => null,
    };

    internal static XvarnaComputeSession? UnwrapCompute(object? value) => value switch
    {
        XvarnaComputeSession session => session,
        GH_ObjectWrapper { Value: XvarnaComputeSession session } => session,
        _ => null,
    };

    internal static string ExecutionLine(DaenaExecutionReport? execution)
    {
        if (execution is null)
        {
            return "Backend: canonical CPU f64";
        }
        string backend = execution.Backend switch
        {
            DaenaExecutionBackend.Cpu => "canonical CPU f64",
            DaenaExecutionBackend.PortableGpu => "VAYU portable GPU",
            _ => "VAYU mixed GPU/CPU",
        };
        string adapter = string.IsNullOrWhiteSpace(execution.AdapterName)
            ? string.Empty
            : $"; adapter: {execution.AdapterName}";
        string fallback = execution.RuntimeFallbackBatchCount == 0 && !execution.UsedCreationFallback
            ? string.Empty
            : $"; fallback batches: {execution.RuntimeFallbackBatchCount:N0}";
        string reuse = execution.UsedReusableBuffers ? "; persistent arena reused" : string.Empty;
        return string.Create(CultureInfo.InvariantCulture,
            $"Backend: {backend}{adapter}; {execution.BatchCount:N0} batch(es), {execution.RayCount:N0} rays, {execution.DispatchCount:N0} dispatch(es){reuse}{fallback}{Environment.NewLine}" +
            $"VAYU: upload {execution.UploadMicroseconds / 1000.0:N3} ms; execute {execution.ExecutionMicroseconds / 1000.0:N3} ms; readback {execution.ReadbackMicroseconds / 1000.0:N3} ms; max precision error {execution.MaximumPrecisionErrorMeters:G6} m");
    }

    internal static string BackendMessage(DaenaExecutionReport? execution) => execution?.Backend switch
    {
        DaenaExecutionBackend.PortableGpu => "VAYU GPU",
        DaenaExecutionBackend.Mixed => "VAYU Mixed",
        _ => "CPU f64",
    };

    internal static ulong ParseUnsigned(string value, string name, bool allowAll = false)
    {
        if (allowAll && string.Equals(value, "-1", StringComparison.Ordinal)) return ulong.MaxValue;
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
            throw new ArgumentException($"{name} '{value}' is not {(allowAll ? "-1 or " : string.Empty)}an unsigned 64-bit integer.");
        return parsed;
    }

    internal static bool Compatible(int sourceCount, int valueCount) =>
        valueCount is 0 or 1 || valueCount == sourceCount;

    internal static T Select<T>(IReadOnlyList<T> values, int index, T fallback) =>
        values.Count == 0 ? fallback : values[values.Count == 1 ? 0 : index];

    internal static IReadOnlyList<ViewTargetTriangle> TriangulateTargets(
        IReadOnlyList<Mesh> meshes,
        IReadOnlyList<string> targetIds,
        IReadOnlyList<string> categoryMasks,
        IReadOnlyList<double> weights)
    {
        if (meshes.Count == 0) throw new ArgumentException("At least one target mesh is required.");
        if (!Compatible(meshes.Count, targetIds.Count)
            || !Compatible(meshes.Count, categoryMasks.Count)
            || !Compatible(meshes.Count, weights.Count))
            throw new ArgumentException("Target IDs, category masks, and weights must contain zero, one, or one item per target mesh.");

        List<ViewTargetTriangle> patches = [];
        for (int meshIndex = 0; meshIndex < meshes.Count; meshIndex++)
        {
            Mesh mesh = meshes[meshIndex].DuplicateMesh();
            mesh.Faces.ConvertQuadsToTriangles();
            ulong targetId = targetIds.Count == 0
                ? checked((ulong)meshIndex + 1)
                : ParseUnsigned(Select(targetIds, meshIndex, string.Empty), "Target ID");
            ulong category = categoryMasks.Count == 0
                ? 1UL
                : ParseUnsigned(Select(categoryMasks, meshIndex, "1"), "Target category", true);
            double weight = Select(weights, meshIndex, 1.0);
            for (int faceIndex = 0; faceIndex < mesh.Faces.Count; faceIndex++)
            {
                MeshFace face = mesh.Faces[faceIndex];
                if (!face.IsTriangle) throw new InvalidOperationException("Target triangulation produced a non-triangle face.");
                Point3d first = mesh.Vertices.Point3dAt(face.A);
                Point3d second = mesh.Vertices.Point3dAt(face.B);
                Point3d third = mesh.Vertices.Point3dAt(face.C);
                patches.Add(new(targetId, Point(first), Point(second), Point(third), category, weight));
            }
        }
        if (patches.Count == 0) throw new ArgumentException("Target meshes contain no faces.");
        return patches;
    }

    internal static SpatialPoint Point(Point3d point) => new(point.X, point.Y, point.Z);
    internal static SpatialPoint Vector(Vector3d vector) => new(vector.X, vector.Y, vector.Z);
    internal static Point3d RhinoPoint(SpatialPoint point) => new(point.X, point.Y, point.Z);
}
