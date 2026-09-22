using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

/// <summary>Builds a deterministic area-aware sensor grid directly from Rhino geometry.</summary>
public sealed class SensorGridComponent : ScheduledAnalysisComponent<SensorGridWork, SurfaceGridResult>
{
    /// <summary>Initializes the surface sensor-grid component.</summary>
    public SensorGridComponent()
        : base(
            "XVARNA Sensor Grid",
            "XV Sensor Grid",
            "Converts Mesh, Brep, Surface, and Extrusion geometry into a deterministic area-preserving analysis mesh with stable oriented sensors.",
            "XVARNA",
            "02 Sensors")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("ee467ab9-54ed-4b71-a35f-a747689736e9");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override string AnalysisName => "MEHR Sensor Grid";

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddGeometryParameter("Geometry", "G", "Mesh, Brep, Surface, or Extrusion geometry to sample without mutating the source.", GH_ParamAccess.list);
        inputManager.AddNumberParameter("Cell Size", "S", "Maximum analysis-cell edge length in active Rhino model units.", GH_ParamAccess.item, 1.0);
        inputManager.AddNumberParameter("Offset", "Eps", "Normal offset of each sensor in model units; 0 uses document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddTextParameter("First Sensor ID", "ID", "Non-zero first unsigned 64-bit sensor identifier.", GH_ParamAccess.item, "1");
        inputManager.AddIntegerParameter("Maximum Cells", "Max", "Hard resource ceiling preventing accidental over-resolution.", GH_ParamAccess.item, 1_000_000);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddGenericParameter("Sensor Grid", "Grid", "Immutable area-aware grid for XV Solar Potential and downstream automation.", GH_ParamAccess.item);
        outputManager.AddMeshParameter("Analysis Mesh", "M", "One triangle per analysis sensor, preserving exact sampled area.", GH_ParamAccess.item);
        outputManager.AddPointParameter("Sensors", "P", "Offset oriented sensor points for XV Irradiance, XV Sun Hours, and XV Sky View.", GH_ParamAccess.list);
        outputManager.AddVectorParameter("Normals", "N", "One unit surface normal per sensor.", GH_ParamAccess.list);
        outputManager.AddNumberParameter("Areas", "A", "One exact cell area in squared model units per sensor.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Sensor IDs", "ID", "Stable unsigned 64-bit IDs aligned with all outputs.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Source Faces", "F", "Canonical source-triangle index for every generated cell.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Depth", "D", "Longest-edge subdivision depth for every generated cell.", GH_ParamAccess.list);
        outputManager.AddTextParameter("Hash", "H", "Deterministic BLAKE3 geometry/options/grid identity.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Resolution, area conservation, skipped geometry, resource policy, and identity report.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override bool TryReadWork(IGH_DataAccess dataAccess, out SensorGridWork work)
    {
        work = default;
        List<GeometryBase> geometry = [];
        double cellSize = 1.0;
        double offset = 0.0;
        string firstIdText = "1";
        int maximumCells = 1_000_000;
        if (!dataAccess.GetDataList(0, geometry) || geometry.Count == 0
            || !dataAccess.GetData(1, ref cellSize))
        {
            return false;
        }
        dataAccess.GetData(2, ref offset);
        dataAccess.GetData(3, ref firstIdText);
        dataAccess.GetData(4, ref maximumCells);

        try
        {
            if (!double.IsFinite(cellSize) || cellSize <= 0.0)
            {
                throw new ArgumentOutOfRangeException(nameof(cellSize), "Cell Size must be finite and positive.");
            }
            if (!ulong.TryParse(firstIdText, NumberStyles.None, CultureInfo.InvariantCulture, out ulong firstId) || firstId == 0)
            {
                throw new ArgumentException("First Sensor ID must be a non-zero unsigned 64-bit integer.");
            }
            if (maximumCells <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(maximumCells), "Maximum Cells must be positive.");
            }
            RhinoDoc? document = RhinoDoc.ActiveDoc;
            double tolerance = document?.ModelAbsoluteTolerance ?? 1.0e-6;
            double resolvedOffset = offset > 0.0 && double.IsFinite(offset) ? offset : tolerance;
            Mesh combined = BuildSourceMesh(geometry, cellSize, tolerance);
            PackedRhinoMesh packed = RhinoMeshAdapter.Pack(combined);
            work = new(
                packed.Positions,
                packed.Triangles,
                new(cellSize, resolvedOffset, firstId, checked((ulong)maximumCells)),
                geometry.Count,
                cellSize,
                resolvedOffset,
                firstId,
                maximumCells);
            return true;
        }
        catch (Exception exception) when (exception is ArgumentException
            or OverflowException
            or InvalidOperationException
            or XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
            return false;
        }
    }

    /// <inheritdoc />
    protected override ulong EstimateWorkUnits(SensorGridWork work) => checked((ulong)(work.Triangles.Length / 3));

    /// <inheritdoc />
    protected override SurfaceGridResult Execute(
        SensorGridWork work,
        CancellationToken cancellationToken,
        IProgress<(ulong Completed, ulong Total, string Phase)> progress)
    {
        ulong total = EstimateWorkUnits(work);
        progress.Report((0, total, "Longest-edge subdivision"));
        cancellationToken.ThrowIfCancellationRequested();
        SurfaceGridResult result = SurfaceGridGenerator.Generate(work.Positions, work.Triangles, work.Options);
        progress.Report((total, total, "Area and identity validation"));
        return result;
    }

    /// <inheritdoc />
    protected override void Emit(IGH_DataAccess dataAccess, SensorGridWork work, SurfaceGridResult result)
    {
        Mesh analysisMesh = CreateAnalysisMesh(result.Cells, null);
        double relativeAreaError = result.SourceArea > 0.0
            ? Math.Abs(result.SampledArea - result.SourceArea) / result.SourceArea
            : 0.0;
        string report = string.Create(
            CultureInfo.InvariantCulture,
            $"Engine: native deterministic longest-edge surface grid{Environment.NewLine}" +
            $"Input: {work.GeometryCount:N0} item(s); {result.SourceFaceCount:N0} canonical source triangles; {result.SkippedFaceCount:N0} skipped{Environment.NewLine}" +
            $"Grid: {result.Cells.Count:N0} cells; target edge {work.CellSize:G10}; achieved max edge {result.MaximumCellEdgeLength:G10}; max depth {result.MaximumSubdivisionDepth:N0}{Environment.NewLine}" +
            $"Area: source {result.SourceArea:G12}; sampled {result.SampledArea:G12}; relative conservation error {relativeAreaError:E3}{Environment.NewLine}" +
            $"Policy: sensor offset {work.Offset:G10}; first ID {work.FirstId}; hard ceiling {work.MaximumCells:N0}{Environment.NewLine}" +
            $"Identity: {result.ContentHash}");
        if (result.SkippedFaceCount > 0)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, $"Skipped {result.SkippedFaceCount:N0} invalid or degenerate source face(s). Inspect source geometry with XV Mesh Check.");
        }
        dataAccess.SetData(0, result);
        dataAccess.SetData(1, analysisMesh);
        dataAccess.SetDataList(2, result.Cells.Select(cell => new Point3d(cell.PositionX, cell.PositionY, cell.PositionZ)));
        dataAccess.SetDataList(3, result.Cells.Select(cell => new Vector3d(cell.NormalX, cell.NormalY, cell.NormalZ)));
        dataAccess.SetDataList(4, result.Cells.Select(cell => cell.Area));
        dataAccess.SetDataList(5, result.Cells.Select(cell => cell.SensorId.ToString(CultureInfo.InvariantCulture)));
        dataAccess.SetDataList(6, result.Cells.Select(cell => checked((int)cell.SourceFaceIndex)));
        dataAccess.SetDataList(7, result.Cells.Select(cell => checked((int)cell.SubdivisionDepth)));
        dataAccess.SetData(8, result.ContentHash);
        dataAccess.SetData(9, report);
        Message = string.Create(CultureInfo.InvariantCulture, $"GRID\n{result.Cells.Count:N0} cells");
    }

    internal static Mesh CreateAnalysisMesh(IReadOnlyList<SurfaceCell> cells, IReadOnlyList<Color>? colors)
    {
        Mesh mesh = new();
        for (int index = 0; index < cells.Count; index++)
        {
            SurfaceCell cell = cells[index];
            int vertex = mesh.Vertices.Count;
            mesh.Vertices.Add(cell.AX, cell.AY, cell.AZ);
            mesh.Vertices.Add(cell.BX, cell.BY, cell.BZ);
            mesh.Vertices.Add(cell.CX, cell.CY, cell.CZ);
            mesh.Faces.AddFace(vertex, vertex + 1, vertex + 2);
            if (colors is not null)
            {
                Color color = colors[index];
                mesh.VertexColors.Add(color);
                mesh.VertexColors.Add(color);
                mesh.VertexColors.Add(color);
            }
        }
        mesh.Normals.ComputeNormals();
        mesh.Compact();
        return mesh;
    }

    private static Mesh BuildSourceMesh(IReadOnlyList<GeometryBase> geometry, double cellSize, double tolerance)
    {
        Mesh combined = new();
        MeshingParameters parameters = new()
        {
            MaximumEdgeLength = cellSize,
            MinimumEdgeLength = Math.Min(cellSize * 0.1, Math.Max(tolerance, cellSize * 0.01)),
            RefineGrid = true,
            JaggedSeams = false,
            SimplePlanes = true,
        };
        foreach (GeometryBase item in geometry)
        {
            IEnumerable<Mesh> meshes = item switch
            {
                Mesh mesh => [mesh.DuplicateMesh()],
                Brep brep => Mesh.CreateFromBrep(brep, parameters) ?? [],
                Extrusion extrusion => Mesh.CreateFromBrep(extrusion.ToBrep(), parameters) ?? [],
                Surface surface => Mesh.CreateFromBrep(surface.ToBrep(), parameters) ?? [],
                _ => throw new ArgumentException($"Geometry type {item.GetType().Name} is not supported. Use Mesh, Brep, Surface, or Extrusion."),
            };
            foreach (Mesh mesh in meshes)
            {
                combined.Append(mesh);
            }
        }
        if (combined.Faces.Count == 0)
        {
            throw new ArgumentException("Input geometry produced no mesh faces.");
        }
        combined.Vertices.CombineIdentical(true, true);
        combined.Vertices.CullUnused();
        combined.Compact();
        return combined;
    }
}

/// <summary>Immutable packed input for scheduled native surface subdivision.</summary>
public readonly record struct SensorGridWork(
    double[] Positions,
    uint[] Triangles,
    SurfaceGridOptions Options,
    int GeometryCount,
    double CellSize,
    double Offset,
    ulong FirstId,
    int MaximumCells);
