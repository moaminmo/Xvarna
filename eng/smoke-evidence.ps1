[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Cli
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$fixtures = Join-Path $repositoryRoot "validation\evidence"
$output = Join-Path $repositoryRoot "artifacts\validation\evidence"
New-Item -ItemType Directory -Force -Path $output | Out-Null

$design = Join-Path $output "saltelli-design.json"
$values = Join-Path $output "saltelli-outputs.json"
$analysis = Join-Path $output "saltelli-analysis.json"
Invoke-XvarnaCommand $Cli evidence sensitivity-design `
    (Join-Path $fixtures "variables.json") $design --method saltelli --samples 4096 --seed 42
$rows = (Get-Content -LiteralPath $design -Raw | ConvertFrom-Json).rows
$evaluations = @($rows | ForEach-Object { ,@([double]$_.values[0] + 2.0 * [double]$_.values[1]) })
$evaluations | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $values -Encoding utf8
Invoke-XvarnaCommand $Cli evidence sensitivity-analyze $design $values $analysis
$indices = (Get-Content -LiteralPath $analysis -Raw | ConvertFrom-Json).sobol
if ([Math]::Abs([double]$indices[0].totalOrder - 0.2) -gt 0.04 -or
    [Math]::Abs([double]$indices[1].totalOrder - 0.8) -gt 0.04) {
    throw "Saltelli/Jansen analytic validation exceeded its tolerance."
}

$passport = Join-Path $output "evidence-passport.json"
Invoke-XvarnaCommand $Cli evidence passport (Join-Path $fixtures "passport.json") $passport
if ([string]::IsNullOrWhiteSpace([string](Get-Content -LiteralPath $passport -Raw | ConvertFrom-Json).contentHash)) {
    throw "Evidence Passport was not sealed."
}

$robust = Join-Path $output "robust.json"
Invoke-XvarnaCommand $Cli evidence robust (Join-Path $fixtures "robust.json") $robust
$robustResult = Get-Content -LiteralPath $robust -Raw | ConvertFrom-Json
if ([Math]::Abs([double]$robustResult[0].cvarObjectives[0] - 5.0) -gt 1e-10) {
    throw "Robust CVaR analytic validation failed."
}

$rank = Join-Path $output "uncertainty-rank.json"
Invoke-XvarnaCommand $Cli evidence rank (Join-Path $fixtures "uncertainty.json") $rank
$rankResult = Get-Content -LiteralPath $rank -Raw | ConvertFrom-Json
if ($rankResult[0].robustParetoRank -ne 0 -or $rankResult[1].robustParetoRank -ne 1) {
    throw "Interval-Pareto analytic validation failed."
}

$fidelity = Join-Path $output "multifidelity-selection.json"
Invoke-XvarnaCommand $Cli evidence fidelity (Join-Path $fixtures "multifidelity.json") $fidelity
$selection = Get-Content -LiteralPath $fidelity -Raw | ConvertFrom-Json
if ($selection.recommendations.Count -ne 2 -or
    @($selection.recommendations | Where-Object { $_.candidateId -le 3 }).Count -ne 0) {
    throw "Multi-fidelity reference selection failed."
}

Write-Host "0.18 RASHNU/VAHMAN evidence smoke passed: passport, Saltelli/Jansen, robust CVaR, interval Pareto, and multi-fidelity selection."
