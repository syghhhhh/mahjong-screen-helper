[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RgbFile,
    [Parameter(Mandatory = $true)][uint32]$Width,
    [Parameter(Mandatory = $true)][uint32]$Height,
    [uint32]$X = 0,
    [uint32]$Y = 0,
    [Parameter(Mandatory = $true)][uint32]$RegionWidth,
    [Parameter(Mandatory = $true)][uint32]$RegionHeight,
    [Parameter(Mandatory = $true)][int]$ExpectedSlots,
    [string]$OriginalScreenshot,
    [string]$SourceUrl
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$rgbPath = (Resolve-Path -LiteralPath $RgbFile).Path
if ($OriginalScreenshot) { $OriginalScreenshot = (Resolve-Path -LiteralPath $OriginalScreenshot).Path }
$runId = 'local-vision-{0}' -f [guid]::NewGuid().ToString('N')
$runDir = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $runId)
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$logPath = Join-Path $runDir 'vision-probe.log'
$summaryPath = Join-Path $runDir 'vision-probe.json'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$command = 'cargo run -p vision-core --example inspect_raw --locked -- <local RGB> {0} {1} {2} {3} {4} {5}' -f $Width,$Height,$X,$Y,$RegionWidth,$RegionHeight
$output = @(& cargo run --manifest-path (Join-Path $repoRoot 'Cargo.toml') -p vision-core --example inspect_raw --locked -- $rgbPath $Width $Height $X $Y $RegionWidth $RegionHeight 2>&1 | ForEach-Object { [string]$_ })
$output | Set-Content -LiteralPath $logPath -Encoding utf8
$exitCode = $LASTEXITCODE
$line = @($output | Where-Object { $_ -match '^detected ([0-9]+) tile candidates$' } | Select-Object -Last 1)
$detected = if ($line.Count -eq 1) { [int]([regex]::Match($line[0], '^detected ([0-9]+)').Groups[1].Value) } else { -1 }
$passed = $exitCode -eq 0 -and $detected -eq $ExpectedSlots
$summary = [pscustomobject]@{
    schema_version = '1.0'
    run_id = $runId
    kind = 'local-vision-slot-probe'
    source_sha = $sourceSha
    command = $command
    input_sha256 = (Get-FileHash -LiteralPath $rgbPath -Algorithm SHA256).Hash.ToLowerInvariant()
    original_screenshot_sha256 = if ($OriginalScreenshot) { (Get-FileHash -LiteralPath $OriginalScreenshot -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
    source_url = $SourceUrl
    region = [pscustomobject]@{ x=$X; y=$Y; width=$RegionWidth; height=$RegionHeight }
    expected_slots = $ExpectedSlots
    detected_slots = $detected
    passed = $passed
    log = $logPath
    environment = [pscustomobject]@{ os=[System.Environment]::OSVersion.VersionString; architecture=[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
    completed_at = (Get-Date).ToUniversalTime().ToString('o')
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output ("Local vision slot probe {0}: {1}/{2}; summary: {3}" -f $(if ($passed) {'passed'} else {'failed'}), $detected, $ExpectedSlots, $summaryPath)
if (-not $passed) { exit 1 }
