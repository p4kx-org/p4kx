$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')

function Start-P4kxMsiObservedParent {
    param($Request)
    $path = Join-Path $Request.directory 'request.json'
    Write-P4kxDetachedReceipt $path $Request
    $executable = (Get-Process -Id $PID).Path
    $script = Join-Path $PSScriptRoot 'msi-observer-parent.ps1'
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',('"'+$script+'"'),'-RequestPath',('"'+$path+'"'))
    $process = Start-Process -FilePath $executable -ArgumentList $arguments -WindowStyle Hidden -PassThru -WorkingDirectory $Request.location
    try {
        Write-P4kxDetachedReceipt (Join-Path $Request.directory 'handle.json') @{
            schema=1;role='native-msi-observer-handle';pid=$process.Id;started_utc_ticks=$process.StartTime.ToUniversalTime().Ticks;
            executable=$executable;launcher_pid=$PID;request_sha256=(Get-P4kxDetachedDigest $path)
        }
    } finally { $process.Dispose() }
}

function Get-P4kxMsiObserverParentState {
    param($Request, $Handle)
    if ($Handle.schema -ne 1 -or $Handle.role -cne 'native-msi-observer-handle' -or $Handle.pid -le 0 -or
        $Handle.started_utc_ticks -le 0 -or $Handle.request_sha256 -cne (Get-P4kxDetachedDigest (Join-Path $Request.directory 'request.json'))) { throw 'MSI observer handle differs from its original request.' }
    try { $owned = Open-P4kxDetachedOwnedProcess $Handle $Request -Role 'native-msi-observer-handle' }
    catch {
        $observed = Get-Process -Id $Handle.pid -ErrorAction SilentlyContinue
        if ($null -eq $observed) { return @{lifetime_ended=$true;exit_code=$null;state='absent'} }
        try {
            if ($observed.StartTime.ToUniversalTime().Ticks -ne $Handle.started_utc_ticks) { return @{lifetime_ended=$true;exit_code=$null;state='pidReused'} }
        } finally { $observed.Dispose() }
        throw
    }
    try { return @{lifetime_ended=$owned.HasExited;exit_code=$(if($owned.HasExited){$owned.ExitCode}else{$null});state=$(if($owned.HasExited){'exited'}else{'running'})} }
    finally { $owned.Dispose() }
}

function Wait-P4kxMsiObserverParent {
    param($Request, $Handle, [int]$Milliseconds)
    $deadline = ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        $state = Get-P4kxMsiObserverParentState $Request $Handle
        if ($state.lifetime_ended) { return $state }
        $remaining = [Math]::Min($Milliseconds-$timer.ElapsedMilliseconds, ($deadline-[DateTimeOffset]::UtcNow).TotalMilliseconds)
        if ($remaining -le 0) { return $state }
        Start-Sleep -Milliseconds ([int][Math]::Min(100, $remaining))
    } while ($true)
}

function Stop-P4kxMsiObserverParent {
    param($Request, $Handle)
    $state = Get-P4kxMsiObserverParentState $Request $Handle
    if ($state.lifetime_ended) { return $state }
    try { $owned = Open-P4kxDetachedOwnedProcess $Handle $Request -Role 'native-msi-observer-handle' }
    catch {
        $state = Get-P4kxMsiObserverParentState $Request $Handle
        if ($state.lifetime_ended) { return $state }
        throw
    }
    try {
        if (-not $owned.WaitForExit(5000)) {
            $owned.Kill($false)
            if (-not $owned.WaitForExit(5000)) { throw 'Original observer parent did not stop after its existing cleanup grace.' }
        }
        return @{lifetime_ended=$owned.HasExited;exit_code=$owned.ExitCode;state='stoppedAfterDeadline'}
    } finally { $owned.Dispose() }
}

function Stop-P4kxMsiObserverExpiredParent {
    param($Request, $Handle)
    if ([DateTimeOffset]::UtcNow -lt (ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc)) { throw 'MSI observer cannot signal its parent before the original allowance expires.' }
    $state = Stop-P4kxMsiObserverParent $Request $Handle
    Write-P4kxDetachedReceipt (Join-Path $Request.directory 'deadline-stop.json') @{
        schema=1;role='native-msi-observer-deadline-stop';request_sha256=$Handle.request_sha256;parent=$state;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');observer_deadline_utc=$Request.observer_deadline_utc;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    return $state
}
