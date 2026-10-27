[CmdletBinding()]
param(
    [string]$Version = "v0.0.1",
    [string]$TaskPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $TaskPath) {
    $TaskPath = Join-Path $repoRoot ("tasks\{0}\task.json" -f $Version)
}

if (-not (Test-Path -LiteralPath $TaskPath -PathType Leaf)) {
    throw "Task file not found: $TaskPath"
}

$json = Get-Content -LiteralPath $TaskPath -Raw -Encoding utf8 | ConvertFrom-Json
$allowedStatuses = @("planned", "in_progress", "implemented", "self_tested", "regression_tested", "succeed", "failed", "blocked")
$requiredTaskFields = @("id", "title", "status", "depends_on", "scope", "acceptance_criteria", "version_tests", "affected_tests", "regression_profile", "local_evidence_required", "evidence")

if ($json.schema_version -ne "1.0") { throw "Unsupported schema_version: $($json.schema_version)" }
if ($json.project -ne "mahjong-screen-helper") { throw "Unexpected project: $($json.project)" }
if ($json.release -ne $Version) { throw "Release mismatch: expected $Version, got $($json.release)" }
if (-not $json.tasks -or $json.tasks.Count -lt 1) { throw "At least one task is required" }

$allIds = @($json.tasks | ForEach-Object { [string]$_.id })
$seen = @{}
foreach ($task in @($json.tasks)) {
    foreach ($field in $requiredTaskFields) {
        if ($null -eq $task.PSObject.Properties[$field]) { throw "Task $($task.id) is missing field '$field'" }
    }
    if ($seen.ContainsKey($task.id)) { throw "Duplicate task id: $($task.id)" }
    $seen[$task.id] = $true
    if ([string]$task.id -notmatch '^V[0-9]{3}-[0-9]{3}$') { throw "Invalid task id: $($task.id)" }
    if ($allowedStatuses -notcontains [string]$task.status) { throw "Invalid status '$($task.status)' for task $($task.id)" }
    if ([string]::IsNullOrWhiteSpace([string]$task.title)) { throw "Task $($task.id) has an empty title" }
    if (@($task.acceptance_criteria).Count -lt 1) { throw "Task $($task.id) needs acceptance_criteria" }
    if (@($task.scope).Count -lt 1) { throw "Task $($task.id) needs scope" }
    if (@($task.version_tests).Count -lt 1 -or @($task.affected_tests).Count -lt 1) { throw "Task $($task.id) needs declared tests" }
    if ($task.regression_profile -ne 'full') { throw "Task $($task.id) must use full regression" }
    $localKinds = @($task.local_evidence_required)
    if (@($localKinds | Select-Object -Unique).Count -ne $localKinds.Count) { throw "Task $($task.id) has duplicate local evidence kinds" }
    foreach ($kind in $localKinds) {
        if ([string]$kind -cnotmatch '^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$') { throw "Invalid local evidence kind for task $($task.id): $kind" }
    }
    foreach ($testPath in @($task.version_tests) + @($task.affected_tests)) {
        if ([string]$testPath -notmatch '^tests/(version|regression)/[A-Za-z0-9.\-]+(/[A-Za-z0-9.\-]+)*\.test\.ps1$') {
            throw "Invalid test path for task $($task.id): $testPath"
        }
        if ($task.status -in @('implemented', 'self_tested', 'regression_tested', 'succeed')) {
            if (-not (Test-Path -LiteralPath (Join-Path $repoRoot ([string]$testPath)) -PathType Leaf)) {
                throw "Declared test does not exist for task $($task.id): $testPath"
            }
        }
    }
    foreach ($dependency in @($task.depends_on)) {
        if ($allIds -notcontains [string]$dependency) { throw "Task $($task.id) has missing dependency $dependency" }
        if ($dependency -eq $task.id) { throw "Task $($task.id) depends on itself" }
    }
    if ($task.status -eq 'succeed') {
        if ($null -eq $task.evidence) { throw "Task $($task.id) cannot be succeed without evidence" }
        if ([string]$task.evidence.source_sha -notmatch '^[0-9a-f]{40}$') { throw "Task $($task.id) has invalid evidence SHA" }
        foreach ($field in @('version_test_run', 'regression_test_run', 'completed_at')) {
            if ([string]::IsNullOrWhiteSpace([string]$task.evidence.$field)) { throw "Task $($task.id) has incomplete evidence: $field" }
        }
        if (@($task.evidence.commands).Count -lt 2 -or @($task.evidence.logs).Count -lt 2) { throw "Task $($task.id) has incomplete test evidence" }
        if ($localKinds.Count -gt 0 -and ($null -eq $task.evidence.local_evidence_paths -or @($task.evidence.local_evidence_paths).Count -lt $localKinds.Count)) {
            throw "Task $($task.id) lacks required local evidence paths"
        }
    }
}

if ($null -eq $json.release_gate -or $json.release_gate.required -ne $true) {
    throw "release_gate.required must be true"
}

Write-Output ("Task file valid: {0}" -f (Resolve-Path -LiteralPath $TaskPath))
