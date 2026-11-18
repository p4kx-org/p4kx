$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')
. (Join-Path $PSScriptRoot 'msi-snapshot-console.ps1')

function Get-P4kxMsiSnapshotBudget {
    param([string]$Deadline, [ValidateRange(1,100000)][int]$MaximumMilliseconds=100000)
    $remaining=Get-P4kxMsiRemainingBudget $Deadline
    $cleanup=[int][Math]::Min(5000,[Math]::Max(1,[Math]::Floor($remaining/10)))
    $capture=[int][Math]::Min($MaximumMilliseconds,$remaining-$cleanup)
    if ($capture -le 0) { throw 'MSI snapshot cannot fit its capture and owned-process cleanup in the original deadline.' }
    return @{capture_ms=$capture;cleanup_ms=$cleanup}
}

function Invoke-P4kxMsiObserverSnapshot {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [ValidateRange(1,9)][int]$Sequence,
        [ValidateRange(0,75000)][int]$WaitMilliseconds,
        [ValidateRange(1,100000)][int]$MaximumMilliseconds=100000
    )
    $console=New-P4kxMsiSnapshotConsole -Sequence $Sequence
    Write-P4kxMsiSnapshotConsoleMarker $console 'caller' 'begin'
    $receive=(Get-Item 'Function:Receive-P4kxMsiSnapshotConsole').ScriptBlock
    $progress={param([string]$Stream,[string]$Chunk); & $receive $console $Stream $Chunk}.GetNewClosure()
    $budget=Get-P4kxMsiSnapshotBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $MaximumMilliseconds
    $root=[IO.Path]::GetFullPath($Directory)
    $capture=Join-Path $root ('snapshots/{0:d2}' -f $Sequence)
    if ([IO.Path]::Exists($capture)) { throw 'MSI snapshot capture sequence must be fresh.' }
    New-Item -ItemType Directory -Path $capture | Out-Null
    $started=[DateTimeOffset]::UtcNow
    $record=@{schema=1;role='native-msi-snapshot-caller';revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;
        attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;sequence=$Sequence;started_utc=$started.ToString('o');
        deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;capture_ms=$budget.capture_ms;cleanup_ms=$budget.cleanup_ms;
        requested_wait_ms=$WaitMilliseconds;binding_unproven=$true;completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    Write-P4kxDetachedReceipt (Join-Path $capture 'caller-start.json') $record
    Write-P4kxMsiSnapshotConsoleMarker $console 'caller' 'end'
    $worker=Join-Path $PSScriptRoot 'msi-observer-snapshot-worker.ps1'
    $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',$worker,'-Directory',$root,'-Sequence',[string]$Sequence,'-WaitMilliseconds',[string]$WaitMilliseconds)
    $observation=@{};$result=$null;$failure=$null
    $timer=[Diagnostics.Stopwatch]::StartNew()
    try {
        $budget=Get-P4kxMsiSnapshotBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $budget.capture_ms
        $record.capture_ms=$budget.capture_ms;$record.cleanup_ms=$budget.cleanup_ms
        $result=Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $arguments $capture 'snapshot' -Milliseconds $budget.capture_ms -CleanupMilliseconds $budget.cleanup_ms -MaximumBytes 65536 -Observation $observation -OnProgress $progress
        if ($result.exit_code -ne 0) { throw 'MSI snapshot worker did not return a successful capture.' }
    } catch { $failure=$_.Exception }
    Write-P4kxMsiSnapshotConsoleMarker $console 'callerReturn' 'begin'
    $record.role='native-msi-snapshot-return'
    $record.elapsed_ms=$timer.ElapsedMilliseconds
    $record.worker=$observation
    $record.snapshot_returned=($null -eq $failure -and $null -ne $result -and $result.exit_code -eq 0)
    $record.failure_type=$(if($null -ne $failure){$failure.GetType().FullName}else{$null})
    Write-P4kxDetachedReceipt (Join-Path $capture 'caller-result.json') $record
    Write-P4kxMsiSnapshotConsoleMarker $console 'callerReturn' 'end'
    if ($null -ne $failure) { throw $failure }
}
