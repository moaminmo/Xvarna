[CmdletBinding()]
param()

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repositoryRoot
try {
    Invoke-XvarnaCommand rustup toolchain install 1.96.0 --profile default --component clippy rustfmt
    $rustTarget = Get-XvarnaRustTarget
    Invoke-XvarnaCommand rustup target add $rustTarget --toolchain 1.96.0
    Invoke-XvarnaCommand dotnet restore Xvarna.sln
    Write-Host "XVARNA bootstrap complete. Native target: $rustTarget"
}
finally {
    Pop-Location
}
