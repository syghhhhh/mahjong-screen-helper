[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Destination)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $repoRoot '.local-test-data\runs'
$output = [System.IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $output -Force | Out-Null
if (-not (Test-Path -LiteralPath $runRoot)) { return }

foreach ($file in Get-ChildItem -LiteralPath $runRoot -File -Recurse) {
    if ($file.Name -notin @('version-tests.log', 'version-tests.json', 'all-tests.log', 'all-tests.json')) { continue }
    $relative = $file.FullName.Substring($runRoot.Length).TrimStart('\')
    $target = Join-Path $output $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $target
}
