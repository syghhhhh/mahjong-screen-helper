$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot '..\..\support\rule-diagnostic.ps1')
exit $LASTEXITCODE
