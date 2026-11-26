$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Invoke-P4kxMsiHostedFinishWait {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][string]$CollectionCutoffUtc
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
    $launch = Read-P4kxMsiFinishLaunch $context
    Write-P4kxMsiFinishWaitNotice launch-read end -Hosted
    Write-P4kxMsiObserverProgress finishWaitBinding end @progress
    Wait-P4kxMsiFinishReceipt $context $launch -Hosted
    Write-Host '::notice title=Native MSI Finish wait host::The bound terminal receipt was observed. Native completion is unproven.'
}
