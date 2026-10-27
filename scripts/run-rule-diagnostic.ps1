[CmdletBinding()]
param(
    [ValidateRange(1, 30)][int]$MaxAttempts = 10,
    [ValidateRange(1, 30)][int]$TimeoutSeconds = 30
)
$ErrorActionPreference = 'Stop'
$utf8 = New-Object Text.UTF8Encoding($false)
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
if ($manifest.schema_version -ne '1.0' -or $manifest.kind -ne 'native-rule-probe-package' -or
    [string]$manifest.source_sha -cnotmatch '^[0-9a-f]{40}$' -or [string]$manifest.exe_sha256 -cnotmatch '^[0-9a-f]{64}$') {
    throw 'Invalid rule probe manifest; nothing was executed'
}
$exe = Join-Path $PSScriptRoot 'RuleConcurrencyProbe.exe'
# Hold a read-only sharing lock from hash validation through the final attempt.
$binary = [IO.File]::Open($exe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try {
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $hash = ([BitConverter]::ToString($hasher.ComputeHash($binary))).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
    if ($hash -cne $manifest.exe_sha256) { throw 'Probe SHA-256 mismatch; nothing was executed' }
    $runRoot = Join-Path $PSScriptRoot ('results-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $runRoot | Out-Null
    $environment = [ordered]@{
        os=[Environment]::OSVersion.VersionString
        architecture=[string][Runtime.InteropServices.RuntimeInformation]::OSArchitecture
        logical_processors=[Environment]::ProcessorCount
        inherited_settings=[ordered]@{RUST_MIN_STACK=$env:RUST_MIN_STACK;RUST_TEST_THREADS=$env:RUST_TEST_THREADS;RUST_BACKTRACE=$env:RUST_BACKTRACE;_NO_DEBUG_HEAP=$env:_NO_DEBUG_HEAP}
    }
    try { $environment.processor = @(Get-CimInstance Win32_Processor -OperationTimeoutSec 3 | ForEach-Object Name) } catch { $environment.processor = @('unavailable') }
    $report = [ordered]@{
        schema_version='1.0';kind='native-host-rule-comparison';source_sha=$manifest.source_sha
        exe_sha256=$hash;environment=$environment;max_attempts=$MaxAttempts;timeout_seconds=$TimeoutSeconds
        attempts=@();all_attempts_passed=$true;acceptance_verified=$false;completed_at=$null
        limitations='Finite diagnostic samples only. No debugger, affinity, heap or thread overrides are applied. Not hardware diagnosis, product acceptance or native-parallel-resolution gate evidence.'
    }
    for ($index=1; $index -le $MaxAttempts; $index++) {
        $child = New-Object Diagnostics.Process
        $child.StartInfo.FileName = $exe
        $child.StartInfo.Arguments = '--exact independently_owned_hands_keep_exact_results_across_ten_workers --nocapture'
        $child.StartInfo.UseShellExecute = $false
        $child.StartInfo.CreateNoWindow = $true
        $child.StartInfo.RedirectStandardOutput = $true
        $child.StartInfo.RedirectStandardError = $true
        $child.StartInfo.StandardOutputEncoding = $utf8
        $child.StartInfo.StandardErrorEncoding = $utf8
        $launched = $false
        $row = [ordered]@{attempt=$index;started_at=[DateTime]::UtcNow.ToString('o');command='RuleConcurrencyProbe.exe --exact independently_owned_hands_keep_exact_results_across_ten_workers --nocapture';exit_code=$null;exit_code_hex=$null;timed_out=$false;passed=$false;stdout="attempt-$index.stdout.log";stderr="attempt-$index.stderr.log";tool_error=$null}
        try {
            $launched = $child.Start()
            if (-not $launched) { throw 'Child process did not start' }
            $stdout = $child.StandardOutput.ReadToEndAsync()
            $stderr = $child.StandardError.ReadToEndAsync()
            $row.timed_out = -not $child.WaitForExit($TimeoutSeconds * 1000)
            if ($row.timed_out) { $child.Kill() }
            $child.WaitForExit()
            $out = $stdout.GetAwaiter().GetResult()
            $err = $stderr.GetAwaiter().GetResult()
            [IO.File]::WriteAllText((Join-Path $runRoot $row.stdout), $out, $utf8)
            [IO.File]::WriteAllText((Join-Path $runRoot $row.stderr), $err, $utf8)
            $row.exit_code = $child.ExitCode
            $row.exit_code_hex = '0x{0:X8}' -f ([int64]$child.ExitCode -band 0xffffffffL)
            $row.passed = -not $row.timed_out -and $child.ExitCode -eq 0 -and
                $out -match '(?m)^running 1 test\s*$' -and
                $out -match '(?m)^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'
        } catch { $row.tool_error = 'Process start, observation or log persistence failed' }
        finally {
            try { if ($launched -and -not $child.HasExited) { $child.Kill(); $child.WaitForExit() } }
            finally { $child.Dispose() }
        }
        $report.attempts += [pscustomobject]$row
        $report.all_attempts_passed = $report.all_attempts_passed -and $row.passed
        $report.completed_at = [DateTime]::UtcNow.ToString('o')
        $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $runRoot 'report.json') -Encoding utf8
        Write-Output ("Attempt {0}: exit={1}; timeout={2}; passed={3}" -f $index,$row.exit_code_hex,$row.timed_out,$row.passed)
        if (-not $row.passed) { break }
    }
    Write-Output "Report: $runRoot\report.json"
} finally { $binary.Dispose() }
if (-not $report.all_attempts_passed) { exit 1 }
exit 0
