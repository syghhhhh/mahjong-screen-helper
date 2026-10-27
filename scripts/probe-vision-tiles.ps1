[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RgbFile,
    [Parameter(Mandatory = $true)][uint32]$Width,
    [Parameter(Mandatory = $true)][uint32]$Height,
    [Parameter(Mandatory = $true)][string]$TilesDir,
    [Parameter(Mandatory = $true)][string]$ExpectedHand,
    [string]$OriginalScreenshot,
    [string]$SourceUrl
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$rgbPath = (Resolve-Path -LiteralPath $RgbFile).Path
$tilesPath = (Resolve-Path -LiteralPath $TilesDir).Path
if ($OriginalScreenshot) { $OriginalScreenshot = (Resolve-Path -LiteralPath $OriginalScreenshot).Path }
$runId = 'local-vision-tiles-{0}' -f [guid]::NewGuid().ToString('N')
$runDir = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $runId)
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$logPath = Join-Path $runDir 'vision-tiles.log'
$summaryPath = Join-Path $runDir 'vision-tiles.json'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$command = 'cargo run -p vision-core --example inspect_tiles --locked -- <local RGB> {0} {1} <local tiles>' -f $Width,$Height
$previousPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    $output = @(& cargo run --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p vision-core --example inspect_tiles --locked -j 1 -- $rgbPath $Width $Height $tilesPath 2>&1 | ForEach-Object { [string]$_ })
    $exitCode = $LASTEXITCODE
} finally {
    $ErrorActionPreference = $previousPreference
}
$output | Set-Content -LiteralPath $logPath -Encoding utf8
$detectedLine = @($output | Where-Object { $_ -match '^detected ([0-9]+) tile candidates$' } | Select-Object -Last 1)
$labelsLine = @($output | Where-Object { $_ -match '^best labels: (.*)$' } | Select-Object -Last 1)
$confidenceLine = @($output | Where-Object { $_ -match '^minimum similarity: ([0-9.]+)$' } | Select-Object -Last 1)
$acceptedLine = @($output | Where-Object { $_ -match '^accepted by default thresholds: (true|false)$' } | Select-Object -Last 1)
$detected = if ($detectedLine.Count -eq 1) { [int]([regex]::Match($detectedLine[0], '^detected ([0-9]+)').Groups[1].Value) } else { -1 }
$labels = if ($labelsLine.Count -eq 1) { [regex]::Match($labelsLine[0], '^best labels: (.*)$').Groups[1].Value } else { '' }
$confidence = if ($confidenceLine.Count -eq 1) { [double]([regex]::Match($confidenceLine[0], '^minimum similarity: ([0-9.]+)$').Groups[1].Value) } else { 0 }
$accepted = $acceptedLine.Count -eq 1 -and $acceptedLine[0] -eq 'accepted by default thresholds: true'
$expected = ($ExpectedHand -split '\s+' | Where-Object { $_ }) -join ' '
$passed = $exitCode -eq 0 -and $labels -eq $expected -and $detected -eq @($expected -split ' ').Count
$templateHashes = @(
    Get-ChildItem -LiteralPath $tilesPath -File -Filter '*.png' |
        Sort-Object Name |
        ForEach-Object { [pscustomobject]@{ name=$_.Name; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } }
)
$summary = [pscustomobject]@{
    schema_version = '1.0'
    run_id = $runId
    kind = 'local-vision-candidate-probe'
    source_sha = $sourceSha
    command = $command
    input_sha256 = (Get-FileHash -LiteralPath $rgbPath -Algorithm SHA256).Hash.ToLowerInvariant()
    original_screenshot_sha256 = if ($OriginalScreenshot) { (Get-FileHash -LiteralPath $OriginalScreenshot -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
    template_hashes = $templateHashes
    source_url = $SourceUrl
    expected_hand = $expected
    candidate_labels = $labels
    detected_slots = $detected
    minimum_similarity = $confidence
    accepted_by_default_thresholds = $accepted
    candidate_labels_correct = $passed
    log = $logPath
    environment = [pscustomobject]@{ os=[System.Environment]::OSVersion.VersionString; architecture=[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
    completed_at = (Get-Date).ToUniversalTime().ToString('o')
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output ("Local candidate probe {0}: {1}; accepted={2}; summary: {3}" -f $(if ($passed) {'passed'} else {'failed'}), $labels, $accepted, $summaryPath)
if (-not $passed) { exit 1 }
