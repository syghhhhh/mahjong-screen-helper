$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$output = Join-Path $repoRoot ('.local-test-data\packages\version-{0}\RiichiScreenHelper' -f [guid]::NewGuid().ToString('N'))
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts\package-portable.ps1') -Version v0.0.1 -Configuration Release -OutputDirectory $output
if ($LASTEXITCODE -ne 0) { throw 'Release packaging failed' }
$exe = Join-Path $output 'RiichiScreenHelper.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Portable executable missing' }
if (-not (Test-Path -LiteralPath "$output.zip" -PathType Leaf)) { throw 'Portable zip missing' }
if (-not (Test-Path -LiteralPath (Join-Path $output 'licenses\README.md') -PathType Leaf)) { throw 'License inventory missing' }
$inventoryPath = Join-Path $output 'licenses\DEPENDENCIES.json'
if (-not (Test-Path -LiteralPath $inventoryPath -PathType Leaf)) { throw 'Dependency license inventory missing' }
$inventory = Get-Content -LiteralPath $inventoryPath -Raw | ConvertFrom-Json
if (@($inventory.packages).Count -lt 1) { throw 'Dependency license inventory empty' }
foreach ($package in $inventory.packages) {
    if (-not $package.license -or @($package.texts).Count -lt 1) { throw "License evidence missing: $($package.name)" }
    foreach ($text in $package.texts) {
        $path = Join-Path (Join-Path $output 'licenses') $text.path
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "License text missing: $($package.name)" }
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $text.sha256) { throw "License text hash mismatch: $($package.name)" }
    }
}
$unexpected = @(Get-ChildItem -LiteralPath $output -File -Recurse | Where-Object { $_.Name -match '^(python|node)(\.exe)?$|\.py$' })
if ($unexpected.Count -ne 0) { throw 'Portable package contains a development runtime' }
$totalBytes = (Get-ChildItem -LiteralPath $output -File -Recurse | Measure-Object Length -Sum).Sum
if ($totalBytes -gt 100MB) { throw "Portable package exceeds lightweight size target: $totalBytes bytes" }
$process = Start-Process -FilePath $exe -ArgumentList '--smoke' -WorkingDirectory $output -WindowStyle Hidden -PassThru -Wait
if ($process.ExitCode -ne 0) { throw 'Portable executable smoke failed' }
Write-Output "Version portable package smoke passed: $totalBytes bytes"
