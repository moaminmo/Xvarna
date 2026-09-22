using Rhino.Geometry;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Interop;

internal sealed record PackedRhinoMesh(
    Mesh CanonicalMesh,
    double[] Positions,
    uint[] Triangles,
    int SourceQuadCount);

internal static class RhinoMeshAdapter
{
    internal static PackedRhinoMesh Pack(Mesh source)
    {
        ArgumentNullException.ThrowIfNull(source);
        Mesh canonical = source.DuplicateMesh();
        int sourceQuadCount = canonical.Faces.QuadCount;
        if (sourceQuadCount > 0)
        {
            canonical.Faces.ConvertQuadsToTriangles();
        }

        double[] positions = new double[checked(canonical.Vertices.Count * 3)];
        for (int index = 0; index < canonical.Vertices.Count; index++)
        {
            Point3d point = canonical.Vertices.Point3dAt(index);
            int offset = index * 3;
            positions[offset] = point.X;
            positions[offset + 1] = point.Y;
            positions[offset + 2] = point.Z;
        }

        uint[] triangles = new uint[checked(canonical.Faces.Count * 3)];
        for (int index = 0; index < canonical.Faces.Count; index++)
        {
            MeshFace face = canonical.Faces[index];
            if (!face.IsTriangle)
            {
                throw new InvalidOperationException("Canonical mesh still contains a non-triangle face.");
            }
            int offset = index * 3;
            triangles[offset] = checked((uint)face.A);
            triangles[offset + 1] = checked((uint)face.B);
            triangles[offset + 2] = checked((uint)face.C);
        }

        return new(canonical, positions, triangles, sourceQuadCount);
    }

    internal static Mesh RepairConservative(
        Mesh canonical,
        IReadOnlyList<MeshFaceIssue> faceIssues,
        out int removedFaceCount,
        out int removedVertexCount)
    {
        Mesh repaired = canonical.DuplicateMesh();
        int verticesBefore = repaired.Vertices.Count;
        int[] facesToRemove = faceIssues
            .Select((issue, index) => (issue, index))
            .Where(item => item.issue != MeshFaceIssue.None)
            .Select(item => item.index)
            .ToArray();

        removedFaceCount = facesToRemove.Length;
        if (facesToRemove.Length > 0)
        {
            repaired.Faces.DeleteFaces(facesToRemove, true);
        }
        repaired.Vertices.CullUnused();
        removedVertexCount = verticesBefore - repaired.Vertices.Count;
        repaired.Normals.ComputeNormals();
        repaired.Compact();
        return repaired;
    }
}
