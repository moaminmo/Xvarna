[CmdletBinding()]
param(
    [ValidateSet("auto", "gpu", "cpu")]
    [string]$Backend = "auto",
    [string]$Adapter = "",
    [double]$ParityTolerance = 0.000001,
    [int]$MaximumRaysPerDispatch = 64,
    [ValidatePattern("^[a-z0-9-]*$")]
    [string]$EvidenceLabel = ""
)

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repositoryRoot "artifacts\cli\win-x64\xvarna.exe"
if (-not (Test-Path -LiteralPath $cli)) {
    throw "Build the CLI before domain validation: $cli"
}
if (-not [double]::IsFinite($ParityTolerance) -or $ParityTolerance -lt 0) {
    throw "ParityTolerance must be finite and non-negative."
}
if ($MaximumRaysPerDispatch -lt 1) {
    throw "MaximumRaysPerDispatch must be positive."
}

$validationOutput = Join-Path $repositoryRoot "artifacts\validation"
New-Item -ItemType Directory -Force -Path $validationOutput | Out-Null
$common = @(
    "--backend", $Backend,
    "--max-rays-per-dispatch", $MaximumRaysPerDispatch.ToString([Globalization.CultureInfo]::InvariantCulture),
    "--verify-cpu",
    "--parity-tolerance", $ParityTolerance.ToString("R", [Globalization.CultureInfo]::InvariantCulture),
    "--json"
)
if (-not [string]::IsNullOrWhiteSpace($Adapter)) {
    $common += @("--adapter", $Adapter.Trim())
}

$cases = @(
    @{
        Name = "target-view"
        Arguments = @(
            "target-view", "validation\meshes\open_quad.obj",
            "--observer", "1,0,0.5,0.5,1,0,0,0,0,1,1",
            "--target", "7,2,0,0,2,0.5,1,2,1,0,2,1",
            "--samples", "256", "--two-sided", "--max-distance", "10"
        ) + $common
    },
    @{
        Name = "view-corridor"
        Arguments = @(
            "view-corridor", "validation\meshes\open_quad.obj",
            "--corridor", "9,0,0.5,0.5,2,0.5,0.5,0,0,1,0.75",
            "--samples", "256"
        ) + $common
    },
    @{
        Name = "observer-path"
        Arguments = @(
            "observer-path", "validation\meshes\open_quad.obj",
            "--path", "5,0,0,1,1;0,0.5,0.5;0.5,0.5,0.5;1,0.5,0.5",
            "--target", "7,2,0,0,2,0.5,1,2,1,0,2,1",
            "--spacing", "0.25", "--samples", "64", "--two-sided", "--max-distance", "10"
        ) + $common
    }
)

Push-Location $repositoryRoot
try {
    foreach ($case in $cases) {
        $caseArguments = $case.Arguments
        $jsonText = (& $cli @caseArguments | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) {
            throw "VAYU $($case.Name) validation failed with exit code $LASTEXITCODE."
        }
        try {
            $report = $jsonText | ConvertFrom-Json -Depth 32
        }
        catch {
            throw "VAYU $($case.Name) emitted invalid JSON: $jsonText"
        }
        if ($report.schemaVersion -ne "0.16.0" -or -not $report.cpuParity.passed) {
            throw "VAYU $($case.Name) failed schema/parity validation."
        }
        if ($report.cpuParity.identityMismatches -ne 0 -or $report.execution.rays -le 0) {
            throw "VAYU $($case.Name) returned invalid identity or execution provenance."
        }
        if ($Backend -eq "gpu" -and $report.execution.backend -ne "VAYU Portable GPU") {
            throw "VAYU $($case.Name) did not use the required GPU backend."
        }
        $suffix = if ([string]::IsNullOrEmpty($EvidenceLabel)) { "" } else { "-$EvidenceLabel" }
        $destination = Join-Path $validationOutput "vayu-domain-$($case.Name)$suffix.json"
        Set-Content -LiteralPath $destination -Value $jsonText -Encoding utf8NoBOM
        Write-Host "VAYU domain PASS: $($case.Name) -> $destination"
    }
}
finally {
    Pop-Location
}
