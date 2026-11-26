$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-ready.ps1')

function Invoke-P4kxMsiHostedFinishWait {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][string]$CollectionCutoffUtc,
        [int]$MonitorPid,
        [long]$MonitorStartedUtcTicks,
        [string]$LaunchDigest
    )
    Write-P4kxMsiFinishWaitNotice context-binding begin -Hosted
    $context = Get-P4kxMsiFinishContext $Directory
    $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
    if ($context.request.mode -cne 'Lifecycle' -or $cutoff -ne $context.collection_cutoff) {
        throw 'Hosted Finish wait requires the original Lifecycle collection cutoff.'
    }
    Write-P4kxMsiFinishWaitNotice context-binding end -Hosted

    $progress = @{Directory=$context.directory; RequestDigest=$context.digest; FileOnly=$true}
    Write-P4kxMsiObserverProgress finishWaitBinding begin @progress
    Write-P4kxMsiFinishWaitNotice launch-read begin -Hosted
    $launchParameters = @{Context=$context}
    if ($PSBoundParameters.ContainsKey('LaunchDigest')) { $launchParameters.ExpectedDigest = $LaunchDigest }
    $launch = Read-P4kxMsiFinishLaunch @launchParameters
    Write-P4kxMsiFinishWaitNotice launch-read end -Hosted
    Write-P4kxMsiObserverProgress finishWaitBinding end @progress
    if ($MonitorPid -gt 0) { Write-P4kxMsiHostedFinishReady $context $MonitorPid $MonitorStartedUtcTicks }
    $waitParameters = @{Context=$context;Launch=$launch;Hosted=$true}
    if ($launchParameters.ContainsKey('ExpectedDigest')) { $waitParameters.LaunchDigest = $LaunchDigest }
    Wait-P4kxMsiFinishReceipt @waitParameters
    Write-Host '::notice title=Native MSI Finish wait host::The bound terminal receipt was observed. Native completion is unproven.'
}
