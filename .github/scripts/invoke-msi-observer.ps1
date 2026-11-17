param(
    [Parameter(Mandatory)][ValidateSet('Start','Snapshot','Finish')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory,
    [string]$Invocation,
    [ValidateSet('Lifecycle','Control')][string]$Mode='Lifecycle',
    [ValidateRange(1,9)][int]$Sequence=1,
    [ValidateRange(0,75000)][int]$WaitMilliseconds=75000,
    [ValidateRange(1,10000)][int]$ControlMilliseconds=10000
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if ($Stage -ceq 'Start') {
    if ([string]::IsNullOrEmpty($Invocation)) { throw 'Observer start requires its original invocation file.' }
    $request = New-P4kxMsiObserverRequest $root ([IO.Path]::GetFullPath($Invocation)) $Mode $ControlMilliseconds
    Start-P4kxMsiObservedParent $request
    return
}
$request = Read-P4kxDetachedReceipt (Join-Path $root 'request.json')
Assert-P4kxMsiObserverBinding $request $root
$handle = Read-P4kxDetachedReceipt (Join-Path $root 'handle.json')
$state = Get-P4kxMsiObserverParentState $request $handle
if ($Stage -ceq 'Snapshot') {
    $state = Wait-P4kxMsiObserverParent $request $handle $WaitMilliseconds
    if (-not $state.lifetime_ended -and [DateTimeOffset]::UtcNow -ge (ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc)) {
        $state=Stop-P4kxMsiObserverExpiredParent $request $handle
    }
    [void](Save-P4kxMsiObserverCheckpoint $request $handle $Sequence $state)
    if ($state.lifetime_ended -and $env:GITHUB_ENV) { 'P4KX_MSI_OBSERVER_FINISHED=true' >> $env:GITHUB_ENV }
    return
}
$failure=$null;$snapshot=$null;$proved=$false
try {
    if (-not $state.lifetime_ended) {
        $deadline=ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc
        $remaining=[Math]::Max(0,($deadline-[DateTimeOffset]::UtcNow).TotalMilliseconds)
        $state=Wait-P4kxMsiObserverParent $request $handle ([int]$remaining)
    }
    if (-not $state.lifetime_ended) {
        $state=Stop-P4kxMsiObserverExpiredParent $request $handle
    }
    $snapshot=Save-P4kxMsiObserverCheckpoint $request $handle 9 $state
    if (-not $state.lifetime_ended -or $state.state -ceq 'stoppedAfterDeadline' -or [IO.File]::Exists((Join-Path $root 'deadline-stop.json'))) { throw 'Original diagnostic parent did not complete before observer cleanup.' }
    Assert-P4kxMsiObserverOriginalResult $request $handle $snapshot
    $proved=$true
} catch { $failure=$_.Exception }
$lifecycleComplete = $proved -and $request.mode -ceq 'Lifecycle'
Write-P4kxDetachedReceipt (Join-Path $root 'observer-result.json') @{
    schema=1;role='native-msi-observer-result';revision=$request.revision;run=$request.run;attempt=$request.attempt;target=$request.target;mode=$request.mode;
    request_sha256=$handle.request_sha256;parent=$state;original_snapshot=$snapshot;control_passed=($proved -and $request.mode -ceq 'Control');
    completed_success=$lifecycleComplete;native_completion_unproven=(-not $lifecycleComplete);mutation_authority=$false;
    failure_type=$(if($null -ne $failure){$failure.GetType().FullName}else{$null})
}
if ($null -ne $failure) { throw $failure }
