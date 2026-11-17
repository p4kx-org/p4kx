param(
    [Parameter(Mandatory)][ValidateSet('Start','Snapshot','Finish')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory,
    [int]$Milliseconds=90000
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
$root = [IO.Path]::GetFullPath($Directory)
$requestPath = Join-Path $root 'request.json'
$handlePath = Join-Path $root 'handle.json'
if ($Stage -ceq 'Start') {
    $request = New-P4kxDetachedControlRequest $root $Milliseconds
    Write-P4kxDetachedReceipt $requestPath $request
    $executable = (Get-Process -Id $PID).Path
    $worker = Join-Path $PSScriptRoot 'msi-detached-control-worker.ps1'
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',('"'+$worker+'"'),'-RequestPath',('"'+$requestPath+'"'))
    $process = Start-Process -FilePath $executable -ArgumentList $arguments -WindowStyle Hidden -PassThru -WorkingDirectory $root
    try {
        Write-P4kxDetachedReceipt $handlePath @{
            schema=1;role='native-msi-detached-control-handle';pid=$process.Id;started_utc_ticks=$process.StartTime.ToUniversalTime().Ticks;
            executable=$executable;launcher_pid=$PID;request_sha256=(Get-P4kxDetachedDigest $requestPath)
        }
    } finally { $process.Dispose() }
    return
}
$request = Read-P4kxDetachedReceipt $requestPath
Assert-P4kxDetachedBinding $request $root
$handle = Read-P4kxDetachedReceipt $handlePath
$process = Open-P4kxDetachedOwnedProcess $handle $request
$primaryFailure = $null
try {
    if ($handle.launcher_pid -eq $PID -or $process.HasExited) { throw 'Cross-step worker did not survive its distinct launcher.' }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $startPath = Join-Path $root 'worker-start.json'
    while (-not [IO.File]::Exists($startPath)) {
        if ($timer.ElapsedMilliseconds -ge 5000 -or $process.HasExited) { throw 'Detached control did not expose its original worker phase.' }
        Start-Sleep -Milliseconds 25
    }
    $start = Read-P4kxDetachedReceipt $startPath
    if ($start.pid -ne $handle.pid -or $start.started_utc_ticks -ne $handle.started_utc_ticks -or
        $start.request_sha256 -cne $handle.request_sha256 -or $start.tracking_sha256 -cne $request.tracking_sha256) {
        throw 'Detached worker identity or inherited tracking binding differs.'
    }
    $journalPath = Join-Path $root 'diagnostics/phase-journal.jsonl'
    $snapshotPath = $journalPath
    if ($Stage -ceq 'Snapshot') {
        $checkpoint = Join-Path $root 'checkpoint'
        New-Item -ItemType Directory -Path $checkpoint | Out-Null
        foreach ($name in @('request.json','handle.json','worker-start.json')) {
            Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $checkpoint $name)
        }
        $snapshotPath = Join-Path $checkpoint 'phase-journal.jsonl'
        Copy-Item -LiteralPath $journalPath -Destination $snapshotPath
    }
    $journal = Read-P4kxMsiPhaseJournal $snapshotPath
    if (-not $journal.present -or -not $journal.complete -or $journal.events -ne 2 -or
        $null -ne $journal.terminal -or $journal.active.Count -ne 2 -or
        $journal.active -cnotcontains 'fixture/cross-step-wait' -or $journal.active -cnotcontains 'worker') {
        throw 'Detached control lost its original still-active harmless phase.'
    }
    if ($Stage -ceq 'Snapshot') {
        Write-P4kxDetachedReceipt (Join-Path $checkpoint 'checkpoint.json') @{
            schema=1;role='native-msi-detached-control-checkpoint';revision=$request.revision;run=$request.run;attempt=$request.attempt;target=$request.target;
            worker_pid=$process.Id;started_utc_ticks=$handle.started_utc_ticks;observed_by_pid=$PID;request_sha256=$handle.request_sha256;
            tracking_sha256=$start.tracking_sha256;hosted_runner=$request.hosted_runner;deadline_utc=$request.deadline_utc;
            worker_deadline_utc=$request.worker_deadline_utc;observed_utc=[DateTimeOffset]::UtcNow.ToString('o');worker_alive=(-not $process.HasExited);journal=$journal;completed_success=$false;
            native_completion_unproven=$true;mutation_authority=$false
        }
        return
    }
    [void](Get-P4kxMsiRemainingBudget $request.deadline_utc)
    Write-P4kxDetachedReceipt (Join-Path $root 'release.json') @{request_sha256=$handle.request_sha256}
    if (-not $process.WaitForExit(5000)) { throw 'Released harmless worker did not exit within its completion bound.' }
    if ($process.ExitCode -ne 0) { throw 'Released harmless worker failed.' }
    $result = Read-P4kxDetachedReceipt (Join-Path $root 'worker-result.json')
    $closed = Read-P4kxMsiPhaseJournal $journalPath
    if ($result.request_sha256 -cne $handle.request_sha256 -or $result.control_passed -ne $true -or
        -not $closed.complete -or $closed.events -ne 4 -or $closed.active.Count -ne 0 -or $closed.terminal -cne 'end') {
        throw 'Harmless worker completion lacks original source and closed phase proof.'
    }
    Write-P4kxDetachedReceipt (Join-Path $root 'complete.json') @{
        schema=1;role='native-msi-detached-control-complete';revision=$request.revision;run=$request.run;attempt=$request.attempt;target=$request.target;
        worker_pid=$handle.pid;started_utc_ticks=$handle.started_utc_ticks;request_sha256=$handle.request_sha256;
        control_passed=$true;worker_exited=$process.HasExited;exit_code=$process.ExitCode;journal=$closed;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
} catch { $primaryFailure = $_.Exception; throw }
finally {
    $failures = [Collections.Generic.List[Exception]]::new()
    try {
        if ($Stage -ceq 'Finish' -and -not $process.HasExited) {
            $process.Kill($false)
            if (-not $process.WaitForExit(1000)) { throw 'Owned harmless worker did not terminate during control cleanup.' }
        }
    } catch { $failures.Add($_.Exception) }
    try { $process.Dispose() } catch { $failures.Add($_.Exception) }
    if ($failures.Count -gt 0) {
        if ($null -ne $primaryFailure) { $failures.Insert(0, $primaryFailure) }
        throw [AggregateException]::new('Detached control and owned-process cleanup failed.', $failures)
    }
}
