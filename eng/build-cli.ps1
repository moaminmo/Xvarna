[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug"
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Enable-XvarnaLocalRustToolchain
$rustTarget = Get-XvarnaRustTarget
$cargoArguments = @("build", "--locked", "-p", "xvarna-cli", "--target", $rustTarget)
if ($Configuration -eq "Release") {
    $cargoArguments += "--release"
}

Push-Location $repositoryRoot
try {
    Invoke-XvarnaCommand cargo @cargoArguments
    $profile = $Configuration.ToLowerInvariant()
    $source = Join-Path $repositoryRoot "target\$rustTarget\$profile\xvarna.exe"
    $destination = Join-Path $repositoryRoot "artifacts\cli\win-x64"
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination "xvarna.exe") -Force
    Write-Host "CLI: $source"
}
finally {
    Pop-Location
}
