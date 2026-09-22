[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$packageRoot = Join-Path $repoRoot 'artifacts\packages'
$manifest = Get-Content -LiteralPath (Join-Path $packageRoot 'release-manifest.json') -Raw | ConvertFrom-Json
$version = $manifest.version
$outputRoot = Join-Path $repoRoot 'artifacts\human-test'
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$outputPath = Join-Path $outputRoot "XVARNA-$version-test.zip"
$temporaryPath = "$outputPath.tmp"
$expectedHashes = @{}

function Add-ArchiveBytes {
    param($Archive, [string]$Name, [byte[]]$Bytes)
    $entry = $Archive.CreateEntry($Name, [IO.Compression.CompressionLevel]::Optimal)
    $stream = $entry.Open()
    try { $stream.Write($Bytes, 0, $Bytes.Length) } finally { $stream.Dispose() }
    $expectedHashes[$Name] = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes))
}

$componentGuide = Get-Content -LiteralPath (Join-Path $repoRoot 'docs\testing-fa\02-components.html') -Raw
if ([regex]::Matches($componentGuide, '<tr data-component>').Count -ne 49) { throw 'Persian guide must describe all 49 components.' }
$examples = @(Get-ChildItem -LiteralPath (Join-Path $repoRoot 'examples\grasshopper') -Filter '*.ghx' -File | Sort-Object Name)
if ($examples.Count -ne 12) { throw 'Expected twelve Grasshopper examples.' }

$outputStream = [IO.File]::Open($temporaryPath, [IO.FileMode]::Create)
$archive = [IO.Compression.ZipArchive]::new($outputStream, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($rhino in @(8, 9)) {
        $sourceName = "xvarna-$version-rhino$rhino-win-x64.zip"
        $record = @($manifest.artifacts | Where-Object name -eq $sourceName)
        if ($record.Count -ne 1) { throw "Source not uniquely present in release manifest: $sourceName" }
        $sourcePath = Join-Path $packageRoot $sourceName
        if ((Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash -ne $record[0].sha256) { throw "Source package checksum mismatch: $sourceName" }
        $source = [IO.Compression.ZipFile]::OpenRead($sourcePath)
        try {
            $names = @('Xvarna.Grasshopper.gha', 'Xvarna.Native.dll', 'xvarna_core.dll')
            if ($rhino -eq 9) { $names += 'Xvarna.Grasshopper2.rhp' }
            foreach ($name in $names) {
                $entry = $source.GetEntry($name)
                if ($null -eq $entry) { throw "Missing binary: $name in $sourceName" }
                $inputStream = $entry.Open()
                $memory = [IO.MemoryStream]::new()
                try {
                    $inputStream.CopyTo($memory)
                    Add-ArchiveBytes $archive "Plugin/Rhino$rhino/$name" $memory.ToArray()
                } finally { $memory.Dispose(); $inputStream.Dispose() }
            }
        } finally { $source.Dispose() }
    }
    foreach ($example in $examples) { Add-ArchiveBytes $archive "Examples/$($example.Name)" ([IO.File]::ReadAllBytes($example.FullName)) }
    foreach ($name in @('01-install.html', '02-components.html')) {
        Add-ArchiveBytes $archive $name ([IO.File]::ReadAllBytes((Join-Path $repoRoot "docs\testing-fa\$name")))
    }
    foreach ($name in @('LICENSE', 'NOTICE')) { Add-ArchiveBytes $archive $name ([IO.File]::ReadAllBytes((Join-Path $repoRoot $name))) }
} finally { $archive.Dispose(); $outputStream.Dispose() }

# Check the finished archive byte-for-byte against its inputs before replacing
# a previous test package. The evidence remains local, outside the tester ZIP.
$verify = [IO.Compression.ZipFile]::OpenRead($temporaryPath)
try {
    if ($verify.Entries.Count -ne 23) { throw "Expected exactly 23 files; found $($verify.Entries.Count)." }
    foreach ($entry in $verify.Entries) {
        if (-not $expectedHashes.ContainsKey($entry.FullName)) { throw "Unexpected tester file: $($entry.FullName)" }
        $stream = $entry.Open()
        try { $actual = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) } finally { $stream.Dispose() }
        if ($actual -ne $expectedHashes[$entry.FullName]) { throw "Tester archive content mismatch: $($entry.FullName)" }
    }
} finally { $verify.Dispose() }
[IO.File]::Move($temporaryPath, $outputPath, $true)
$size = (Get-Item -LiteralPath $outputPath).Length
Write-Host "Simple tester package: $outputPath"
Write-Host "Verified: 7 host binaries, 12 GH1 examples, 2 Persian guides, 2 license/notice files."
Write-Host "Size: $([Math]::Round($size / 1MB, 2)) MiB. No CLI, SDK, Yak, benchmark, SBOM or reviewer paperwork included."
