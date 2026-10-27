$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$fixtureRoot = Join-Path $repoRoot ('.local-test-data\runner-logging-contract-{0}' -f [guid]::NewGuid().ToString('N'))
foreach ($relative in @('scripts', 'tests\version\v0.0.1', 'tests\regression')) {
    New-Item -ItemType Directory -Path (Join-Path $fixtureRoot $relative) -Force | Out-Null
}
foreach ($name in @('test-version.ps1', 'test-all.ps1')) {
    Copy-Item -LiteralPath (Join-Path $repoRoot "scripts\$name") -Destination (Join-Path $fixtureRoot "scripts\$name")
}
$unicode = -join @([char]0x4e2d, [char]0x6587, [char]0x03a9)
$utf8Bom = New-Object System.Text.UTF8Encoding($true)
$strictUtf8 = New-Object System.Text.UTF8Encoding($false, $true)
function Write-Fixture([string]$Relative, [string]$Text) {
    [IO.File]::WriteAllText((Join-Path $fixtureRoot $Relative), $Text.Replace('__UNICODE__', $unicode), $utf8Bom)
}
Write-Fixture 'scripts\validate-task.ps1' "Write-Output 'fixture validator passed'"
Write-Fixture 'bootstrap.ps1' @'
param([string]$Runner, [string]$RunId)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(437)
$OutputEncoding = [Console]::OutputEncoding
& (Join-Path $PSScriptRoot "scripts\$Runner") -Version v0.0.1 -RunId $RunId
exit $LASTEXITCODE
'@
Write-Fixture 'tests\version\v0.0.1\a-fails.test.ps1' @'
Write-Output ('encoding-' + [Console]::OutputEncoding.CodePage)
Write-Output 'stdout-__UNICODE__'
[Console]::Error.WriteLine('stderr-__UNICODE__')
exit 7
'@
Write-Fixture 'tests\version\v0.0.1\b-passes.test.ps1' "Write-Output 'later-pass'"
Write-Fixture 'tests\regression\pass.test.ps1' "Write-Output 'regression-pass'"
& git -C $fixtureRoot init --quiet
if ($LASTEXITCODE -ne 0) { throw 'Cannot initialize local logging fixture' }
& git -C $fixtureRoot add .
if ($LASTEXITCODE -ne 0) { throw 'Cannot stage local logging fixture' }
& git -C $fixtureRoot -c user.name=LoggingContract -c user.email=logging-contract@example.invalid commit --quiet -m 'local logging fixture'
if ($LASTEXITCODE -ne 0) { throw 'Cannot commit local logging fixture' }

function Invoke-Runner([string]$Script, [string]$RunId) {
    $arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f (Join-Path $fixtureRoot 'bootstrap.ps1')), '-Runner', $Script, '-RunId', $RunId)
    $process = Start-Process -FilePath powershell.exe -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $fixtureRoot "$RunId-console.log") -RedirectStandardError (Join-Path $fixtureRoot "$RunId-error.log")
    # Cache the handle before exit; Windows PowerShell 5 can otherwise expose
    # a null ExitCode after Start-Process has released its process handle.
    $null = $process.Handle
    if (-not $process.WaitForExit(30000)) {
        Stop-Process -InputObject $process -Force
        throw "Logging fixture timed out: $Script"
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 1) { throw "Runner hid the intentional failure: $Script exit $($process.ExitCode)" }
}
function Assert-Log([string]$Relative, [string[]]$Expected) {
    $path = Join-Path $fixtureRoot $Relative
    $bytes = [IO.File]::ReadAllBytes($path)
    if ([Array]::IndexOf($bytes, [byte]0) -ge 0) { throw "Log contains UTF-16 NUL bytes: $Relative" }
    $text = $strictUtf8.GetString($bytes)
    foreach ($value in $Expected) {
        if (-not $text.Contains($value)) { throw "Log lost expected text '$value': $Relative" }
    }
}

