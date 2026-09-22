[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [switch]$SkipYak
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$packageRoot = Join-Path $repositoryRoot "artifacts\packages"
$stageRoot = Join-Path $repositoryRoot "artifacts\yak-stage"
$nativeEngine = Join-Path $repositoryRoot "artifacts\native\win-x64\xvarna_core.dll"
$version = "1.0.0-rc.1"
$validationFiles = @(
    "annual-irradiance.md",
    "surface-intelligence-and-pv-proxy.md",
    "spatial-visibility-and-privacy.md",
    "advanced-view-and-optimization.md",
    "portable-gpu.md",
    "vayu-domain-acceleration.md",
    "vayu-production-throughput.md",
    "zamyad-vayu-lifecycle.md",
    "daena-spatial-solar-intelligence.md",
    "daylight-radiance.md",
    "study-platform.md",
    "core-time-interoperability.md",
    "product-experience-release-quality.md",
    "evidence-multifidelity.md"
    "grasshopper2-connector.md"
)

$evidenceSchemaFiles = @(
    "evidence-passport.schema.json",
    "global-sensitivity-design.schema.json"
)

$evidenceExampleFiles = @(
    "multifidelity.json",
    "passport.json",
    "robust.json",
    "uncertainty.json",
    "variables.json"
)

function Assert-ChildPath {
    param([string]$Parent, [string]$Target)
    $separator = [System.IO.Path]::DirectorySeparatorChar
    $parentFull = [System.IO.Path]::GetFullPath($Parent).TrimEnd($separator) + $separator
    $targetFull = [System.IO.Path]::GetFullPath($Target)
    if (-not $targetFull.StartsWith($parentFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing filesystem operation outside '$parentFull': $targetFull"
    }
    return $targetFull
}

function Reset-PackageDirectory {
    param([string]$Parent, [string]$Target)
    $safeTarget = Assert-ChildPath -Parent $Parent -Target $Target
    if (Test-Path -LiteralPath $safeTarget) {
        Remove-Item -LiteralPath $safeTarget -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $safeTarget | Out-Null
    return $safeTarget
}

function Copy-ValidationBundle {
    param([string]$Destination)
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    foreach ($file in $validationFiles) {
        Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\validation\$file") -Destination $Destination -Force
    }
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "schemas\xvarna-scene-1.schema.json") -Destination $Destination -Force
    foreach ($file in $evidenceSchemaFiles) {
        Copy-Item -LiteralPath (Join-Path $repositoryRoot "schemas\$file") -Destination $Destination -Force
    }
}

function Copy-StudyBundle {
    param([string]$Destination)
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "schemas\xvarna-study-0.16.schema.json") -Destination $Destination -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\study\platform-manifest.json") -Destination $Destination -Force
}

function New-Archive {
    param([string]$SourceDirectory, [string]$Archive)
    if (Test-Path -LiteralPath $Archive) { Remove-Item -LiteralPath $Archive -Force }
    Compress-Archive -Path (Join-Path $SourceDirectory "*") -DestinationPath $Archive -CompressionLevel Optimal
}

& "$PSScriptRoot\build.ps1" -Configuration $Configuration
if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE." }
New-Item -ItemType Directory -Force -Path $packageRoot | Out-Null
Get-ChildItem -LiteralPath $packageRoot -File | Where-Object {
    $_.Name -like "xvarna-$version-*" -and $_.Extension -in ".zip", ".yak"
} | ForEach-Object {
    [void](Assert-ChildPath -Parent $packageRoot -Target $_.FullName)
    Remove-Item -LiteralPath $_.FullName -Force
}

$rhino9Yak = if (Test-Path -LiteralPath "C:\Program Files\Rhino 9\System\Yak.exe") {
    "C:\Program Files\Rhino 9\System\Yak.exe"
} else {
    "C:\Program Files\Rhino 9 WIP\System\Yak.exe"
}
$targets = @(
    @{ Name = "xvarna-$version-rhino8-win-x64"; Framework = "net8.0"; Rhino = 8; Tag = "rh8"; Yak = "C:\Program Files\Rhino 8\System\Yak.exe" },
    @{ Name = "xvarna-$version-rhino9-win-x64"; Framework = "net10.0"; Rhino = 9; Tag = "rh9"; Yak = $rhino9Yak }
)

foreach ($target in $targets) {
    $source = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\bin\$Configuration\$($target.Framework)"
    $destination = Reset-PackageDirectory -Parent $packageRoot -Target (Join-Path $packageRoot $target.Name)
    Copy-Item -LiteralPath (Join-Path $source "Xvarna.Grasshopper.gha") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $source "Xvarna.Native.dll") -Destination $destination -Force
    if ($target.Rhino -eq 9) {
        $grasshopper2Source = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper2\bin\$Configuration\net10.0"
        Copy-Item -LiteralPath (Join-Path $grasshopper2Source "Xvarna.Grasshopper2.rhp") -Destination $destination -Force
    }
    Copy-Item -LiteralPath $nativeEngine -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "LICENSE") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "NOTICE") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "CHANGELOG.md") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\installation-rhino.md") -Destination (Join-Path $destination "README.md") -Force
    Copy-ValidationBundle -Destination (Join-Path $destination "validation")
    Copy-StudyBundle -Destination (Join-Path $destination "study")
    if (Test-Path -LiteralPath (Join-Path $repositoryRoot "examples\grasshopper")) {
        Copy-Item -LiteralPath (Join-Path $repositoryRoot "examples\grasshopper") -Destination (Join-Path $destination "examples") -Recurse -Force
    }
    if ($target.Rhino -eq 9 -and (Test-Path -LiteralPath (Join-Path $repositoryRoot "examples\grasshopper2"))) {
        Copy-Item -LiteralPath (Join-Path $repositoryRoot "examples\grasshopper2") -Destination (Join-Path $destination "examples\grasshopper2") -Recurse -Force
    }
    New-Archive -SourceDirectory $destination -Archive (Join-Path $packageRoot "$($target.Name).zip")

    if (-not $SkipYak) {
        if (-not (Test-Path -LiteralPath $target.Yak -PathType Leaf)) {
            throw "Rhino $($target.Rhino) Yak.exe not found at '$($target.Yak)'. Use -SkipYak only on machines without Rhino."
        }
        $stage = Reset-PackageDirectory -Parent $stageRoot -Target (Join-Path $stageRoot "rhino$($target.Rhino)")
        Copy-Item -Path (Join-Path $destination "*") -Destination $stage -Recurse -Force
        Copy-Item -LiteralPath (Join-Path $repositoryRoot "packaging\yak\manifest.template.yml") -Destination (Join-Path $stage "manifest.yml") -Force
        Push-Location $stage
        try {
            & $target.Yak build --platform win
            if ($LASTEXITCODE -ne 0) { throw "Yak build failed for Rhino $($target.Rhino)." }
        } finally {
            Pop-Location
        }
        $yakFiles = @(Get-ChildItem -LiteralPath $stage -Filter "*.yak" -File)
        if ($yakFiles.Count -ne 1) { throw "Expected one Yak artifact for Rhino $($target.Rhino); found $($yakFiles.Count)." }
        Copy-Item -LiteralPath $yakFiles[0].FullName -Destination (Join-Path $packageRoot $yakFiles[0].Name) -Force
    }
}

