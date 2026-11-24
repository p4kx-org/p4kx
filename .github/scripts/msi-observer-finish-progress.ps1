$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Wait-P4kxMsiFinishProgressSample($Context, $Launch, [int]$Sequence) {
    $offset = if ($Sequence -eq 4) { 30 } else { 60 }
    $started = [DateTimeOffset]::new([long]$Launch.supervisor_started_utc_ticks, [TimeSpan]::Zero)
    $scheduled = $started.AddSeconds($offset)
    $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $Context.request.observer_deadline_utc
    if ($scheduled -gt $cutoff) { $scheduled = $cutoff }
    $receiptPath = Join-Path $Context.directory 'result.json'
    $budget = [Math]::Min(30000, ($scheduled - [DateTimeOffset]::UtcNow).TotalMilliseconds)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while (-not [IO.File]::Exists($receiptPath)) {
        $remaining = [Math]::Min(($scheduled - [DateTimeOffset]::UtcNow).TotalMilliseconds, $budget - $clock.Elapsed.TotalMilliseconds)
        if ($remaining -le 0) { break }
        Start-Sleep -Milliseconds ([int][Math]::Min(100, [Math]::Ceiling($remaining)))
    }
}

function Save-P4kxMsiFinishProgress([string]$Directory, [ValidateRange(4,5)][int]$Sequence) {
    Write-P4kxMsiObserverProgress finishProbeBinding begin -Sequence $Sequence
    $context = Get-P4kxMsiFinishContext $Directory
    $launch = Read-P4kxMsiFinishLaunch $context
    $marker = @{Sequence=$Sequence;RequestDigest=$context.digest}
    Write-P4kxMsiObserverProgress finishProbeBinding end @marker
    Write-P4kxMsiObserverProgress finishProbeWait begin @marker
    Wait-P4kxMsiFinishProgressSample $context $launch $Sequence
    Write-P4kxMsiObserverProgress finishProbeWait end @marker
    Write-P4kxMsiObserverProgress finishProbeParent begin @marker
    $parent = Get-P4kxMsiObserverParentState $context.request $context.handle
    Write-P4kxMsiObserverProgress finishProbeParent end @marker
    Write-P4kxMsiObserverProgress finishProbeCheckpoint begin @marker
    $checkpoint = Save-P4kxMsiObserverCheckpoint $context.request $context.handle $Sequence $parent
    Write-P4kxMsiObserverProgress finishProbeCheckpoint end @marker
    Write-P4kxMsiObserverProgress finishProbeReceipt begin @marker
    $sources = [ordered]@{
        'finish-launch.json' = 'finish-supervision/launch.json'
        'finish-result.json' = 'finish-supervision/result.json'
        'finish-caller-start.json' = 'snapshots/09/caller-start.json'
        'finish-caller-result.json' = 'snapshots/09/caller-result.json'
        'finish-parent-state.json' = 'snapshots/09/parentState-end.json'
    }
    foreach ($stage in @('wait','stopExpiredParent','saveCheckpoint','validateResult','writeResult')) {
        foreach ($status in @('begin','end')) {
            $sources["finish-$stage-$status.json"] = "snapshots/09/$stage-$status.json"
        }
    }
    $files = [ordered]@{}
    foreach ($name in $sources.Keys) {
        $files[$name] = Copy-P4kxMsiObserverFile (Join-Path $context.root $sources[$name]) (Join-Path $checkpoint $name) 16KB
    }
    Write-P4kxDetachedReceipt (Join-Path $checkpoint 'finish-progress.json') @{
        schema=1;role='native-msi-finish-progress';revision=$context.request.revision;run=$context.request.run
        attempt=$context.request.attempt;target=$context.request.target;sequence=$Sequence;request_sha256=$context.digest
        checkpoint_sha256=(Get-P4kxDetachedDigest (Join-Path $checkpoint 'checkpoint.json'));files=$files
        supervisor_pid=$launch.supervisor_pid;supervisor_started_utc_ticks=$launch.supervisor_started_utc_ticks
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');cutoff_utc=$launch.cutoff_utc
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    Write-P4kxMsiObserverProgress finishProbeReceipt end @marker
}
