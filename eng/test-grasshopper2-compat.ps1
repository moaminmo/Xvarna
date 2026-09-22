[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",
    [string]$RhinoRoot = "",
    [switch]$InProcess
)

$ErrorActionPreference = "Stop"

if (-not $InProcess) {
    & (Join-Path $PSHOME "pwsh.exe") -NoProfile -File $PSCommandPath -Configuration $Configuration -RhinoRoot $RhinoRoot -InProcess
    if ($LASTEXITCODE -ne 0) { throw "Grasshopper 2 isolated compatibility process failed with exit code $LASTEXITCODE." }
    return
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($RhinoRoot)) {
    $RhinoRoot = @("C:\Program Files\Rhino 9", "C:\Program Files\Rhino 9 WIP") |
        Where-Object { Test-Path -LiteralPath (Join-Path $_ "Plug-ins\Grasshopper2\net8.0\Grasshopper2.dll") } |
        Select-Object -First 1
}
if ([string]::IsNullOrWhiteSpace($RhinoRoot)) { throw "A Rhino 9 installation containing Grasshopper 2 was not found." }

$rhinoCommon = Join-Path $RhinoRoot "System\RhinoCommon.dll"
$eto = Join-Path $RhinoRoot "System\Eto.dll"
$grasshopperRoot = Join-Path $RhinoRoot "Plug-ins\Grasshopper2\net8.0"
$grasshopperIo = Join-Path $grasshopperRoot "GrasshopperIO.dll"
$mathNet = Join-Path $grasshopperRoot "MathNet.Numerics.dll"
if (-not (Test-Path -LiteralPath $grasshopperIo)) {
    $grasshopperIo = Join-Path $env:USERPROFILE ".nuget\packages\grasshopper2\9.0.26237.15343-beta\ref\net8.0\GrasshopperIO.dll"
}
$grasshopper = Join-Path $grasshopperRoot "Grasshopper2.dll"
$connectorRoot = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper2\bin\$Configuration\net10.0"
$managedConnector = Join-Path $connectorRoot "Xvarna.Native.dll"
$connector = Join-Path $connectorRoot "Xvarna.Grasshopper2.rhp"
foreach ($path in @($rhinoCommon, $eto, $grasshopperIo, $mathNet, $grasshopper, $managedConnector, $connector)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required GH2 compatibility-test file is missing: $path" }
}

$loadContext = [System.Runtime.Loader.AssemblyLoadContext]::Default
$loadContext.LoadFromAssemblyPath($eto) | Out-Null
$loadContext.LoadFromAssemblyPath($rhinoCommon) | Out-Null
$ioAssembly = $loadContext.LoadFromAssemblyPath($grasshopperIo)
$loadContext.LoadFromAssemblyPath($mathNet) | Out-Null
$gh2Assembly = $loadContext.LoadFromAssemblyPath($grasshopper)
$loadContext.LoadFromAssemblyPath($managedConnector) | Out-Null
$xvarnaAssembly = $loadContext.LoadFromAssemblyPath($connector)
$componentBase = $gh2Assembly.GetType("Grasshopper2.Components.Component", $true)
$pluginBase = $gh2Assembly.GetType("Grasshopper2.Framework.Plugin", $true)
$components = @($xvarnaAssembly.GetTypes() | Where-Object { $_.IsClass -and -not $_.IsAbstract -and $componentBase.IsAssignableFrom($_) })
$plugins = @($xvarnaAssembly.GetTypes() | Where-Object { $_.IsClass -and -not $_.IsAbstract -and $pluginBase.IsAssignableFrom($_) })
$expected = @(
    "AnnualDaylightComponent", "AnnualIrradianceComponent", "BackendComponent", "CacheComponent",
    "ComputeDevicesComponent", "ComputeRuntimeComponent", "DaylightValidationComponent", "DiagnosticsComponent",
    "EngineInfoComponent", "EvidenceComponent", "IntervisibilityComponent", "Isovist3dComponent",
    "IsovistComponent", "LandmarkVisibilityComponent", "LegendComponent", "MaterialsComponent",
    "MeshCheckComponent", "MultiFidelityComponent", "ObserverPathComponent", "OccluderHighlightComponent",
    "OpticalMaterialsComponent", "OptimizerStartComponent", "OptimizerStepComponent", "PeriodComponent",
    "PointDaylightComponent", "QualityComponent", "RadianceExportComponent", "RayQueryComponent",
    "ScenarioCompareComponent", "SceneComponent", "SchedulerComponent", "SensitivityDesignComponent",
    "SensorGridComponent", "ShadowMaskComponent", "SkyViewComponent", "SolarEnvelopeComponent",
    "SolarPotentialComponent", "SolarScenariosComponent", "StudyCommitComponent", "StudyManifestComponent",
    "StudyRankComponent", "StudyReportComponent", "StudyWorkspaceComponent", "SunHoursComponent",
    "SunVectorsComponent", "TargetViewComponent", "UnitsComponent", "ViewCorridorComponent",
    "VisibilityGraphComponent"
)
$actual = @($components | ForEach-Object Name)
$missing = @($expected | Where-Object { $_ -notin $actual })
if ($missing.Count -ne 0) { throw "GH2 connector is missing component(s): $($missing -join ', ')" }
if ($components.Count -ne $expected.Count) { throw "Expected $($expected.Count) GH2 components but found $($components.Count)." }
if ($plugins.Count -ne 1) { throw "Expected one Grasshopper2.Framework.Plugin registration but found $($plugins.Count)." }

