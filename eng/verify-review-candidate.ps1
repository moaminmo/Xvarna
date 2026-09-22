[CmdletBinding()]
param([Parameter(Mandatory)][string]$Archive)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $root "artifacts"))
$archivePath = [System.IO.Path]::GetFullPath($Archive)
if (-not $archivePath.StartsWith(($artifactRoot.TrimEnd('\') + '\'), [StringComparison]::OrdinalIgnoreCase)) { throw "Archive must be under the repository artifacts directory." }
if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf)) { throw "Review archive not found: $archivePath" }
$outerSumPath = "$archivePath.sha256"
if (-not (Test-Path -LiteralPath $outerSumPath -PathType Leaf)) { throw "Outer checksum is missing." }
$expectedOuter = ((Get-Content -LiteralPath $outerSumPath -Raw).Trim() -split '\s+')[0]
$actualOuter = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($expectedOuter -ne $actualOuter) { throw "Review archive checksum mismatch." }

$verifyRoot = Join-Path $artifactRoot "review-verify"
if (Test-Path -LiteralPath $verifyRoot) { Remove-Item -LiteralPath $verifyRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $verifyRoot | Out-Null
try {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $verifyRoot -Force
    $required = @(
        "review-readiness.json", "release-manifest.json", "SHA256SUMS.txt", "xvarna.cdx.json",
        "review\reviewer-guide.md", "review\test-session-template.md", "review\beta-kpis.template.json",
        "evidence\phase-1\host-matrix.json", "evidence\phase-2\hardware-matrix.json",
        "evidence\phase-2\performance-sml.parquet", "evidence\phase-2\cpu-gpu-parity.parquet",
        "evidence\phase-2\radiance-validation.parquet", "REVIEW-CANDIDATE-SHA256SUMS.txt"
    )
    foreach ($relative in $required) { if (-not (Test-Path -LiteralPath (Join-Path $verifyRoot $relative) -PathType Leaf)) { throw "Required review artifact missing: $relative" } }
    $readiness = Get-Content -LiteralPath (Join-Path $verifyRoot "review-readiness.json") -Raw | ConvertFrom-Json -Depth 32
    if ($readiness.decision -ne "READY_TO_SEND_FOR_EXTERNAL_REVIEW_NOT_PUBLIC_RELEASE") { throw "Unexpected readiness decision." }
    foreach ($relative in @("evidence\phase-2\performance-sml.parquet", "evidence\phase-2\cpu-gpu-parity.parquet", "evidence\phase-2\radiance-validation.parquet")) {
        $bytes = [IO.File]::ReadAllBytes((Join-Path $verifyRoot $relative)); if ($bytes.Length -lt 8) { throw "Invalid Parquet file: $relative" }
        $head = [Text.Encoding]::ASCII.GetString($bytes, 0, 4); $tail = [Text.Encoding]::ASCII.GetString($bytes, $bytes.Length - 4, 4)
        if ($head -ne "PAR1" -or $tail -ne "PAR1") { throw "Parquet magic mismatch: $relative" }
    }
    foreach ($line in Get-Content -LiteralPath (Join-Path $verifyRoot "REVIEW-CANDIDATE-SHA256SUMS.txt")) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $parts = $line -split '\s+', 2; $relative = $parts[1].Replace('/', '\'); $path = Join-Path $verifyRoot $relative
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Checksummed file missing: $relative" }
        $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant(); if ($actual -ne $parts[0]) { throw "Nested checksum mismatch: $relative" }
    }
} finally {
    if (Test-Path -LiteralPath $verifyRoot) { Remove-Item -LiteralPath $verifyRoot -Recurse -Force }
}
Write-Host "Review candidate verified: outer hash, required files, nested hashes, JSON decision, and Parquet magic."

