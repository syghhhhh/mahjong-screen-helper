[CmdletBinding()]
param(
    [string]$Version = 'v0.0.1',
    [ValidateSet('Debug', 'Release')][string]$Configuration = 'Release',
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
$zip = "$output.zip"
if (Test-Path -LiteralPath $output) { throw "Output directory already exists: $output" }
if (Test-Path -LiteralPath $zip) { throw "Output zip already exists: $zip" }
$profile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$cargoArgs = @('build', '--manifest-path', (Join-Path $repoRoot 'Cargo.toml'), '-p', 'riichi-screen-helper', '--locked', '-j', '1')
if ($Configuration -eq 'Release') { $cargoArgs += '--release' }
Push-Location -LiteralPath $repoRoot
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
} finally { Pop-Location }
$exe = Join-Path $repoRoot ("target\$profile\RiichiScreenHelper.exe")
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Built executable missing: $exe" }
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'validate-portable-imports.ps1') -Executable $exe
if ($LASTEXITCODE -ne 0) { throw 'Portable DLL dependency audit failed' }

New-Item -ItemType Directory -Path $output -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $output 'config') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $output 'models\template-pack') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $output 'models\tile-classifier') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $output 'licenses') -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $output 'RiichiScreenHelper.exe')
Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination (Join-Path $output 'LICENSE')
Copy-Item -LiteralPath (Join-Path $repoRoot 'README.md') -Destination (Join-Path $output 'README.md')
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs\third-party-licenses.md') -Destination (Join-Path $output 'licenses\README.md')
Copy-Item -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Destination (Join-Path $output 'licenses\Cargo.lock')
& (Join-Path $repoRoot 'scripts\generate-license-bundle.ps1') -Destination (Join-Path $output 'licenses')
if ($LASTEXITCODE -ne 0) { throw 'License bundle generation failed' }
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs\vision-template-pack.md') -Destination (Join-Path $output 'models\template-pack\README.md')
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs\model-manifest.md') -Destination (Join-Path $output 'models\tile-classifier\README.md')
$intro = @(
    "Riichi Screen Helper $Version ($Configuration)"
    'Double-click RiichiScreenHelper.exe to start. No Python, Node, or Rust installation is required.'
    'Automatic recognition uses complete local templates, or a compatible installed ONNX classifier plus all five action templates.'
    'Optional model installation: see models/tile-classifier/README.md. Trained weights are not included.'
    'Use the Learn Template button to capture labeled tiles and action buttons locally; existing templates are never overwritten.'
    'This development package supports manual hand entry to verify Top-3 tile recommendations.'
    'The release still requires real Mahjong Soul screenshot and template licensing validation.'
) -join [Environment]::NewLine
$intro | Set-Content -LiteralPath (Join-Path $output 'START-HERE.txt') -Encoding utf8
'Calibration creates calibration.json. Raw screenshots are not written to this directory.' |
    Set-Content -LiteralPath (Join-Path $output 'config\README.txt') -Encoding utf8

Compress-Archive -LiteralPath $output -DestinationPath $zip -CompressionLevel Optimal
Write-Output ("Portable package created: {0}" -f $output)
Write-Output ("Portable zip created: {0}" -f $zip)
