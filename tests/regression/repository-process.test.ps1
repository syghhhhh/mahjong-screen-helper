$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "ASSERTION FAILED: $Message" }
}

$gitIgnore = Join-Path $repoRoot ".gitignore"
Assert-True (Test-Path -LiteralPath $gitIgnore) "missing .gitignore"
$ignoreText = Get-Content -LiteralPath $gitIgnore -Raw
Assert-True ($ignoreText -match "\.local-test-data/") "local test data is not ignored"
Assert-True (Test-Path -LiteralPath (Join-Path $repoRoot "tests\fixtures\synthetic")) "synthetic fixture directory missing"
Assert-True (Test-Path -LiteralPath (Join-Path $repoRoot "docs\development-workflow.md")) "workflow document missing"

$ciPath = Join-Path $repoRoot '.github\workflows\ci.yml'
Assert-True (Test-Path -LiteralPath $ciPath) 'CI workflow missing'
$ci = Get-Content -LiteralPath $ciPath -Raw -Encoding utf8
foreach ($name in @('task-schema', 'version-tests', 'regression-tests', 'build-smoke', 'package-smoke')) {
    Assert-True ($ci -match "(?m)^  ${name}:") "CI check missing: $name"
}
Assert-True ($ci -match 'pull_request:') 'CI does not run for pull requests'
Assert-True ($ci -notmatch 'pull_request_target:') 'CI uses pull_request_target'
Assert-True ($ci -notmatch 'uses:\s+[^\s]+@v\d') 'CI action is not pinned to a commit SHA'
Assert-True (Test-Path -LiteralPath (Join-Path $repoRoot '.github\CODEOWNERS')) 'CODEOWNERS missing'

Write-Output "Repository process regression passed"
