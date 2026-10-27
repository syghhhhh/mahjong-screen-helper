$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p vision-core --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'vision-core regression tests failed' }
Write-Output 'Regression vision tests passed'
