[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Destination)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $repoRoot '.local-test-data\runs'
$output = [System.IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $output -Force | Out-Null
if (Test-Path -LiteralPath $runRoot) { foreach ($file in Get-ChildItem -LiteralPath $runRoot -File -Recurse) {
    if ($file.Name -notin @('version-tests.log', 'version-tests.json', 'all-tests.log', 'all-tests.json')) { continue }
    $relative = $file.FullName.Substring($runRoot.Length).TrimStart('\')
    $target = Join-Path $output $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $target
}
}

# Only the build-only control diagnostic is eligible from package directories.
# Never collect arbitrary logs, executables, model files, or screenshots here.
$packageRoot = Join-Path $repoRoot '.local-test-data\packages'
if (Test-Path -LiteralPath $packageRoot) { foreach ($file in Get-ChildItem -LiteralPath $packageRoot -File -Filter 'expected-rejection.log' -Recurse) {
    $relative = $file.FullName.Substring($packageRoot.Length).TrimStart('\').Replace('\','/')
    if ($relative -notmatch '^version-[0-9a-f]{32}/import-control/expected-rejection\.log$') { continue }
    $target = Join-Path $output ("package-controls/$relative")
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $target
} }
