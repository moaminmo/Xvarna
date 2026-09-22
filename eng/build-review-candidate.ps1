[CmdletBinding()]
param(
    [ValidateSet("Smoke", "Full")][string]$EvidenceProfile = "Smoke",
    [switch]$SkipFullCheck,
    [switch]$IncludeYak,
    [switch]$AllowDirty
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$version = "1.0.0-rc.1"
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $root "artifacts"))
$candidateRoot = [System.IO.Path]::GetFullPath((Join-Path $artifactRoot "review-candidate\$version"))
if (-not $candidateRoot.StartsWith(($artifactRoot.TrimEnd('\') + '\'), [StringComparison]::OrdinalIgnoreCase)) { throw "Unsafe candidate path." }
$dirtyBefore = -not [string]::IsNullOrWhiteSpace((& git -C $root status --porcelain | Out-String))
if ($dirtyBefore -and -not $AllowDirty) { throw "Review candidates must be built from a clean checkout. Commit or stash changes, or use -AllowDirty only for script validation." }

if (-not $SkipFullCheck) {
    & (Join-Path $PSScriptRoot "check.ps1")
    if ($LASTEXITCODE -ne 0) { throw "Full check failed." }
}
& (Join-Path $PSScriptRoot "package.ps1") -Configuration Release -SkipYak:(-not $IncludeYak)
if ($LASTEXITCODE -ne 0) { throw "Packaging failed." }
& (Join-Path $PSScriptRoot "run-phase2-evidence.ps1") -Profile $EvidenceProfile
if ($LASTEXITCODE -ne 0) { throw "Evidence capture failed." }
& (Join-Path $PSScriptRoot "generate-sbom.ps1")
if ($LASTEXITCODE -ne 0) { throw "SBOM generation failed." }

if (Test-Path -LiteralPath $candidateRoot) { Remove-Item -LiteralPath $candidateRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $candidateRoot | Out-Null
$packages = Join-Path $root "artifacts\packages"
Copy-Item -LiteralPath (Join-Path $packages "release-manifest.json"), (Join-Path $packages "SHA256SUMS.txt") -Destination $candidateRoot -Force
$packageManifest = Get-Content -LiteralPath (Join-Path $packages "release-manifest.json") -Raw | ConvertFrom-Json -Depth 32
foreach ($artifact in $packageManifest.artifacts) {
    $sourceArtifact = Join-Path $packages $artifact.name
    if (-not (Test-Path -LiteralPath $sourceArtifact -PathType Leaf)) { throw "Manifest artifact is missing: $($artifact.name)" }
    $sourceHash = (Get-FileHash -LiteralPath $sourceArtifact -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($sourceHash -ne $artifact.sha256) { throw "Manifest artifact hash mismatch: $($artifact.name)" }
    Copy-Item -LiteralPath $sourceArtifact -Destination $candidateRoot -Force
}
Copy-Item -LiteralPath (Join-Path $root "artifacts\acceptance\phase-1") -Destination (Join-Path $candidateRoot "evidence\phase-1") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root "artifacts\acceptance\phase-2") -Destination (Join-Path $candidateRoot "evidence\phase-2") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root "artifacts\release\$version\xvarna.cdx.json") -Destination $candidateRoot -Force
Copy-Item -LiteralPath (Join-Path $root "docs\beta") -Destination (Join-Path $candidateRoot "review") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root "docs\validation\claims-register.md"), (Join-Path $root "docs\validation\benchmark-methodology.md") -Destination (Join-Path $candidateRoot "review") -Force
Copy-Item -LiteralPath (Join-Path $root "case-studies") -Destination (Join-Path $candidateRoot "case-studies") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root "docs\release\go-no-go.template.md") -Destination (Join-Path $candidateRoot "go-no-go.md") -Force

Copy-Item -LiteralPath (Join-Path $root "docs\beta\beta-kpis.template.json") -Destination (Join-Path $candidateRoot "evidence\phase-2\beta-kpis.json") -Force
Copy-Item -LiteralPath (Join-Path $root "docs\beta\usability-study.md"), (Join-Path $root "docs\beta\reviewer-signoff.md") -Destination (Join-Path $candidateRoot "evidence\phase-2") -Force
$commit = (& git -C $root rev-parse HEAD).Trim()
$readiness = [ordered]@{
    schemaVersion = "1.0.0"; product = "XVARNA"; candidate = $version; generatedUtc = [DateTimeOffset]::UtcNow.ToString("O")
    sourceCommit = $commit; sourceDirtyAtStart = $dirtyBefore; evidenceProfile = $EvidenceProfile
    automated = [ordered]@{ buildAndTests = if ($SkipFullCheck) { "NOT_RUN_BY_THIS_COMMAND" } else { "PASS" }; packages = "PASS"; phase2EvidencePipeline = "PASS"; sbom = "PASS"; checksums = "PASS" }
    external = [ordered]@{ interactiveHostReview = "DEFERRED_BY_OWNER"; amdHardware = "PENDING"; beta20Testers10Projects = "PENDING"; independentSignoff = "PENDING"; fourteenDaySoak = "PENDING"; signing = "PENDING"; doiWebsiteAndPublication = "PENDING" }
    decision = "READY_TO_SEND_FOR_EXTERNAL_REVIEW_NOT_PUBLIC_RELEASE"
}
$readiness | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $candidateRoot "review-readiness.json") -Encoding utf8NoBOM

$innerFiles = Get-ChildItem -LiteralPath $candidateRoot -File -Recurse | Sort-Object FullName
$innerSums = foreach ($file in $innerFiles) { "$((Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($file.FullName.Substring($candidateRoot.Length + 1).Replace('\','/'))" }
Set-Content -LiteralPath (Join-Path $candidateRoot "REVIEW-CANDIDATE-SHA256SUMS.txt") -Value $innerSums -Encoding utf8NoBOM
$archive = Join-Path (Split-Path -Parent $candidateRoot) "xvarna-$version-review-candidate-win-x64.zip"
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -Path (Join-Path $candidateRoot "*") -DestinationPath $archive -CompressionLevel Optimal
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath "$archive.sha256" -Value "$archiveHash  $([System.IO.Path]::GetFileName($archive))" -Encoding utf8NoBOM
& (Join-Path $PSScriptRoot "verify-review-candidate.ps1") -Archive $archive
if ($LASTEXITCODE -ne 0) { throw "Review-candidate verification failed." }
Write-Host "Review candidate: $archive"
Write-Host "Decision: READY_TO_SEND_FOR_EXTERNAL_REVIEW_NOT_PUBLIC_RELEASE"
