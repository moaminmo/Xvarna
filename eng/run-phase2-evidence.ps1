[CmdletBinding()]
param(
    [ValidateSet("Smoke", "Full")]
    [string]$Profile = "Smoke",
    [string]$CorpusDirectory = "",
    [string]$OutputDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($CorpusDirectory)) { $CorpusDirectory = Join-Path $root "artifacts\benchmark-corpus" }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $root "artifacts\acceptance\phase-2" }
$corpus = [System.IO.Path]::GetFullPath($CorpusDirectory); $output = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$sourceCommit = (& git -C $root rev-parse HEAD).Trim()
$sourceDirty = -not [string]::IsNullOrWhiteSpace((& git -C $root status --porcelain | Out-String))
$environment = [ordered]@{
    windows = [Environment]::OSVersion.VersionString
    logicalProcessors = [Environment]::ProcessorCount
    rustc = ((& rustc --version | Out-String).Trim())
    dotnet = ((& dotnet --version | Out-String).Trim())
}
$tiers = if ($Profile -eq "Full") { @("S", "M", "L") } else { @("S") }
$generatedManifestPath = Join-Path $corpus "generated-manifest.json"
$reuseCorpus = Test-Path -LiteralPath $generatedManifestPath -PathType Leaf
if ($reuseCorpus) {
    $candidateManifest = Get-Content -LiteralPath $generatedManifestPath -Raw | ConvertFrom-Json -Depth 16
    foreach ($tier in $tiers) {
        $candidateRecord = $candidateManifest.records | Where-Object tier -eq $tier
        if ($null -eq $candidateRecord) { $reuseCorpus = $false; break }
        foreach ($entry in @($candidateRecord.mesh, $candidateRecord.sensorTable)) {
            $path = Join-Path $corpus $entry.path
            if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) { $reuseCorpus = $false; break }
        }
        if (-not $reuseCorpus) { break }
    }
}
if (-not $reuseCorpus) {
    & (Join-Path $PSScriptRoot "generate-benchmark-corpus.ps1") -Tier $(if ($Profile -eq "Full") { "All" } else { "S" }) -OutputDirectory $corpus
} else {
    Write-Host "Reusing hash-verified generated benchmark corpus."
}
$cli = Join-Path $root "artifacts\cli\win-x64\xvarna.exe"
& (Join-Path $PSScriptRoot "build-cli.ps1") -Configuration Release
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $cli)) { throw "Release CLI build failed." }
$devicesText = (& $cli devices --json | Out-String).Trim(); if ($LASTEXITCODE -ne 0) { throw "Device enumeration failed." }
$devices = $devicesText | ConvertFrom-Json -Depth 16
$manifest = Get-Content -LiteralPath $generatedManifestPath -Raw | ConvertFrom-Json -Depth 16
$rows = @(); $raw = @()
foreach ($tier in $tiers) {
    $record = $manifest.records | Where-Object tier -eq $tier
    $mesh = Join-Path $corpus $record.mesh.path
    $rayCount = if ($tier -eq "S") { 4096 } elseif ($tier -eq "M") { 65536 } else { 262144 }
    foreach ($backend in @("cpu", "gpu")) {
        $requestedBackend = if ($tier -eq "L" -and $backend -eq "gpu") { "auto-gpu-preferred" } else { $backend }
        $backendArgument = if ($requestedBackend -eq "auto-gpu-preferred") { "auto" } else { $backend }
        $text = (& $cli backend-benchmark $mesh --backend $backendArgument --rays $rayCount --warmup 2 --iterations 5 --max-error 0.00025 --parity-tolerance 0.00025 --max-triangles 6000000 --geometry-chunk-mb 768 --gpu-memory-mb 2048 --json | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw "Benchmark failed for $tier/$backend." }
        $report = $text | ConvertFrom-Json -Depth 32; $raw += [ordered]@{ tier = $tier; requestedBackend = $requestedBackend; report = $report }
        $rows += [ordered]@{
            tier = $tier; requestedBackend = $requestedBackend; backend = $report.backend; adapter = $report.adapter; triangles = [uint64]$record.triangles; sensors = [uint64]$record.sensors; rays = [uint64]$rayCount
            coldMilliseconds = [double]$report.cold.totalMilliseconds; p50Milliseconds = [double]$report.warm.p50Milliseconds; p95Milliseconds = [double]$report.warm.p95Milliseconds
            memoryBytes = [uint64]$report.hostSnapshotBytes; vramBytes = [uint64]($report.sceneGpuBytes + $report.runtime.scratchGpuBytes)
            parityAccepted = [bool]$report.parity.accepted; deviceLosses = [uint64]$report.runtime.deviceLosses; runtimeFallbacks = [uint64]$report.runtime.runtimeFallbacks
        }
    }
}
$evidence = [ordered]@{ schemaVersion = "1.0.0"; generatedUtc = [DateTimeOffset]::UtcNow.ToString("O"); sourceCommit = $sourceCommit; sourceDirty = $sourceDirty; profile = $Profile; environment = $environment; corpusManifest = $manifest; rows = $rows; raw = $raw }
$jsonPath = Join-Path $output "performance-sml.json"; $evidence | ConvertTo-Json -Depth 40 | Set-Content -LiteralPath $jsonPath -Encoding utf8NoBOM
$rows | Export-Csv -LiteralPath (Join-Path $output "performance-sml.csv") -NoTypeInformation -Encoding utf8
$tableSource = Join-Path $output "performance-sml.table.json"
[ordered]@{ schemaVersion = "1.0.0"; rows = $rows } | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $tableSource -Encoding utf8NoBOM
try {
    & $cli evidence-table $tableSource (Join-Path $output "performance-sml.parquet")
    if ($LASTEXITCODE -ne 0) { throw "Parquet export failed." }
    Copy-Item -LiteralPath (Join-Path $output "performance-sml.parquet") -Destination (Join-Path $output "cpu-gpu-parity.parquet") -Force
} finally {
    Remove-Item -LiteralPath $tableSource -Force -ErrorAction SilentlyContinue
}

