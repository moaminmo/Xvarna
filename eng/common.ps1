Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Enable-XvarnaLocalRustToolchain {
    # rustup names `stable` and `1.96.0` separately even when both resolve to the
    # identical compiler. Reuse the already-installed stable toolchain only when
    # its exact compiler version matches the repository pin.
    $stableVersion = & rustc +stable --version 2>$null
    if ($LASTEXITCODE -eq 0 -and $stableVersion -match '^rustc 1\.96\.0\s') {
        $env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
    }
}

function Get-XvarnaRustTarget {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere) {
        $installation = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($installation)) {
            return "x86_64-pc-windows-msvc"
        }
    }

    if (Get-Command gcc.exe -ErrorAction SilentlyContinue) {
        return "x86_64-pc-windows-gnu"
    }

    throw "No supported Windows C/C++ linker was found. Install Visual Studio Build Tools with the C++ workload."
}

function Invoke-XvarnaCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Arguments
    )

    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE`: $Executable $($Arguments -join ' ')"
    }
}
