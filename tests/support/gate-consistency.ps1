$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$runRoot = Join-Path $repoRoot ('.local-test-data\gate-consistency-{0}' -f [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null

function Invoke-Git([string]$Root, [string[]]$Arguments) {
    & git -c core.autocrlf=false -C $Root @Arguments 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Miniature repository Git command failed: $Arguments" }
}

# These runners create small, explicit contract summaries. They never execute
# product tests or pretend that this fixture establishes product acceptance.
$runner = @'
param([string]$Version, [string]$RunId)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$case = Get-Content (Join-Path $root '.local-test-data\scenario.json') -Raw | ConvertFrom-Json
$kind = if ((Split-Path $PSCommandPath -Leaf) -eq 'test-version.ps1') { 'version' } else { 'all' }
if ($kind -eq 'all') {
    switch ($case.name) {
        'source-change' { 'edited while tests ran' | Set-Content (Join-Path $root 'src\example.txt') }
        'head-change' {
            & git -C $root -c user.name=GateTest -c user.email=gate@example.invalid commit --allow-empty -m 'concurrent commit' | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'HEAD-change control failed' }
        }
        'task-change' {
            $path = Join-Path $root 'tasks\v0.0.1\task.json'
            $doc = Get-Content $path -Raw | ConvertFrom-Json
            $doc.tasks[0].title = 'Concurrent task edit must survive'
            $doc | ConvertTo-Json -Depth 20 | Set-Content $path -Encoding utf8
        }
        'local-evidence-change' { 'changed fixture' | Set-Content (Join-Path $root '.local-test-data\fixture.txt') }
    }
}
$run = Join-Path $root ('.local-test-data\runs\' + $RunId)
New-Item -ItemType Directory -Path $run -Force | Out-Null
$log = Join-Path $run 'test.log'
'Miniature gate contract fixture only' | Set-Content $log
$test = Join-Path $root $(if ($kind -eq 'version') { 'tests\version\v0.0.1\minimal.test.ps1' } else { 'tests\regression\minimal.test.ps1' })
$summary = [pscustomobject]@{
    run_id=$RunId; kind=$kind; passed=$true; source_sha=$case.source_sha; log=$log
    environment=[pscustomobject]@{os='Windows test fixture';architecture='x64'}
    results=@([pscustomobject]@{test=$test;exit_code=0;skipped=($case.name -eq 'skipped-result')})
    fixture_hashes=@()
}
$file = if ($kind -eq 'version') { 'version-tests.json' } else { 'all-tests.json' }
$summary | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $run $file) -Encoding utf8
exit 0
'@
$results = @()
foreach ($name in @('source-change', 'head-change', 'task-change', 'local-evidence-change', 'skipped-result', 'stable')) {
    $caseRoot = Join-Path $runRoot $name
    foreach ($directory in @('scripts', 'src', 'tasks\v0.0.1', 'tests\version\v0.0.1', 'tests\regression', '.local-test-data')) {
        New-Item -ItemType Directory -Path (Join-Path $caseRoot $directory) -Force | Out-Null
    }
    foreach ($script in @('task-gate.ps1', 'validate-task.ps1', 'validate-local-evidence.ps1')) {
        Copy-Item -LiteralPath (Join-Path $repoRoot "scripts\$script") -Destination (Join-Path $caseRoot "scripts\$script")
    }
    foreach ($script in @('test-version.ps1', 'test-all.ps1')) {
        $runner | Set-Content (Join-Path $caseRoot "scripts\$script") -Encoding utf8
    }
    '.local-test-data/' | Set-Content (Join-Path $caseRoot '.gitignore')
    'original source' | Set-Content (Join-Path $caseRoot 'src\example.txt')
    foreach ($path in @('tests/version/v0.0.1/minimal.test.ps1', 'tests/regression/minimal.test.ps1')) {
        '# miniature gate contract fixture' | Set-Content (Join-Path $caseRoot $path)
    }
    $document = [pscustomobject]@{
        schema_version='1.0';project='mahjong-screen-helper';release='v0.0.1'
        tasks=@([pscustomobject]@{
            id='V001-001';title='Gate fixture';status='regression_tested';depends_on=@();scope=@('workflow')
            acceptance_criteria=@('Stable gate snapshot');version_tests=@('tests/version/v0.0.1/minimal.test.ps1')
            affected_tests=@('tests/regression/minimal.test.ps1');regression_profile='full';evidence=$null
            local_evidence_required=@('gate-fixture')
        })
        release_gate=[pscustomobject]@{required=$true}
    }
    $taskPath = Join-Path $caseRoot 'tasks\v0.0.1\task.json'
    $document | ConvertTo-Json -Depth 20 | Set-Content $taskPath -Encoding utf8
    Invoke-Git $caseRoot @('init', '-q')
    Invoke-Git $caseRoot @('add', '.')
    Invoke-Git $caseRoot @('-c', 'user.name=GateTest', '-c', 'user.email=gate@example.invalid', 'commit', '-qm', 'gate fixture')
    $sha = (& git -C $caseRoot rev-parse HEAD).Trim()
    [pscustomobject]@{name=$name;source_sha=$sha} | ConvertTo-Json | Set-Content (Join-Path $caseRoot '.local-test-data\scenario.json')
    $fixture = Join-Path $caseRoot '.local-test-data\fixture.txt'
    $localLog = Join-Path $caseRoot '.local-test-data\local.log'
    $localSummary = Join-Path $caseRoot '.local-test-data\local.json'
    'original fixture' | Set-Content $fixture
    'fixture checked' | Set-Content $localLog
    [pscustomobject]@{
        kind='gate-fixture';run_id='local-fixture';source_sha=$sha;passed=$true;command='Miniature gate fixture'
        log=$localLog;environment=[pscustomobject]@{os='Windows';architecture='x64'}
        completed_at=(Get-Date).ToUniversalTime().ToString('o')
        fixture_hashes=@([pscustomobject]@{path=$fixture;sha256=(Get-FileHash $fixture -Algorithm SHA256).Hash.ToLowerInvariant()})
    } | ConvertTo-Json -Depth 12 | Set-Content $localSummary -Encoding utf8
    $before = (Get-FileHash $taskPath -Algorithm SHA256).Hash
    $previous = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $caseRoot 'scripts\task-gate.ps1') -TaskId V001-001 -LocalEvidencePaths $localSummary 2>&1 | ForEach-Object { [string]$_ })
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previous }
    $output | Set-Content (Join-Path $caseRoot '.local-test-data\gate.log') -Encoding utf8
    $after = Get-Content $taskPath -Raw | ConvertFrom-Json
    $problem = $null
    if ($name -eq 'stable') {
        if ($code -ne 0 -or $after.tasks[0].status -ne 'succeed' -or $after.tasks[0].evidence.source_sha -ne $sha) { $problem='stable gate did not succeed with exact source evidence' }
        $backups = @(Get-ChildItem (Join-Path $caseRoot '.local-test-data\runs') -Recurse -Filter 'task-before-gate.json')
        if ($backups.Count -ne 1 -or (Get-FileHash $backups[0].FullName -Algorithm SHA256).Hash -ne $before) { $problem='atomic gate backup is missing or differs from original task file' }
    } else {
        if ($code -eq 0 -or $after.tasks[0].status -ne 'regression_tested' -or $null -ne $after.tasks[0].evidence) { $problem='gate accepted an invalidated test run' }
        if ($name -eq 'task-change') {
            if ($after.tasks[0].title -ne 'Concurrent task edit must survive') { $problem='gate overwrote the concurrent task edit' }
        } elseif ((Get-FileHash $taskPath -Algorithm SHA256).Hash -ne $before) { $problem='failed gate modified the task file' }
    }
    $results += [pscustomobject]@{case=$name;gate_exit_code=$code;passed=($null -eq $problem);problem=$problem}
    Write-Output ("Gate consistency {0}: {1}" -f $name, $(if ($problem) { $problem } else { 'passed' }))
}
$results | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $runRoot 'summary.json') -Encoding utf8
if (@($results | Where-Object passed -EQ $false).Count -gt 0) { throw "Gate consistency failures; see $runRoot" }
Write-Output 'Gate consistency integration passed (six miniature repository cases)'
exit 0
