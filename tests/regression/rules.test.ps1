$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p riichi-core --test public_contract --locked
if ($LASTEXITCODE -ne 0) { throw 'riichi-core public contract regression failed' }
Write-Output 'Rules public contract regression passed'
