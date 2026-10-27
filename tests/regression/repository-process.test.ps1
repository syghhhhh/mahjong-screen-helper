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

Write-Output "Repository process regression passed"
