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

# A miniature repository proves collection does not include nearby screenshots,
# models, binaries, or lookalike diagnostic names, including package-only jobs.
$collectionRoot = Join-Path $repoRoot ('.local-test-data\ci-collection-test-{0}' -f [guid]::NewGuid().ToString('N'))
foreach ($hasRuns in @($true, $false)) {
    $caseRoot = Join-Path $collectionRoot ([string]$hasRuns)
    New-Item -ItemType Directory -Path (Join-Path $caseRoot 'scripts') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot 'scripts\collect-ci-evidence.ps1') -Destination (Join-Path $caseRoot 'scripts\collect-ci-evidence.ps1')
    $package = Join-Path $caseRoot '.local-test-data\packages\version-0123456789abcdef0123456789abcdef'
    $control = Join-Path $package 'import-control'
    $unrelated = Join-Path $package 'unrelated'
    New-Item -ItemType Directory -Path $control, $unrelated -Force | Out-Null
    'expected dynamic CRT rejection' | Set-Content -LiteralPath (Join-Path $control 'expected-rejection.log')
    'not executable; negative fixture only' | Set-Content -LiteralPath (Join-Path $control 'control.exe')
    'private image placeholder' | Set-Content -LiteralPath (Join-Path $control 'screenshot.png')
    'private model placeholder' | Set-Content -LiteralPath (Join-Path $control 'weights.onnx')
    'unrelated diagnostic' | Set-Content -LiteralPath (Join-Path $unrelated 'expected-rejection.log')
    if ($hasRuns) {
        $runs = Join-Path $caseRoot '.local-test-data\runs\run-a'
        New-Item -ItemType Directory -Path $runs -Force | Out-Null
        '{}' | Set-Content -LiteralPath (Join-Path $runs 'version-tests.json')
        'private screenshot placeholder' | Set-Content -LiteralPath (Join-Path $runs 'original.png')
        'private diagnostic placeholder' | Set-Content -LiteralPath (Join-Path $runs 'arbitrary.log')
    }
    $destination = Join-Path $caseRoot 'collected'
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $caseRoot 'scripts\collect-ci-evidence.ps1') -Destination $destination
    Assert-True ($LASTEXITCODE -eq 0) 'CI evidence collector failed'
    $collected = @(Get-ChildItem -LiteralPath $destination -File -Recurse)
    Assert-True ($collected.Count -eq $(if ($hasRuns) { 2 } else { 1 })) 'CI evidence collection included private or unrelated files'
    Assert-True (@($collected | Where-Object Name -EQ 'expected-rejection.log').Count -eq 1) 'Package-only diagnostics were lost'
    Assert-True (@($collected | Where-Object Extension -In @('.png', '.onnx', '.exe')).Count -eq 0) 'CI evidence collection exposed private assets'
}

Write-Output "Repository process regression passed"
