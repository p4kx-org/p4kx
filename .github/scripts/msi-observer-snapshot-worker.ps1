param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateRange(1,9)][int]$Sequence,
    [ValidateRange(0,75000)][int]$WaitMilliseconds
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-progress.ps1')
$root=[IO.Path]::GetFullPath($Directory)
$progress=Join-Path $root ('snapshots/{0:d2}' -f $Sequence)
Write-P4kxMsiObserverProgress readRequest begin -Directory $progress
$request=Read-P4kxDetachedReceipt (Join-Path $root 'request.json')
Write-P4kxMsiObserverProgress readRequest end -Directory $progress
Write-P4kxMsiObserverProgress binding begin -Directory $progress
Assert-P4kxMsiObserverBinding $request $root
Write-P4kxMsiObserverProgress binding end -Directory $progress
Write-P4kxMsiObserverProgress readHandle begin -Directory $progress
$handle=Read-P4kxDetachedReceipt (Join-Path $root 'handle.json')
Write-P4kxMsiObserverProgress readHandle end -Directory $progress
Write-P4kxMsiObserverProgress parentState begin -Directory $progress -RequestDigest $handle.request_sha256
$state=Get-P4kxMsiObserverParentState $request $handle
Write-P4kxMsiObserverProgress parentState end -Directory $progress -RequestDigest $handle.request_sha256
Write-P4kxMsiObserverProgress wait begin -Directory $progress -RequestDigest $handle.request_sha256
$state=Wait-P4kxMsiObserverParent $request $handle $WaitMilliseconds
Write-P4kxMsiObserverProgress wait end -Directory $progress -RequestDigest $handle.request_sha256
if (-not $state.lifetime_ended -and [DateTimeOffset]::UtcNow -ge (ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc)) {
    Write-P4kxMsiObserverProgress stopExpiredParent begin -Directory $progress -RequestDigest $handle.request_sha256
    $state=Stop-P4kxMsiObserverExpiredParent $request $handle
    Write-P4kxMsiObserverProgress stopExpiredParent end -Directory $progress -RequestDigest $handle.request_sha256
}
Write-P4kxMsiObserverProgress saveCheckpoint begin -Directory $progress -RequestDigest $handle.request_sha256
[void](Save-P4kxMsiObserverCheckpoint $request $handle $Sequence $state)
Write-P4kxMsiObserverProgress saveCheckpoint end -Directory $progress -RequestDigest $handle.request_sha256
Write-P4kxMsiObserverProgress workflowOutput begin -Directory $progress -RequestDigest $handle.request_sha256
if ($state.lifetime_ended -and $env:GITHUB_ENV) { 'P4KX_MSI_OBSERVER_FINISHED=true' >> $env:GITHUB_ENV }
Write-P4kxMsiObserverProgress workflowOutput end -Directory $progress -RequestDigest $handle.request_sha256
