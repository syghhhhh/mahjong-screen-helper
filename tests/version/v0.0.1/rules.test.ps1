$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p riichi-core --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'riichi-core tests failed' }
Write-Output 'Version rules tests passed'
