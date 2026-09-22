[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug"
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Enable-XvarnaLocalRustToolchain
$rustTarget = Get-XvarnaRustTarget
$cargoArguments = @("build", "--locked", "-p", "xvarna-ffi", "--target", $rustTarget)
if ($Configuration -eq "Release") {
    $cargoArguments += "--release"
}

Push-Location $repositoryRoot
try {
    Invoke-XvarnaCommand cargo @cargoArguments

    $profile = $Configuration.ToLowerInvariant()
    $source = Join-Path $repositoryRoot "target\$rustTarget\$profile\xvarna_core.dll"
    $destination = Join-Path $repositoryRoot "artifacts\native\win-x64"
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination "xvarna_core.dll") -Force
    Write-Host "Native engine: $source"
}
finally {
    Pop-Location
}