$cliName = "xvarna-$version-cli-win-x64"
$cliDestination = Reset-PackageDirectory -Parent $packageRoot -Target (Join-Path $packageRoot $cliName)
Copy-Item -LiteralPath (Join-Path $repositoryRoot "artifacts\cli\win-x64\xvarna.exe") -Destination $cliDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "LICENSE") -Destination $cliDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "NOTICE") -Destination $cliDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "CHANGELOG.md") -Destination $cliDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\cli.md") -Destination (Join-Path $cliDestination "README.md") -Force
Copy-ValidationBundle -Destination (Join-Path $cliDestination "validation")
$cliExamples = Join-Path $cliDestination "examples"
New-Item -ItemType Directory -Force -Path $cliExamples | Out-Null
Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\weather\tehran-mini.epw") -Destination $cliExamples -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\meshes\open_quad.obj") -Destination $cliExamples -Force
Copy-StudyBundle -Destination (Join-Path $cliExamples "study")
$evidenceExample = Join-Path $cliExamples "evidence"
New-Item -ItemType Directory -Force -Path $evidenceExample | Out-Null
foreach ($file in $evidenceExampleFiles) {
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\evidence\$file") -Destination $evidenceExample -Force
}
$daylightExample = Join-Path $cliExamples "daylight"
New-Item -ItemType Directory -Force -Path $daylightExample | Out-Null
Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\daylight\fast-lux.txt") -Destination $daylightExample -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "validation\daylight\radiance-reference-lux.txt") -Destination $daylightExample -Force
New-Archive -SourceDirectory $cliDestination -Archive (Join-Path $packageRoot "$cliName.zip")

