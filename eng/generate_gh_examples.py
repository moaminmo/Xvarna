from __future__ import print_function

import os
import traceback
from System import Activator
from System.Drawing import PointF, RectangleF
from System.Reflection import Assembly

import Grasshopper
import Rhino

repo = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
gha_path = os.environ.get("XVARNA_EXAMPLE_GHA", os.path.join(repo, "dotnet", "Xvarna.Grasshopper", "bin", "Release", "net8.0", "Xvarna.Grasshopper.gha"))
output_dir = os.environ.get("XVARNA_EXAMPLE_OUTPUT", os.path.join(repo, "examples", "grasshopper"))
generated_types = set()

def add_panel(document, text, x, y, width=560.0, height=145.0):
    panel = Grasshopper.Kernel.Special.GH_Panel()
    panel.CreateAttributes()
    panel.UserText = text
    panel.Attributes.Pivot = PointF(float(x), float(y))
    panel.Attributes.Bounds = RectangleF(float(x), float(y), float(width), float(height))
    if not document.AddObject(panel, False, 0):
        raise RuntimeError("Could not add instruction panel")

def add_component(document, assembly, type_name, x, y):
    component_type = assembly.GetType(type_name, True, False)
    component = Activator.CreateInstance(component_type)
    component.CreateAttributes()
    component.Attributes.Pivot = PointF(float(x), float(y))
    if not document.AddObject(component, False, 0):
        raise RuntimeError("Could not add " + type_name)

