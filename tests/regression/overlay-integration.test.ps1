$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p runtime-core --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'runtime-core regression tests failed' }
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p riichi-screen-helper --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'desktop presentation regression tests failed' }
& cargo build --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p riichi-screen-helper --locked
if ($LASTEXITCODE -ne 0) { throw 'desktop app regression build failed' }
$app = Start-Process -FilePath (Join-Path $repoRoot 'target\debug\RiichiScreenHelper.exe') -ArgumentList '--smoke' -WindowStyle Hidden -PassThru -Wait
if ($app.ExitCode -ne 0) { throw 'desktop overlay regression smoke failed' }
Write-Output 'Regression runtime integration tests passed'
