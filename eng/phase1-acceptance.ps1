[CmdletBinding()]
param(
    [ValidateSet("Inventory", "Automated", "Full")]
    [string]$Mode = "Inventory",
    [string]$Configuration = "Release",
    [switch]$ProductOwnerApproval,
    [string]$ProductOwnerName = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "artifacts\acceptance\phase-1"
New-Item -ItemType Directory -Force -Path $out | Out-Null

$sources = Get-ChildItem -LiteralPath (Join-Path $root "dotnet\Xvarna.Grasshopper2\Components") -Filter "*.cs" -File
$components = foreach ($source in $sources) {
    $text = Get-Content -LiteralPath $source.FullName -Raw
    foreach ($match in [regex]::Matches($text, '\[IoId\("(?<id>[0-9a-f-]{36})"\)\]\s*public sealed class (?<type>[A-Za-z0-9_]+)')) {
        [pscustomobject]@{ id = $match.Groups['id'].Value; type = $match.Groups['type'].Value }
    }
}
$components = @($components | Sort-Object type)
if ($components.Count -ne 49) { throw "Expected 49 GH2 capabilities, found $($components.Count)." }
if (($components.id | Sort-Object -Unique).Count -ne 49) { throw "Component semantic IDs are not unique." }

$docs = @(Get-ChildItem -LiteralPath (Join-Path $root "docs\components") -Filter "*.md" -File)
$canvases = @(Get-ChildItem -LiteralPath (Join-Path $root "examples\grasshopper") -Filter "*.ghx" -File)
if ($docs.Count -ne 49) { throw "Expected 49 component reference pages, found $($docs.Count)." }
if ($canvases.Count -ne 12) { throw "Expected 12 native GH1 tutorial canvases, found $($canvases.Count)." }
if ($canvases.Where({ $_.Length -lt 10KB }).Count -ne 0) { throw "One or more tutorial canvases are unexpectedly small." }

$automated = [ordered]@{ rhino8Gh1 = $null; rhino9Gh1 = $null; rhino9Gh2 = $null }
if ($Mode -ne "Inventory") {
    & (Join-Path $PSScriptRoot "test-rhino8-compat.ps1") -Configuration $Configuration
    $automated.rhino8Gh1 = "PASS"
    & (Join-Path $PSScriptRoot "test-rhino9-compat.ps1") -Configuration $Configuration
    $automated.rhino9Gh1 = "PASS"
    & (Join-Path $PSScriptRoot "test-grasshopper2-compat.ps1") -Configuration $Configuration
    $automated.rhino9Gh2 = "PASS"
}

$componentMatrixPath = Join-Path $out "component-49x3.csv"
$previousRows = @{}
if (Test-Path -LiteralPath $componentMatrixPath) {
    foreach ($row in @(Import-Csv -LiteralPath $componentMatrixPath)) {
        $previousRows["$($row.Host)|$($row.SemanticId)"] = $row
    }
}
$rows = foreach ($hostName in @("Rhino 8 + GH1", "Rhino 9 + GH1", "Rhino 9 + GH2")) {
    foreach ($component in $components) {
        $key = "$hostName|$($component.id)"
        $previous = $previousRows[$key]
        [pscustomobject]@{
            Host = $hostName; SemanticId = $component.id; Component = $component.type
            RegisteredAndConstructed = if ($Mode -eq "Inventory") { "NOT_RUN" } else { "PASS" }
            Icon = if ($Mode -eq "Inventory") { "NOT_RUN" } else { "PASS" }
            Place = if ($null -eq $previous) { "PENDING_MANUAL" } else { $previous.Place }
            Wire = if ($null -eq $previous) { "PENDING_MANUAL" } else { $previous.Wire }
            Solve = if ($null -eq $previous) { "PENDING_MANUAL" } else { $previous.Solve }
            SaveReopen = if ($null -eq $previous) { "PENDING_MANUAL" } else { $previous.SaveReopen }
            Tester = if ($null -eq $previous) { "" } else { $previous.Tester }
            Evidence = if ($null -eq $previous) { "" } else { $previous.Evidence }
            Notes = if ($null -eq $previous) { "" } else { $previous.Notes }
        }
    }
}
$rows | Export-Csv -LiteralPath $componentMatrixPath -NoTypeInformation -Encoding utf8

$completedRows = @($rows | Where-Object {
    $_.Place -eq "PASS" -and $_.Wire -eq "PASS" -and $_.Solve -eq "PASS" -and $_.SaveReopen -eq "PASS"
}).Count
$saveReopenPath = Join-Path $out "save-reopen-results.json"
if (-not (Test-Path -LiteralPath $saveReopenPath)) {
    [ordered]@{ schemaVersion = "1.0"; status = "PENDING_MANUAL"; runs = @() } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $saveReopenPath -Encoding utf8
}
$saveReopen = Get-Content -LiteralPath $saveReopenPath -Raw | ConvertFrom-Json
$asyncText = Get-Content -LiteralPath (Join-Path $out "async-cancel-device-loss.md") -Raw
$onboardingText = Get-Content -LiteralPath (Join-Path $out "onboarding-observation.md") -Raw
$asyncPassed = $asyncText -match '(?im)^Status:\s*\*\*PASS\*\*'
$onboardingPassed = $onboardingText -match '(?im)^Status:\s*\*\*PASS\*\*'
$saveReopenPassed = $saveReopen.status -eq "PASS"
$approvalPassed = $ProductOwnerApproval -and -not [string]::IsNullOrWhiteSpace($ProductOwnerName)
$blockers = @()
if ($completedRows -ne 147) { $blockers += "Interactive component rows: $completedRows/147 complete." }
if (-not $saveReopenPassed) { $blockers += "Save/reopen acceptance is not PASS." }
if (-not $asyncPassed) { $blockers += "Async/cancel/device-loss acceptance is not PASS." }
if (-not $onboardingPassed) { $blockers += "Independent onboarding acceptance is not PASS." }
if (-not $approvalPassed) { $blockers += "Product-owner approval and name were not supplied." }

$matrix = [ordered]@{
    schemaVersion = "1.0"; generatedUtc = [DateTimeOffset]::UtcNow.ToString("O")
    candidateCommit = (git -C $root rev-parse HEAD).Trim(); configuration = $Configuration
    inventory = [ordered]@{ semanticComponents = 49; componentReferencePages = 49; tutorialCanvases = 12 }
    automatedRuntime = $automated
    interactiveAcceptance = [ordered]@{
        status = if ($completedRows -eq 147) { "PASS" } else { "PENDING_MANUAL" }; requiredRows = 147; completedRows = $completedRows
        rule = "PASS requires place, wire, solve and save/reopen evidence from package-installed binaries for every row."
    }
    supportingAcceptance = [ordered]@{ saveReopen = $saveReopenPassed; asyncCancelDeviceLoss = $asyncPassed; onboarding = $onboardingPassed }
    productOwner = [ordered]@{ approved = $approvalPassed; name = $ProductOwnerName }
    phaseDecision = if ($blockers.Count -eq 0 -and $Mode -eq "Full") { "PASS" } else { "FAIL" }
    decisionReason = if ($blockers.Count -eq 0 -and $Mode -eq "Full") { "All Phase 1 acceptance gates passed." } else { $blockers -join " " }
}
$matrix | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $out "host-matrix.json") -Encoding utf8

if ($Mode -eq "Full") {
    $log = Join-Path $out "full-check.log"
    & (Join-Path $PSScriptRoot "check.ps1") *>&1 | Tee-Object -FilePath $log
    if ($LASTEXITCODE -ne 0) { throw "Full check failed; see $log" }
}

Write-Host "Phase 1 inventory: 49 components, 49 reference pages, 12 native canvases."
Write-Host "Phase 1 decision: $($matrix.phaseDecision). $($matrix.decisionReason)"