def save_example(assembly, file_name, title, instructions, type_names):
    document = Grasshopper.Kernel.GH_Document()
    add_panel(document, title + "\r\n\r\n" + instructions, 40, 40)
    for index, type_name in enumerate(type_names):
        add_component(document, assembly, type_name, 60 + 220 * (index % 4), 230 + 180 * (index // 4))
    path = os.path.join(output_dir, file_name)
    io = Grasshopper.Kernel.GH_DocumentIO(document)
    if not io.SaveQuiet(path):
        raise RuntimeError("Grasshopper could not save " + path)
    document.Dispose()
    generated_types.update(type_names)
    print("Generated " + path)

def expected_components(assembly):
    expected = set()
    for component_type in assembly.GetExportedTypes():
        if component_type.Namespace and component_type.Namespace.startswith("Xvarna.Grasshopper.Components") and not component_type.IsAbstract and component_type.Name.endswith("Component"):
            expected.add(component_type.FullName)
    if len(expected) != 49:
        raise RuntimeError("Expected 49 exported GH1 components, found " + str(len(expected)))
    return expected

def main():
    if not os.path.isfile(gha_path):
        raise RuntimeError("XVARNA assembly not found: " + gha_path)
    if not os.path.isdir(output_dir):
        os.makedirs(output_dir)
    assembly = Assembly.LoadFrom(gha_path)
    for name in os.listdir(output_dir):
        if name.lower().endswith(".ghx"):
            os.remove(os.path.join(output_dir, name))
    save_example(assembly, "01-xvarna-preflight.ghx", "XVARNA 1.0 RC - Preflight", "Start here. XV Info must report Ready=True. Set project-wide quality and units, inspect compute devices, then run Diagnostics before a publication study.", [
        "Xvarna.Grasshopper.Components.Setup.EngineInfoComponent", "Xvarna.Grasshopper.Components.Setup.QualityComponent", "Xvarna.Grasshopper.Components.Setup.UnitsComponent", "Xvarna.Grasshopper.Components.Setup.ComputeDevicesComponent", "Xvarna.Grasshopper.Components.Setup.DiagnosticsComponent", "Xvarna.Grasshopper.Components.Setup.CacheComponent", "Xvarna.Grasshopper.Components.Setup.SchedulerComponent"])
    save_example(assembly, "02-vayu-scene-visibility.ghx", "VAYU - Scene and visibility", "Reference Rhino geometry into XV Mesh Check, compile it with XV Scene, then pass the immutable scene to XV Backend. Connect the session to the visibility analyses; use Diagnostics and Runtime to capture evidence.", [
        "Xvarna.Grasshopper.Components.Scene.MeshCheckComponent", "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Scene.RayQueryComponent", "Xvarna.Grasshopper.Components.Setup.BackendComponent", "Xvarna.Grasshopper.Components.Setup.ComputeRuntimeComponent", "Xvarna.Grasshopper.Components.Visibility.IsovistComponent", "Xvarna.Grasshopper.Components.Visibility.Isovist3dComponent", "Xvarna.Grasshopper.Components.Visibility.LandmarkVisibilityComponent", "Xvarna.Grasshopper.Components.Visibility.VisibilityGraphComponent", "Xvarna.Grasshopper.Components.Setup.LegendComponent"])
    save_example(assembly, "03-hvare-daylight-solar.ghx", "HVARE - Daylight and solar", "Build one scene and one optical-material catalog. Point daylight is interactive; Annual Daylight provides sDA/ASE/UDI; Radiance Export creates the reference bundle. Solar scenarios and the freeform envelope support early massing.", [
        "Xvarna.Grasshopper.Components.Visibility.MaterialLibraryComponent", "Xvarna.Grasshopper.Components.Daylight.OpticalMaterialsComponent", "Xvarna.Grasshopper.Components.Daylight.PointDaylightComponent", "Xvarna.Grasshopper.Components.Daylight.AnnualDaylightComponent", "Xvarna.Grasshopper.Components.Daylight.RadianceExportComponent", "Xvarna.Grasshopper.Components.Daylight.DaylightValidationComponent", "Xvarna.Grasshopper.Components.Solar.AnnualIrradianceComponent", "Xvarna.Grasshopper.Components.Solar.SkyViewComponent", "Xvarna.Grasshopper.Components.Solar.SolarScenariosComponent", "Xvarna.Grasshopper.Components.Solar.SolarEnvelopeComponent"])
    save_example(assembly, "04-vahman-study-report.ghx", "VAHMAN - Study, optimization, report", "Create a versioned manifest, initialize or resume the workspace, evaluate the returned candidate branches, commit aligned objectives, then generate the portable HTML/Parquet/glTF report.", [
        "Xvarna.Grasshopper.Components.Study.StudyManifestComponent", "Xvarna.Grasshopper.Components.Study.StudyWorkspaceComponent", "Xvarna.Grasshopper.Components.Study.StudyCommitComponent", "Xvarna.Grasshopper.Components.Study.StudyReportComponent", "Xvarna.Grasshopper.Components.Study.OptimizerStartComponent", "Xvarna.Grasshopper.Components.Study.OptimizerStepComponent"])
    save_example(assembly, "05-rashnu-evidence-multifidelity.ghx", "RASHNU - Evidence and multi-fidelity", "Seal every important result with XV Evidence. Use XV Sensitivity+ to generate exact Saltelli/Jansen or Morris evaluation rows, then return aligned outputs. Use XV Fidelity to select the most informative costly Radiance/reference jobs per unit cost.", [
        "Xvarna.Grasshopper.Components.Study.EvidencePassportComponent", "Xvarna.Grasshopper.Components.Study.GlobalSensitivityComponent", "Xvarna.Grasshopper.Components.Study.MultiFidelityComponent"])
    save_example(assembly, "06-urban-sun-hours.ghx", "Tutorial 06 - Urban sun and blockers", "Mesh check, compile the urban context, generate a period and sun vectors, then connect sensor points/normals to Sun Hours. Inspect exact first-blocker IDs and map them back with Highlight.", [
        "Xvarna.Grasshopper.Components.Scene.MeshCheckComponent", "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Time.PeriodComponent", "Xvarna.Grasshopper.Components.Time.SunVectorsComponent", "Xvarna.Grasshopper.Components.Solar.SunHoursComponent", "Xvarna.Grasshopper.Components.Visibility.OccluderHighlightComponent"])
    save_example(assembly, "07-parametric-shade-delta.ghx", "Tutorial 07 - Parametric shade delta", "Keep the context static and feed changing shade geometry to the dynamic scene layer. Watch revision/refit/rebuild telemetry, then compare shadow masks and solar potential after each controlled edit.", [
        "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Setup.BackendComponent", "Xvarna.Grasshopper.Components.Solar.ShadowMaskComponent", "Xvarna.Grasshopper.Components.Solar.SolarPotentialComponent", "Xvarna.Grasshopper.Components.Setup.ComputeRuntimeComponent"])
    save_example(assembly, "08-target-weighted-green-view.ghx", "Tutorial 08 - Target, weighted and green view", "Connect eye points, forward vectors and target meshes directly. Keep target view, weighted view and green view as separate metrics; use category masks and Highlight for explainable obstruction evidence.", [
        "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Visibility.TargetViewComponent", "Xvarna.Grasshopper.Components.Visibility.ScenarioCompareComponent", "Xvarna.Grasshopper.Components.Visibility.OccluderHighlightComponent", "Xvarna.Grasshopper.Components.Setup.LegendComponent"])
    save_example(assembly, "09-facade-privacy.ghx", "Tutorial 09 - Facade privacy", "Connect observer and facade target points to the directed Intervisibility matrix. Optional target facings and sensitivities produce transparent privacy risk; inspect states, exposure and blocker IDs.", [
        "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Visibility.IntervisibilityComponent", "Xvarna.Grasshopper.Components.Visibility.VisibilityGraphComponent", "Xvarna.Grasshopper.Components.Visibility.OccluderHighlightComponent"])
    save_example(assembly, "10-annual-daylight-reference.ghx", "Tutorial 10 - Annual daylight and Radiance", "Create workplane sensors, assign optical materials, run the Fast Path sDA/ASE/UDI study, export the identical geometry/material/sensor bundle to Radiance, then compare aligned lux values with Daylight Validate.", [
        "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Solar.SensorGridComponent", "Xvarna.Grasshopper.Components.Daylight.OpticalMaterialsComponent", "Xvarna.Grasshopper.Components.Daylight.AnnualDaylightComponent", "Xvarna.Grasshopper.Components.Daylight.RadianceExportComponent", "Xvarna.Grasshopper.Components.Daylight.DaylightValidationComponent"])
    save_example(assembly, "11-dynamic-observer-corridor.ghx", "Tutorial 11 - Dynamic path and corridor", "Define a moving observer path and protected landmark corridor. Compare time-aligned view quality and conflict attribution; use 3D Isovist for local spatial context.", [
        "Xvarna.Grasshopper.Components.Scene.SceneBuildComponent", "Xvarna.Grasshopper.Components.Visibility.ObserverPathComponent", "Xvarna.Grasshopper.Components.Visibility.ViewCorridorComponent", "Xvarna.Grasshopper.Components.Visibility.Isovist3dComponent", "Xvarna.Grasshopper.Components.Visibility.LandmarkVisibilityComponent"])
    save_example(assembly, "12-end-to-end-decision-report.ghx", "Tutorial 12 - Reproducible decision report", "Build a manifest, evaluate candidate branches with solar/daylight/view objectives, commit complete batches, rank feasible Pareto options, seal evidence, and export the self-contained report and viewer.", [
        "Xvarna.Grasshopper.Components.Study.StudyManifestComponent", "Xvarna.Grasshopper.Components.Study.StudyWorkspaceComponent", "Xvarna.Grasshopper.Components.Study.StudyCommitComponent", "Xvarna.Grasshopper.Components.Study.StudyRankComponent", "Xvarna.Grasshopper.Components.Study.EvidencePassportComponent", "Xvarna.Grasshopper.Components.Study.StudyReportComponent"])
    expected = expected_components(assembly)
    if generated_types != expected:
        raise RuntimeError("Tutorial canvas coverage is not 49/49; missing=" + str(expected.difference(generated_types)) + ", unexpected=" + str(generated_types.difference(expected)))
    print("Generated canvas coverage for all 49 XVARNA components across 12 canvases")

try:
    main()
except Exception:
    error_path = os.path.join(repo, "artifacts", "gh-example-error.txt")
    if not os.path.isdir(os.path.dirname(error_path)):
        os.makedirs(os.path.dirname(error_path))
    with open(error_path, "w") as error_file:
        error_file.write(traceback.format_exc())
finally:
    Rhino.RhinoApp.Exit()