Invoke-Runner 'test-version.ps1' 'version-logging'
Assert-Log '.local-test-data\runs\version-logging\version-tests.log' @('RUN powershell.exe', 'encoding-65001', "stdout-$unicode", "stderr-$unicode", 'later-pass')
$version = Get-Content -LiteralPath (Join-Path $fixtureRoot '.local-test-data\runs\version-logging\version-tests.json') -Raw -Encoding utf8 | ConvertFrom-Json
if ($version.passed -ne $false -or @($version.results).Count -ne 3 -or $version.results[1].exit_code -ne 7 -or $version.results[2].exit_code -ne 0) {
    throw 'Version runner did not retain failure and later success separately'
}
Invoke-Runner 'test-all.ps1' 'all-logging'
Assert-Log '.local-test-data\runs\all-logging\all-tests.log' @('RUN powershell.exe', 'encoding-65001', "stdout-$unicode", "stderr-$unicode", 'later-pass', 'regression-pass')
$all = Get-Content -LiteralPath (Join-Path $fixtureRoot '.local-test-data\runs\all-logging\all-tests.json') -Raw -Encoding utf8 | ConvertFrom-Json
if ($all.passed -ne $false -or @($all.results).Count -ne 3 -or $all.results[1].exit_code -ne 1 -or $all.results[2].exit_code -ne 0) {
    throw 'All runner did not retain nested version failure and successful regression separately'
}
Write-Fixture 'tests\version\v0.0.1\a-fails.test.ps1' "Write-Output 'fixture now passes'"
Write-Fixture 'tests\version\v0.0.1\b-passes.test.ps1' @'
$log = Join-Path $PSScriptRoot '..\..\..\.local-test-data\runs\version-readonly\version-tests.log'
[IO.File]::SetAttributes($log, [IO.FileAttributes]::ReadOnly)
Write-Output 'this line cannot be persisted'
'@
$readonlyLog = Join-Path $fixtureRoot '.local-test-data\runs\version-readonly\version-tests.log'
try {
    Invoke-Runner 'test-version.ps1' 'version-readonly'
    $readonlySummary = Join-Path $fixtureRoot '.local-test-data\runs\version-readonly\version-tests.json'
    if (Test-Path -LiteralPath $readonlySummary) {
        $summary = Get-Content -LiteralPath $readonlySummary -Raw -Encoding utf8 | ConvertFrom-Json
        if ($summary.passed -eq $true) { throw 'Runner claimed success after losing log output' }
    }
} finally {
    if (Test-Path -LiteralPath $readonlyLog) { [IO.File]::SetAttributes($readonlyLog, [IO.FileAttributes]::Normal) }
}
Write-Fixture 'tests\version\v0.0.1\b-passes.test.ps1' "Write-Output 'fixture now passes too'"
Write-Fixture 'tests\regression\pass.test.ps1' @'
$log = Join-Path $PSScriptRoot '..\..\.local-test-data\runs\all-readonly\all-tests.log'
[IO.File]::SetAttributes($log, [IO.FileAttributes]::ReadOnly)
Write-Output 'this regression line cannot be persisted'
'@
$readonlyAllLog = Join-Path $fixtureRoot '.local-test-data\runs\all-readonly\all-tests.log'
try {
    Invoke-Runner 'test-all.ps1' 'all-readonly'
    $readonlySummary = Join-Path $fixtureRoot '.local-test-data\runs\all-readonly\all-tests.json'
    if (Test-Path -LiteralPath $readonlySummary) {
        $summary = Get-Content -LiteralPath $readonlySummary -Raw -Encoding utf8 | ConvertFrom-Json
        if ($summary.passed -eq $true) { throw 'All runner claimed success after losing log output' }
    }
} finally {
    if (Test-Path -LiteralPath $readonlyAllLog) { [IO.File]::SetAttributes($readonlyAllLog, [IO.FileAttributes]::Normal) }
}
Write-Output 'Runner UTF-8 logging and failure propagation contract passed'
