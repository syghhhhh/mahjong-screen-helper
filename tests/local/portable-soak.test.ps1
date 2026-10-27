[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [ValidateRange(0.02, 1440)][double]$DurationMinutes = 30,
    [ValidateRange(1, 60)][int]$SampleSeconds = 5
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$exePath = (Resolve-Path -LiteralPath $Executable).Path
if ([IO.Path]::GetExtension($exePath) -ne '.exe') { throw 'Expected a Windows executable' }
$runId = 'local-portable-soak-{0}' -f [guid]::NewGuid().ToString('N')
$runDir = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $runId)
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$summaryPath = Join-Path $runDir 'portable-soak.json'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$startedAt = (Get-Date).ToUniversalTime()
$deadline = $startedAt.AddMinutes($DurationMinutes)
$samples = [Collections.Generic.List[object]]::new()
$failure = $null
$app = $null

try {
    $app = Start-Process -FilePath $exePath -WorkingDirectory (Split-Path -Parent $exePath) -WindowStyle Hidden -PassThru
    while ((Get-Date).ToUniversalTime() -lt $deadline) {
        Start-Sleep -Seconds $SampleSeconds
        $app.Refresh()
        if ($app.HasExited) {
            $failure = "Portable app exited early with code $($app.ExitCode)"
            break
        }
        $samples.Add([pscustomobject]@{
            at = (Get-Date).ToUniversalTime().ToString('o')
            private_bytes = [long]$app.PrivateMemorySize64
            working_set_bytes = [long]$app.WorkingSet64
            cpu_seconds = [math]::Round($app.TotalProcessorTime.TotalSeconds, 3)
        })
    }
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($null -ne $app) {
        $app.Refresh()
        if (-not $app.HasExited) {
            Stop-Process -Id $app.Id -Force
            $app.WaitForExit(10000) | Out-Null
        }
        $app.Dispose()
    }
}

$endedAt = (Get-Date).ToUniversalTime()
$elapsedSeconds = ($endedAt - $startedAt).TotalSeconds
$maxPrivate = [long](($samples | Measure-Object -Property private_bytes -Maximum).Maximum)
$maxWorkingSet = [long](($samples | Measure-Object -Property working_set_bytes -Maximum).Maximum)
$lastCpu = if ($samples.Count -gt 0) { $samples[$samples.Count - 1].cpu_seconds } else { 0 }
$memoryLimitBytes = 250MB
$passed = $null -eq $failure -and $samples.Count -gt 0 -and $elapsedSeconds -ge ($DurationMinutes * 60) -and $maxWorkingSet -lt $memoryLimitBytes
if ($null -eq $failure -and $maxWorkingSet -ge $memoryLimitBytes) {
    $failure = 'Working set exceeded 250 MB target'
}
$summary = [pscustomobject]@{
    schema_version = '1.0'
    run_id = $runId
    kind = 'local-idle-overlay-soak'
    source_sha = $sourceSha
    executable_sha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant()
    command = 'tests/local/portable-soak.test.ps1 -Executable <portable exe> -DurationMinutes {0} -SampleSeconds {1}' -f $DurationMinutes,$SampleSeconds
    duration_target_minutes = $DurationMinutes
    elapsed_seconds = [math]::Round($elapsedSeconds, 3)
    samples = $samples
    sample_count = $samples.Count
    max_private_bytes = $maxPrivate
    max_working_set_bytes = $maxWorkingSet
    cpu_seconds = $lastCpu
    memory_limit_bytes = $memoryLimitBytes
    passed = $passed
    failure = $failure
    limitation = 'Idle overlay without a live Mahjong Soul target; capture latency and false actions are not measured.'
    environment = [pscustomobject]@{
        os = [Environment]::OSVersion.VersionString
        architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    started_at = $startedAt.ToString('o')
    completed_at = $endedAt.ToString('o')
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output "Idle portable soak $(if ($passed) { 'passed' } else { 'failed' }); samples=$($samples.Count); peak working set=$([math]::Round($maxWorkingSet / 1MB, 1)) MB; summary: $summaryPath"
if (-not $passed) { exit 1 }
