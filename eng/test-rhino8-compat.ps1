[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",
    [string]$RhinoRoot = "C:\Program Files\Rhino 8",
    [switch]$InProcess
)

if (-not $InProcess) {
    $arguments = @(
        "-NoProfile", "-File", $PSCommandPath,
        "-Configuration", $Configuration,
        "-RhinoRoot", $RhinoRoot,
        "-InProcess"
    )
    & (Join-Path $PSHOME "pwsh.exe") @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Rhino 8 isolated compatibility process failed with exit code $LASTEXITCODE."
    }
    return
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$rhinoCommon = Join-Path $RhinoRoot "System\RhinoCommon.dll"
$grasshopperRoot = Join-Path $RhinoRoot "Plug-ins\Grasshopper"
$grasshopper = Join-Path $grasshopperRoot "Grasshopper.dll"
$grasshopperIo = Join-Path $grasshopperRoot "GH_IO.dll"
$connectorRoot = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\bin\$Configuration\net8.0"
$managedConnector = Join-Path $connectorRoot "Xvarna.Native.dll"
$grasshopperConnector = Join-Path $connectorRoot "Xvarna.Grasshopper.gha"

foreach ($path in @($rhinoCommon, $grasshopperIo, $grasshopper, $managedConnector, $grasshopperConnector)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Required compatibility-test file is missing: $path"
    }
}

$loadContext = [System.Runtime.Loader.AssemblyLoadContext]::Default
$loadContext.LoadFromAssemblyPath($rhinoCommon) | Out-Null
$loadContext.LoadFromAssemblyPath($grasshopperIo) | Out-Null
$grasshopperAssembly = $loadContext.LoadFromAssemblyPath($grasshopper)
$loadContext.LoadFromAssemblyPath($managedConnector) | Out-Null
$xvarnaAssembly = $loadContext.LoadFromAssemblyPath($grasshopperConnector)
$componentBase = $grasshopperAssembly.GetType("Grasshopper.Kernel.GH_Component", $true)
$components = @(
    $xvarnaAssembly.GetTypes() |
        Where-Object { $_.IsClass -and -not $_.IsAbstract -and $componentBase.IsAssignableFrom($_) } |
        ForEach-Object { [Activator]::CreateInstance($_) }
)

$expectedNicknames = @(
    "XV Info",
    "XV Devices",
    "XV Backend",
    "XV Runtime",
    "XV Cache",
    "XV Scheduler",
    "XV Quality",
    "XV Units",
    "XV Diagnostics",
    "XV Mesh Check",
    "XV Ray Query",
    "XV Scene",
    "XV Period",
    "XV Sun Vectors",
    "XV Sun Hours",
    "XV Sky View",
    "XV Shadow Mask"
    "XV Irradiance"
    "XV Sensor Grid"
    "XV Solar Potential"
    "XV Isovist"
    "XV Intervisibility"
    "XV Visibility Graph"
    "XV Target View"
    "XV Corridor"
    "XV View Path"
    "XV Study"
    "XV Optimizer"
    "XV Opt Step"
    "XV Manifest"
    "XV Workspace"
    "XV Commit"
    "XV Report"
    "XV Evidence"
    "XV Sensitivity+"
    "XV Fidelity"
    "XV Materials"
    "XV Isovist 3D"
    "XV Landmark"
    "XV Compare"
    "XV Solar Scenarios"
    "XV Solar Envelope"
    "XV Highlight"
    "XV Optical"
    "XV Daylight"
    "XV Annual Daylight"
    "XV Radiance"
    "XV Daylight Δ"
    "XV Legend"
)
$actualNicknames = @($components | ForEach-Object NickName)
$missing = @($expectedNicknames | Where-Object { $_ -notin $actualNicknames })
if ($missing.Count -ne 0) {
    throw "Rhino 8 compatibility load is missing component(s): $($missing -join ', ')"
}
if ($components.Count -ne $expectedNicknames.Count) {
    throw "Rhino 8 compatibility load expected $($expectedNicknames.Count) components but found $($components.Count)."
}
$missingIcons = @($components | Where-Object { $null -eq $_.Icon_24x24 })
if ($missingIcons.Count -ne 0) {
    throw "Rhino 8 compatibility load found components without product icons: $($missingIcons.NickName -join ', ')"
}
$duplicateGuids = @(
    $components |
        Group-Object ComponentGuid |
        Where-Object Count -ne 1
)
if ($duplicateGuids.Count -ne 0) {
    throw "Rhino 8 compatibility load found duplicate component GUIDs."
}

$components |
    Sort-Object Category, SubCategory, NickName |
    ForEach-Object { Write-Host "$($_.NickName) | $($_.Category) > $($_.SubCategory) | $($_.ComponentGuid)" }
Write-Host "Rhino 8 runtime compatibility passed for $($components.Count) XVARNA components."
