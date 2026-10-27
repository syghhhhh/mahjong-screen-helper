$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot '..\support\gate-consistency.ps1')
exit $LASTEXITCODE
