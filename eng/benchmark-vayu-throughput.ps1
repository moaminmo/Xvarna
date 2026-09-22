[CmdletBinding()]
param(
    [ValidateSet("auto", "gpu", "cpu")]
    [string]$Backend = "auto",
    [string]$Adapter = "",
    [int[]]$RayCounts = @(4096, 65536, 262144),
    [ValidateRange(0, 100)]
    [int]$Warmup = 2,
    [ValidateRange(1, 100)]
    [int]$Iterations = 5,
    [ValidatePattern("^[a-z0-9-]*$")]
    [string]$EvidenceLabel = ""
)

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repositoryRoot "artifacts\cli\win-x64\xvarna.exe"
if (-not (Test-Path -LiteralPath $cli)) {
    throw "Build the Release CLI before throughput validation: $cli"
}
$normalizedRayCounts = @($RayCounts)
if ($normalizedRayCounts.Count -eq 0 -or @($normalizedRayCounts | Where-Object { $_ -lt 1 }).Count -gt 0) {
    throw "RayCounts must contain positive values."
}

$validationOutput = Join-Path $repositoryRoot "artifacts\validation"
New-Item -ItemType Directory -Force -Path $validationOutput | Out-Null
$reports = [Collections.Generic.List[object]]::new()

Push-Location $repositoryRoot
try {
    foreach ($rayCount in $normalizedRayCounts) {
        $arguments = @(
            "backend-benchmark", "validation\meshes\open_quad.obj",
            "--backend", $Backend,
            "--rays", $rayCount.ToString([Globalization.CultureInfo]::InvariantCulture),
            "--warmup", $Warmup.ToString([Globalization.CultureInfo]::InvariantCulture),
            "--iterations", $Iterations.ToString([Globalization.CultureInfo]::InvariantCulture),
            "--json"
        )
        if (-not [string]::IsNullOrWhiteSpace($Adapter)) {
            $arguments += @("--adapter", $Adapter.Trim())
        }
        $jsonText = (& $cli @arguments | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) {
            throw "VAYU throughput case $rayCount failed with exit code $LASTEXITCODE."
        }
        try {
            $report = $jsonText | ConvertFrom-Json -Depth 32
        }
        catch {
            throw "VAYU throughput case $rayCount emitted invalid JSON: $jsonText"
        }
        if ($report.schemaVersion -ne "0.16.0" -or -not $report.parity.accepted) {
            throw "VAYU throughput case $rayCount failed schema/parity validation."
        }
        if ($report.warm.p50Milliseconds -lt 0 -or $report.runtime.deviceLosses -ne 0) {
            throw "VAYU throughput case $rayCount returned invalid timing/device telemetry."
        }
        if ($Backend -eq "gpu") {
            $expectedReuseCount = 1 + $Warmup + $Iterations
            $wrongBackend = $report.backend -ne "VAYU Portable GPU"
            $wrongReuseCount = $report.runtime.reusableBatches -ne $expectedReuseCount
            $wrongGenerationCount = $report.runtime.scratchGenerations -ne 1
            if ($wrongBackend -or $wrongReuseCount -or $wrongGenerationCount -or -not $report.runtime.deviceHealthy) {
                throw "VAYU throughput case $rayCount did not satisfy persistent-arena invariants."
            }
        }
        $reports.Add($report)
        Write-Host "VAYU throughput PASS: $rayCount rays; p50 $($report.warm.p50Milliseconds) ms; $($report.backend)"
    }

    $suffix = if ([string]::IsNullOrEmpty($EvidenceLabel)) { "" } else { "-$EvidenceLabel" }
    $destination = Join-Path $validationOutput "vayu-throughput-$Backend$suffix.json"
    $evidence = [ordered]@{
        schemaVersion = "0.16.0"
        generatedUtc = [DateTimeOffset]::UtcNow.ToString("O", [Globalization.CultureInfo]::InvariantCulture)
        backendPolicy = $Backend
        adapterFilter = $Adapter
        warmupCount = $Warmup
        iterationCount = $Iterations
        cases = $reports
    }
    $evidence | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $destination -Encoding utf8NoBOM
    Write-Host "VAYU throughput evidence: $destination"
}
finally {
    Pop-Location
}
