using System.Globalization;
using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper2.Interop;

internal static class Gh2AnalysisAdapters
{
    internal static bool Compatible(int valueCount, int sourceCount) =>
        valueCount is 0 or 1 || valueCount == sourceCount;

    internal static T Select<T>(IReadOnlyList<T> values, int index, T fallback) =>
        values.Count == 0 ? fallback : values[values.Count == 1 ? 0 : index];

    internal static ulong ParseUnsigned(string value, string name, bool allowAll = false)
    {
        if (allowAll && value.Trim() == "-1") return ulong.MaxValue;
        if (!ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong parsed))
            throw new ArgumentException($"{name} '{value}' must be {(allowAll ? "-1 or " : string.Empty)}an unsigned 64-bit integer.");
        return parsed;
    }

    internal static DaylightSensor[] Sensors(
        IReadOnlyList<Point3d> points,
        IReadOnlyList<Vector3d> normals,
        IReadOnlyList<double> areas)
    {
        if (points.Count == 0) throw new ArgumentException("At least one sensor point is required.");
        if (!Compatible(normals.Count, points.Count) || !Compatible(areas.Count, points.Count))
            throw new ArgumentException("Normals and areas must contain zero, one, or one item per sensor.");

        DaylightSensor[] sensors = new DaylightSensor[points.Count];
        for (int index = 0; index < points.Count; index++)
        {
            Vector3d normal = Select(normals, index, Vector3d.ZAxis);
            if (!normal.Unitize()) throw new ArgumentException($"Sensor normal {index} is zero or invalid.");
            double area = Select(areas, index, 1.0);
            Point3d point = points[index];
            sensors[index] = new(checked((ulong)index + 1), point.X, point.Y, point.Z,
                normal.X, normal.Y, normal.Z, area);
        }
        return sensors;
    }

    internal static IReadOnlyList<ViewTargetTriangle> TriangulateTargets(
        IReadOnlyList<Mesh> meshes,
        IReadOnlyList<string> targetIds,
        IReadOnlyList<string> categoryMasks,
        IReadOnlyList<double> weights)
    {
        if (meshes.Count == 0) throw new ArgumentException("At least one target mesh is required.");
        if (!Compatible(targetIds.Count, meshes.Count) || !Compatible(categoryMasks.Count, meshes.Count)
            || !Compatible(weights.Count, meshes.Count))
            throw new ArgumentException("Target IDs, categories, and weights must be empty, one item, or aligned with target meshes.");

        List<ViewTargetTriangle> patches = [];
        for (int meshIndex = 0; meshIndex < meshes.Count; meshIndex++)
        {
            Mesh mesh = meshes[meshIndex].DuplicateMesh();
            mesh.Faces.ConvertQuadsToTriangles();
            ulong id = targetIds.Count == 0 ? checked((ulong)meshIndex + 1)
                : ParseUnsigned(Select(targetIds, meshIndex, string.Empty), "Target ID");
            ulong category = categoryMasks.Count == 0 ? 1UL
                : ParseUnsigned(Select(categoryMasks, meshIndex, "1"), "Target category", true);
            double weight = Select(weights, meshIndex, 1.0);
            for (int faceIndex = 0; faceIndex < mesh.Faces.Count; faceIndex++)
            {
                MeshFace face = mesh.Faces[faceIndex];
                Point3d a = mesh.Vertices.Point3dAt(face.A);
                Point3d b = mesh.Vertices.Point3dAt(face.B);
                Point3d c = mesh.Vertices.Point3dAt(face.C);
                patches.Add(new(id, Point(a), Point(b), Point(c), category, weight));
            }
        }
        if (patches.Count == 0) throw new ArgumentException("Target meshes contain no faces.");
        return patches;
    }

    internal static SpatialPoint Point(Point3d point) => new(point.X, point.Y, point.Z);
    internal static SpatialPoint Vector(Vector3d vector) => new(vector.X, vector.Y, vector.Z);
}
