[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [ValidateRange(1, 100)][int]$Samples = 5,
    [ValidateRange(100, 60000)][int]$LimitMilliseconds = 2000
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$exePath = (Resolve-Path -LiteralPath $Executable).Path
if ([IO.Path]::GetExtension($exePath) -ne '.exe') { throw 'Expected a Windows executable' }
$runId = 'local-portable-startup-{0}' -f [guid]::NewGuid().ToString('N')
$runDir = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $runId)
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$summaryPath = Join-Path $runDir 'portable-startup.json'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$results = [Collections.Generic.List[object]]::new()
$failure = $null

for ($sample = 1; $sample -le $Samples; $sample++) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        $process = Start-Process -FilePath $exePath -ArgumentList '--smoke' -WorkingDirectory (Split-Path -Parent $exePath) -WindowStyle Hidden -PassThru -Wait
        $timer.Stop()
        $results.Add([pscustomobject]@{
            sample = $sample
            elapsed_ms = [math]::Round($timer.Elapsed.TotalMilliseconds, 1)
            exit_code = $process.ExitCode
        })
        $process.Dispose()
        if ($results[$results.Count - 1].exit_code -ne 0) {
            $failure = "Portable startup smoke failed on sample $sample"
            break
        }
    } catch {
        $timer.Stop()
        $failure = $_.Exception.Message
        break
    }
}

$maximum = [double](($results | Measure-Object -Property elapsed_ms -Maximum).Maximum)
$passed = $null -eq $failure -and $results.Count -eq $Samples -and $maximum -lt $LimitMilliseconds
if ($null -eq $failure -and $maximum -ge $LimitMilliseconds) { $failure = 'Startup exceeded the local time target' }
$summary = [pscustomobject]@{
    schema_version = '1.0'
    run_id = $runId
    kind = 'local-warm-startup-smoke'
    source_sha = $sourceSha
    executable_sha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant()
    command = 'tests/local/portable-startup.test.ps1 -Executable <portable exe> -Samples {0} -LimitMilliseconds {1}' -f $Samples,$LimitMilliseconds
    results = $results
    sample_count = $results.Count
    maximum_ms = $maximum
    limit_ms = $LimitMilliseconds
    passed = $passed
    failure = $failure
    limitation = 'Warm launches on a development machine; clean Windows cold-start acceptance is separate.'
    environment = [pscustomobject]@{
        os = [Environment]::OSVersion.VersionString
        architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    completed_at = (Get-Date).ToUniversalTime().ToString('o')
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output "Warm portable startup $(if ($passed) { 'passed' } else { 'failed' }); max=$maximum ms; summary: $summaryPath"
if (-not $passed) { exit 1 }
