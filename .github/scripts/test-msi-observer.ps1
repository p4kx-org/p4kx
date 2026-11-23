param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateSet('Complete','Timeout','Admission')][string]$Case='Complete',
    [ValidateRange(1000,10000)][int]$Milliseconds=3000,
    [ValidateRange(0,1000)][int]$SnapshotWaitMilliseconds=0,[switch]$DetachedFinish
)
$ErrorActionPreference = 'Stop'
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
function Invoke-ObserverFinish {
    if ($DetachedFinish) {
        $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',$entry,'-Stage','StartFinish','-Directory',$observer)
        $launched=Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $arguments $root 'detached-finish-launch' -Milliseconds 4000
        Assert-Observer ($launched.exit_code -eq 0) 'The Finish launcher or its redirected streams did not close.'
        if ($Case -ceq 'Complete') {
            $launch=Read-P4kxDetachedReceipt (Join-Path $observer 'finish-supervision/launch.json')
            $supervisor=Get-Process -Id $launch.supervisor_pid
            try {
                Assert-Observer (-not $supervisor.HasExited -and $launch.request_sha256 -ceq $handle.request_sha256 -and
                    $supervisor.StartTime.ToUniversalTime().Ticks -eq $launch.supervisor_started_utc_ticks -and
                    [string]::Equals($supervisor.Path,[Environment]::ProcessPath,[StringComparison]::OrdinalIgnoreCase) -and
                    -not [IO.File]::Exists((Join-Path $observer 'finish-supervision/result.json'))) 'The exact supervisor did not outlive the launcher and its redirected streams.'
            } finally { $supervisor.Dispose() }
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
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddMilliseconds($Milliseconds).ToString('o')
    & $entry -Stage Start -Directory $observer -Invocation $invocation -Mode Control -ControlMilliseconds $Milliseconds
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
