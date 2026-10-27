[CmdletBinding()]
param(
    [string]$Version = 'v0.0.1',
    [string]$RunId = ('all-{0}' -f [guid]::NewGuid().ToString('N'))
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $RunId)
if (Test-Path -LiteralPath $runRoot) { throw "Run id already exists: $RunId" }
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
$logPath = Join-Path $runRoot 'all-tests.log'
$summaryPath = Join-Path $runRoot 'all-tests.json'
$started = (Get-Date).ToUniversalTime().ToString('o')
$results = @()
$passed = $true
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceSha -notmatch '^[0-9a-f]{40}$') { throw 'Cannot resolve source commit' }

function Invoke-CheckedScript([string]$Path, [string[]]$Arguments = @()) {
    $command = 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" {1}' -f $Path, ($Arguments -join ' ')
    Add-Content -LiteralPath $logPath -Value ("RUN $command") -Encoding utf8
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $Path @Arguments 2>&1 |
            ForEach-Object { [string]$_ } | Tee-Object -FilePath $logPath -Append
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    $script:results += [pscustomobject]@{ test = $Path; command = $command.Trim(); exit_code = $code; skipped = $false }
    if ($code -ne 0) { $script:passed = $false }
}

try {
    Invoke-CheckedScript (Join-Path $PSScriptRoot 'validate-task.ps1') @('-Version', $Version)
    Invoke-CheckedScript (Join-Path $PSScriptRoot 'test-version.ps1') @('-Version', $Version)
    $regressionRoot = Join-Path $repoRoot 'tests\regression'
    $tests = @(Get-ChildItem -LiteralPath $regressionRoot -Filter '*.test.ps1' -File | Sort-Object FullName)
    if ($tests.Count -eq 0) { throw "No regression tests found: $regressionRoot" }
    foreach ($test in $tests) { Invoke-CheckedScript $test.FullName }
} catch {
    $passed = $false
    Add-Content -LiteralPath $logPath -Value ("ERROR {0}" -f $_.Exception.Message) -Encoding utf8
    Write-Output ("ERROR {0}" -f $_.Exception.Message)
}

$summary = [pscustomobject]@{
    schema_version = '1.0'; run_id = $RunId; kind = 'all'; version = $Version
    passed = $passed; started_at = $started; completed_at = (Get-Date).ToUniversalTime().ToString('o')
    source_sha = $sourceSha; log = $logPath; results = @($results)
    environment = [pscustomobject]@{ os = [System.Environment]::OSVersion.VersionString; architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output ("All tests {0}; summary: {1}" -f $(if ($passed) {'passed'} else {'failed'}), $summaryPath)
if (-not $passed) { exit 1 }
