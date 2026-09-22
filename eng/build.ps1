[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug"
)

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
& "$PSScriptRoot\build-native.ps1" -Configuration $Configuration
if ($LASTEXITCODE -ne 0) {
    throw "Native build failed with exit code $LASTEXITCODE."
}
& "$PSScriptRoot\build-cli.ps1" -Configuration $Configuration
if ($LASTEXITCODE -ne 0) {
    throw "CLI build failed with exit code $LASTEXITCODE."
}

Push-Location $repositoryRoot
try {
    Invoke-XvarnaCommand dotnet build Xvarna.sln --configuration $Configuration --no-restore

    $nativeEngine = Join-Path $repositoryRoot "artifacts\native\win-x64\xvarna_core.dll"
    foreach ($framework in @("net8.0", "net10.0")) {
        $pluginDirectory = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper\bin\$Configuration\$framework"
        Copy-Item -LiteralPath $nativeEngine -Destination (Join-Path $pluginDirectory "xvarna_core.dll") -Force
    }
    $grasshopper2Directory = Join-Path $repositoryRoot "dotnet\Xvarna.Grasshopper2\bin\$Configuration\net10.0"
    if (Test-Path -LiteralPath $grasshopper2Directory) {
        Copy-Item -LiteralPath $nativeEngine -Destination (Join-Path $grasshopper2Directory "xvarna_core.dll") -Force
    }
    Write-Host "Managed connectors built for Rhino 8/GH1 net8.0 and Rhino 9/GH1+GH2 net10.0."
}
finally {
    Pop-Location
}
