$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p screen-capture --features interactive-test --test local_window_capture --locked -j 1
if ($LASTEXITCODE -ne 0) { throw 'Interactive local capture test failed' }
Write-Output 'Interactive local capture test passed'
