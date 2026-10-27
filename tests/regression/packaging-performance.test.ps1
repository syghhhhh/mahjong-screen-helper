$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$output = Join-Path $repoRoot ('.local-test-data\packages\regression-{0}\RiichiScreenHelper' -f [guid]::NewGuid().ToString('N'))
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts\package-portable.ps1') -Version v0.0.1 -Configuration Debug -OutputDirectory $output
if ($LASTEXITCODE -ne 0) { throw 'Debug packaging regression failed' }
$exe = Join-Path $output 'RiichiScreenHelper.exe'
$process = Start-Process -FilePath $exe -ArgumentList '--smoke' -WorkingDirectory $output -WindowStyle Hidden -PassThru -Wait
if ($process.ExitCode -ne 0) { throw 'Portable startup regression failed' }
Write-Output 'Portable startup regression passed'
