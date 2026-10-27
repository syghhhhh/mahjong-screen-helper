$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p screen-capture --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'screen-capture tests failed' }
Write-Output 'Version capture and calibration tests passed'
