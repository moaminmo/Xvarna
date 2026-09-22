[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Cli,
    [ValidateRange(2, 500)]
    [int]$Iterations = 12,
    [ValidateRange(4, 256)]
    [int]$MutationCount = 24,
    [switch]$CollectManagedCoverage,
    [ValidateSet("Debug", "Release")]
    [string]$CoverageConfiguration = "Debug",
    [ValidateRange(0.0, 1.0)]
    [double]$MinimumManagedLineRate = 0.0
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$sourceMesh = Join-Path $repositoryRoot "validation\meshes\open_quad.obj"
$outputRoot = Join-Path $repositoryRoot "artifacts\release-quality"

if (-not (Test-Path -LiteralPath $Cli)) {
    throw "XVARNA CLI was not found: $Cli"
}
if (-not (Test-Path -LiteralPath $sourceMesh)) {
    throw "Release-quality mesh fixture was not found: $sourceMesh"
}

New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
$baselineScene = Join-Path $outputRoot "baseline.xvscene"
$baselineObj = Join-Path $outputRoot "baseline.obj"
Invoke-XvarnaCommand $Cli scene-pack $sourceMesh $baselineScene --repair-winding --json
$expectedHash = (Get-FileHash -LiteralPath $baselineScene -Algorithm SHA256).Hash

# Soak the complete parse -> repair -> serialize path in separate processes. Besides catching
# crashes, the byte identity is a strict nondeterminism gate across every iteration.
for ($iteration = 0; $iteration -lt $Iterations; $iteration++) {
    $candidateScene = Join-Path $outputRoot ("soak-{0:D4}.xvscene" -f $iteration)
    $candidateObj = Join-Path $outputRoot ("soak-{0:D4}.obj" -f $iteration)
    & $Cli scene-pack $sourceMesh $candidateScene --repair-winding --json *> $null
    if ($LASTEXITCODE -ne 0) { throw "Scene-pack soak failed at iteration $iteration." }
    if ((Get-FileHash -LiteralPath $candidateScene -Algorithm SHA256).Hash -ne $expectedHash) {
        throw "Scene serialization became nondeterministic at iteration $iteration."
    }
    & $Cli scene-unpack $candidateScene $candidateObj --json *> $null
    if ($LASTEXITCODE -ne 0) { throw "Scene-unpack soak failed at iteration $iteration." }
}

# Exercise a stable analysis repeatedly and require a stable scientific content identity.
$analysisHash = $null
for ($iteration = 0; $iteration -lt $Iterations; $iteration++) {
    $json = & $Cli daylight-factor $sourceMesh `
        --sensor "1,0.5,0.5,1,0,0,1,1" `
        --patches 64 `
        --json
    if ($LASTEXITCODE -ne 0) { throw "Daylight soak failed at iteration $iteration." }
    $report = $json | ConvertFrom-Json
    $resultHashProperty = $report.PSObject.Properties["resultHash"]
    if ($null -eq $resultHashProperty -or [string]::IsNullOrWhiteSpace([string]$resultHashProperty.Value)) {
        throw "Daylight soak did not expose a resultHash."
    }
    $currentHash = [string]$resultHashProperty.Value
    if ($null -eq $analysisHash) { $analysisHash = $currentHash }
    elseif ($analysisHash -ne $currentHash) {
        throw "Daylight scientific identity changed at iteration $iteration."
    }
}

# Deterministically mutate framing, metadata, payload, and checksum bytes. Every malformed
# binary must fail closed without producing an output model or terminating the harness.
$baselineBytes = [System.IO.File]::ReadAllBytes($baselineScene)
for ($index = 0; $index -lt $MutationCount; $index++) {
    $mutated = [byte[]]$baselineBytes.Clone()
    $offset = [int](($index * 7919 + 3) % $mutated.Length)
    $mutated[$offset] = $mutated[$offset] -bxor [byte](1 -shl ($index % 8))
    $mutationPath = Join-Path $outputRoot ("mutation-{0:D3}.xvscene" -f $index)
    $mutationOutput = Join-Path $outputRoot ("mutation-{0:D3}.obj" -f $index)
    [System.IO.File]::WriteAllBytes($mutationPath, $mutated)
    & $Cli scene-unpack $mutationPath $mutationOutput --json *> $null
    if ($LASTEXITCODE -eq 0) {
        throw "Corrupt XVSCN mutation $index was accepted."
    }
}
for ($index = 1; $index -le 8; $index++) {
    $length = [Math]::Max(0, $baselineBytes.Length - $index * [Math]::Max(1, [int]($baselineBytes.Length / 11)))
    $truncated = New-Object byte[] $length
    if ($length -gt 0) { [Array]::Copy($baselineBytes, $truncated, $length) }
    $truncatedPath = Join-Path $outputRoot ("truncated-{0:D2}.xvscene" -f $index)
    [System.IO.File]::WriteAllBytes($truncatedPath, $truncated)
    & $Cli scene-unpack $truncatedPath $baselineObj --json *> $null
    if ($LASTEXITCODE -eq 0) {
        throw "Truncated XVSCN corpus entry $index was accepted."
    }
}

$coverageRate = $null
if ($CollectManagedCoverage) {
    $coverageRoot = Join-Path $outputRoot "coverage"
    Invoke-XvarnaCommand dotnet test `
        (Join-Path $repositoryRoot "dotnet\Xvarna.Tests\Xvarna.Tests.csproj") `
        --configuration $CoverageConfiguration `
        --no-build `
        --collect:"XPlat Code Coverage" `
        --results-directory $coverageRoot
    $coverageFiles = @(Get-ChildItem -LiteralPath $coverageRoot -Filter "coverage.cobertura.xml" -Recurse)
    if ($coverageFiles.Count -eq 0) { throw "Managed coverage collector produced no Cobertura report." }
    $rates = foreach ($coverageFile in $coverageFiles) {
        [xml]$coverage = Get-Content -LiteralPath $coverageFile.FullName -Raw
        [double]::Parse($coverage.coverage.'line-rate', [Globalization.CultureInfo]::InvariantCulture)
    }
    $coverageRate = ($rates | Measure-Object -Minimum).Minimum
    if ($coverageRate -lt $MinimumManagedLineRate) {
        throw "Managed line coverage $($coverageRate.ToString('P2')) is below the required $($MinimumManagedLineRate.ToString('P2'))."
    }
}

$coverageText = if ($null -eq $coverageRate) { "not requested" } else { $coverageRate.ToString("P2") }
Write-Host "Release-quality gate passed: $Iterations deterministic soak iterations, $MutationCount corruptions, 8 truncations; managed coverage $coverageText."
