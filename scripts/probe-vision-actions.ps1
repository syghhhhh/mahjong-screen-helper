[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$PositiveRgb,
    [Parameter(Mandatory = $true)][string[]]$NegativeRgbFiles,
    [Parameter(Mandatory = $true)][uint32]$Width,
    [Parameter(Mandatory = $true)][uint32]$Height,
    [Parameter(Mandatory = $true)][string]$ActionsDir,
    [Parameter(Mandatory = $true)][ValidateSet('Chi','Pon','Kan','Riichi')][string]$ExpectedAction,
    [string]$OriginalScreenshot,
    [string]$SourceUrl
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$positivePath = (Resolve-Path -LiteralPath $PositiveRgb).Path
$negativePaths = @($NegativeRgbFiles | ForEach-Object { (Resolve-Path -LiteralPath $_).Path })
$actionsPath = (Resolve-Path -LiteralPath $ActionsDir).Path
if ($OriginalScreenshot) { $OriginalScreenshot = (Resolve-Path -LiteralPath $OriginalScreenshot).Path }
$runId = 'local-vision-actions-{0}' -f [guid]::NewGuid().ToString('N')
$runDir = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $runId)
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$logPath = Join-Path $runDir 'vision-actions.log'
$summaryPath = Join-Path $runDir 'vision-actions.json'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$runs = @()
foreach ($sample in @($positivePath) + $negativePaths) {
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& cargo run --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p vision-core --example inspect_actions --release --locked -j 1 -- $sample $Width $Height $actionsPath 2>&1 | ForEach-Object { [string]$_ })
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    $line = @($output | Where-Object { $_ -match '^actions: ' } | Select-Object -Last 1)
    $actions = if ($line.Count -eq 1) { $line[0] -replace '^actions: ','' } else { '' }
    $runs += [pscustomobject]@{
        input_sha256 = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash.ToLowerInvariant()
        role = if ($sample -eq $positivePath) { 'positive' } else { 'negative' }
        actions = $actions
        exit_code = $exitCode
        output = $output
    }
}
$runs | ForEach-Object { "[$($_.role)] $($_.input_sha256)"; $_.output } | Set-Content -LiteralPath $logPath -Encoding utf8
$positiveOk = $runs[0].exit_code -eq 0 -and $runs[0].actions -eq "Some([$ExpectedAction])"
$negativeOk = @($runs | Select-Object -Skip 1 | Where-Object { $_.exit_code -ne 0 -or $_.actions -ne 'Some([])' }).Count -eq 0
$passed = $positiveOk -and $negativeOk
$templateHashes = @(
    Get-ChildItem -LiteralPath $actionsPath -File -Filter '*.png' |
        Sort-Object Name |
        ForEach-Object { [pscustomobject]@{ name=$_.Name; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } }
)
$summary = [pscustomobject]@{
    schema_version = '1.0'
    run_id = $runId
    kind = 'local-vision-action-probe'
    source_sha = $sourceSha
    command = 'cargo run -p vision-core --example inspect_actions --release --locked -- <local RGB> <width> <height> <local actions>'
    original_screenshot_sha256 = if ($OriginalScreenshot) { (Get-FileHash -LiteralPath $OriginalScreenshot -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
    source_url = $SourceUrl
    template_hashes = $templateHashes
    expected_action = $ExpectedAction
    cases = @($runs | ForEach-Object { [pscustomobject]@{ role=$_.role; input_sha256=$_.input_sha256; actions=$_.actions; exit_code=$_.exit_code } })
    passed = $passed
    log = $logPath
    environment = [pscustomobject]@{ os=[System.Environment]::OSVersion.VersionString; architecture=[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
    completed_at = (Get-Date).ToUniversalTime().ToString('o')
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output ("Local action probe {0}: {1} positive, {2} negative; summary: {3}" -f $(if ($passed) {'passed'} else {'failed'}), [int]$positiveOk, $negativePaths.Count, $summaryPath)
if (-not $passed) { exit 1 }
