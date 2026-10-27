[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceSha -cnotmatch '^[0-9a-f]{40}$') { throw 'Missing source SHA' }
$dirty = @(git -C $repoRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0 -or $dirty.Count -ne 0) { throw 'Diagnostic package requires a clean source checkout' }
$output = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\', '/')
if (Test-Path -LiteralPath $output) { throw 'Diagnostic output directory already exists' }
$buildRoot = Join-Path $repoRoot ('.local-test-data\runs\rule-probe-build-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $buildRoot | Out-Null
$child = New-Object Diagnostics.Process
$child.StartInfo.FileName = (Get-Command cargo -ErrorAction Stop).Source
$child.StartInfo.Arguments = 'test -p riichi-core --test concurrency --no-run --locked -j 1 --message-format=json'
$child.StartInfo.WorkingDirectory = $repoRoot
$child.StartInfo.UseShellExecute = $false
$child.StartInfo.CreateNoWindow = $true
$child.StartInfo.RedirectStandardOutput = $true
$child.StartInfo.RedirectStandardError = $true
$child.StartInfo.StandardOutputEncoding = New-Object Text.UTF8Encoding($false)
$child.StartInfo.StandardErrorEncoding = New-Object Text.UTF8Encoding($false)
$launched = $false
try {
    $launched = $child.Start()
    $stdout = $child.StandardOutput.ReadToEndAsync()
    $stderr = $child.StandardError.ReadToEndAsync()
    $timedOut = -not $child.WaitForExit(120000)
    if ($timedOut) { $child.Kill() }
    $child.WaitForExit()
    $out = $stdout.GetAwaiter().GetResult()
    $err = $stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $buildRoot 'cargo.jsonl'), $out)
    [IO.File]::WriteAllText((Join-Path $buildRoot 'cargo.stderr.log'), $err)
    if ($timedOut -or $child.ExitCode -ne 0) { throw "Diagnostic compile failed or timed out; see $buildRoot" }
} finally {
    try { if ($launched -and -not $child.HasExited) { $child.Kill(); $child.WaitForExit() } }
    finally { $child.Dispose() }
}
$artifacts = @($out -split '\r?\n' | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'concurrency' -and $_.profile.test -eq $true -and $_.executable })
if ($artifacts.Count -ne 1) { throw 'Cannot identify one exact concurrency test executable from Cargo output' }
$exe = [string]$artifacts[0].executable
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'validate-portable-imports.ps1') -Executable $exe
if ($LASTEXITCODE -ne 0) { throw 'Diagnostic probe needs a separate runtime' }
$afterSha = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot recheck diagnostic source commit' }
$afterDirty = @(git -C $repoRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0 -or $afterSha -cne $sourceSha -or $afterDirty.Count -ne 0) { throw 'Source changed during diagnostic build' }
New-Item -ItemType Directory -Path (Join-Path $output 'source') -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $output 'RuleConcurrencyProbe.exe')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'run-rule-diagnostic.ps1') -Destination (Join-Path $output 'Run-RuleDiagnostic.ps1')
Copy-Item -LiteralPath (Join-Path $repoRoot 'crates\riichi-core\src\lib.rs') -Destination (Join-Path $output 'source\rule-core.rs')
Copy-Item -LiteralPath (Join-Path $repoRoot 'crates\riichi-core\tests\concurrency.rs') -Destination (Join-Path $output 'source\concurrency.rs')
Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination (Join-Path $output 'LICENSE')
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs\native-rule-comparison.md') -Destination (Join-Path $output 'README.md')
'@echo off', 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0Run-RuleDiagnostic.ps1"', 'set "diagnostic_exit=%errorlevel%"', 'echo Results are in the results-* folder.', 'pause', 'exit /b %diagnostic_exit%' |
    Set-Content -LiteralPath (Join-Path $output 'Run-Diagnostics.cmd') -Encoding ascii
$files = @(Get-ChildItem -LiteralPath $output -File -Recurse | ForEach-Object {
    [pscustomobject]@{path=$_.FullName.Substring($output.Length+1).Replace('\','/');sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant();bytes=$_.Length}
})
[pscustomobject]@{
    schema_version='1.0';kind='native-rule-probe-package';source_sha=$sourceSha
    exe_sha256=(Get-FileHash -LiteralPath (Join-Path $output 'RuleConcurrencyProbe.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    files=$files;built_at=[DateTime]::UtcNow.ToString('o');acceptance_verified=$false
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'manifest.json') -Encoding utf8
Write-Output "Diagnostic package: $output"
exit 0
