$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')

function Get-P4kxMsiFinishContext([string]$Directory) {
    $root = [IO.Path]::GetFullPath($Directory)
    $requestPath = Join-Path $root 'request.json'
    $request = Read-P4kxDetachedReceipt $requestPath
    Assert-P4kxMsiObserverBinding $request $root
    $handle = Read-P4kxDetachedReceipt (Join-Path $root 'handle.json')
    $digest = Get-P4kxDetachedDigest $requestPath
    if ($handle.schema -ne 1 -or $handle.role -cne 'native-msi-observer-handle' -or $handle.request_sha256 -cne $digest -or
        $handle.pid -le 0 -or $handle.started_utc_ticks -le 0) { throw 'Finish requires the bound original MSI observer handle.' }
    $cutoff = Get-P4kxMsiFinishCutoff $request
    return @{
        root=$root; request=$request; handle=$handle; digest=$digest
        directory=(Join-Path $root 'finish-supervision'); cutoff=$cutoff; collection_cutoff=$cutoff.AddSeconds(3)
    }
}

function Start-P4kxMsiFinishProcess($Context, [string]$EntryPoint, [string]$LogName, [string[]]$Options) {
    $script = Join-Path $PSScriptRoot $EntryPoint
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',('"' + $script + '"'),
        '-Directory',('"' + $Context.root + '"')) + $Options
    $start = @{
        FilePath=[Environment]::ProcessPath;ArgumentList=$arguments;WindowStyle='Hidden';PassThru=$true
        WorkingDirectory=$Context.request.location
    }
    if ($LogName) {
        $start.RedirectStandardOutput = Join-Path $Context.directory "$LogName-stdout.log"
        $start.RedirectStandardError = Join-Path $Context.directory "$LogName-stderr.log"
    }
    Start-Process @start
}

function Start-P4kxMsiFinishSupervision([string]$Directory) {
    $context = Get-P4kxMsiFinishContext $Directory
    $null = Get-P4kxMsiSnapshotBudget $context.cutoff.ToString('o')
    New-Item -ItemType Directory -Path $context.directory | Out-Null
    $process = Start-P4kxMsiFinishProcess $context 'msi-observer-finish-supervisor.ps1' '' @()
    try {
        Write-P4kxDetachedReceipt (Join-Path $context.directory 'launch.json') @{
            schema=1;role='native-msi-finish-supervisor-launch';request_sha256=$context.digest
            supervisor_pid=$process.Id;supervisor_started_utc_ticks=$process.StartTime.ToUniversalTime().Ticks
            cutoff_utc=$context.cutoff.ToString('o');mutation_authority=$false
        }
    } finally { $process.Dispose() }
}

function Read-P4kxMsiFinishLaunch($Context) {
    $launch = Read-P4kxDetachedReceipt (Join-Path $context.directory 'launch.json')
    if ($launch.schema -ne 1 -or $launch.role -cne 'native-msi-finish-supervisor-launch' -or
        $launch.request_sha256 -cne $context.digest -or $launch.cutoff_utc -cne $context.cutoff.ToString('o') -or
        $launch.supervisor_pid -le 0 -or $launch.supervisor_started_utc_ticks -le 0 -or $launch.mutation_authority -ne $false) {
        throw 'Finish supervisor launch differs from the original observer.'
    }
    return $launch
}

