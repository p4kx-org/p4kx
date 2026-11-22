$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')
. (Join-Path $PSScriptRoot 'msi-snapshot-console.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-delay.ps1')

function Get-P4kxMsiSnapshotBudget {
    param([string]$Deadline, [ValidateRange(1,2147483647)][int]$MaximumMilliseconds=100000,
        [DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $remaining=Get-P4kxMsiRemainingBudget $Deadline $Now
    $cleanup=[int][Math]::Min(5000,[Math]::Max(1,[Math]::Floor($remaining/10)))
    $capture=[int][Math]::Min($MaximumMilliseconds,$remaining-$cleanup)
    if ($capture -le 0) { throw 'MSI snapshot cannot fit its capture and owned-process cleanup in the original deadline.' }
    return @{capture_ms=$capture;cleanup_ms=$cleanup}
}

function Get-P4kxMsiCheckpoint2CaptureBudget {
    param($Window, [ValidateRange(1,100000)][int]$MaximumMilliseconds,
        [DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $remaining=Get-P4kxMsiRemainingBudget $Window.capture_deadline_utc $Now
    return Get-P4kxMsiSnapshotBudget $Window.sequence_deadline_utc ([int][Math]::Min($remaining,$MaximumMilliseconds)) $Now
}

function Get-P4kxMsiFinishBudget {
    param($Request, [int]$MaximumMilliseconds, [DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $observer=ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc
    $diagnostic=ConvertFrom-P4kxMsiBudgetTimestamp $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
    $prepared=ConvertFrom-P4kxMsiBudgetTimestamp $Request.prepared_at_utc
    if ($Request.deadline_utc -cne $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC -or
        $observer -gt $diagnostic -or $prepared -gt $Now -or $observer -le $prepared -or
        ($observer-$prepared).TotalMinutes -gt 12) {
        throw 'MSI Finish watchdog differs from the original observer allowance.'
    }
    # Leave five seconds of the existing hosted cleanup minute for caller completion.
    $cutoff=$observer.AddSeconds(55)
    Get-P4kxMsiSnapshotBudget $cutoff.ToString('o') $MaximumMilliseconds $Now
}

function Invoke-P4kxMsiObserverSnapshot {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [ValidateRange(1,9)][int]$Sequence,
        [ValidateRange(0,75000)][int]$WaitMilliseconds,
        [ValidateRange(1,2147483647)][int]$MaximumMilliseconds=100000,
        [switch]$ScheduledDelay, [switch]$OriginalRawBoundary, [switch]$Finish
    )
    if ($Finish -and ($Sequence -ne 9 -or $WaitMilliseconds -ne 0 -or $OriginalRawBoundary -or $ScheduledDelay)) {
        throw 'MSI Finish requires its fixed checkpoint 09.'
    }
    if (-not $Finish -and ($Sequence -eq 9 -or $MaximumMilliseconds -gt 100000)) {
        throw 'MSI snapshot cannot use the Finish checkpoint or exceed its capture bound.'
    }
    $console=New-P4kxMsiSnapshotConsole -Sequence $Sequence
    Write-P4kxMsiSnapshotConsoleMarker $console 'caller' 'begin'
    $receive=(Get-Item 'Function:Receive-P4kxMsiSnapshotConsole').ScriptBlock
    $progress={param([string]$Stream,[string]$Chunk); & $receive $console $Stream $Chunk}.GetNewClosure()
    if ($ScheduledDelay -and ($Sequence -ne 2 -or $WaitMilliseconds -ne 0)) {
        throw 'Scheduled capture only supports a zero-wait checkpoint 2.'
    }
    if ($OriginalRawBoundary) {
        if ($Sequence -ne 2 -or $WaitMilliseconds -ne 0 -or $ScheduledDelay) {
            throw 'Original raw capture requires the zero-wait passive checkpoint 2.'
        }
        $MaximumMilliseconds = [Math]::Min($MaximumMilliseconds, 15000)
    }
    $window=$null;$budget=$null;$finishRequest=$null
    if (-not $ScheduledDelay -and -not $Finish) { $budget=Get-P4kxMsiSnapshotBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $MaximumMilliseconds }
    $root=[IO.Path]::GetFullPath($Directory)
    $capture=Join-Path $root ('snapshots/{0:d2}' -f $Sequence)
    if ([IO.Path]::Exists($capture)) { throw 'MSI snapshot capture sequence must be fresh.' }
    New-Item -ItemType Directory -Path $capture | Out-Null
    $started=[DateTimeOffset]::UtcNow
    $record=@{schema=1;role=$(if($Finish){'native-msi-finish-caller'}else{'native-msi-snapshot-caller'});revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;
        attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;sequence=$Sequence;started_utc=$started.ToString('o');
        deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;capture_ms=$budget.capture_ms;cleanup_ms=$budget.cleanup_ms;
        requested_wait_ms=$WaitMilliseconds;binding_unproven=$true;completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    if ($ScheduledDelay) { $record.scheduled_delay=$true;$record.refusal=$null }
    $worker=Join-Path $PSScriptRoot 'msi-observer-snapshot-worker.ps1'
    $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',$worker,'-Directory',$root,'-Sequence',[string]$Sequence,'-WaitMilliseconds',[string]$WaitMilliseconds)
    if ($OriginalRawBoundary) {
        $record.original_raw_boundary = $true
        $arguments += '-OriginalRawBoundary'
    }
    if ($Finish) { $arguments += '-Finish' }
    $observation=@{};$result=$null;$failure=$null
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $admitted=$false
    try {
        if ($Finish) {
            Write-P4kxDetachedReceipt (Join-Path $capture 'caller-start.json') $record
            $finishRequest=Read-P4kxDetachedReceipt (Join-Path $root 'request.json')
            $budget=Get-P4kxMsiFinishBudget $finishRequest $MaximumMilliseconds
            $record.observer_deadline_utc=$finishRequest.observer_deadline_utc
            $record.capture_ms=$budget.capture_ms;$record.cleanup_ms=$budget.cleanup_ms
        }
        if ($ScheduledDelay) {
            $record.refusal='delay-binding'
            $window=Read-P4kxMsiCheckpoint2Delay $root
            $record.sequence_deadline_utc=$window.sequence_deadline_utc
            $record.capture_deadline_utc=$window.capture_deadline_utc
            $record.refusal='remaining-budget'
            $budget=Get-P4kxMsiCheckpoint2CaptureBudget $window $MaximumMilliseconds
            $record.capture_ms=$budget.capture_ms;$record.cleanup_ms=$budget.cleanup_ms
        }
        if (-not $Finish) { Write-P4kxDetachedReceipt (Join-Path $capture 'caller-start.json') $record }
        Write-P4kxMsiSnapshotConsoleMarker $console 'caller' 'end'
        $budget=if ($ScheduledDelay) { Get-P4kxMsiCheckpoint2CaptureBudget $window $budget.capture_ms }
            elseif ($Finish) { Get-P4kxMsiFinishBudget $finishRequest $budget.capture_ms }
            else { Get-P4kxMsiSnapshotBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $budget.capture_ms }
        $record.capture_ms=$budget.capture_ms;$record.cleanup_ms=$budget.cleanup_ms
        $admitted=$true
        if ($ScheduledDelay) { $record.refusal=$null }
        $name=if($Finish){'finish'}else{'snapshot'}
        $result=Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $arguments $capture $name -Milliseconds $budget.capture_ms -CleanupMilliseconds $budget.cleanup_ms -MaximumBytes 65536 -Observation $observation -OnProgress $progress
        if ($result.exit_code -ne 0) {
            if ($Finish) { throw 'MSI Finish worker did not return successfully.' }
            throw 'MSI snapshot worker did not return a successful capture.'
        }
    } catch { $failure=$_.Exception }
    Write-P4kxMsiSnapshotConsoleMarker $console 'callerReturn' 'begin'
    $record.role=if($Finish){'native-msi-finish-return'}else{'native-msi-snapshot-return'}
    $record.elapsed_ms=$timer.ElapsedMilliseconds
    $record.worker=$observation
    if ($ScheduledDelay) { $record.capture_admitted=$admitted }
    $returned=($null -eq $failure -and $null -ne $result -and $result.exit_code -eq 0)
    if ($Finish) { $record.finish_returned=$returned }
    else { $record.snapshot_returned=$returned }
    $record.failure_type=$(if($null -ne $failure){$failure.GetType().FullName}else{$null})
    Write-P4kxDetachedReceipt (Join-Path $capture 'caller-result.json') $record
    Write-P4kxMsiSnapshotConsoleMarker $console 'callerReturn' 'end'
    if ($null -ne $failure) { throw $failure }
}
