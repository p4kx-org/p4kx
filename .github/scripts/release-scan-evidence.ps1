$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')

function Get-P4kxReleaseScanAssets([string]$AssetDirectory, [string]$Version) {
    $contract = Get-P4kxPlatformContract
    foreach ($row in $contract.targets) {
        foreach ($package in $row.packages) {
            $name = Get-P4kxAssetName -Target $row.target -Version $Version -Package $package
            $file = Get-Item -LiteralPath (Join-Path $AssetDirectory $name) -ErrorAction Stop
            if ($file.PSIsContainer -or $file.Length -le 0 -or $file.Name -cne $name) {
                throw "The required native scan asset '$name' is not an exact non-empty file."
            }
            $file
        }
    }
}

function Test-P4kxScanInteger($Value) {
    return ($Value -is [int] -or $Value -is [long]) -and $Value -ge 0
}

function Assert-P4kxReleaseScanEvidence(
    [string]$EvidencePath,
    [string]$AssetDirectory,
    [string]$Version,
    [string]$Revision,
    [string]$RunId
) {
    $assets = @(Get-P4kxReleaseScanAssets $AssetDirectory $Version)
    if ($Revision -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$' -or $RunId -cnotmatch '^[1-9][0-9]*$') {
        throw 'Release scan verification requires the exact source revision and workflow run.'
    }
    $evidence = Get-Content -LiteralPath $EvidencePath -Raw -Encoding utf8 | ConvertFrom-Json
    if (-not (Test-P4kxScanInteger $evidence.schema) -or $evidence.schema -ne 1 -or
        $evidence.version -isnot [string] -or $evidence.version -cne $Version -or
        $evidence.revision -isnot [string] -or $evidence.revision -cne $Revision -or
        $evidence.run_id -isnot [string] -or $evidence.run_id -cne $RunId -or $evidence.rows -isnot [array] -or
        $evidence.rows.Count -ne $assets.Count) {
        throw 'VirusTotal evidence does not cover the exact release version, source, run and native package set.'
    }
    $rows = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    foreach ($row in $evidence.rows) {
        if ($row.name -isnot [string] -or -not $row.name -or
            $row.sha256 -isnot [string] -or $row.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            -not (Test-P4kxScanInteger $row.engines) -or $row.engines -eq 0 -or
            -not (Test-P4kxScanInteger $row.flagged) -or $row.flagged -gt $row.engines -or
            -not $rows.TryAdd($row.name, $row)) {
            throw 'VirusTotal evidence contains an invalid or duplicate package report.'
        }
    }
    foreach ($asset in $assets) {
        if (-not $rows.ContainsKey($asset.Name)) {
            throw "VirusTotal evidence is missing the exact package '$($asset.Name)'."
        }
        $digest = (Get-FileHash -LiteralPath $asset.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($rows[$asset.Name].sha256 -cne $digest) {
            throw "VirusTotal evidence does not identify the exact bytes of '$($asset.Name)'."
        }
    }
}