$instances = @()
$ids = @($components | ForEach-Object {
    $attribute = $_.CustomAttributes | Where-Object { $_.AttributeType.FullName -eq "GrasshopperIO.IoIdAttribute" } | Select-Object -First 1
    if ($null -eq $attribute) { throw "GH2 component $($_.FullName) has no IoId." }
    $instance = [Activator]::CreateInstance($_)
    if ($null -eq $instance.Icon) { throw "GH2 component $($_.FullName) has no product icon." }
    $instances += $instance
    $attribute.ConstructorArguments[0].Value.ToString()
})
if (($ids | Sort-Object -Unique).Count -ne $ids.Count) { throw "GH2 connector contains duplicate IoId attributes." }

$grasshopper1Root = Join-Path $RhinoRoot "Plug-ins\Grasshopper"
$grasshopper1Io = Join-Path $grasshopper1Root "GH_IO.dll"
$grasshopper1 = Join-Path $grasshopper1Root "Grasshopper.dll"
$grasshopper1Connector = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\bin\$Configuration\net10.0\Xvarna.Grasshopper.gha"
foreach ($path in @($grasshopper1Io, $grasshopper1, $grasshopper1Connector)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required GH1 semantic-parity file is missing: $path" }
}
$loadContext.LoadFromAssemblyPath($grasshopper1Io) | Out-Null
$grasshopper1Assembly = $loadContext.LoadFromAssemblyPath($grasshopper1)
$grasshopper1ConnectorAssembly = $loadContext.LoadFromAssemblyPath($grasshopper1Connector)
$grasshopper1Base = $grasshopper1Assembly.GetType("Grasshopper.Kernel.GH_Component", $true)
$grasshopper1Ids = @($grasshopper1ConnectorAssembly.GetTypes() | Where-Object {
    $_.IsClass -and -not $_.IsAbstract -and $grasshopper1Base.IsAssignableFrom($_)
} | ForEach-Object { ([Activator]::CreateInstance($_)).ComponentGuid.ToString() })
$semanticDifference = @($ids | Where-Object { $_ -notin $grasshopper1Ids }) + @($grasshopper1Ids | Where-Object { $_ -notin $ids })
if ($semanticDifference.Count -ne 0 -or $grasshopper1Ids.Count -ne $ids.Count) {
    throw "GH1/GH2 semantic identity sets differ: $($semanticDifference -join ', ')"
}

$components | Sort-Object Name | ForEach-Object { Write-Host "$($_.Name) | $($_.FullName)" }
$probeType = $xvarnaAssembly.GetType("Xvarna.Grasshopper2.Interop.Gh2ContractProbe", $true)
$probeReport = $probeType.GetMethod("Run").Invoke($null, @())
$exampleReport = $probeType.GetMethod("ValidateExamples").Invoke($null, [object[]]@([string]$repositoryRoot))
Write-Host "Contract probe: $probeReport"
Write-Host "Example contracts: $exampleReport"
Write-Host "Grasshopper 2 runtime compatibility and 49/49 GH1 semantic-ID parity passed from $RhinoRoot."
