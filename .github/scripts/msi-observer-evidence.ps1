$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-process.ps1')

function Copy-P4kxMsiObserverFile {
    param([string]$Source, [string]$Destination, [int]$MaximumBytes)
    if (-not [IO.File]::Exists($Source)) { return @{status='absent'} }
    try {
        $sourceStream = [IO.File]::Open($Source,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
        try {
            if ($sourceStream.Length -gt $MaximumBytes) { throw 'Original observer metadata exceeds its existing bound.' }
            $bytes = [byte[]]::new($MaximumBytes+1)
            $length = 0
            while ($length -lt $bytes.Length) {
                $read = $sourceStream.Read($bytes,$length,$bytes.Length-$length)
                if ($read -eq 0) { break }
                $length += $read
            }
            if ($length -gt $MaximumBytes) { throw 'Original observer metadata exceeds its existing bound.' }
        } finally { $sourceStream.Dispose() }
        $destinationStream = [IO.File]::Open($Destination,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
        try { $destinationStream.Write($bytes,0,$length);$destinationStream.Flush($true) } finally { $destinationStream.Dispose() }
        return @{status='captured';bytes=$length;sha256=(Get-P4kxDetachedDigest $Destination)}
    } catch { return @{status='unreadable';failure_type=$_.Exception.GetType().FullName} }
}

function Read-P4kxMsiOriginalBoundary {
    param([string]$Path, [string]$Target,
        [ValidateSet('preamble/original-raw','lifecycle/install')][string]$Phase)
    $events = [Collections.Generic.List[hashtable]]::new()
    $journal = Read-P4kxMsiPhaseJournal $Path -ObservedEvents $events
    $platform = Get-P4kxPlatform -Target $Target
    $phases = @($Phase)
    if ($Phase -ceq 'lifecycle/install' -and $Target -ceq 'x86_64-pc-windows-msvc') {
        $phases += 'lifecycle/previous-install'
    }
    $first = $null
    $status = if ($journal.present) { 'notObserved' } else { 'unavailable' }
    foreach ($event in $events) {
        if ($event.target -cne $Target -or $event.process_arch -ine $platform.runner_arch) { throw 'Original phase journal CPU or target differs.' }
        if ($event.phase -cnotin $phases) { continue }
        if ($null -eq $first -and $event.status -ceq 'begin') { $first = $event.id }
        if ($event.id -ne $first) { continue }
        if ($event.status -ceq 'end') { $status = 'reached' }
        if ($Phase -ceq 'lifecycle/install' -and $event.status -ceq 'error') { $status = 'failed' }
    }
    return @{journal=$journal;status=$status}
}

function Wait-P4kxMsiOriginalBoundary {
    param($Request, $Handle, $ParentState, [string]$ProgressDirectory,
        [ValidateSet('preamble/original-raw','lifecycle/install')][string]$Phase)
    $journal = Join-Path $Request.output_directory 'diagnostics/phase-journal.jsonl'
    $sample = Join-Path $ProgressDirectory 'phase-boundary-sample.jsonl'
    if ([IO.File]::Exists($sample)) { throw 'Passive checkpoint requires a fresh journal sample.' }
    $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc
    $window = [TimeSpan]::FromSeconds(5)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $sampledLength = -1
    while (-not $ParentState.lifetime_ended) {
        $remaining = [Math]::Min(($window-$timer.Elapsed).TotalMilliseconds, ($cutoff-[DateTimeOffset]::UtcNow).TotalMilliseconds)
        if ($remaining -le 0) { break }
        $source = [IO.FileInfo]::new($journal)
        if ($source.Exists -and $source.Length -ne $sampledLength) {
            try {
                $copy = Copy-P4kxMsiObserverFile $journal $sample 1MB
                if ($copy.status -ceq 'unreadable') { throw 'Passive checkpoint could not read the original live journal.' }
                if ($copy.status -ceq 'captured') {
                    $sampledLength = $copy.bytes
                    $observed = Read-P4kxMsiOriginalBoundary $sample $Request.target $Phase
                    if ($observed.status -cin @('reached','failed') -or $observed.journal.terminal) { break }
                }
            } finally { [IO.File]::Delete($sample) }
        }
        Start-Sleep -Milliseconds ([int][Math]::Min(100, $remaining))
        $ParentState = Get-P4kxMsiObserverParentState $Request $Handle
    }
    return $ParentState
}

function Save-P4kxMsiObserverCheckpoint {
    param($Request, $Handle, [int]$Sequence, $ParentState,
        [ValidateSet('preamble/original-raw','lifecycle/install')][string]$OriginalBoundary)
    $directory = Join-Path $Request.directory ('checkpoints/{0:d2}' -f $Sequence)
    if ([IO.Path]::Exists($directory)) { throw 'MSI observer checkpoint sequence must be fresh.' }
    New-Item -ItemType Directory -Path $directory | Out-Null
    $captured = @{}
    foreach ($name in @('request.json','handle.json','parent-start.json','parent-result.json','deadline-stop.json','diagnostic-sources.json')) {
        $captured[$name] = Copy-P4kxMsiObserverFile (Join-Path $Request.directory $name) (Join-Path $directory $name) 16384
    }
    $original = Join-Path $Request.output_directory 'diagnostics'
    foreach ($name in @('diagnostic-start.json','diagnostic-result.json','diagnostic-inputs.json','diagnostic-sources.json')) {
        $captured['original-'+$name] = Copy-P4kxMsiObserverFile (Join-Path $original $name) (Join-Path $directory ('original-'+$name)) 16384
    }
    $captured['phase-journal.jsonl'] = Copy-P4kxMsiObserverFile (Join-Path $original 'phase-journal.jsonl') (Join-Path $directory 'phase-journal.jsonl') 1MB
    $journal = $null;$journalFailure=$null
    $boundary = 'unavailable'
    $copiedJournal = Join-Path $directory 'phase-journal.jsonl'
    try {
        if ($OriginalBoundary) {
            if ($captured['phase-journal.jsonl'].status -ceq 'captured') {
                $observed = Read-P4kxMsiOriginalBoundary $copiedJournal $Request.target $OriginalBoundary
                $journal = $observed.journal
                $boundary = $observed.status
            }
        } else {
            $journal = Read-P4kxMsiPhaseJournal $copiedJournal
        }
    } catch { $journalFailure=$_.Exception.GetType().FullName }
    $receipt = @{schema=1;role='native-msi-observer-checkpoint';revision=$Request.revision;run=$Request.run;attempt=$Request.attempt;target=$Request.target;mode=$Request.mode;
        sequence=$Sequence;request_sha256=$Handle.request_sha256;parent=$ParentState;observed_utc=[DateTimeOffset]::UtcNow.ToString('o');
        deadline_utc=$Request.deadline_utc;observer_deadline_utc=$Request.observer_deadline_utc;files=$captured;journal=$journal;journal_failure_type=$journalFailure;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    if ($OriginalBoundary -ceq 'preamble/original-raw') { $receipt.original_raw_capture = $boundary }
    if ($OriginalBoundary -ceq 'lifecycle/install') { $receipt.original_install_phase = $boundary }
    Write-P4kxDetachedReceipt (Join-Path $directory 'checkpoint.json') $receipt
    return $directory
}

function Assert-P4kxMsiObserverClosedJournal {
    param($Request, $Result, [string]$Snapshot)
    $events = [Collections.Generic.List[hashtable]]::new()
    $journal = Read-P4kxMsiPhaseJournal (Join-Path $Snapshot 'phase-journal.jsonl') -ObservedEvents $events
    if (-not $journal.present -or -not $journal.complete -or $journal.active.Count -ne 0 -or $journal.terminal -cne 'end') { throw 'Original worker journal is not closed and successful.' }
    if ($Result.journal.present -ne $true -or $Result.journal.complete -ne $true -or $Result.journal.events -ne $journal.events -or
        $Result.journal.active.Count -ne 0 -or $Result.journal.terminal -cne $journal.terminal) { throw 'Original worker result and original journal disagree.' }
    $row = Get-P4kxPlatform -Target $Request.target
    if (@($events | Where-Object { $_.target -cne $Request.target -or $_.process_arch -ine $row.runner_arch }).Count -ne 0) { throw 'Original worker phase CPU or target differs.' }
    if ($Request.mode -ceq 'Lifecycle') {
        $cleanup = @($events | Where-Object { $_.phase -ceq 'final-cleanup' -and $_.status -ceq 'end' })
        if ($cleanup.Count -ne 1) { throw 'Original installer cleanup completion is unproven.' }
    }
}

function Assert-P4kxMsiObserverOriginalResult {
    param($Request, $Handle, [string]$Snapshot)
    $checkpoint = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'checkpoint.json')
    foreach ($name in @('request.json','handle.json','parent-start.json','parent-result.json','original-diagnostic-start.json','original-diagnostic-result.json','phase-journal.jsonl')) {
        if ($checkpoint.files[$name].status -cne 'captured' -or $checkpoint.files[$name].sha256 -cne (Get-P4kxDetachedDigest (Join-Path $Snapshot $name))) { throw 'Required original observer evidence is absent, unreadable or changed.' }
    }
    if ($checkpoint.schema -ne 1 -or $checkpoint.role -cne 'native-msi-observer-checkpoint' -or $checkpoint.mode -cne $Request.mode -or
        $checkpoint.request_sha256 -cne $Handle.request_sha256 -or $checkpoint.files['request.json'].sha256 -cne $Handle.request_sha256 -or
        $checkpoint.completed_success -ne $false -or $checkpoint.native_completion_unproven -ne $true -or $checkpoint.mutation_authority -ne $false) { throw 'Original checkpoint request or observation-only verdict differs.' }
    $start = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'parent-start.json')
    $returned = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'parent-result.json')
    foreach ($parent in @($start,$returned)) {
        if ($parent.pid -ne $Handle.pid -or $parent.started_utc_ticks -ne $Handle.started_utc_ticks -or
            $parent.request_sha256 -cne $Handle.request_sha256 -or $parent.tracking_sha256 -cne $Request.tracking_sha256 -or
            $parent.mode -cne $Request.mode -or $parent.mutation_authority -ne $false -or $parent.completed_success -ne $false -or $parent.native_completion_unproven -ne $true) { throw 'Original observer parent identity or verdict binding differs.' }
    }
    if ($start.role -cne 'native-msi-observer-parent-start' -or $returned.role -cne 'native-msi-observer-parent-result' -or $returned.invocation_succeeded -ne $true) { throw 'Original MSI diagnostic invocation did not succeed.' }
    $returnedAt = ConvertFrom-P4kxMsiBudgetTimestamp $returned.returned_utc
    if ($returnedAt -gt (ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc) -or
        $returnedAt -lt (ConvertFrom-P4kxMsiBudgetTimestamp $Request.prepared_at_utc)) { throw 'Original parent returned outside its unchanged observer allowance.' }
    $diagnosticStart = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'original-diagnostic-start.json')
    $result = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'original-diagnostic-result.json')
    foreach ($proof in @($diagnosticStart,$result)) {
        if ($proof.revision -cne $Request.revision -or $proof.run -cne $Request.run -or $proof.attempt -cne $Request.attempt -or
            $proof.target -cne $Request.target -or $proof.deadline_utc -cne $Request.deadline_utc -or $proof.mutation_authority -ne $false) { throw 'Original diagnostic source, run, CPU or deadline differs.' }
    }
    if ($diagnosticStart.schema -ne 1 -or $result.schema -ne 1 -or $diagnosticStart.completed_success -ne $false -or $diagnosticStart.native_completion_unproven -ne $true -or
        $diagnosticStart.role -cne 'native-msi-diagnostic-start' -or $diagnosticStart.parent_pid -ne $Handle.pid -or
        $diagnosticStart.worker_script -cne $Request.script -or $diagnosticStart.worker_sha256 -cne $Request.script_sha256 -or
        $result.role -cne 'native-msi-diagnostic-result' -or $result.completed_success -ne $true -or $result.native_completion_unproven -ne $false -or
        $result.journal_invalid -ne $false -or $result.worker_result_returned -ne $true -or $result.worker.started -ne $true -or
        $result.worker.pid -le 0 -or $result.worker.pid -eq $Handle.pid -or $result.worker.exited -ne $true -or $result.worker.exit_code -ne 0 -or $result.worker.signal_contract -cne 'direct-process-only' -or
        $diagnosticStart.budget_ms -ne $result.budget_ms -or $result.budget_ms -le 0 -or $result.budget_ms -gt 600000) { throw 'Original worker result is incomplete, failed or outside its existing allowance.' }
    Assert-P4kxMsiObserverClosedJournal $Request $result $Snapshot
    if ($Request.mode -ceq 'Lifecycle') {
        $row = Get-P4kxPlatform -Target $Request.target
        foreach ($name in @('original-diagnostic-inputs.json','original-diagnostic-sources.json')) {
            if ($checkpoint.files[$name].status -cne 'captured' -or $checkpoint.files[$name].sha256 -cne (Get-P4kxDetachedDigest (Join-Path $Snapshot $name))) { throw 'Required original MSI input/source evidence is absent or changed.' }
        }
        $sources = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'original-diagnostic-sources.json')
        if ($sources.revision -cne $Request.revision -or $sources.run -cne $Request.run -or $sources.attempt -cne $Request.attempt -or
            $sources.target -cne $Request.target -or $sources.process_arch -ine $row.runner_arch) { throw 'Original worker source receipt differs.' }
        $inputs = Read-P4kxDetachedReceipt (Join-Path $Snapshot 'original-diagnostic-inputs.json')
        if ($inputs.schema -ne 1 -or $inputs.role -cne 'native-msi-diagnostic-inputs' -or $inputs.revision -cne $Request.revision -or
            $inputs.run -cne $Request.run -or $inputs.attempt -cne $Request.attempt -or $inputs.target -cne $Request.target) { throw 'Original worker input receipt differs.' }
        $expectedSources = @(Get-P4kxMsiDiagnosticSources $PSScriptRoot)
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        if ($sources.sources.Count -ne $expectedSources.Count) { throw 'Original worker source inventory is incomplete.' }
        foreach ($source in $sources.sources) {
            if (-not $names.Add($source.name) -or @($expectedSources | Where-Object { $_.name -ceq $source.name -and $_.sha256 -ceq $source.sha256 }).Count -ne 1) { throw 'Original worker did not execute the admitted source bytes.' }
        }
    }
}
