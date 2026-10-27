[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$SourceSha,
    [string[]]$RequiredKinds = @(),
    [string[]]$Paths = @()
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$localRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.local-test-data'))
$localPrefix = $localRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
if ($SourceSha -cnotmatch '^[0-9a-f]{40}$') { throw 'Invalid source SHA for local evidence' }
if ($RequiredKinds.Count -eq 0 -and $Paths.Count -gt 0) { throw 'Unexpected local evidence paths' }
if ($RequiredKinds.Count -gt 0 -and $Paths.Count -eq 0) { throw 'Required local evidence is missing' }

function Resolve-LocalFile([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) { throw 'Empty local evidence path' }
    $candidate = if ([IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path $repoRoot $Path }
    $resolved = (Resolve-Path -LiteralPath $candidate -ErrorAction Stop).Path
    $full = [IO.Path]::GetFullPath($resolved)
    if (-not $full.StartsWith($localPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Evidence must stay inside .local-test-data: $Path"
    }
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "Evidence file missing: $Path" }
    return $full
}

$seenKinds = @{}
$seenRuns = @{}
foreach ($path in $Paths) {
    $summaryPath = Resolve-LocalFile $path
    $summary = Get-Content -LiteralPath $summaryPath -Raw -Encoding utf8 | ConvertFrom-Json
    $kind = [string]$summary.kind
    if ($RequiredKinds -cnotcontains $kind) { throw "Unexpected local evidence kind: $kind" }
    if ($seenKinds.ContainsKey($kind)) { throw "Duplicate local evidence kind: $kind" }
    $seenKinds[$kind] = $true
    $runId = [string]$summary.run_id
    if ([string]::IsNullOrWhiteSpace($runId) -or $seenRuns.ContainsKey($runId)) { throw "Missing or duplicate local run ID: $runId" }
    $seenRuns[$runId] = $true
    if ($summary.passed -ne $true) { throw "Local evidence did not pass: $kind" }
    if ([string]$summary.source_sha -cne $SourceSha) { throw "Local evidence source SHA mismatch: $kind" }
    if ([string]::IsNullOrWhiteSpace([string]$summary.command)) { throw "Local evidence command missing: $kind" }
    if ([string]::IsNullOrWhiteSpace([string]$summary.environment.os) -or [string]::IsNullOrWhiteSpace([string]$summary.environment.architecture)) {
        throw "Local evidence environment missing: $kind"
    }
    $timestamp = [datetimeoffset]::MinValue
    if (-not [datetimeoffset]::TryParse([string]$summary.completed_at, [ref]$timestamp)) { throw "Local evidence completion time missing: $kind" }
    if ([string]::IsNullOrWhiteSpace([string]$summary.log)) { throw "Local evidence log missing: $kind" }
    $null = Resolve-LocalFile ([string]$summary.log)
    $fixtures = @($summary.fixture_hashes)
    if ($null -eq $summary.fixture_hashes -or $fixtures.Count -eq 0) { throw "Local fixture hashes missing: $kind" }
    foreach ($fixture in $fixtures) {
        $fixturePath = Resolve-LocalFile ([string]$fixture.path)
        $expected = [string]$fixture.sha256
        if ($expected -cnotmatch '^[0-9a-f]{64}$') { throw "Invalid local fixture hash: $kind" }
        $actual = (Get-FileHash -LiteralPath $fixturePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -cne $expected) { throw "Local fixture hash mismatch: $kind" }
    }
}
foreach ($kind in $RequiredKinds) {
    if (-not $seenKinds.ContainsKey($kind)) { throw "Required local evidence kind missing: $kind" }
}
Write-Output "Local evidence verified: $($RequiredKinds -join ', ')"
