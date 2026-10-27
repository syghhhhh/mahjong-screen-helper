[CmdletBinding()]
param(
    [string]$Version = 'v0.0.1',
    [Parameter(Mandatory = $true)][string]$TaskId,
    [string[]]$LocalEvidencePaths = @()
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$taskRelative = ('tasks/{0}/task.json' -f $Version)
$taskPath = Join-Path $repoRoot $taskRelative

& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'validate-task.ps1') -Version $Version
if ($LASTEXITCODE -ne 0) { throw 'Task schema validation failed' }
$document = Get-Content -LiteralPath $taskPath -Raw -Encoding utf8 | ConvertFrom-Json
$task = @($document.tasks | Where-Object id -Eq $TaskId) | Select-Object -First 1
if ($null -eq $task) { throw "Task not found: $TaskId" }
if ($task.status -ne 'regression_tested') { throw "Task $TaskId must be regression_tested" }
if (@($task.acceptance_criteria).Count -eq 0 -or @($task.version_tests).Count -eq 0 -or @($task.affected_tests).Count -eq 0) {
    throw 'Acceptance criteria and affected tests are required'
}
foreach ($dependency in @($task.depends_on)) {
    $parent = @($document.tasks | Where-Object id -Eq $dependency) | Select-Object -First 1
    if ($null -eq $parent -or $parent.status -ne 'succeed') { throw "Dependency has not succeeded: $dependency" }
}

$dirty = @(git -C $repoRoot -c core.quotepath=false status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Git working tree state' }
foreach ($entry in $dirty) {
    $changed = $entry.Substring(3).Trim().Replace('\','/')
    if ($changed -ne $taskRelative) { throw "Commit source and test changes before gate: $changed" }
}
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceSha -notmatch '^[0-9a-f]{40}$') { throw 'Source commit is missing' }
& (Join-Path $PSScriptRoot 'validate-local-evidence.ps1') -SourceSha $sourceSha -RequiredKinds @($task.local_evidence_required) -Paths $LocalEvidencePaths

$versionRunId = 'gate-version-{0}-{1}' -f $Version, [guid]::NewGuid().ToString('N')
$allRunId = 'gate-all-{0}' -f [guid]::NewGuid().ToString('N')
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'test-version.ps1') -Version $Version -RunId $versionRunId
if ($LASTEXITCODE -ne 0) { throw 'Version tests failed; task state was not changed' }
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'test-all.ps1') -Version $Version -RunId $allRunId
if ($LASTEXITCODE -ne 0) { throw 'Full regression failed; task state was not changed' }

$versionSummaryPath = Join-Path $repoRoot ('.local-test-data\runs\{0}\version-tests.json' -f $versionRunId)
$allSummaryPath = Join-Path $repoRoot ('.local-test-data\runs\{0}\all-tests.json' -f $allRunId)
if (-not (Test-Path -LiteralPath $versionSummaryPath -PathType Leaf) -or -not (Test-Path -LiteralPath $allSummaryPath -PathType Leaf)) {
    throw 'Expected test summaries are missing'
}
$versionSummary = Get-Content -LiteralPath $versionSummaryPath -Raw -Encoding utf8 | ConvertFrom-Json
$all = Get-Content -LiteralPath $allSummaryPath -Raw -Encoding utf8 | ConvertFrom-Json
if ($versionSummary.run_id -ne $versionRunId -or $all.run_id -ne $allRunId -or $versionSummary.kind -ne 'version' -or $all.kind -ne 'all') {
    throw 'Run identity mismatch'
}
if ($versionSummary.passed -ne $true -or $all.passed -ne $true -or $versionSummary.source_sha -ne $sourceSha -or $all.source_sha -ne $sourceSha) {
    throw 'Test success or source SHA mismatch'
}
foreach ($summary in @($versionSummary, $all)) {
    if (-not (Test-Path -LiteralPath $summary.log -PathType Leaf)) { throw "Test log missing: $($summary.log)" }
    if ([string]::IsNullOrWhiteSpace([string]$summary.environment.os) -or [string]::IsNullOrWhiteSpace([string]$summary.environment.architecture)) {
        throw 'Test environment information missing'
    }
    if (@($summary.results).Count -eq 0) { throw 'No executed tests recorded' }
    foreach ($result in @($summary.results)) {
        if ($result.exit_code -ne 0 -or $result.skipped -eq $true) { throw "Failed or skipped test: $($result.test)" }
    }
}

$executed = @(@($versionSummary.results) + @($all.results) | ForEach-Object { [System.IO.Path]::GetFullPath([string]$_.test) })
foreach ($relative in @($task.version_tests) + @($task.affected_tests)) {
    $file = Join-Path $repoRoot ([string]$relative)
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Declared test missing: $relative" }
    if ($executed -notcontains [System.IO.Path]::GetFullPath($file)) { throw "Declared test not executed: $relative" }
}
foreach ($fixture in @($versionSummary.fixture_hashes)) {
    $file = Join-Path $repoRoot ([string]$fixture.path)
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Fixture missing: $($fixture.path)" }
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $fixture.sha256) { throw "Fixture hash mismatch: $($fixture.path)" }
}

$task.evidence = [pscustomobject]@{
    source_sha = $sourceSha
    version_test_run = $versionRunId
    regression_test_run = $allRunId
    commands = @("scripts/test-version.ps1 -Version $Version -RunId $versionRunId", "scripts/test-all.ps1 -Version $Version -RunId $allRunId")
    logs = @($versionSummary.log, $all.log)
    summaries = @($versionSummaryPath, $allSummaryPath)
    fixture_hashes = @($versionSummary.fixture_hashes)
    local_evidence_paths = @($LocalEvidencePaths)
    environment = $versionSummary.environment
    completed_at = (Get-Date).ToUniversalTime().ToString('o')
}
$task.status = 'succeed'
$document | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $taskPath -Encoding utf8
Write-Output "Task $TaskId marked succeed for source commit $sourceSha"
