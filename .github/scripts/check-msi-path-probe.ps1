param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Msi,
    [Parameter(Mandatory)][string]$NextMsi,
    [Parameter(Mandatory)][string]$NextVersion,
    [Parameter(Mandatory)][string]$OtherArchitectureMsi,
    [Parameter(Mandatory)][string]$UpgradeCode,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$PreviousMsi
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path-probe-run.ps1')
$context = New-P4kxPathProbeContext $Target $Msi $UpgradeCode $OutputDirectory
$nextPath = Resolve-P4kxMsiInput $NextMsi
Assert-P4kxPrependProbe $nextPath $context.platform $NextVersion $UpgradeCode | Out-Null
$other = @((Get-P4kxPlatformContract).targets | Where-Object { $_.os -ceq 'windows' -and $_.arch -cne $context.platform.arch })[0]
$otherPath = Resolve-P4kxMsiInput $OtherArchitectureMsi
Assert-P4kxPrependProbe $otherPath $other $context.metadata.version $UpgradeCode | Out-Null
$baseline = Get-P4kxUserPathState
Assert-P4kxUninstalled $UpgradeCode $context.folder $context.shortcut $baseline
$baseline | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $context.logs 'host-original-path.json') -Encoding utf8NoBOM
$calls = @{
    Target=$Target; Msi=$context.msi; NextMsi=$nextPath; NextVersion=$NextVersion
    UpgradeCode=$UpgradeCode; OtherArchitectureMsi=$otherPath; FixtureDirectory=$FixtureDirectory
}
$lifecycleScript = Join-Path $PSScriptRoot 'check-msi.ps1'
if ($PreviousMsi -and $context.platform.arch -cne 'amd64') { throw 'Historical AMD64 calibration cannot run on ARM64.' }
$results = @(Invoke-P4kxPathProbeScenarios $context $baseline $calls $lifecycleScript $PreviousMsi)
Write-P4kxPathProbeReport $context $baseline $results
