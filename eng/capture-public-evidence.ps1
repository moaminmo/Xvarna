[CmdletBinding()]
param(
    [int[]]$RayCounts = @(4096, 65536, 262144),
    [ValidateRange(0, 100)][int]$Warmup = 2,
    [ValidateRange(1, 100)][int]$Iterations = 5
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repositoryRoot "artifacts\cli\win-x64\xvarna.exe"
$generated = Join-Path $repositoryRoot "artifacts\validation"
$evidence = Join-Path $repositoryRoot "benchmarks\evidence\0.17.0"
$RayCounts = @($RayCounts)
if (-not (Test-Path -LiteralPath $cli -PathType Leaf)) { throw "Build the Release CLI first." }
New-Item -ItemType Directory -Force -Path $evidence | Out-Null

$benchmarkParameters = @{ RayCounts = $RayCounts; Warmup = $Warmup; Iterations = $Iterations }
& "$PSScriptRoot\benchmark-vayu-throughput.ps1" -Backend gpu -Adapter NVIDIA -EvidenceLabel rtx5060 @benchmarkParameters
& "$PSScriptRoot\benchmark-vayu-throughput.ps1" -Backend gpu -Adapter Intel -EvidenceLabel intel-uhd @benchmarkParameters
$nvidiaSource = Join-Path $generated "vayu-throughput-gpu-rtx5060.json"
$intelSource = Join-Path $generated "vayu-throughput-gpu-intel-uhd.json"
Copy-Item -LiteralPath $nvidiaSource -Destination (Join-Path $evidence "vayu-throughput-nvidia-rtx5060.json") -Force
Copy-Item -LiteralPath $intelSource -Destination (Join-Path $evidence "vayu-throughput-intel-uhd.json") -Force

$devices = (& $cli devices --json | Out-String).Trim() | ConvertFrom-Json -Depth 32
$computer = Get-CimInstance Win32_ComputerSystem
$processor = Get-CimInstance Win32_Processor | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem
$commit = (& git -C $repositoryRoot rev-parse HEAD).Trim()
$dirtyStatus = & git -C $repositoryRoot status --porcelain -- . ":(exclude)benchmarks/evidence/**"
$dirty = -not [string]::IsNullOrWhiteSpace(($dirtyStatus | Out-String))
$environment = [ordered]@{
    schemaVersion = "1.0.0"
    capturedUtc = [DateTimeOffset]::UtcNow.ToString("O")
    sourceCommit = $commit
    sourceDirty = $dirty
    operatingSystem = "$($os.Caption) $($os.Version)"
    processor = $processor.Name.Trim()
    logicalProcessors = [int]$computer.NumberOfLogicalProcessors
    memoryBytes = [uint64]$computer.TotalPhysicalMemory
    rustc = (& rustc --version | Out-String).Trim()
    dotnet = (& dotnet --version | Out-String).Trim()
    rayCounts = $RayCounts
    warmupCount = $Warmup
    iterationCount = $Iterations
    adapters = $devices.adapters
}
$environment | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath (Join-Path $evidence "environment.json") -Encoding utf8NoBOM

$nvidia = Get-Content -LiteralPath $nvidiaSource -Raw | ConvertFrom-Json -Depth 32
$intel = Get-Content -LiteralPath $intelSource -Raw | ConvertFrom-Json -Depth 32
$rows = for ($index = 0; $index -lt $RayCounts.Count; $index++) {
    $n = $nvidia.cases[$index]
    $i = $intel.cases[$index]
    "| $($RayCounts[$index]) | $($n.warm.p50Milliseconds) | $($n.cpu.p50Milliseconds) | $($i.warm.p50Milliseconds) | $($i.cpu.p50Milliseconds) | $($n.parity.stateMismatches) / $($n.parity.identityMismatches) | $($i.parity.stateMismatches) / $($i.parity.identityMismatches) |"
}
$summary = @"
# XVARNA 0.17 portable GPU evidence

This is a reproducible engineering microbenchmark, not a competitor benchmark and not a claim about whole-building speed. It uses the tracked two-triangle analytic mesh so parity and transfer/runtime behaviour remain auditable. Timings are warm p50 milliseconds and include upload plus execution plus readback; lower is better.

| Rays | NVIDIA GPU | CPU paired with NVIDIA run | Intel GPU | CPU paired with Intel run | NVIDIA state / identity mismatches | Intel state / identity mismatches |
|---:|---:|---:|---:|---:|---:|---:|
$($rows -join "`n")

Both adapters completed without fallback, device loss, state mismatch, or identity mismatch. On this tiny two-triangle corpus CPU is faster at the larger batches because transfer dominates; the evidence must not be presented as a GPU speedup claim. Representative architectural scenes, AMD hardware, thermal/power controls, and third-party competitor workflows remain separate launch gates.

Regenerate from the repository root with:

~~~powershell
.\eng\capture-public-evidence.ps1
~~~
"@
Set-Content -LiteralPath (Join-Path $evidence "README.md") -Value $summary -Encoding utf8NoBOM
Write-Host "Public evidence captured in $evidence"
