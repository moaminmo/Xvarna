using System.Drawing;
using Grasshopper.Kernel;
using Rhino;
using Rhino.Geometry;
using Xvarna.Grasshopper.Interop;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Scene;

/// <summary>Audits mesh geometry and optionally performs conservative repair.</summary>
public sealed class MeshCheckComponent : GH_Component
{
    /// <summary>Initializes the XVARNA mesh-check component.</summary>
    public MeshCheckComponent()
        : base(
            "XVARNA Mesh Check",
            "XV Mesh Check",
            "Audits mesh geometry in the native XVARNA engine and optionally removes invalid, non-finite, degenerate, and duplicate faces without mutating the source mesh.",
            "XVARNA",
            "01 Scene")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("3a43edab-dc24-4e89-86a4-0c1c929935a7");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager inputManager)
    {
        inputManager.AddMeshParameter("Mesh", "M", "Mesh to audit. Source geometry is never mutated.", GH_ParamAccess.item);
        inputManager.AddNumberParameter("Tolerance", "T", "Positive model-space tolerance. Set to 0 to use the active Rhino document tolerance.", GH_ParamAccess.item, 0.0);
        inputManager.AddBooleanParameter("Repair", "R", "Remove invalid, non-finite, degenerate, and duplicate faces and cull unused vertices. No welding or hole filling is performed.", GH_ParamAccess.item, false);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager outputManager)
    {
        outputManager.AddMeshParameter("Checked Mesh", "M", "Triangulated checked mesh, or a conservative repaired copy when Repair is true.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Analysis Ready", "OK", "True when the mesh has usable faces and no face requires conservative removal.", GH_ParamAccess.item);
        outputManager.AddBooleanParameter("Watertight", "W", "True when topology is clean, closed, manifold, and consistently wound.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Report", "R", "Deterministic mesh-health report with topology, metric, precision, and hash information.", GH_ParamAccess.item);
        outputManager.AddIntegerParameter("Bad Faces", "BF", "Original canonical triangle indices requiring removal.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Bad Vertices", "BV", "Original vertex indices that are non-finite or isolated.", GH_ParamAccess.list);
        outputManager.AddIntegerParameter("Boundary Edges", "BE", "Number of edges referenced by exactly one accepted face.", GH_ParamAccess.item);
        outputManager.AddIntegerParameter("Non-Manifold Edges", "NM", "Number of edges referenced by more than two accepted faces.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Area", "A", "Accepted surface area in squared model units.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Volume", "V", "Absolute enclosed volume when watertight; NaN otherwise.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Hash", "H", "BLAKE3 content hash of the output mesh.", GH_ParamAccess.item);
        outputManager.AddTextParameter("Repair Report", "RR", "Explicit before/after repair provenance.", GH_ParamAccess.item);
        outputManager.AddNumberParameter("Precision Error", "PE", "Estimated absolute f32 rounding error in model units.", GH_ParamAccess.item);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        Mesh? source = null;
        double tolerance = 0.0;
        bool repairRequested = false;
        if (!dataAccess.GetData(0, ref source) || source is null)
        {
            return;
        }
        dataAccess.GetData(1, ref tolerance);
        dataAccess.GetData(2, ref repairRequested);

        tolerance = ResolveTolerance(tolerance);
        try
        {
            PackedRhinoMesh packed = RhinoMeshAdapter.Pack(source);
            MeshAuditResult before = MeshAuditor.Audit(packed.Positions, packed.Triangles, tolerance);
            int[] badFaces = FindFaceIssues(before.FaceIssues);
            int[] badVertices = FindVertexIssues(before.VertexIssues);
            Mesh outputMesh = packed.CanonicalMesh;
            MeshAuditResult final = before;
            string repairReport = "Repair disabled; source mesh was not mutated.";
            string report = before.ToReport(CurrentUnitName());

            if (repairRequested)
            {
                outputMesh = RhinoMeshAdapter.RepairConservative(
                    packed.CanonicalMesh,
                    before.FaceIssues,
                    out int removedFaces,
                    out int removedVertices);
                PackedRhinoMesh repairedPacked = RhinoMeshAdapter.Pack(outputMesh);
                final = MeshAuditor.Audit(repairedPacked.Positions, repairedPacked.Triangles, tolerance);
                repairReport = $"Conservative repair: removed {removedFaces:N0} face(s), removed {removedVertices:N0} unused vertex/vertices, welded 0, filled 0 holes. Source mesh unchanged. Hash {before.ContentHash} → {final.ContentHash}.";
                report = $"BEFORE REPAIR{Environment.NewLine}{before.ToReport(CurrentUnitName())}{Environment.NewLine}{Environment.NewLine}AFTER REPAIR{Environment.NewLine}{final.ToReport(CurrentUnitName())}";
            }

            if (packed.SourceQuadCount > 0)
            {
                repairReport += $" Canonical audit triangulated {packed.SourceQuadCount:N0} quad(s) deterministically.";
            }

            EmitMessages(final, repairRequested, badFaces.Length);
            dataAccess.SetData(0, outputMesh);
            dataAccess.SetData(1, final.IsAnalysisReady);
            dataAccess.SetData(2, final.IsWatertight);
            dataAccess.SetData(3, report);
            dataAccess.SetDataList(4, badFaces);
            dataAccess.SetDataList(5, badVertices);
            dataAccess.SetData(6, checked((int)final.BoundaryEdgeCount));
            dataAccess.SetData(7, checked((int)final.NonManifoldEdgeCount));
            dataAccess.SetData(8, final.SurfaceArea);
            dataAccess.SetData(9, final.IsWatertight ? Math.Abs(final.SignedVolume) : double.NaN);
            dataAccess.SetData(10, final.ContentHash);
            dataAccess.SetData(11, repairReport);
            dataAccess.SetData(12, final.EstimatedF32Error);
        }
        catch (Exception exception) when (exception is XvarnaNativeException
            or DllNotFoundException
            or BadImageFormatException
            or OverflowException
            or ArgumentException
            or InvalidOperationException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }

    private static double ResolveTolerance(double requested)
    {
        if (requested > 0.0 && double.IsFinite(requested))
        {
            return requested;
        }
        return RhinoDoc.ActiveDoc?.ModelAbsoluteTolerance ?? 1.0e-6;
    }

    private static string CurrentUnitName() =>
        RhinoDoc.ActiveDoc?.ModelUnitSystem.ToString() ?? "model unit";

    private static int[] FindFaceIssues(IReadOnlyList<MeshFaceIssue> issues) =>
        issues
            .Select((issue, index) => (issue, index))
            .Where(item => item.issue != MeshFaceIssue.None)
            .Select(item => item.index)
            .ToArray();

    private static int[] FindVertexIssues(IReadOnlyList<MeshVertexIssue> issues) =>
        issues
            .Select((issue, index) => (issue, index))
            .Where(item => item.issue != MeshVertexIssue.None)
            .Select(item => item.index)
            .ToArray();

    private void EmitMessages(MeshAuditResult result, bool repaired, int originalBadFaceCount)
    {
        if (!result.IsAnalysisReady)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, "Mesh contains faces that must be removed before XVARNA analysis. Enable Repair or inspect Bad Faces.");
        }
        if (result.NonManifoldEdgeCount > 0 || result.InconsistentWindingEdgeCount > 0)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, "Topology is non-manifold or inconsistently wound. Ray analysis may proceed, but enclosed volume is not valid.");
        }
        if (result.BoundaryEdgeCount > 0)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, $"Open mesh: {result.BoundaryEdgeCount:N0} boundary edge(s). This can be valid for occlusion but not enclosed volume.");
        }
        if (result.ExceedsF32PrecisionBudget)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, "Coordinates exceed the declared f32 precision budget. A future GPU path must rebase, chunk, or use the CPU f64 fallback.");
        }
        if (repaired && originalBadFaceCount > 0 && result.IsAnalysisReady)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Remark, $"Conservative repair removed {originalBadFaceCount:N0} problematic face(s) from the output copy.");
        }
    }
}
