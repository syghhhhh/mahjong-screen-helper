$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p riichi-core --test public_contract --test concurrency --locked
if ($LASTEXITCODE -ne 0) { throw 'riichi-core public and concurrency contract regression failed' }
Write-Output 'Rules public and concurrency contract regression passed'
