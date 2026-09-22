[CmdletBinding()]
param()

. "$PSScriptRoot\common.ps1"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Enable-XvarnaLocalRustToolchain
$rustTarget = Get-XvarnaRustTarget

Push-Location $repositoryRoot
try {
    $formatArguments = @("fmt", "--all", "--", "--check")
    Invoke-XvarnaCommand cargo @formatArguments
    $clippyArguments = @("clippy", "--workspace", "--all-targets", "--target", $rustTarget, "--", "-D", "warnings")
    Invoke-XvarnaCommand cargo @clippyArguments
    Invoke-XvarnaCommand cargo test --workspace --target $rustTarget
    & "$PSScriptRoot\build-native.ps1" -Configuration Debug
    if ($LASTEXITCODE -ne 0) {
        throw "Native build failed with exit code $LASTEXITCODE."
    }
    & "$PSScriptRoot\build-cli.ps1" -Configuration Debug
    if ($LASTEXITCODE -ne 0) {
        throw "CLI build failed with exit code $LASTEXITCODE."
    }
    $validationOutput = Join-Path $repositoryRoot "artifacts\validation"
    New-Item -ItemType Directory -Force -Path $validationOutput | Out-Null
    $gcc = Get-Command gcc -ErrorAction SilentlyContinue
    if ($null -ne $gcc) {
        $abiLayoutExecutable = Join-Path $validationOutput "abi-layout.exe"
        $gccArguments = @(
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-I",
            (Join-Path $repositoryRoot "include"),
            (Join-Path $repositoryRoot "validation\c\abi_layout.c"),
            "-o",
            $abiLayoutExecutable
        )
        # GCC's cc1 lives outside mingw64\bin; put the matching runtime DLLs
        # first so another toolchain's libstdc++/iconv cannot be selected.
        $originalPath = $env:PATH
        try {
            $gccRuntime = Split-Path -Parent $gcc.Source
            $env:PATH = "$gccRuntime;$originalPath"
            Invoke-XvarnaCommand $gcc.Source @gccArguments
            Invoke-XvarnaCommand $abiLayoutExecutable
        }
        finally {
            $env:PATH = $originalPath
        }
    }
    $cli = Join-Path $repositoryRoot "artifacts\cli\win-x64\xvarna.exe"
    Invoke-XvarnaCommand $cli devices --json
    Invoke-XvarnaCommand $cli cache status --json
    & "$PSScriptRoot\validate-core-interop.ps1" -Cli $cli
    if ($LASTEXITCODE -ne 0) {
        throw "Core/time/interoperability validation failed with exit code $LASTEXITCODE."
    }
    Invoke-XvarnaCommand $cli backend-benchmark `
        "validation\meshes\open_quad.obj" `
        --backend auto `
        --rays 4096 `
        --max-rays-per-dispatch 2048 `
        --geometry-chunk-mb 1 `
        --gpu-memory-mb 64 `
        --warmup 1 `
        --iterations 2 `
        --json
    & "$PSScriptRoot\validate-vayu-domain.ps1" -Backend auto -MaximumRaysPerDispatch 64
    if ($LASTEXITCODE -ne 0) {
        throw "VAYU domain validation failed with exit code $LASTEXITCODE."
    }
    Invoke-XvarnaCommand $cli annual-irradiance `
        "validation\meshes\open_quad.obj" `
        "validation\weather\tehran-mini.epw" `
        --sensor "0.5,0.5,1,0,0,1" `
        --timeline-csv (Join-Path $validationOutput "annual-smoke.csv") `
        --json
    Invoke-XvarnaCommand $cli daylight-point `
        "validation\meshes\open_quad.obj" `
        --sensor "1,0.5,0.5,1,0,0,1,1" `
        --moment "1718966400,0,0,1,50000,10000" `
        --patches 64 `
        --json
    Invoke-XvarnaCommand $cli daylight-factor `
        "validation\meshes\open_quad.obj" `
        --sensor "1,0.5,0.5,1,0,0,1,1" `
        --patches 64 `
        --json
    Invoke-XvarnaCommand $cli annual-daylight `
        "validation\meshes\open_quad.obj" `
        "validation\weather\tehran-mini.epw" `
        --sensor "1,0.5,0.5,1,0,0,1,1" `
        --patches 64 `
        --timeline-csv (Join-Path $validationOutput "annual-daylight-smoke.csv") `
        --json
    Invoke-XvarnaCommand $cli radiance-export `
        "validation\meshes\open_quad.obj" `
        (Join-Path $validationOutput "radiance-bundle") `
        --sensor "1,0.5,0.5,1,0,0,1,1"
    $radianceCandidates = @("C:\Radiance\bin")
    $rtrace = Get-Command rtrace -ErrorAction SilentlyContinue
    if ($null -ne $rtrace) {
        $radianceCandidates += Split-Path -Parent $rtrace.Source
    }
    $radianceBin = $radianceCandidates | Where-Object {
        -not [string]::IsNullOrWhiteSpace($_) -and
        (Test-Path -LiteralPath (Join-Path $_ "rtrace.exe"))
    } | Select-Object -First 1
    if ($null -ne $radianceBin) {
        Invoke-XvarnaCommand $cli radiance-point `
            "validation\meshes\open_quad.obj" `
            --sensor "1,0.5,0.5,1,0,0,1,1" `
            --moment "1718966400,0,0,1,50000,10000" `
            --radiance-bin $radianceBin `
            --json
    }
    else {
        Write-Host "Radiance execution skipped: rtrace.exe was not found."
    }
    Invoke-XvarnaCommand $cli daylight-compare `
        "validation\daylight\fast-lux.txt" `
        "validation\daylight\radiance-reference-lux.txt" `
        --absolute-lux 1 `
        --relative 0.01 `
        --json
    Invoke-XvarnaCommand $cli surface-grid `
        "validation\meshes\open_quad.obj" `
        (Join-Path $validationOutput "surface-grid-smoke.csv") `
        --cell-size 0.25 `
        --mesh-obj (Join-Path $validationOutput "surface-grid-smoke.obj") `
        --json
    Invoke-XvarnaCommand $cli isovist `
        "validation\meshes\open_quad.obj" `
        --viewpoint "0.5,0.5,1,0,0,1,1,0,0" `
        --samples 360 `
        --max-distance 10 `
        --boundary-csv (Join-Path $validationOutput "isovist-smoke.csv") `
        --json
    Invoke-XvarnaCommand $cli intervisibility `
        "validation\meshes\open_quad.obj" `
        --observer "0.5,0.5,1,1" `
        --target "0.5,0.5,-1,1" `
        --matrix-csv (Join-Path $validationOutput "intervisibility-smoke.csv") `
        --json
    Invoke-XvarnaCommand $cli visibility-graph `
        "validation\meshes\open_quad.obj" `
        --node "0.25,0.25,1" `
        --node "0.75,0.75,1" `
        --node "0.5,0.5,-1" `
        --pairs-csv (Join-Path $validationOutput "visibility-pairs-smoke.csv") `
        --metrics-csv (Join-Path $validationOutput "visibility-metrics-smoke.csv") `
        --json
    Invoke-XvarnaCommand $cli visibility-graph `
        "validation\meshes\open_quad.obj" `
        --node "0,0,1" `
        --node "1,0,1" `
        --node "2,0,1" `
        --node "3,0,1" `
        --sparse `
        --max-distance 1.1 `
        --max-neighbors 2 `
        --json
    Invoke-XvarnaCommand $cli isovist-3d `
        "validation\meshes\open_quad.obj" `
        --viewpoint "0.5,0.5,1" `
        --samples 64 `
        --max-distance 10 `
        --top-k 2 `
        --counterfactual 1 `
        --json
    Invoke-XvarnaCommand $cli landmark-visibility `
        "validation\meshes\open_quad.obj" `
        --observer "1,0.5,0.5,1,0,0,-1,0,1,0" `
        --landmark "2,0.5,0.5,-1,0.1,1" `
        --samples 8 `
        --json
    Invoke-XvarnaCommand $cli solar-scenarios `
        "validation\meshes\open_quad.obj" `
        --sensor "1,0.5,0.5,-1,0,0,1" `
        --sun "0,0,0,1,1,1" `
        --scenario 1 `
        --scenario 2 `
        --material "5,0.5,0.4,0.1" `
        --assign "1,5" `
        --json
    Invoke-XvarnaCommand $cli solar-envelope `
        "validation\meshes\open_quad.obj" `
        --sensor "1,0.5,0.5,0,0,0,1" `
        --candidate "1,2.5,0.5,0" `
        --sun "0,1,0,1,1,1" `
        --radius 0.1 `
        --preserve 0.8 `
        --shade 0.5 `
        --json
    Invoke-XvarnaCommand $cli target-view `
        "validation\meshes\open_quad.obj" `
        --observer "1,0,0,1,1,0,0,0,0,1,1" `
        --target "7,2,-1,0,2,0,2,2,1,0,1,1" `
        --green-mask 1 `
        --two-sided `
        --backend auto `
        --max-rays-per-dispatch 16 `
        --verify-cpu `
        --json
    Invoke-XvarnaCommand $cli view-corridor `
        "validation\meshes\open_quad.obj" `
        --corridor "1,0.5,0.5,1,0.5,0.5,-1,0,1,0,0.25" `
        --samples 128 `
        --backend auto `
        --max-rays-per-dispatch 32 `
        --verify-cpu `
        --json
    Invoke-XvarnaCommand $cli observer-path `
        "validation\meshes\open_quad.obj" `
        --path "1,0,0,1,1;0,0,1;1,0,1" `
        --target "7,2,-1,0,2,0,2,2,1,0,1,1" `
        --green-mask 1 `
        --two-sided `
        --backend auto `
        --max-rays-per-dispatch 16 `
        --verify-cpu `
        --json
    Invoke-XvarnaCommand $cli study-rank `
        --variable "1,continuous,0,1" `
        --direction min `
        --direction max `
        --candidate "1,0;0.2;0.5,0.8;" `
        --candidate "2,0;0.7;0.6,0.9;" `
        --reference "1,0" `
        --json
    Invoke-XvarnaCommand $cli optimizer-benchmark --population 16 --generations 3 --seed 42 --json
    & "$PSScriptRoot\smoke-study-platform.ps1" -Cli $cli
    if ($LASTEXITCODE -ne 0) {
        throw "Study platform smoke failed with exit code $LASTEXITCODE."
    }
    & "$PSScriptRoot\smoke-evidence.ps1" -Cli $cli
    if ($LASTEXITCODE -ne 0) {
        throw "Scientific evidence and multi-fidelity smoke failed with exit code $LASTEXITCODE."
    }
    Invoke-XvarnaCommand dotnet build Xvarna.sln --configuration Debug --no-restore
    Invoke-XvarnaCommand dotnet test dotnet\Xvarna.Tests\Xvarna.Tests.csproj --configuration Debug --no-build
    & "$PSScriptRoot\release-quality.ps1" `
        -Cli $cli `
        -Iterations 4 `
        -MutationCount 8 `
        -CollectManagedCoverage `
        -CoverageConfiguration Debug `
        -MinimumManagedLineRate 0.85
    if ($LASTEXITCODE -ne 0) {
        throw "Release-quality soak/fuzz/coverage gate failed with exit code $LASTEXITCODE."
    }
    Invoke-XvarnaCommand dotnet format Xvarna.sln --verify-no-changes --no-restore --verbosity minimal
    $rhino8 = "C:\Program Files\Rhino 8\System\RhinoCommon.dll"
    if (Test-Path -LiteralPath $rhino8) {
        & "$PSScriptRoot\test-rhino8-compat.ps1" -Configuration Debug
        if ($LASTEXITCODE -ne 0) {
            throw "Rhino 8 runtime compatibility failed with exit code $LASTEXITCODE."
        }
    }
    else {
        Write-Host "Rhino 8 runtime compatibility skipped: host not installed."
    }
    $rhino9Root = @(
        "C:\Program Files\Rhino 9"
        "C:\Program Files\Rhino 9 WIP"
    ) | Where-Object {
        Test-Path -LiteralPath (Join-Path $_ "System\RhinoCommon.dll")
    } | Select-Object -First 1
    if ($null -ne $rhino9Root) {
        & "$PSScriptRoot\test-rhino9-compat.ps1" -Configuration Debug -RhinoRoot $rhino9Root
        if ($LASTEXITCODE -ne 0) {
            throw "Rhino 9 runtime compatibility failed with exit code $LASTEXITCODE."
        }
        $grasshopper2 = Join-Path $rhino9Root "Plug-ins\Grasshopper2\net8.0\Grasshopper2.dll"
        if (Test-Path -LiteralPath $grasshopper2) {
            & "$PSScriptRoot\test-grasshopper2-compat.ps1" -Configuration Debug -RhinoRoot $rhino9Root
            if ($LASTEXITCODE -ne 0) {
                throw "Grasshopper 2 runtime compatibility failed with exit code $LASTEXITCODE."
            }
        }
        else {
            Write-Host "Grasshopper 2 runtime compatibility skipped: SDK/runtime not present in $rhino9Root."
        }
    }
    else {
        Write-Host "Rhino 9 runtime compatibility skipped: host not installed."
    }
    Write-Host "All XVARNA static checks and Rhino 8/9 + Grasshopper 1/2 compatibility tests passed."
}
finally {
    Pop-Location
}
