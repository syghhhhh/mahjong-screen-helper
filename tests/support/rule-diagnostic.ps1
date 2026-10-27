$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$runRoot = Join-Path $repoRoot ('.local-test-data\rule-diagnostic-contract-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null
$control = Join-Path $runRoot 'control.exe'
& rustc --edition 2024 --crate-name rule_probe_control -C 'target-feature=+crt-static' (Join-Path $repoRoot 'tests\fixtures\synthetic\rule_probe_control.rs') -o $control
if ($LASTEXITCODE -ne 0) { throw 'Could not compile diagnostic runner fixture' }
$originalMode = $env:MAHJONG_PROBE_CONTROL_MODE
$originalMarker = $env:MAHJONG_PROBE_CONTROL_MARKER
try {
    foreach ($mode in @('pass', 'fail', 'empty', 'timeout', 'tampered')) {
        $folder = Join-Path $runRoot $mode
        New-Item -ItemType Directory -Path $folder | Out-Null
        Copy-Item -LiteralPath $control -Destination (Join-Path $folder 'RuleConcurrencyProbe.exe')
        Copy-Item -LiteralPath (Join-Path $repoRoot 'scripts\run-rule-diagnostic.ps1') -Destination (Join-Path $folder 'Run-RuleDiagnostic.ps1')
        [pscustomobject]@{schema_version='1.0';kind='native-rule-probe-package';source_sha=('a' * 40);exe_sha256=(Get-FileHash $control -Algorithm SHA256).Hash.ToLowerInvariant()} |
            ConvertTo-Json | Set-Content (Join-Path $folder 'manifest.json') -Encoding utf8
        if ($mode -eq 'tampered') { [IO.File]::AppendAllText((Join-Path $folder 'RuleConcurrencyProbe.exe'), 'changed') }
        $env:MAHJONG_PROBE_CONTROL_MODE = $mode
        $env:MAHJONG_PROBE_CONTROL_MARKER = Join-Path $folder 'executed.txt'
        $previous = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            $output = @(& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $folder 'Run-RuleDiagnostic.ps1') -MaxAttempts 3 -TimeoutSeconds 1 2>&1 | ForEach-Object { [string]$_ })
            $code = $LASTEXITCODE
        } finally { $ErrorActionPreference = $previous }
        $output | Set-Content (Join-Path $folder 'runner.log') -Encoding utf8
        $reports = @(Get-ChildItem -LiteralPath $folder -Recurse -File -Filter 'report.json')
        if ($mode -eq 'tampered') {
            if ($code -eq 0 -or (Test-Path -LiteralPath $env:MAHJONG_PROBE_CONTROL_MARKER) -or $reports.Count -ne 0) { throw 'Tampered diagnostic binary was executed' }
        } else {
            if ($reports.Count -ne 1) { throw "Missing diagnostic report: $mode" }
            $report = Get-Content $reports[0].FullName -Raw -Encoding utf8 | ConvertFrom-Json
            if ($report.acceptance_verified -ne $false -or $report.exe_sha256 -ne (Get-FileHash $control -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Diagnostic scope/hash metadata invalid' }
            $expectedAttempts = if ($mode -eq 'pass') { 3 } else { 1 }
            if (@($report.attempts).Count -ne $expectedAttempts) { throw "Diagnostic did not stop at first failure: $mode" }
            if ($mode -eq 'pass') {
                if ($code -ne 0 -or $report.all_attempts_passed -ne $true) { throw 'Valid diagnostic fixture rejected' }
            } else {
                if ($code -eq 0 -or $report.all_attempts_passed -ne $false) { throw "Invalid diagnostic fixture accepted: $mode" }
            }
            if ($mode -eq 'fail') {
                $attempt = $report.attempts[0]
                if ($attempt.exit_code -ne 7) { throw 'Native exit code was lost' }
                $out = Get-Content (Join-Path $reports[0].DirectoryName $attempt.stdout) -Raw -Encoding utf8
                $err = Get-Content (Join-Path $reports[0].DirectoryName $attempt.stderr) -Raw -Encoding utf8
                $chinese = [string][char]0x4e2d + [char]0x6587
                if (-not $out.Contains("$chinese failure output") -or -not $err.Contains("$chinese failure error")) { throw 'UTF-8 output or stderr lost' }
            }
            if ($mode -eq 'timeout' -and $report.attempts[0].timed_out -ne $true) { throw 'Timeout was not recorded' }
        }
        Write-Output "Diagnostic runner control $mode passed"
    }
} finally {
    $env:MAHJONG_PROBE_CONTROL_MODE = $originalMode
    $env:MAHJONG_PROBE_CONTROL_MARKER = $originalMarker
}
Write-Output "Rule diagnostic contract passed; retained controls: $runRoot"
exit 0