$sdkName = "xvarna-$version-sdk-win-x64"
$sdkDestination = Reset-PackageDirectory -Parent $packageRoot -Target (Join-Path $packageRoot $sdkName)
$sdkInclude = Join-Path $sdkDestination "include"
$sdkNative = Join-Path $sdkDestination "native\win-x64"
$sdkNet8 = Join-Path $sdkDestination "managed\net8.0"
$sdkNet10 = Join-Path $sdkDestination "managed\net10.0"
New-Item -ItemType Directory -Force -Path $sdkInclude, $sdkNative, $sdkNet8, $sdkNet10 | Out-Null
Copy-Item -LiteralPath (Join-Path $repositoryRoot "include\xvarna.h") -Destination $sdkInclude -Force
Copy-Item -LiteralPath $nativeEngine -Destination $sdkNative -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "dotnet\Xvarna.Native\bin\$Configuration\net8.0\Xvarna.Native.dll") -Destination $sdkNet8 -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "dotnet\Xvarna.Native\bin\$Configuration\net10.0\Xvarna.Native.dll") -Destination $sdkNet10 -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "LICENSE") -Destination $sdkDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "NOTICE") -Destination $sdkDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "CHANGELOG.md") -Destination $sdkDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\sdk.md") -Destination (Join-Path $sdkDestination "README.md") -Force
Copy-ValidationBundle -Destination (Join-Path $sdkDestination "validation")
Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\sdk\daylight.md") -Destination $sdkDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\sdk\study-platform.md") -Destination $sdkDestination -Force
Copy-Item -LiteralPath (Join-Path $repositoryRoot "docs\sdk\evidence.md") -Destination $sdkDestination -Force
Copy-StudyBundle -Destination (Join-Path $sdkDestination "study")
New-Archive -SourceDirectory $sdkDestination -Archive (Join-Path $packageRoot "$sdkName.zip")

$artifacts = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object {
    $_.Name -like "xvarna-$version-*" -and $_.Extension -in ".zip", ".yak"
} | Sort-Object Name)
$checksumLines = $artifacts | ForEach-Object {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
Set-Content -LiteralPath (Join-Path $packageRoot "SHA256SUMS.txt") -Value $checksumLines -Encoding utf8NoBOM

$commit = (& git -C $repositoryRoot rev-parse HEAD).Trim()
$dirty = -not [string]::IsNullOrWhiteSpace((& git -C $repositoryRoot status --porcelain | Out-String))
$artifactRecords = @($artifacts | ForEach-Object {
    [ordered]@{
        name = $_.Name
        bytes = $_.Length
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
    }
})
$releaseManifest = [ordered]@{
    schemaVersion = "1.0.0"
    product = "XVARNA"
    version = $version
    channel = "local-release-candidate"
    published = $false
    builtUtc = [DateTimeOffset]::UtcNow.ToString("O")
    sourceCommit = $commit
    sourceDirty = $dirty
    compatibility = [ordered]@{
        os = "Windows x64"
        rhino = @("8.20+", "9")
        grasshopper = @("1 on Rhino 8/9", "2 on Rhino 9")
        dotnet = @("8.0", "10.0")
        grasshopper2Sdk = "9.0.26237.15343-beta"
    }
    studySchemaVersion = "0.16.0"
    artifacts = $artifactRecords
}
$releaseManifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $packageRoot "release-manifest.json") -Encoding utf8NoBOM
Write-Host "Packages created in $packageRoot"
Write-Host "Artifacts: $($artifacts.Count); source commit: $commit; dirty: $dirty"
