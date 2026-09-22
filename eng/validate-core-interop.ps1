[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Cli
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$outputRoot = Join-Path $repositoryRoot "artifacts\validation\core-interop"
$sourceMesh = Join-Path $repositoryRoot "validation\meshes\open_quad.obj"
$sourceWeather = Join-Path $repositoryRoot "validation\weather\tehran-mini.epw"
$schema = Join-Path $repositoryRoot "schemas\xvarna-scene-1.schema.json"

if (-not (Test-Path -LiteralPath $Cli)) {
    throw "XVARNA CLI was not found: $Cli"
}
foreach ($path in @($sourceMesh, $sourceWeather, $schema)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Core interoperability fixture is missing: $path"
    }
}

New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
$glb = Join-Path $outputRoot "open-quad.glb"
$ply = Join-Path $outputRoot "open-quad.ply"
$scene = Join-Path $outputRoot "open-quad.xvscene"
$unpacked = Join-Path $outputRoot "open-quad-unpacked.obj"
$wea = Join-Path $outputRoot "tehran-mini.wea"
$weatherCsv = Join-Path $outputRoot "tehran-mini.csv"
$sceneRepeat = Join-Path $outputRoot "open-quad-repeat.xvscene"

$meshJson = & $Cli mesh-convert $sourceMesh $glb --repair-winding --json
if ($LASTEXITCODE -ne 0) { throw "OBJ to GLB conversion failed." }
$meshReport = $meshJson | ConvertFrom-Json
if ($meshReport.outputTriangles -ne 2) { throw "OBJ to GLB changed the fixture triangle count." }

Invoke-XvarnaCommand $Cli mesh-convert $glb $ply --json
Invoke-XvarnaCommand $Cli scene-pack $ply $scene --repair-winding --json
Invoke-XvarnaCommand $Cli scene-pack $ply $sceneRepeat --repair-winding --json

$firstSceneHash = (Get-FileHash -LiteralPath $scene -Algorithm SHA256).Hash
$secondSceneHash = (Get-FileHash -LiteralPath $sceneRepeat -Algorithm SHA256).Hash
if ($firstSceneHash -ne $secondSceneHash) {
    throw "Canonical XVSCN serialization is not deterministic."
}

$sceneJson = & $Cli scene-unpack $scene $unpacked --json
if ($LASTEXITCODE -ne 0) { throw "XVSCN verification/unpack failed." }
$sceneReport = $sceneJson | ConvertFrom-Json
if ($sceneReport.expandedTriangles -ne 2 -or $sceneReport.instances -ne 1) {
    throw "XVSCN round trip changed scene topology or instancing."
}

$weaJson = & $Cli weather-convert $sourceWeather $wea --missing zero --preserve-gaps --json
if ($LASTEXITCODE -ne 0) { throw "EPW to WEA conversion failed." }
$weaReport = $weaJson | ConvertFrom-Json
if ($weaReport.records -le 0) { throw "Weather conversion produced no records." }

$csvJson = & $Cli weather-convert $wea $weatherCsv --missing reject --preserve-gaps --json
if ($LASTEXITCODE -ne 0) { throw "WEA to lossless CSV conversion failed." }
$csvReport = $csvJson | ConvertFrom-Json
if ($csvReport.records -ne $weaReport.records) {
    throw "Weather round trip changed the interval count."
}

Get-Content -LiteralPath $schema -Raw | ConvertFrom-Json | Out-Null
Write-Host "Core/time/interoperability validation passed: deterministic scene, mesh, and weather round trips."