function Wait-P4kxMsiFinishReceipt($Context, $Launch) {
    $receiptPath = Join-Path $Context.directory 'result.json'
    $resultDigest = $null
    $budget = ($Context.collection_cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $wallRemaining = ($Context.collection_cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
        $remaining = [Math]::Min($wallRemaining, $budget - $clock.Elapsed.TotalMilliseconds)
        if ($remaining -le 0) { break }
        if ([IO.File]::Exists($receiptPath)) {
            $resultDigest = Get-P4kxDetachedDigest $receiptPath
            break
        }
        Start-Sleep -Milliseconds ([int][Math]::Min(100, [Math]::Ceiling($remaining)))
    }
    $observedAt = [DateTimeOffset]::UtcNow
    if ($observedAt -gt $Context.collection_cutoff -or $clock.Elapsed.TotalMilliseconds -gt $budget) {
        $resultDigest = $null
    }
    Write-P4kxDetachedReceipt (Join-Path $Context.directory 'wait-result.json') @{
        schema=1;role='native-msi-finish-receipt-wait';request_sha256=$Context.digest
        supervisor_pid=$Launch.supervisor_pid;supervisor_started_utc_ticks=$Launch.supervisor_started_utc_ticks
        deadline_utc=$Context.collection_cutoff.ToString('o');observed_utc=$observedAt.ToString('o')
        result_sha256=$resultDigest;mutation_authority=$false
    }
    if (-not $resultDigest) { throw 'The bound Finish supervisor produced no result within the original allowance.' }
}

function Wait-P4kxMsiSupervisedFinish([string]$Directory) {
    $context = Get-P4kxMsiFinishContext $Directory
    $launch = Read-P4kxMsiFinishLaunch $context
    Wait-P4kxMsiFinishReceipt $context $launch
}

function Read-P4kxMsiSupervisedFinish([string]$Directory) {
    $context = Get-P4kxMsiFinishContext $Directory
    $launch = Read-P4kxMsiFinishLaunch $context
    $receiptPath = Join-Path $context.directory 'result.json'
    $wait = Read-P4kxDetachedReceipt (Join-Path $context.directory 'wait-result.json')
    if ($wait.schema -ne 1 -or $wait.role -cne 'native-msi-finish-receipt-wait' -or
        $wait.request_sha256 -cne $context.digest -or $wait.supervisor_pid -ne $launch.supervisor_pid -or
        $wait.supervisor_started_utc_ticks -ne $launch.supervisor_started_utc_ticks -or
        $wait.deadline_utc -cne $context.collection_cutoff.ToString('o') -or $wait.mutation_authority -ne $false -or
        $wait.result_sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        $wait.result_sha256 -cne (Get-P4kxDetachedDigest $receiptPath)) {
        throw 'Finish requires the exact terminal receipt from its bounded wait.'
    }
    $waitObserved = ConvertFrom-P4kxMsiBudgetTimestamp $wait.observed_utc
    if ($waitObserved -gt $context.collection_cutoff -or $waitObserved -gt [DateTimeOffset]::UtcNow) {
        throw 'The terminal Finish receipt was observed outside its original allowance.'
    }
    $result = Read-P4kxDetachedReceipt $receiptPath
    if ($result.schema -ne 1 -or $result.role -cne 'native-msi-finish-supervisor-result' -or
        $result.request_sha256 -cne $context.digest -or $result.supervisor_pid -ne $launch.supervisor_pid -or
        $result.supervisor_started_utc_ticks -ne $launch.supervisor_started_utc_ticks -or $result.mutation_authority -ne $false -or
        $result.caller_pid -le 0 -or $result.caller_pid -in @($launch.supervisor_pid,$context.handle.pid) -or
        $result.caller_exited -ne $true -or $result.caller_exit_code -ne 0 -or $result.timed_out -ne $false -or
        (ConvertFrom-P4kxMsiBudgetTimestamp $result.observed_exit_utc) -gt $context.cutoff) {
        throw 'The bound Finish caller did not complete successfully within the original allowance.'
    }
    $observedExit = ConvertFrom-P4kxMsiBudgetTimestamp $result.observed_exit_utc
    $launchedAt = [DateTime]::new([long]$launch.supervisor_started_utc_ticks,[DateTimeKind]::Utc)
    if ($observedExit -lt $launchedAt -or $observedExit -gt $waitObserved) {
        throw 'Finish supervisor exit observation differs from its process lifetime.'
    }
    $callerPath = Join-Path $context.root 'snapshots/09/caller-result.json'
    if ($result.caller_result_sha256 -cne (Get-P4kxDetachedDigest $callerPath)) { throw 'The original Finish caller receipt differs.' }
    $caller = Read-P4kxDetachedReceipt $callerPath
    $observer = Read-P4kxDetachedReceipt (Join-Path $context.root 'observer-result.json')
    $snapshot = Join-Path $context.root 'checkpoints/09'
    foreach ($proof in @($caller,$observer)) {
        if ($proof.target -cne $context.request.target -or $proof.revision -cne $context.request.revision -or
            $proof.run -cne $context.request.run -or $proof.attempt -cne $context.request.attempt -or $proof.mutation_authority -ne $false) {
            throw 'Original Finish proof belongs to a different target, source or attempt.'
        }
    }
    $lifecycle = $context.request.mode -ceq 'Lifecycle'
    if ($caller.schema -ne 1 -or $caller.role -cne 'native-msi-finish-return' -or $caller.sequence -ne 9 -or
        $caller.finish_returned -ne $true -or $caller.worker.exited -ne $true -or $caller.worker.exit_code -ne 0 -or
        $caller.worker.signal_contract -cne 'direct-process-only' -or $observer.schema -ne 1 -or
        $observer.role -cne 'native-msi-observer-result' -or $observer.mode -cne $context.request.mode -or
        $observer.request_sha256 -cne $context.digest -or $observer.original_snapshot -cne $snapshot -or
        $observer.completed_success -ne $lifecycle -or $observer.native_completion_unproven -ne (-not $lifecycle) -or
        $observer.control_passed -ne (-not $lifecycle) -or $observer.parent.lifetime_ended -ne $true -or
        $observer.parent.state -ceq 'stoppedAfterDeadline' -or [IO.File]::Exists((Join-Path $context.root 'deadline-stop.json'))) {
        throw 'The original Finish caller or observer result is incomplete or unproven.'
    }
    Assert-P4kxMsiObserverOriginalResult $context.request $context.handle $snapshot
}
