param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
$platform = Get-P4kxPlatform -Target $Target
$minimum = Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'minimum.json') -Raw | ConvertFrom-Json -AsHashtable
$current = Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'current.json') -Raw | ConvertFrom-Json
Assert-P4kxRuntimeEvidence $minimum $platform $Version $env:GITHUB_SHA
if ($minimum.workflow_run -cne "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID") {
    throw 'macOS package smoke evidence belongs to a different workflow run.'
}
if (@($minimum.package_smokes).Count -ne 1) { throw 'Expected one minimum macOS package smoke.' }
$minimum.package_smokes[0].current_hosted_smoke = $current
Assert-P4kxPackageRuntime $minimum $platform $AssetDirectory $Version
$path = Join-Path $AssetDirectory "P4KX-$Version-$Target-runtime.json"
$minimum | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
