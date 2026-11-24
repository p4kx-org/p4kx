param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateSet('Complete','Timeout','Admission')][string]$Case='Complete',
    [ValidateRange(1000,10000)][int]$Milliseconds=3000,
    [ValidateRange(0,1000)][int]$SnapshotWaitMilliseconds=0,[switch]$DetachedFinish,[switch]$LiveFinishProbe
)
$ErrorActionPreference = 'Stop'
if ($LiveFinishProbe -and ($Case -cne 'Complete' -or -not $DetachedFinish)) {
    throw 'The active Finish probe requires the harmless detached Complete fixture.'
}
$controlMilliseconds = if ($LiveFinishProbe) { 60000 } else { $Milliseconds }
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
if (-not $IsWindows -or [IO.Path]::Exists($Directory)) { throw 'Harmless observer needs a fresh Windows fixture directory.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$root = [IO.Path]::GetFullPath($Directory)
$output = Join-Path $root 'original diagnostic with spaces'
$observer = Join-Path $root 'observer with spaces'
$invocation = Join-Path $root 'invocation.json'
$entry = Join-Path $PSScriptRoot 'invoke-msi-observer.ps1'
$mode = switch ($Case) { 'Complete' {'observer-wait'} 'Timeout' {'timeout'} 'Admission' {'admission'} }
$parameters = @{Target=$env:P4KX_TARGET;Msi=$mode;FixtureDirectory=$PSScriptRoot;OutputDirectory=$output}
if ($Case -ceq 'Admission') { $parameters.AdmissionOnly=$true }
Write-P4kxDetachedReceipt $invocation $parameters
$saved = @{deadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;actions=$env:GITHUB_ACTIONS;runner=$env:RUNNER_ENVIRONMENT;tracking=$env:RUNNER_TRACKING_ID;target=$env:P4KX_TARGET}
$request=$null;$handle=$null
function Read-ObserverProgress([string]$Output) {
    $prefix = '[P4KX-MSI-OBSERVER]'
    [regex]::Split($Output, '\r?\n') |
        Where-Object { $_.StartsWith($prefix, [StringComparison]::Ordinal) } |
        ForEach-Object { $_.Substring($prefix.Length) | ConvertFrom-Json -AsHashtable -DateKind String }
}

function Assert-ObserverFinishProbeWatchdog($Launch) {
    . (Join-Path $PSScriptRoot 'msi-observer-finish-progress.ps1')
    Assert-ObserverRefused { Invoke-P4kxMsiBoundedFinishProbe $observer 5 -MaximumMilliseconds 8000 } `
        'The held Finish probe outlived its short watchdog.'
    $directory = Get-P4kxMsiFinishProbeDirectory $observer 5
    $watchdog = Read-P4kxDetachedReceipt (Join-Path $directory 'watchdog-result.json')
    $marker = Read-P4kxDetachedReceipt (Join-Path $directory 'finishProbeWait-begin.json')
    Assert-Observer ($watchdog.sequence -eq 5 -and $watchdog.budget_ms -eq 8000 -and
        $watchdog.worker.started -eq $true -and $watchdog.worker.exited -eq $true -and
        $watchdog.worker.timed_out -eq $true -and $watchdog.worker.signal_contract -ceq 'direct-process-only' -and
        $watchdog.worker.pid -notin @($handle.pid,$Launch.supervisor_pid) -and
        $watchdog.mutation_authority -eq $false -and $watchdog.native_completion_unproven -eq $true -and
        $marker.request_sha256 -ceq $Launch.request_sha256 -and
        -not [IO.File]::Exists((Join-Path $directory 'finishProbeWait-end.json')) -and
        -not [IO.Directory]::Exists((Join-Path $observer 'checkpoints/05')) -and
        -not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) 'Probe timeout lost durable partial evidence or disturbed the original observer.'
    [IO.Directory]::Move($directory,(Join-Path $root 'timed-out-probe-05'))
}

function Assert-ObserverLiveFinishProbe($Launch) {
    Assert-ObserverFinishProbeWatchdog $Launch
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$entry,
        '-Stage','ProbeFinish','-Directory',$observer,'-Sequence','4')
    $probe = Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $arguments $root 'live-finish-probe' -Milliseconds 50000
    Assert-Observer ($probe.exit_code -eq 0) 'The held Finish probe did not return successfully.'
    $progressDirectory = Join-Path $observer 'finish-probes/04'
    $records = @(Read-ObserverProgress ([IO.File]::ReadAllText((Join-Path $progressDirectory 'probe-stdout.log'))))
    $observed = @($records | ForEach-Object { $_.stage + '.' + $_.status })
    $expected = @('finishProbeCaller.begin','finishProbeBinding.begin','finishProbeBinding.end',
        'finishProbeWait.begin','finishProbeWait.end','finishProbeParent.begin','finishProbeParent.end',
        'finishProbeCheckpoint.begin','finishProbeCheckpoint.end','finishProbeReceipt.begin',
        'finishProbeReceipt.end','finishProbeCaller.end')
    Assert-Observer (($observed -join ' ') -ceq ($expected -join ' ')) 'The held Finish probe lost its ordered console boundaries.'
    foreach ($marker in $records) {
        $bound = $marker.stage -cne 'finishProbeCaller' -and
            -not ($marker.stage -ceq 'finishProbeBinding' -and $marker.status -ceq 'begin')
        $savedMarker = Read-P4kxDetachedReceipt (Join-Path $progressDirectory ($marker.stage + '-' + $marker.status + '.json'))
        Assert-Observer ($savedMarker.observed_utc -ceq $marker.observed_utc -and
            $marker.sequence -eq 4 -and $marker.completed_success -eq $false -and
            $marker.native_completion_unproven -eq $true -and $marker.mutation_authority -eq $false -and
            (-not $bound -or $marker.request_sha256 -ceq $Launch.request_sha256)) 'The held Finish probe marker lost its observation-only identity.'
    }

    $directory = Join-Path $observer 'checkpoints/04'
    $checkpointPath = Join-Path $directory 'checkpoint.json'
    $checkpoint = Read-P4kxDetachedReceipt $checkpointPath
    $progress = Read-P4kxDetachedReceipt (Join-Path $directory 'finish-progress.json')
    $started = [DateTimeOffset]::new([long]$Launch.supervisor_started_utc_ticks, [TimeSpan]::Zero)
    $sampled = ConvertFrom-P4kxMsiBudgetTimestamp $progress.observed_utc
    Assert-Observer ($sampled -ge $started.AddSeconds(30) -and $progress.sequence -eq 4 -and
        $progress.request_sha256 -ceq $Launch.request_sha256 -and
        $progress.checkpoint_sha256 -ceq (Get-P4kxDetachedDigest $checkpointPath) -and
        $progress.supervisor_pid -eq $Launch.supervisor_pid -and
        $progress.supervisor_started_utc_ticks -eq $Launch.supervisor_started_utc_ticks -and
        $progress.files['finish-result.json'].status -ceq 'absent' -and
        $progress.completed_success -eq $false -and $progress.native_completion_unproven -eq $true -and
        $progress.mutation_authority -eq $false) 'The held Finish probe lost its scheduled observation and original receipt binding.'
    Assert-Observer (-not $checkpoint.parent.lifetime_ended -and $checkpoint.journal.complete -eq $true -and
        $checkpoint.journal.events -eq 2 -and $checkpoint.journal.active.Count -eq 2 -and
        $checkpoint.journal.active -ccontains 'worker' -and $checkpoint.journal.active -ccontains 'fixture/wait' -and
        $null -eq $checkpoint.journal.terminal) 'The Finish probe did not observe the held original worker.'
    'PASS: Finish probe 4 observed the active harmless worker and preserved all 12 boundaries. The short watchdog stopped only probe 5.'
}

function Invoke-ObserverFinish {
    if ($DetachedFinish) {
        $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',$entry,'-Stage','StartFinish','-Directory',$observer)
        $launched=Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $arguments $root 'detached-finish-launch' -Milliseconds 4000
        Assert-Observer ($launched.exit_code -eq 0) 'The Finish launcher or its redirected streams did not close.'
        if ($Case -ceq 'Complete') {
            $records = @(Read-ObserverProgress $launched.stdout)
            $observed = @($records | ForEach-Object { $_.stage + '.' + $_.status })
            $expected = @('finishLaunchBinding.begin','finishLaunchBinding.end','finishLaunchProcess.begin',
                'finishLaunchProcess.end','finishLaunchReceipt.begin','finishLaunchReceipt.end')
            Assert-Observer (($observed -join ' ') -ceq ($expected -join ' ')) 'Finish launcher lost its ordered console boundaries.'
            foreach ($marker in $records) {
                Assert-Observer ($marker.completed_success -eq $false -and $marker.mutation_authority -eq $false -and
                    $marker.native_completion_unproven -eq $true) 'Finish launch marker claimed mutation or native completion.'
            }
            $launch=Read-P4kxDetachedReceipt (Join-Path $observer 'finish-supervision/launch.json')
            $supervisor=Get-Process -Id $launch.supervisor_pid
            try {
                Assert-Observer (-not $supervisor.HasExited -and $launch.request_sha256 -ceq $handle.request_sha256 -and
                    $supervisor.StartTime.ToUniversalTime().Ticks -eq $launch.supervisor_started_utc_ticks -and
                    [string]::Equals($supervisor.Path,[Environment]::ProcessPath,[StringComparison]::OrdinalIgnoreCase) -and
                    -not [IO.File]::Exists((Join-Path $observer 'finish-supervision/result.json'))) 'The exact supervisor did not outlive the launcher and its redirected streams.'
            } finally { $supervisor.Dispose() }
            if ($LiveFinishProbe) { Assert-ObserverLiveFinishProbe $launch }
            Write-P4kxDetachedReceipt (Join-Path $output 'observer-control-release.json') @{schema=1;role='harmless-msi-observer-release'}
        }
        & $entry -Stage WaitFinish -Directory $observer
        $wait = Read-P4kxDetachedReceipt (Join-Path $observer 'finish-supervision/wait-result.json')
        $terminal = Join-Path $observer 'finish-supervision/result.json'
        Assert-Observer ($wait.role -ceq 'native-msi-finish-receipt-wait' -and
            $wait.request_sha256 -ceq $handle.request_sha256 -and
            $wait.result_sha256 -ceq (Get-P4kxDetachedDigest $terminal) -and
            $wait.mutation_authority -eq $false) 'Bounded wait lost the original terminal receipt.'
        & $entry -Stage CollectFinish -Directory $observer
        return
    }
    & $entry -Stage Finish -Directory $observer -WaitMilliseconds 0
}

function Assert-ObserverFinishProgress {
    $launchPath = Join-Path $observer 'finish-supervision/launch.json'
    $terminalPath = Join-Path $observer 'finish-supervision/result.json'
    $waitPath = Join-Path $observer 'finish-supervision/wait-result.json'
    $launchBytes = [IO.File]::ReadAllBytes($launchPath)
    $firstSequence = if ($LiveFinishProbe) { 5 } else { 4 }
    try {
        $foreign = Read-P4kxDetachedReceipt $launchPath
        $foreign.request_sha256 = 'f' * 64
        [IO.File]::WriteAllText($launchPath, ($foreign | ConvertTo-Json -Compress))
        Assert-ObserverRefused { & $entry -Stage ProbeFinish -Directory $observer -Sequence $firstSequence } 'Foreign Finish launch authorized a progress checkpoint.'
        $checkpoint = Join-Path $observer ('checkpoints/{0:d2}' -f $firstSequence)
        Assert-Observer (-not [IO.Path]::Exists($checkpoint)) 'Refused Finish probe created a checkpoint.'
        $refused = Join-Path $observer ('finish-probes/{0:d2}' -f $firstSequence)
        [IO.Directory]::Move($refused,(Join-Path $root 'refused-finish-probe'))
    } finally { [IO.File]::WriteAllBytes($launchPath, $launchBytes) }

    $launch = Read-P4kxDetachedReceipt $launchPath
    $missing = Join-Path $observer 'snapshots/09/stopExpiredParent-begin.json'
    $oversize = Join-Path $observer 'snapshots/09/stopExpiredParent-end.json'
    $unrelated = Join-Path $observer 'snapshots/09/unrelated.log'
    $savedWait = Join-Path $root 'saved-wait-result.json'
    [IO.File]::Move($waitPath, $savedWait)
    try {
        Assert-Observer (-not [IO.File]::Exists($missing) -and -not [IO.File]::Exists($oversize)) 'Successful fixture unexpectedly occupied cleanup markers.'
        [IO.File]::WriteAllBytes($oversize, [byte[]]::new(16KB+1))
        [IO.File]::WriteAllText($unrelated, 'not part of the observer allowlist')
        foreach ($sequence in $firstSequence..5) {
            & $entry -Stage ProbeFinish -Directory $observer -Sequence $sequence
            $checkpoint = Join-Path $observer ('checkpoints/{0:d2}' -f $sequence)
            $path = Join-Path $checkpoint 'checkpoint.json'
            $proof = Read-P4kxDetachedReceipt (Join-Path $checkpoint 'finish-progress.json')
            Assert-Observer ($proof.role -ceq 'native-msi-finish-progress' -and $proof.sequence -eq $sequence -and
                $proof.request_sha256 -ceq $handle.request_sha256 -and $proof.checkpoint_sha256 -ceq (Get-P4kxDetachedDigest $path) -and
                $proof.supervisor_pid -eq $launch.supervisor_pid -and $proof.supervisor_started_utc_ticks -eq $launch.supervisor_started_utc_ticks -and
                $proof.cutoff_utc -ceq $launch.cutoff_utc -and $proof.target -ceq $request.target -and
                $proof.revision -ceq $request.revision -and $proof.run -ceq $request.run -and $proof.attempt -ceq $request.attempt -and
                $proof.completed_success -eq $false -and $proof.native_completion_unproven -eq $true -and $proof.mutation_authority -eq $false) 'Finish checkpoint lost its exact observation-only binding.'
            Assert-Observer ($proof.files['finish-launch.json'].sha256 -ceq (Get-P4kxDetachedDigest $launchPath) -and
                $proof.files['finish-result.json'].sha256 -ceq (Get-P4kxDetachedDigest $terminalPath) -and
                $proof.files['finish-stopExpiredParent-begin.json'].status -ceq 'absent' -and
                $proof.files['finish-stopExpiredParent-end.json'].status -ceq 'unreadable' -and
                -not [IO.File]::Exists((Join-Path $checkpoint 'finish-stopExpiredParent-end.json')) -and
                -not [IO.File]::Exists((Join-Path $checkpoint 'unrelated.log'))) 'Finish checkpoint did not bound its copied scopes and bytes.'
        }
        Assert-ObserverRefused { & $entry -Stage ProbeFinish -Directory $observer -Sequence 4 } 'Repeated Finish probe could overwrite a checkpoint.'
        Assert-ObserverRefused { & $entry -Stage ProbeFinish -Directory $observer -Sequence 9 } 'Finish probe could occupy the acceptance checkpoint.'
        Assert-ObserverRefused { & $entry -Stage CollectFinish -Directory $observer } 'Progress evidence replaced the required bounded terminal wait.'
    } finally {
        [IO.File]::Delete($oversize)
        [IO.File]::Delete($unrelated)
        [IO.File]::Move($savedWait, $waitPath)
    }
    & $entry -Stage CollectFinish -Directory $observer
}
function Assert-Observer([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Assert-ObserverRefused([scriptblock]$Action, [string]$Message) {
    $refused=$false
    try { & $Action | Out-Null } catch { $refused=$true }
    Assert-Observer $refused $Message
}
try {
    $env:GITHUB_ACTIONS='true';$env:RUNNER_ENVIRONMENT='github-hosted'
    if ([string]::IsNullOrEmpty($env:RUNNER_TRACKING_ID)) { $env:RUNNER_TRACKING_ID='synthetic-observer-control' }
    $env:P4KX_TARGET=(Get-P4kxCurrentPlatform).target
    $parameters.Target=$env:P4KX_TARGET
    [IO.File]::Delete($invocation)
    Write-P4kxDetachedReceipt $invocation $parameters
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddMilliseconds($controlMilliseconds).ToString('o')
    & $entry -Stage Start -Directory $observer -Invocation $invocation -Mode Control -ControlMilliseconds $controlMilliseconds
    $request=Read-P4kxDetachedReceipt (Join-Path $observer 'request.json')
    $handle=Read-P4kxDetachedReceipt (Join-Path $observer 'handle.json')
    $bad=$handle.Clone();$bad.request_sha256='not-the-original-request'
    Assert-ObserverRefused {Get-P4kxMsiObserverParentState $request $bad} 'A foreign original request was accepted.'
    $bad=$handle.Clone();$bad.started_utc_ticks=[long]$bad.started_utc_ticks+1
    Assert-ObserverRefused {$process=Open-P4kxDetachedOwnedProcess $bad $request -Role native-msi-observer-handle;$process.Dispose()} 'A foreign process lifetime was accepted.'
    if ($Case -ceq 'Complete') {
        $marker=Join-Path $output 'observer-control-active.json'
        while (-not [IO.File]::Exists($marker)) {
            Assert-Observer (-not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) 'Original harmless parent ended before its active journal.'
            Assert-Observer ([DateTimeOffset]::UtcNow -lt (ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc)) 'Harmless active phase marker exceeded its original deadline.'
            Start-Sleep -Milliseconds 20
        }
        $snapshotTimer=[Diagnostics.Stopwatch]::StartNew()
        & $entry -Stage Snapshot -Directory $observer -Sequence 1 -WaitMilliseconds $SnapshotWaitMilliseconds
        Assert-Observer ($snapshotTimer.ElapsedMilliseconds -ge $SnapshotWaitMilliseconds) 'Snapshot returned before its nonzero original observation interval.'
        $checkpoint=Read-P4kxDetachedReceipt (Join-Path $observer 'checkpoints/01/checkpoint.json')
        Assert-Observer (-not $checkpoint.parent.lifetime_ended -and $checkpoint.request_sha256 -ceq $handle.request_sha256 -and
            $checkpoint.journal.events -eq 2 -and $checkpoint.journal.active.Count -eq 2 -and $null -eq $checkpoint.journal.terminal -and
            $checkpoint.completed_success -eq $false -and $checkpoint.native_completion_unproven -eq $true -and $checkpoint.mutation_authority -eq $false) 'Active original journal lost identity or invented completion.'
        if (-not $DetachedFinish) {
            Write-P4kxDetachedReceipt (Join-Path $output 'observer-control-release.json') @{schema=1;role='harmless-msi-observer-release'}
        }
        Invoke-ObserverFinish
    } else {
        Assert-ObserverRefused {Invoke-ObserverFinish} 'Failed or admission-only worker became complete lifecycle evidence.'
    }
    if ($DetachedFinish -and $Case -ceq 'Complete') {
        Assert-ObserverFinishProgress
        . (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')
        $context = Get-P4kxMsiFinishContext $observer
        $launch = Read-P4kxMsiFinishLaunch $context
        $expired = $context.Clone()
        $expired.directory = Join-Path $root 'expired terminal wait'
        $null = New-Item -ItemType Directory -Path $expired.directory
        $expired.collection_cutoff = [DateTimeOffset]::UtcNow.AddSeconds(-1)
        Assert-ObserverRefused { Wait-P4kxMsiFinishReceipt $expired $launch } 'Expired terminal wait accepted an absent result.'
        $absence = Read-P4kxDetachedReceipt (Join-Path $expired.directory 'wait-result.json')
        Assert-Observer ($absence.role -ceq 'native-msi-finish-receipt-wait' -and
            $absence.request_sha256 -ceq $handle.request_sha256 -and $null -eq $absence.result_sha256 -and
            $absence.deadline_utc -ceq $expired.collection_cutoff.ToString('o') -and
            $absence.mutation_authority -eq $false) 'Expired wait did not preserve exact absence evidence.'
        $supervision=Join-Path $context.directory 'result.json'
        $waitPath=Join-Path $context.directory 'wait-result.json'
        $valid=[IO.File]::ReadAllText($supervision)
        $validWait=[IO.File]::ReadAllText($waitPath)
        try {
            $wrongWait=Read-P4kxDetachedReceipt $waitPath
            $wrongWait.request_sha256='f' * 64
            [IO.File]::WriteAllText($waitPath,($wrongWait | ConvertTo-Json -Compress))
            Assert-ObserverRefused { & $entry -Stage CollectFinish -Directory $observer } 'A foreign terminal wait passed validation.'
            [IO.File]::WriteAllText($waitPath,$validWait)
            [IO.File]::WriteAllText($supervision,($valid + ' '))
            Assert-ObserverRefused { & $entry -Stage CollectFinish -Directory $observer } 'The terminal bytes changed after the bounded wait.'
            foreach ($mutation in @(
                { param($proof) $proof.supervisor_pid++ },
                { param($proof) $proof.caller_result_sha256='f' * 64 },
                { param($proof) $proof.observed_exit_utc=[DateTimeOffset]::UtcNow.AddMinutes(2).ToString('o') }
            )) {
                $changed=$valid | ConvertFrom-Json -AsHashtable -DateKind String
                & $mutation $changed
                [IO.File]::WriteAllText($supervision,($changed | ConvertTo-Json -Compress))
                $wait=$validWait | ConvertFrom-Json -AsHashtable -DateKind String
                $wait.result_sha256=Get-P4kxDetachedDigest $supervision
                [IO.File]::WriteAllText($waitPath,($wait | ConvertTo-Json -Compress))
                Assert-ObserverRefused { & $entry -Stage CollectFinish -Directory $observer } 'Wrong owner, receipt or deadline passed detached Finish.'
            }
        } finally {
            [IO.File]::WriteAllText($supervision,$valid)
            [IO.File]::WriteAllText($waitPath,$validWait)
        }
    }
    $finish=Read-P4kxDetachedReceipt (Join-Path $observer 'snapshots/09/caller-result.json')
    $finishedWrite=Read-P4kxDetachedReceipt (Join-Path $observer 'snapshots/09/writeResult-end.json')
    Assert-Observer ($finish.role -ceq 'native-msi-finish-return' -and $finish.sequence -eq 9 -and
        $finish.finish_returned -eq ($Case -ceq 'Complete') -and $finish.worker.exited -eq $true -and
        $finish.worker.pid -ne $handle.pid -and $finish.worker.signal_contract -ceq 'direct-process-only' -and
        $finishedWrite.request_sha256 -ceq $handle.request_sha256 -and $finishedWrite.mutation_authority -eq $false) 'The actual Finish worker lost its separate result and original request binding.'
    $receipt=Read-P4kxDetachedReceipt (Join-Path $observer 'observer-result.json')
    Assert-Observer ($receipt.completed_success -eq $false -and $receipt.native_completion_unproven -eq $true -and
        $receipt.mutation_authority -eq $false -and $receipt.control_passed -eq ($Case -ceq 'Complete')) 'Harmless observer invented a native MSI verdict.'
    $original=Read-P4kxDetachedReceipt (Join-Path $output 'diagnostics/diagnostic-result.json')
    Assert-Observer ($original.worker.started -eq $true -and $original.worker.exited -eq $true -and $original.worker.signal_contract -ceq 'direct-process-only') 'Original parent lost its actual owned worker result.'
    if ($Case -ceq 'Complete') {
        $journal=Read-P4kxMsiPhaseJournal (Join-Path $observer 'checkpoints/09/phase-journal.jsonl')
        Assert-Observer ($journal.events -eq 4 -and $journal.active.Count -eq 0 -and $journal.terminal -ceq 'end' -and $receipt.parent.lifetime_ended) 'Completed control lost its inverse closed original journal.'
        $bad=$request.Clone();$bad.mode='Lifecycle';$bad.script='check-msi.ps1';$bad.script_sha256=Get-P4kxDetachedDigest (Join-Path $PSScriptRoot 'check-msi.ps1')
        Assert-ObserverRefused {Assert-P4kxMsiObserverClosedJournal $bad $original $receipt.original_snapshot} 'A harmless worker without installer cleanup became lifecycle evidence.'
        Assert-ObserverRefused {Save-P4kxMsiObserverCheckpoint $request $handle 1 $receipt.parent} 'A duplicate checkpoint sequence overwrote prior evidence.'
        $bad=$handle.Clone();$bad.request_sha256='foreign-after-exit'
        Assert-ObserverRefused {Get-P4kxMsiObserverParentState $request $bad} 'An absent original parent bypassed handle binding.'
        $bad=$request.Clone();$bad.observer_deadline_utc=$request.prepared_at_utc
        Assert-ObserverRefused {Assert-P4kxMsiObserverOriginalResult $bad $handle $receipt.original_snapshot} 'A late original parent result bypassed the fused observer allowance.'
        $shared=Join-Path $root 'shared-original-journal'
        $writer=[IO.File]::Open($shared,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
        try {
            $writer.WriteByte(1);$writer.Flush($true)
            $copy=Copy-P4kxMsiObserverFile $shared (Join-Path $root 'shared-checkpoint') 16
            Assert-Observer ($copy.status -ceq 'captured' -and $copy.bytes -eq 1) 'Observer could not read the actual original writer sharing contract.'
            $writer.WriteByte(2);$writer.Flush($true)
        } finally { $writer.Dispose() }
        $copy=Copy-P4kxMsiObserverFile $shared (Join-Path $root 'over-bound-checkpoint') 1
        Assert-Observer ($copy.status -ceq 'unreadable' -and -not [IO.File]::Exists((Join-Path $root 'over-bound-checkpoint'))) 'Over-bound original evidence was copied or classified as complete.'
        $bad=$request.Clone();$bad.sources=@($request.sources | Select-Object -Skip 1)
        Assert-ObserverRefused {Assert-P4kxMsiObserverBinding $bad $observer} 'Incomplete source inventory passed observer admission.'
    } elseif ($Case -ceq 'Timeout') {
        Assert-Observer ($original.completed_success -eq $false -and $original.native_completion_unproven -eq $true -and $original.worker_result_returned -eq $false -and
            $original.journal.events -eq 2 -and $original.journal.active.Count -eq 2 -and $null -eq $original.journal.terminal) 'Timeout lost its explicit unfinished original scopes.'
    } else {
        Assert-Observer ($original.role -ceq 'native-msi-admission-result' -and $original.admission_passed -eq $true -and
            $original.completed_success -eq $false -and $original.native_completion_unproven -eq $true) 'Admission mutated into a complete original diagnostic result.'
    }
    Assert-Observer ((Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) 'Original harmless observer parent survived.'
    Assert-Observer (-not (Get-Process -Id $original.worker.pid -ErrorAction SilentlyContinue)) 'Original harmless worker survived.'
    "PASS: actual observer/diagnostic/Portal $Case control, bound original PID, checkpoint and result; no installer execution or native MSI verdict."
} finally {
    if ($null -ne $request -and $null -ne $handle -and -not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) { [void](Stop-P4kxMsiObserverParent $request $handle) }
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$saved.deadline;$env:GITHUB_ACTIONS=$saved.actions;$env:RUNNER_ENVIRONMENT=$saved.runner
    $env:RUNNER_TRACKING_ID=$saved.tracking;$env:P4KX_TARGET=$saved.target
}
