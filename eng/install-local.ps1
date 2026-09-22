[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet(8, 9)]
    [int]$RhinoVersion,
    [ValidateSet("Install", "Uninstall", "List")]
    [string]$Action = "Install",
    [string]$Package
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$packageRoot = Join-Path $repositoryRoot "artifacts\packages"
$yak = if ($RhinoVersion -eq 8) {
    "C:\Program Files\Rhino 8\System\Yak.exe"
} else {
    $release = "C:\Program Files\Rhino 9\System\Yak.exe"
    $wip = "C:\Program Files\Rhino 9 WIP\System\Yak.exe"
    if (Test-Path -LiteralPath $release) { $release } else { $wip }
}
if (-not (Test-Path -LiteralPath $yak -PathType Leaf)) {
    throw "Rhino $RhinoVersion Yak.exe was not found. Install Rhino $RhinoVersion first."
}
if ($Action -eq "List") { & $yak list; exit $LASTEXITCODE }
if ($Action -eq "Uninstall") { & $yak uninstall xvarna; exit $LASTEXITCODE }

if ([string]::IsNullOrWhiteSpace($Package)) {
    $tag = if ($RhinoVersion -eq 8) { "rh8" } else { "rh9" }
    $matches = @(Get-ChildItem -LiteralPath $packageRoot -Filter "xvarna-*-$tag*-win.yak" -File | Sort-Object LastWriteTimeUtc -Descending)
    if ($matches.Count -ne 1) {
        throw "Expected exactly one $tag Yak package in '$packageRoot'; found $($matches.Count). Pass -Package explicitly."
    }
    $Package = $matches[0].FullName
}
$packagePath = [System.IO.Path]::GetFullPath($Package)
if (-not (Test-Path -LiteralPath $packagePath -PathType Leaf)) { throw "Package not found: $packagePath" }
$checksumPath = Join-Path (Split-Path -Parent $packagePath) "SHA256SUMS.txt"
if (Test-Path -LiteralPath $checksumPath -PathType Leaf) {
    $fileNamePattern = [regex]::Escape((Split-Path -Leaf $packagePath))
    $expectedLine = Get-Content -LiteralPath $checksumPath | Where-Object { $_ -match "  $fileNamePattern$" } | Select-Object -First 1
    if ($null -ne $expectedLine) {
        $expected = ($expectedLine -split "\s+", 2)[0]
        $actual = (Get-FileHash -LiteralPath $packagePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) { throw "SHA-256 mismatch for $(Split-Path -Leaf $packagePath)." }
        Write-Host "Checksum verified: $actual"
    }
}
& $yak install $packagePath
exit $LASTEXITCODE
