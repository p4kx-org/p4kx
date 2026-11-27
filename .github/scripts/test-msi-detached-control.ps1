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

function Assert-DetachedReceiptPublication {
    $task = Join-Path $root 'receipt-publication'
    New-Item -ItemType Directory -Path $task | Out-Null
    $receiptPath = Join-Path $task 'source-bound.json'
    $prepared = Join-Path $task 'write-open'
    $released = Join-Path $task 'write-release'
    $writer = Join-Path $task 'writer.ps1'
    $source = (Get-Item 'Function:Write-P4kxDetachedReceipt').ScriptBlock.ToString()
    $write = '$stream.Write($bytes)'
    if ($source.Split($write,[StringSplitOptions]::None).Count -ne 2) { throw 'Expected one receipt byte-write boundary.' }
    $hold = @(
        '[IO.File]::WriteAllText($Prepared,''ready'')'
        '$watch = [Diagnostics.Stopwatch]::StartNew()'
        'while (-not [IO.File]::Exists($Released)) {'
        '    if ($watch.ElapsedMilliseconds -ge 5000) { throw ''Receipt publication test did not release the writer.'' }'
        '    Start-Sleep -Milliseconds 20'
        '}'
    ) -join [char]10
    $header = 'param([string]$Path,[string]$Prepared,[string]$Released)'
    $body = $source.Replace($write,($hold + [char]10 + $write))
    @($header, ('$ErrorActionPreference=''Stop'''), ('function Write-Receipt {' + [char]10 + $body + [char]10 + '}'),
        ('Write-Receipt $Path @{schema=1;observation=''original''}')) |
        Set-Content -LiteralPath $writer -Encoding utf8NoBOM
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$writer,$receiptPath,$prepared,$released)
    $process = New-P4kxPackageProcess $executable $arguments $task
    try {
        [void]$process.Start()
        $process.StandardInput.Close()
        $timer = [Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists($prepared)) {
            if ($timer.ElapsedMilliseconds -ge 4000 -or $process.HasExited) { throw 'Receipt writer did not reach its byte-write boundary.' }
            Start-Sleep -Milliseconds 20
        }
        $visibleBeforeWrite = [IO.File]::Exists($receiptPath)
        $premature = if ($visibleBeforeWrite) { Read-P4kxDetachedReceipt $receiptPath }
        [IO.File]::WriteAllText($released,'release')
        if (-not $process.WaitForExit(2000) -or $process.ExitCode -ne 0) { throw 'The original receipt writer did not return.' }
        if ($visibleBeforeWrite -and $null -eq $premature) { throw 'The receipt filename was visible before its bytes were written.' }
        if ($visibleBeforeWrite) { throw 'A final receipt was visible before the original writer returned.' }
    } finally {
        try { if (-not $process.HasExited) { $process.Kill($false); [void]$process.WaitForExit(1000) } }
        finally { $process.Dispose() }
    }
    $original = Get-P4kxDetachedDigest $receiptPath
    $refused = $false
    try { Write-P4kxDetachedReceipt $receiptPath @{schema=1;observation='replacement'} }
    catch [IO.IOException] { $refused = $true }
    catch [System.Management.Automation.MethodInvocationException] {
        if ($_.Exception.InnerException -isnot [IO.IOException]) { throw }
        $refused = $true
    }
    if (-not $refused -or (Get-P4kxDetachedDigest $receiptPath) -cne $original -or
        (Read-P4kxDetachedReceipt $receiptPath).observation -cne 'original' -or
        @(Get-ChildItem -LiteralPath $task -Force -Filter '*.tmp').Count -ne 0) {
        throw 'Receipt publication overwrote the original or retained a temporary file after refusal.'
    }
}

Assert-DetachedReceiptPublication

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
