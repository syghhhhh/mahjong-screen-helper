$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "ASSERTION FAILED: $Message" }
}

$required = @(
    "docs\development-workflow.md",
    "docs\releases\v0.0.1.md",
    "tasks\v0.0.1\task.json",
    "scripts\validate-task.ps1",
    "scripts\test-version.ps1",
    "scripts\test-all.ps1",
    "scripts\task-gate.ps1",
    "tests\regression\repository-process.test.ps1"
)
foreach ($relative in $required) {
    Assert-True (Test-Path -LiteralPath (Join-Path $repoRoot $relative)) "missing $relative"
}

$task = Get-Content -LiteralPath (Join-Path $repoRoot "tasks\v0.0.1\task.json") -Raw -Encoding utf8 | ConvertFrom-Json
Assert-True ($task.schema_version -eq "1.0") "schema version is not 1.0"
Assert-True ($task.release -eq "v0.0.1") "release is not v0.0.1"
Assert-True (@($task.tasks).Count -ge 1) "task list is empty"
foreach ($item in @($task.tasks)) {
    Assert-True ($item.status -ne "succeed" -or $null -ne $item.evidence) "succeed task $($item.id) lacks evidence"
    Assert-True (@($item.acceptance_criteria).Count -gt 0) "task $($item.id) has no acceptance criteria"
}

$scratch = Join-Path $repoRoot ('.local-test-data\validator-negative-{0}.json' -f [guid]::NewGuid().ToString('N'))
$originalErrorActionPreference = $ErrorActionPreference
try {
    $invalid = Get-Content -LiteralPath (Join-Path $repoRoot 'tasks\v0.0.1\task.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $invalid.tasks[0].status = 'succeed'
    $invalid.tasks[0].evidence = $null
    $invalid | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $scratch -Encoding utf8
    $ErrorActionPreference = 'Continue'
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts\validate-task.ps1') -Version v0.0.1 -TaskPath $scratch 2>&1 | Out-Null
    $ErrorActionPreference = $originalErrorActionPreference
    Assert-True ($LASTEXITCODE -ne 0) 'validator accepted succeed without evidence'

    $invalid.tasks[0].status = 'planned'
    $invalid.tasks[0].version_tests = @()
    $invalid | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $scratch -Encoding utf8
    $ErrorActionPreference = 'Continue'
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts\validate-task.ps1') -Version v0.0.1 -TaskPath $scratch 2>&1 | Out-Null
    $ErrorActionPreference = $originalErrorActionPreference
    Assert-True ($LASTEXITCODE -ne 0) 'validator accepted missing version tests'
} finally {
    $ErrorActionPreference = $originalErrorActionPreference
    Remove-Item -LiteralPath $scratch -ErrorAction SilentlyContinue
}

Write-Output "Version process contract passed"
