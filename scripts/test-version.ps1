[CmdletBinding()]
param(
    [string]$Version = 'v0.0.1',
    [string]$RunId = ('version-{0}-{1}' -f $Version, [guid]::NewGuid().ToString('N'))
)

$ErrorActionPreference = 'Stop'
$env:CARGO_BUILD_JOBS = '1'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $repoRoot ('.local-test-data\runs\{0}' -f $RunId)
if (Test-Path -LiteralPath $runRoot) { throw "Run id already exists: $RunId" }
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
$logPath = Join-Path $runRoot 'version-tests.log'
$summaryPath = Join-Path $runRoot 'version-tests.json'
$started = (Get-Date).ToUniversalTime().ToString('o')
$results = @()
$passed = $true
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceSha -notmatch '^[0-9a-f]{40}$') { throw 'Cannot resolve source commit' }

function Invoke-CheckedScript([string]$Path) {
    $command = 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}"' -f $Path
    Add-Content -LiteralPath $logPath -Value ("RUN $command") -Encoding utf8
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $Path 2>&1 |
            ForEach-Object { [string]$_ } | Tee-Object -FilePath $logPath -Append
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    $script:results += [pscustomobject]@{ test = $Path; command = $command; exit_code = $code; skipped = $false }
    if ($code -ne 0) { $script:passed = $false }
}

try {
    Invoke-CheckedScript (Join-Path $PSScriptRoot 'validate-task.ps1')
    $testRoot = Join-Path $repoRoot ("tests\version\$Version")
    $tests = @(Get-ChildItem -LiteralPath $testRoot -Filter '*.test.ps1' -File | Sort-Object FullName)
    if ($tests.Count -eq 0) { throw "No version tests found: $testRoot" }
    foreach ($test in $tests) { Invoke-CheckedScript $test.FullName }
} catch {
    $passed = $false
    Add-Content -LiteralPath $logPath -Value ("ERROR {0}" -f $_.Exception.Message) -Encoding utf8
    Write-Output ("ERROR {0}" -f $_.Exception.Message)
}

$fixtures = @()
$fixtureRoot = Join-Path $repoRoot 'tests\fixtures\synthetic'
if (Test-Path -LiteralPath $fixtureRoot) {
    $fixtures = @(Get-ChildItem -LiteralPath $fixtureRoot -File -Recurse |
        Where-Object Name -Ne '.gitkeep' |
        ForEach-Object { [pscustomobject]@{ path = $_.FullName.Substring($repoRoot.Length + 1).Replace('\','/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } })
}
$summary = [pscustomobject]@{
    schema_version = '1.0'; run_id = $RunId; kind = 'version'; version = $Version
    passed = $passed; started_at = $started; completed_at = (Get-Date).ToUniversalTime().ToString('o')
    source_sha = $sourceSha; log = $logPath; results = @($results); fixture_hashes = @($fixtures)
    environment = [pscustomobject]@{ os = [System.Environment]::OSVersion.VersionString; architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output ("Version tests {0}; summary: {1}" -f $(if ($passed) {'passed'} else {'failed'}), $summaryPath)
if (-not $passed) { exit 1 }
