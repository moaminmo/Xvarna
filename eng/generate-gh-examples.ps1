[CmdletBinding()]
param(
    [string]$RhinoRoot = "C:\Program Files\Rhino 8",
    [string]$GhaPath,
    [string]$OutputDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$rhino = Join-Path $RhinoRoot "System\Rhino.exe"
$python = Join-Path $PSScriptRoot "generate_gh_examples.py"
if ([string]::IsNullOrWhiteSpace($GhaPath)) {
    $GhaPath = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\bin\Release\net8.0\Xvarna.Grasshopper.gha"
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repositoryRoot "examples\grasshopper"
}
foreach ($required in @($rhino, $python, $GhaPath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Required file not found: $required" }
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$errorFile = Join-Path $repositoryRoot "artifacts\gh-example-error.txt"
if (Test-Path -LiteralPath $errorFile) { Remove-Item -LiteralPath $errorFile -Force }
$env:XVARNA_EXAMPLE_GHA = [System.IO.Path]::GetFullPath($GhaPath)
$env:XVARNA_EXAMPLE_OUTPUT = [System.IO.Path]::GetFullPath($OutputDirectory)
$env:XVARNA_EXAMPLE_SCRIPT = [System.IO.Path]::GetFullPath($python)
$macro = '_NoEcho _-ScriptEditor _Run %XVARNA_EXAMPLE_SCRIPT%'
$runScript = '/runscript="{0}"' -f $macro
$process = Start-Process -FilePath $rhino -ArgumentList @("/nosplash", "/notemplate", $runScript) -WindowStyle Hidden -PassThru
if (-not $process.WaitForExit(120000)) {
    $process.Kill($true)
    throw "Rhino did not complete Grasshopper example generation within 120 seconds."
}
$process.Refresh()
if (Test-Path -LiteralPath $errorFile) {
    throw "Rhino example generation failed:`n$(Get-Content -LiteralPath $errorFile -Raw)"
}
$examples = @(Get-ChildItem -LiteralPath $OutputDirectory -Filter "*.ghx" -File)
if ($process.ExitCode -ne 0 -or $examples.Count -ne 12) {
    throw "Expected twelve Grasshopper examples; Rhino exit=$($process.ExitCode), files=$($examples.Count)."
}
$componentSource = Get-ChildItem -LiteralPath (Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\Components") -Filter "*.cs" -File -Recurse | Get-Content -Raw
$expectedIds = @([regex]::Matches(($componentSource -join "`n"), 'ComponentGuid\s*=>\s*new\("(?<id>[0-9a-f-]{36})"\)') | ForEach-Object { $_.Groups['id'].Value.ToLowerInvariant() } | Sort-Object -Unique)
$serializedIds = @($examples | ForEach-Object { [regex]::Matches((Get-Content -LiteralPath $_.FullName -Raw), '<item name="GUID"[^>]*>(?<id>[0-9a-f-]{36})</item>') | ForEach-Object { $_.Groups['id'].Value.ToLowerInvariant() } } | Sort-Object -Unique)
if ($expectedIds.Count -ne 49) { throw "Expected 49 source component IDs; found $($expectedIds.Count)." }
$missing = @($expectedIds | Where-Object { $_ -notin $serializedIds })
if ($missing.Count -ne 0) { throw "Tutorial serialization coverage is not 49/49. Missing: $($missing -join ', ')" }
$examples | Sort-Object Name | ForEach-Object { Write-Host "Generated $($_.FullName) ($($_.Length) bytes)" }
Write-Host "Verified serialized coverage for all 49 XVARNA component IDs across 12 Rhino-generated canvases."
