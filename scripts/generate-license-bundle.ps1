[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Destination
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$destinationPath = [System.IO.Path]::GetFullPath($Destination)
$json = & cargo metadata --manifest-path (Join-Path $repoRoot 'Cargo.toml') --locked --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
$metadata = $json | ConvertFrom-Json
$registryPackages = @($metadata.packages | Where-Object { $_.source } | Sort-Object name,version)
if ($registryPackages.Count -eq 0) { throw 'No registry packages found in Cargo metadata' }
$textsDirectory = Join-Path $destinationPath 'texts'
New-Item -ItemType Directory -Path $textsDirectory -Force | Out-Null
$entries = @()
foreach ($package in $registryPackages) {
    if (-not $package.license) { throw "Missing declared license: $($package.name) $($package.version)" }
    $sourceDirectory = Split-Path -Parent $package.manifest_path
    $licenseFiles = @(
        Get-ChildItem -LiteralPath $sourceDirectory -File |
            Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)' } |
            Sort-Object Name
    )
    if ($licenseFiles.Count -eq 0) { throw "Missing license text: $($package.name) $($package.version)" }
    $packageDirectory = Join-Path $textsDirectory ("{0}-{1}" -f $package.name,$package.version)
    New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
    $files = @()
    foreach ($file in $licenseFiles) {
        $target = Join-Path $packageDirectory $file.Name
        Copy-Item -LiteralPath $file.FullName -Destination $target
        $files += [pscustomobject]@{
            path = ('texts/{0}-{1}/{2}' -f $package.name,$package.version,$file.Name)
            sha256 = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $entries += [pscustomobject]@{
        name = $package.name
        version = $package.version
        license = $package.license
        source = $package.source
        texts = $files
    }
}
$inventory = [pscustomobject]@{
    schema_version = '1.0'
    scope = 'All locked registry packages returned by cargo metadata; may include target-specific dependencies.'
    cargo_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
    packages = $entries
}
$inventory | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $destinationPath 'DEPENDENCIES.json') -Encoding utf8
Write-Output ("Collected license texts for {0} registry packages" -f $entries.Count)
