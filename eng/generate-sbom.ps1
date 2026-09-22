[CmdletBinding()]
param([string]$OutputPath = "")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($OutputPath)) { $OutputPath = Join-Path $root "artifacts\release\1.0.0-rc.1\xvarna.cdx.json" }
$output = [System.IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $output) | Out-Null

$cargoText = (& cargo metadata --format-version 1 --locked --manifest-path (Join-Path $root "Cargo.toml") | Out-String)
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed." }
$cargo = $cargoText | ConvertFrom-Json -Depth 64
$dotnetText = (& dotnet list (Join-Path $root "Xvarna.sln") package --include-transitive --format json | Out-String)
if ($LASTEXITCODE -ne 0) { throw "dotnet package inventory failed." }
$dotnet = $dotnetText | ConvertFrom-Json -Depth 32

$components = [System.Collections.Generic.List[object]]::new()
$seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($package in $cargo.packages) {
    $key = "cargo:$($package.name):$($package.version)"; if (-not $seen.Add($key)) { continue }
    $component = [ordered]@{ type = "library"; "bom-ref" = $key; group = ""; name = $package.name; version = $package.version; purl = "pkg:cargo/$([Uri]::EscapeDataString($package.name))@$($package.version)" }
    if (-not [string]::IsNullOrWhiteSpace($package.license)) { $component.licenses = @([ordered]@{ expression = $package.license }) }
    if (-not [string]::IsNullOrWhiteSpace($package.repository)) { $component.externalReferences = @([ordered]@{ type = "vcs"; url = $package.repository }) }
    $components.Add($component)
}
foreach ($project in $dotnet.projects) {
    foreach ($framework in $project.frameworks) {
        $topLevel = if ($framework.PSObject.Properties.Name -contains "topLevelPackages") { @($framework.topLevelPackages) } else { @() }
        $transitive = if ($framework.PSObject.Properties.Name -contains "transitivePackages") { @($framework.transitivePackages) } else { @() }
        foreach ($package in @($topLevel) + @($transitive)) {
            if ($null -eq $package) { continue }
            $key = "nuget:$($package.id):$($package.resolvedVersion)"; if (-not $seen.Add($key)) { continue }
            $components.Add([ordered]@{ type = "library"; "bom-ref" = $key; group = ""; name = $package.id; version = $package.resolvedVersion; purl = "pkg:nuget/$([Uri]::EscapeDataString($package.id))@$($package.resolvedVersion)" })
        }
    }
}
$commit = (& git -C $root rev-parse HEAD).Trim()
$serial = "urn:uuid:$([Guid]::NewGuid())"
$document = [ordered]@{
    bomFormat = "CycloneDX"; specVersion = "1.6"; serialNumber = $serial; version = 1
    metadata = [ordered]@{
        timestamp = [DateTimeOffset]::UtcNow.ToString("O")
        tools = [ordered]@{ components = @([ordered]@{ type = "application"; name = "XVARNA SBOM generator"; version = "1.0.0-rc.1" }) }
        component = [ordered]@{ type = "application"; "bom-ref" = "pkg:github/xvarna-labs/xvarna@1.0.0-rc.1"; name = "XVARNA"; version = "1.0.0-rc.1"; purl = "pkg:github/xvarna-labs/xvarna@1.0.0-rc.1"; properties = @([ordered]@{ name = "xvarna:sourceCommit"; value = $commit }) }
    }
    components = @($components | Sort-Object name, version)
}
$document | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $output -Encoding utf8NoBOM
$hash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath "$output.sha256" -Value "$hash  $([System.IO.Path]::GetFileName($output))" -Encoding utf8NoBOM
Write-Host "CycloneDX SBOM: $output ($($components.Count) components)"
