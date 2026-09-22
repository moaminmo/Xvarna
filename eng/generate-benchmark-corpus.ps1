[CmdletBinding()]
param(
    [ValidateSet("S", "M", "L", "All")]
    [string]$Tier = "S",
    [string]$OutputDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $root "artifacts\benchmark-corpus" }
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$definitions = [ordered]@{
    S = [ordered]@{ Cells = 224; Sensors = 10000 }
    M = [ordered]@{ Cells = 707; Sensors = 100000 }
    L = [ordered]@{ Cells = 1582; Sensors = 250000 }
}
$selected = if ($Tier -eq "All") { @("S", "M", "L") } else { @($Tier) }
$records = @()
foreach ($name in $selected) {
    $definition = $definitions[$name]; $cells = [int]$definition.Cells; $sensorCount = [int]$definition.Sensors
    $meshPath = Join-Path $output "xvarna-corpus-$($name.ToLowerInvariant()).obj"
    $sensorPath = Join-Path $output "xvarna-corpus-$($name.ToLowerInvariant())-sensors.csv"
    $writer = [System.IO.StreamWriter]::new($meshPath, $false, [System.Text.UTF8Encoding]::new($false), 1MB)
    try {
        $writer.WriteLine("# XVARNA deterministic CC0-1.0 benchmark tier $name")
        for ($y = 0; $y -le $cells; $y++) {
            for ($x = 0; $x -le $cells; $x++) {
                $z = 0.15 * [Math]::Sin($x * 0.071) * [Math]::Cos($y * 0.053)
                $writer.WriteLine([string]::Format([Globalization.CultureInfo]::InvariantCulture, "v {0:R} {1:R} {2:R}", [double]$x, [double]$y, $z))
            }
        }
        $stride = $cells + 1
        for ($y = 0; $y -lt $cells; $y++) {
            for ($x = 0; $x -lt $cells; $x++) {
                $a = 1 + $y * $stride + $x; $b = $a + 1; $d = $a + $stride; $c = $d + 1
                $writer.WriteLine("f $a $b $c"); $writer.WriteLine("f $a $c $d")
            }
        }
    } finally { $writer.Dispose() }
    $sensorWriter = [System.IO.StreamWriter]::new($sensorPath, $false, [System.Text.UTF8Encoding]::new($false), 1MB)
    try {
        $sensorWriter.WriteLine("sensor_id,x,y,z,nx,ny,nz,area_m2")
        $columns = [int][Math]::Ceiling([Math]::Sqrt($sensorCount))
        for ($index = 0; $index -lt $sensorCount; $index++) {
            $x = (($index % $columns) + 0.5) * $cells / $columns; $y = ([Math]::Floor($index / $columns) + 0.5) * $cells / $columns
            $z = 1.0 + 0.15 * [Math]::Sin($x * 0.071) * [Math]::Cos($y * 0.053)
            $sensorWriter.WriteLine([string]::Format([Globalization.CultureInfo]::InvariantCulture, "{0},{1:R},{2:R},{3:R},0,0,1,{4:R}", $index + 1, $x, $y, $z, [double]($cells * $cells / $sensorCount)))
        }
    } finally { $sensorWriter.Dispose() }
    $mesh = Get-Item -LiteralPath $meshPath; $sensors = Get-Item -LiteralPath $sensorPath
    $records += [ordered]@{
        tier = $name; cellsPerAxis = $cells; vertices = ($cells + 1) * ($cells + 1); triangles = 2 * $cells * $cells; sensors = $sensorCount
        mesh = [ordered]@{ path = $mesh.Name; bytes = $mesh.Length; sha256 = (Get-FileHash -LiteralPath $mesh.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
        sensorTable = [ordered]@{ path = $sensors.Name; bytes = $sensors.Length; sha256 = (Get-FileHash -LiteralPath $sensors.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    Write-Host "Generated tier ${name}: $($records[-1].triangles) triangles, $sensorCount sensors."
}
[ordered]@{ schemaVersion = "1.0.0"; generatedUtc = [DateTimeOffset]::UtcNow.ToString("O"); license = "CC0-1.0"; records = $records } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output "generated-manifest.json") -Encoding utf8NoBOM
