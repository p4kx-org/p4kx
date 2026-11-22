param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateRange(1,9)][int]$Sequence,
    [ValidateRange(0,75000)][int]$WaitMilliseconds,
    [switch]$OriginalRawBoundary, [switch]$Finish
)
$ErrorActionPreference='Stop'
if ($Finish.IsPresent -ne ($Sequence -eq 9) -or ($Finish -and ($WaitMilliseconds -ne 0 -or $OriginalRawBoundary))) {
    throw 'MSI Finish requires its fixed checkpoint 09.'
}
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-progress.ps1')
$root=[IO.Path]::GetFullPath($Directory)
$progress=Join-Path $root ('snapshots/{0:d2}' -f $Sequence)
Write-P4kxMsiObserverProgress readRequest begin -Directory $progress
$request=Read-P4kxDetachedReceipt (Join-Path $root 'request.json')
Write-P4kxMsiObserverProgress readRequest end -Directory $progress
Write-P4kxMsiObserverProgress binding begin -Directory $progress
Assert-P4kxMsiObserverBinding $request $root
if ($OriginalRawBoundary -and ($Sequence -ne 2 -or $WaitMilliseconds -ne 0 -or $request.mode -cne 'Lifecycle')) {
    throw 'Original raw capture requires lifecycle checkpoint 2.'
}
Write-P4kxMsiObserverProgress binding end -Directory $progress
Write-P4kxMsiObserverProgress readHandle begin -Directory $progress
$handle=Read-P4kxDetachedReceipt (Join-Path $root 'handle.json')
Write-P4kxMsiObserverProgress readHandle end -Directory $progress
Write-P4kxMsiObserverProgress parentState begin -Directory $progress -RequestDigest $handle.request_sha256
$state=Get-P4kxMsiObserverParentState $request $handle
Write-P4kxMsiObserverProgress parentState end -Directory $progress -RequestDigest $handle.request_sha256
$observationFailure=$null;$failure=$null;$snapshot=$null;$proved=$false
try {
    if (-not $Finish -or -not $state.lifetime_ended) {
        $wait=$WaitMilliseconds
        if ($Finish) {
            $deadline=ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc
            $wait=[int][Math]::Max(0,[Math]::Ceiling(($deadline-[DateTimeOffset]::UtcNow).TotalMilliseconds))
        }
        Write-P4kxMsiObserverProgress wait begin -Directory $progress -RequestDigest $handle.request_sha256
        if ($OriginalRawBoundary) {
            try { $state=Wait-P4kxMsiOriginalRawBoundary $request $handle $state $progress }
            catch { $observationFailure=$_.Exception }
        } else {
            $state=Wait-P4kxMsiObserverParent $request $handle $wait
        }
        Write-P4kxMsiObserverProgress wait end -Directory $progress -RequestDigest $handle.request_sha256
    }
    if (-not $OriginalRawBoundary -and -not $state.lifetime_ended -and [DateTimeOffset]::UtcNow -ge (ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc)) {
        Write-P4kxMsiObserverProgress stopExpiredParent begin -Directory $progress -RequestDigest $handle.request_sha256
        $state=Stop-P4kxMsiObserverExpiredParent $request $handle
        Write-P4kxMsiObserverProgress stopExpiredParent end -Directory $progress -RequestDigest $handle.request_sha256
    }
    Write-P4kxMsiObserverProgress saveCheckpoint begin -Directory $progress -RequestDigest $handle.request_sha256
    $snapshot=Save-P4kxMsiObserverCheckpoint $request $handle $Sequence $state -OriginalRawBoundary:$OriginalRawBoundary
    Write-P4kxMsiObserverProgress saveCheckpoint end -Directory $progress -RequestDigest $handle.request_sha256
    if ($Finish) {
        Write-P4kxMsiObserverProgress validateResult begin -Directory $progress -RequestDigest $handle.request_sha256
        if (-not $state.lifetime_ended -or $state.state -ceq 'stoppedAfterDeadline' -or [IO.File]::Exists((Join-Path $root 'deadline-stop.json'))) {
            throw 'Original diagnostic parent did not complete before observer cleanup.'
        }
        Assert-P4kxMsiObserverOriginalResult $request $handle $snapshot
        $proved=$true
        Write-P4kxMsiObserverProgress validateResult end -Directory $progress -RequestDigest $handle.request_sha256
    }
} catch {
    if (-not $Finish) { throw }
    $failure=$_.Exception
}
if ($Finish) {
    $lifecycleComplete=$proved -and $request.mode -ceq 'Lifecycle'
    Write-P4kxMsiObserverProgress writeResult begin -Directory $progress -RequestDigest $handle.request_sha256
    Write-P4kxDetachedReceipt (Join-Path $root 'observer-result.json') @{
        schema=1;role='native-msi-observer-result';revision=$request.revision;run=$request.run;attempt=$request.attempt;target=$request.target;mode=$request.mode;
        request_sha256=$handle.request_sha256;parent=$state;original_snapshot=$snapshot;control_passed=($proved -and $request.mode -ceq 'Control');
        completed_success=$lifecycleComplete;native_completion_unproven=(-not $lifecycleComplete);mutation_authority=$false;
        failure_type=$(if($null -ne $failure){$failure.GetType().FullName}else{$null})
    }
    Write-P4kxMsiObserverProgress writeResult end -Directory $progress -RequestDigest $handle.request_sha256
    if ($null -ne $failure) { throw $failure }
    return
}
Write-P4kxMsiObserverProgress workflowOutput begin -Directory $progress -RequestDigest $handle.request_sha256
if (-not $OriginalRawBoundary -and $state.lifetime_ended -and $env:GITHUB_ENV) { 'P4KX_MSI_OBSERVER_FINISHED=true' >> $env:GITHUB_ENV }
Write-P4kxMsiObserverProgress workflowOutput end -Directory $progress -RequestDigest $handle.request_sha256
if ($null -ne $observationFailure) { throw $observationFailure }