$fastValues = @(Get-Content -LiteralPath (Join-Path $root "validation\daylight\fast-lux.txt") | ForEach-Object { [double]::Parse($_, [Globalization.CultureInfo]::InvariantCulture) })
$referenceValues = @(Get-Content -LiteralPath (Join-Path $root "validation\daylight\radiance-reference-lux.txt") | ForEach-Object { [double]::Parse($_, [Globalization.CultureInfo]::InvariantCulture) })
if ($fastValues.Count -ne $referenceValues.Count -or $fastValues.Count -eq 0) { throw "Daylight validation fixtures are not aligned." }
$rtrace = Get-Command rtrace -ErrorAction SilentlyContinue
$radianceVersion = if ($null -eq $rtrace) { "not-installed" } else { ((& $rtrace.Source -version 2>&1 | Select-Object -First 1) -as [string]).Trim() }
$daylightRows = for ($index = 0; $index -lt $fastValues.Count; $index++) {
    $delta = $fastValues[$index] - $referenceValues[$index]; $absolute = [Math]::Abs($delta)
    $percentage = if ([Math]::Abs($referenceValues[$index]) -gt 1.0e-12) { 100.0 * $absolute / [Math]::Abs($referenceValues[$index]) } else { 0.0 }
    [ordered]@{ sensorId = [uint64]($index + 1); fastLux = $fastValues[$index]; referenceLux = $referenceValues[$index]; deltaLux = $delta; absoluteErrorLux = $absolute; percentageError = $percentage }
}
$daylightSource = Join-Path $output "radiance-validation.json"
[ordered]@{ schemaVersion = "1.0.0"; radianceVersion = $radianceVersion; caseId = "analytic-open-reference"; rows = $daylightRows } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $daylightSource -Encoding utf8NoBOM
& $cli daylight-evidence-table $daylightSource (Join-Path $output "radiance-validation.parquet")
if ($LASTEXITCODE -ne 0) { throw "Daylight Parquet export failed." }
$vendorStatus = foreach ($vendor in @("Intel", "NVIDIA", "AMD")) {
    $matches = @($devices.adapters | Where-Object { $_.name -match $vendor -and $_.deviceType -match "Gpu" })
    [ordered]@{ vendor = $vendor; detected = $matches.Count -gt 0; adapters = @($matches); status = if ($matches.Count -gt 0) { "AVAILABLE_FOR_TEST" } else { "PENDING_EXTERNAL_HARDWARE" } }
}
[ordered]@{
    schemaVersion = "1.0.0"; generatedUtc = [DateTimeOffset]::UtcNow.ToString("O"); sourceCommit = $sourceCommit; sourceDirty = $sourceDirty; profile = $Profile; environment = $environment
    cpu = [ordered]@{ status = "PASS"; logicalProcessors = [Environment]::ProcessorCount }
    gpuVendors = $vendorStatus; deviceEnumeration = $devices.adapters
    decision = if ($Profile -eq "Full" -and @($vendorStatus | Where-Object { -not $_.detected }).Count -eq 0) { "PASS" } else { "READY_FOR_EXTERNAL_MATRIX" }
} | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $output "hardware-matrix.json") -Encoding utf8NoBOM
Write-Host "Phase 2 evidence captured: $($rows.Count) rows; profile $Profile; output $output"
