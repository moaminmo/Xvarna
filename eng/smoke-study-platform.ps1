[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Cli
)

$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$cliPath = [System.IO.Path]::GetFullPath($Cli)
$validationRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot "artifacts\validation"))
$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $validationRoot "study-platform-workspace"))
$manifestPath = Join-Path $repositoryRoot "validation\study\platform-manifest.json"
$trackedSchema = Join-Path $repositoryRoot "schemas\xvarna-study-0.16.schema.json"
$generatedSchema = Join-Path $validationRoot "study-manifest.generated.schema.json"

if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf)) {
    throw "Study smoke CLI is missing: $cliPath"
}
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Study smoke manifest is missing: $manifestPath"
}
if (-not (Test-Path -LiteralPath $trackedSchema -PathType Leaf)) {
    throw "Tracked Study JSON Schema is missing: $trackedSchema"
}

New-Item -ItemType Directory -Force -Path $validationRoot | Out-Null
$validationPrefix = $validationRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $workspaceRoot.StartsWith($validationPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to replace workspace outside validation output: $workspaceRoot"
}
if (Test-Path -LiteralPath $workspaceRoot) {
    Remove-Item -LiteralPath $workspaceRoot -Recurse -Force
}

function Invoke-StudyCli {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

    $result = & $cliPath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Study CLI failed ($LASTEXITCODE): $($Arguments -join ' ')"
    }
    return $result
}

Invoke-StudyCli study-schema $generatedSchema | Out-Null
$trackedSchemaObject = Get-Content -LiteralPath $trackedSchema -Raw | ConvertFrom-Json
$generatedSchemaObject = Get-Content -LiteralPath $generatedSchema -Raw | ConvertFrom-Json
if ($trackedSchemaObject.'$id' -ne $generatedSchemaObject.'$id' -or
    $trackedSchemaObject.title -ne $generatedSchemaObject.title -or
    $generatedSchemaObject.properties.schemaVersion.const -ne "0.16.0") {
    throw "Generated and tracked Study Schema identities differ."
}

$initialState = (Invoke-StudyCli study-init $workspaceRoot $manifestPath --json) | ConvertFrom-Json
if ($initialState.revision -ne 0 -or $initialState.completedCount -ne 0) {
    throw "Study workspace did not initialize from a clean revision."
}

$batchPath = Join-Path $validationRoot "study-platform-batch.json"
$resumedPath = Join-Path $validationRoot "study-platform-batch-resumed.json"
$batch = (Invoke-StudyCli study-batch $workspaceRoot --output $batchPath --json | Out-String) | ConvertFrom-Json
$resumed = (Invoke-StudyCli study-batch $workspaceRoot --output $resumedPath --json | Out-String) | ConvertFrom-Json
if ($batch.contentHash -ne $resumed.contentHash -or $batch.batchId -ne $resumed.batchId) {
    throw "Pending Study batch was not resumed exactly."
}
if ($batch.candidates.Count -ne 4) {
    throw "Expected four validation candidates; got $($batch.candidates.Count)."
}

$evaluations = foreach ($candidate in $batch.candidates) {
    $depth = [double]$candidate.values[0]
    $floors = [double]$candidate.values[1]
    $facade = [double]$candidate.values[2]
    $sda = 55.0 + 30.0 * $depth - 2.0 * $facade
    $greenView = 45.0 + 5.0 * $floors + 10.0 * $facade
    $energy = 120.0 - 25.0 * $depth + 3.0 * $floors + 5.0 * $facade
    $lux0 = 500.0 + 800.0 * $depth
    $lux1 = 420.0 + 35.0 * $floors
    $lux2 = 350.0 + 90.0 * $facade
    [ordered]@{
        variantId = [uint64]$candidate.candidateId
        objectiveValues = @(
            @($sda, $greenView),
            @($energy)
        )
        constraintResiduals = @($depth - 0.9)
        metricFields = @(
            [ordered]@{
                fieldId = "daylight.totalLux"
                unit = "lux"
                positions = @(@(0.0, 0.0, 0.8), @(1.0, 0.0, 0.8), @(2.0, 0.0, 0.8))
                values = @($lux0, $lux1, $lux2)
            }
        )
        elapsedMicroseconds = [uint64](1000 + $candidate.candidateId)
        provenanceHash = "validation-$($candidate.candidateId)-0.16"
    }
}
$commitPayload = [ordered]@{ batchId = [uint64]$batch.batchId; evaluations = @($evaluations) }
$commitPath = Join-Path $validationRoot "study-platform-evaluations.json"
[System.IO.File]::WriteAllText(
    $commitPath,
    ($commitPayload | ConvertTo-Json -Depth 12),
    [System.Text.UTF8Encoding]::new($false)
)

$committed = (Invoke-StudyCli study-commit $workspaceRoot $commitPath --json | Out-String) | ConvertFrom-Json
$idempotent = (Invoke-StudyCli study-commit $workspaceRoot $commitPath --json | Out-String) | ConvertFrom-Json
if ($committed.checkpointHash -ne $idempotent.checkpointHash -or
    $committed.completedCount -ne 4 -or
    $committed.activeBatchId) {
    throw "Study commit is not complete and idempotent."
}

$reportRoot = Join-Path $workspaceRoot "report"
$reportResult = (Invoke-StudyCli study-report $workspaceRoot --output $reportRoot --bootstrap 128 --permutations 128 --confidence 0.95 --alpha 0.05 --seed 1701 --json | Out-String) | ConvertFrom-Json
if ($reportResult.schemaVersion -ne "0.16.0" -or $reportResult.variants -ne 4) {
    throw "Study report summary is invalid."
}

$dataPath = Join-Path $reportRoot "study-data.json"
$parquetPath = Join-Path $reportRoot "variants.parquet"
$gltfPath = Join-Path $reportRoot "objective-space.gltf"
$htmlPath = Join-Path $reportRoot "report.html"
$viewerPath = Join-Path $reportRoot "viewer.html"
foreach ($artifact in @($dataPath, $parquetPath, $gltfPath, $htmlPath, $viewerPath)) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Study report artifact is missing: $artifact"
    }
}
$parquetBytes = [System.IO.File]::ReadAllBytes($parquetPath)
$head = [System.Text.Encoding]::ASCII.GetString($parquetBytes, 0, 4)
$tail = [System.Text.Encoding]::ASCII.GetString($parquetBytes, $parquetBytes.Length - 4, 4)
if ($head -ne "PAR1" -or $tail -ne "PAR1") {
    throw "Study table is not an Apache Parquet file."
}
$gltf = Get-Content -LiteralPath $gltfPath -Raw | ConvertFrom-Json
if ($gltf.asset.version -ne "2.0" -or $gltf.meshes.Count -ne 1 -or $gltf.accessors[0].count -ne 4) {
    throw "Study objective-space export is not a valid glTF 2.0 point set."
}
$data = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
$viewer = Get-Content -LiteralPath $viewerPath -Raw
$report = Get-Content -LiteralPath $htmlPath -Raw
if ($data.analysis.sensitivity.Count -eq 0 -or
    $data.analysis.comparisons.Count -ne 3 -or
    -not $viewer.Contains("courtyard-ultra-study") -or
    -not $report.Contains("objective-space.gltf")) {
    throw "Study analysis or standalone HTML payload is incomplete."
}

Write-Host "Study platform smoke passed: resume + idempotent commit + statistical analysis + Parquet/glTF/HTML ($workspaceRoot)."
