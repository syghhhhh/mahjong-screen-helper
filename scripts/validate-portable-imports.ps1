[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Executable)

$ErrorActionPreference = 'Stop'
$absolute = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path
$command = Get-Command dumpbin.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
$dumpbinPath = if ($command) { $command.Source } else { $null }
if (-not $dumpbinPath) {
    $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswherePath -PathType Leaf)) { throw 'Build-time DLL audit requires Visual Studio dumpbin/vswhere' }
    $candidates = @(& $vswherePath -latest -products '*' -find 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Visual Studio tool discovery failed' }
    $dumpbinPath = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
if (-not $dumpbinPath) { throw 'Build-time DLL audit could not locate dumpbin.exe' }

$before = (Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash
$previousPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    $lines = @(& $dumpbinPath /nologo /dependents $absolute 2>&1 | ForEach-Object { [string]$_ })
    $code = $LASTEXITCODE
} finally { $ErrorActionPreference = $previousPreference }
if ($code -ne 0) { throw "DLL import inspection failed: $code" }
$dependencies = @($lines | ForEach-Object {
    if ($_ -match '^\s*([A-Za-z0-9_.-]+\.dll)\s*$') { $Matches[1].ToLowerInvariant() }
} | Sort-Object -Unique)
if ($dependencies.Count -eq 0) { throw 'DLL inspection returned no parseable imports; cannot confirm portability' }
if ((Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash -ne $before) { throw 'Executable changed during DLL inspection' }
$redistributable = @($dependencies | Where-Object { $_ -match '^(vcruntime\d+.*|msvcp\d+.*|msvcr\d+.*|concrt\d+.*|ucrtbased)\.dll$' })
if ($redistributable.Count -ne 0) { throw "Portable executable still needs separately installed VC runtime DLLs: $($redistributable -join ', ')" }
Write-Output ("Portable DLL audit passed; sha256={0}; imports={1}" -f $before.ToLowerInvariant(), ($dependencies -join ', '))
