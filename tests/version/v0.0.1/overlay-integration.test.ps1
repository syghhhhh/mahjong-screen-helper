$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p runtime-core --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'runtime-core tests failed' }
Write-Output 'Version runtime integration tests passed'
