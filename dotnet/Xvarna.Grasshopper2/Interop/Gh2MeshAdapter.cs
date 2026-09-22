using Rhino.Geometry;

namespace Xvarna.Grasshopper2.Interop;

internal readonly record struct PackedMesh(double[] Positions, uint[] Triangles);

internal static class Gh2MeshAdapter
{
    internal static PackedMesh Pack(Mesh source)
    {
        ArgumentNullException.ThrowIfNull(source);
        if (!source.IsValid) throw new ArgumentException("Input mesh is invalid.", nameof(source));
        double[] positions = new double[checked(source.Vertices.Count * 3)];
        for (int i = 0; i < source.Vertices.Count; i++)
        {
            Point3f p = source.Vertices[i];
            positions[i * 3] = p.X; positions[i * 3 + 1] = p.Y; positions[i * 3 + 2] = p.Z;
        }
        List<uint> triangles = new(checked(source.Faces.Count * 6));
        foreach (MeshFace face in source.Faces)
        {
            triangles.Add(checked((uint)face.A)); triangles.Add(checked((uint)face.B)); triangles.Add(checked((uint)face.C));
            if (face.IsQuad)
            {
                triangles.Add(checked((uint)face.A)); triangles.Add(checked((uint)face.C)); triangles.Add(checked((uint)face.D));
            }
        }
        return new PackedMesh(positions, triangles.ToArray());
    }
}
