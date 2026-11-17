param(
    [Parameter(Mandatory)][string]$Directory,
    [int]$LiveBudgetMilliseconds=15000,
    [int]$StageMilliseconds=5000,
    [int]$ExpiredMilliseconds=1000
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')
if (-not $IsWindows -or [IO.Path]::Exists($Directory)) { throw 'Harmless control test needs a fresh Windows fixture directory.' }
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$root = [IO.Path]::GetFullPath($Directory)
$script = Join-Path $PSScriptRoot 'invoke-msi-detached-control.ps1'
$executable = (Get-Process -Id $PID).Path

function Invoke-ControlStage([string]$Stage, [string]$Path, [int]$Budget=$LiveBudgetMilliseconds) {
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$script,'-Stage',$Stage,'-Directory',$Path,'-Milliseconds',[string]$Budget)
    $result = Invoke-P4kxPortalProcess $executable $arguments $root ('stage-'+$Stage.ToLowerInvariant()+'-'+([IO.Path]::GetFileName($Path).Replace(' ','-'))) -Milliseconds $StageMilliseconds
    if ($result.exit_code -ne 0) { throw ('Harmless control stage failed: '+$result.stderr) }
}

$complete = Join-Path $root 'complete with spaces'
Invoke-ControlStage Start $complete
$request = Read-P4kxDetachedReceipt (Join-Path $complete 'request.json')
$handle = Read-P4kxDetachedReceipt (Join-Path $complete 'handle.json')
$bad = $handle.Clone()
$bad.started_utc_ticks = [long]$bad.started_utc_ticks+1
$refused = $false
try { $foreign = Open-P4kxDetachedOwnedProcess $bad $request; $foreign.Dispose() } catch { $refused = $true }
if (-not $refused) { throw 'A mismatched process lifetime was accepted.' }
$bad = $handle.Clone()
$bad.request_sha256 = 'not-the-original-request'
$refused = $false
try { $foreign = Open-P4kxDetachedOwnedProcess $bad $request; $foreign.Dispose() } catch { $refused = $true }
if (-not $refused) { throw 'A foreign original request was accepted.' }
Invoke-ControlStage Snapshot $complete
$checkpoint = Read-P4kxDetachedReceipt (Join-Path $complete 'checkpoint/checkpoint.json')
if (-not $checkpoint.worker_alive -or $checkpoint.completed_success -ne $false -or $checkpoint.mutation_authority -ne $false -or
    $checkpoint.native_completion_unproven -ne $true -or $checkpoint.request_sha256 -cne $handle.request_sha256) {
    throw 'Active checkpoint claimed native completion or lost its original request.'
}
Invoke-ControlStage Finish $complete
$result = Read-P4kxDetachedReceipt (Join-Path $complete 'complete.json')
if (-not $result.control_passed -or -not $result.worker_exited -or $result.exit_code -ne 0 -or
    $result.completed_success -ne $false -or $result.native_completion_unproven -ne $true) {
    throw 'The harmless completion receipt invented installer evidence.'
}
$alive = Get-Process -Id $handle.pid -ErrorAction SilentlyContinue
if ($null -ne $alive) {
    try {
        if ($alive.StartTime.ToUniversalTime().Ticks -eq $handle.started_utc_ticks) { throw 'Owned completed fixture worker survived.' }
    } finally { $alive.Dispose() }
}

$expired = Join-Path $root 'expired'
Invoke-ControlStage Start $expired $ExpiredMilliseconds
$request = Read-P4kxDetachedReceipt (Join-Path $expired 'request.json')
$handle = Read-P4kxDetachedReceipt (Join-Path $expired 'handle.json')
$existing = Get-Process -Id $handle.pid -ErrorAction SilentlyContinue
if ($null -ne $existing) {
    $existing.Dispose()
    $process = Open-P4kxDetachedOwnedProcess $handle $request
    try {
        if (-not $process.WaitForExit($StageMilliseconds) -or $process.ExitCode -eq 0) { throw 'Unreleased fixture did not fail by its original deadline.' }
    } finally {
        try {
            if (-not $process.HasExited) {
                $process.Kill($false)
                if (-not $process.WaitForExit(1000)) { throw 'Owned expired fixture survived cleanup.' }
            }
        } finally { $process.Dispose() }
    }
}
$errorReceipt = Read-P4kxDetachedReceipt (Join-Path $expired 'worker-error.json')
if ($errorReceipt.request_sha256 -cne $handle.request_sha256 -or $errorReceipt.deadline_expired -ne $true) {
    throw 'Expected deadline failure lacks the actual source-bound worker observation.'
}
$journal = Read-P4kxMsiPhaseJournal (Join-Path $expired 'diagnostics/phase-journal.jsonl')
if (-not $journal.complete -or $journal.terminal -cne 'error' -or [IO.File]::Exists((Join-Path $expired 'worker-result.json'))) {
    throw 'Expired fixture lost its failed terminal phase or invented release completion.'
}
'PASS: harmless detached worker lifetime, original handles, active checkpoint, controlled release and deadline refusal; no MSI, Registry, PATH or installer success.'
