$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p game-state --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'game-state regression tests failed' }
Write-Output 'Regression state and action tests passed'
