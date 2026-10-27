$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p screen-capture --test public_contract --locked
if ($LASTEXITCODE -ne 0) { throw 'screen-capture public contract regression failed' }
Write-Output 'Capture and calibration public contract regression passed'
