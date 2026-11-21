param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
if (-not $IsWindows -or [IO.Path]::Exists($Directory)) { throw 'Snapshot boundary control requires a fresh private Windows directory.' }
$root=[IO.Path]::GetFullPath($Directory)
$scripts=Join-Path $root 'source/.github/scripts'
New-Item -ItemType Directory -Path $scripts | Out-Null
New-Item -ItemType Directory -Path (Join-Path $root 'source/.config') | Out-Null
$names=@(Get-P4kxMsiObserverSources | ForEach-Object name)
foreach ($name in $names) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $scripts $name) }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../../.config/platforms.json') -Destination (Join-Path $root 'source/.config/platforms.json')
$processPath=Join-Path $scripts 'msi-observer-process.ps1'
$text=[IO.File]::ReadAllText($processPath).Replace(([string][char]13+[char]10),[string][char]10)
$needle='function Get-P4kxMsiObserverParentState {'+[char]10+'    param($Request, $Handle)'
if (-not $text.Contains($needle)) { throw 'The actual observer state seam differs.' }
$replacement=@'
function Get-P4kxMsiObserverParentState {
    param($Request, $Handle)
    if ($env:P4KX_SNAPSHOT_FIXTURE_BLOCK -ceq 'owned-state') {
        Write-P4kxDetachedReceipt (Join-Path $Request.directory 'controlled-leaf.json') @{
            schema=1;role='harmless-snapshot-leaf';request_sha256=$Handle.request_sha256;pid=$PID;
            completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
        }
        Start-Sleep -Seconds 30
    }
'@
[IO.File]::WriteAllText($processPath,$text.Replace($needle,$replacement.TrimEnd()),[Text.UTF8Encoding]::new($false))
. (Join-Path $scripts 'msi-observer-snapshot-caller.ps1')
. (Join-Path $scripts 'msi-observer-evidence.ps1')
$saved=@{actions=$env:GITHUB_ACTIONS;runner=$env:RUNNER_ENVIRONMENT;tracking=$env:RUNNER_TRACKING_ID;target=$env:P4KX_TARGET;
    revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;deadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;
    block=$env:P4KX_SNAPSHOT_FIXTURE_BLOCK;workflow_env=$env:GITHUB_ENV}
$request=$null;$handle=$null
function Assert-Snapshot([bool]$Condition,[string]$Message) { if (-not $Condition) { throw $Message } }
try {
    $env:GITHUB_ACTIONS='true';$env:RUNNER_ENVIRONMENT='github-hosted'
    if ([string]::IsNullOrEmpty($env:RUNNER_TRACKING_ID)) { $env:RUNNER_TRACKING_ID='synthetic-snapshot-boundary' }
    $env:P4KX_TARGET=(Get-P4kxCurrentPlatform).target
    $env:GITHUB_SHA='harmless-source-copy';$env:GITHUB_RUN_ID='harmless-snapshot';$env:GITHUB_RUN_ATTEMPT='1'
    $env:GITHUB_ENV=Join-Path $root 'workflow-env'
    $controlMilliseconds=20000
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddMilliseconds($controlMilliseconds).ToString('o')
    $budget=Get-P4kxMsiSnapshotBudget ([DateTimeOffset]::UtcNow.AddMinutes(20).ToString('o'))
    Assert-Snapshot ($budget.capture_ms -eq 100000 -and $budget.cleanup_ms -eq 5000) 'Snapshot exceeded its existing runner-step reserve.'
    $budget=Get-P4kxMsiSnapshotBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC 2000
    Assert-Snapshot ($budget.capture_ms -eq 2000 -and $budget.cleanup_ms -gt 0 -and $budget.cleanup_ms -le 5000) 'Snapshot changed its caller capture or cleanup cap.'
    $budget=Get-P4kxMsiSnapshotBudget ([DateTimeOffset]::UtcNow.AddSeconds(1).ToString('o'))
    Assert-Snapshot ($budget.capture_ms -gt 0 -and $budget.capture_ms+$budget.cleanup_ms -le 1000) 'A short original deadline was increased.'
    $refused=$false
    try { Get-P4kxMsiSnapshotBudget ([DateTimeOffset]::UtcNow.AddSeconds(-1).ToString('o')) | Out-Null } catch { $refused=$true }
    Assert-Snapshot $refused 'An expired original deadline admitted a snapshot.'
    $output=Join-Path $root 'original'
    $observer=Join-Path $root 'observer'
    $invocation=Join-Path $root 'invocation.json'
    Write-P4kxDetachedReceipt $invocation @{Target=$env:P4KX_TARGET;Msi='observer-wait';FixtureDirectory=$scripts;OutputDirectory=$output}
    & (Join-Path $scripts 'invoke-msi-observer.ps1') -Stage Start -Directory $observer -Invocation $invocation -Mode Control -ControlMilliseconds $controlMilliseconds
    $request=Read-P4kxDetachedReceipt (Join-Path $observer 'request.json')
    $handle=Read-P4kxDetachedReceipt (Join-Path $observer 'handle.json')
    $startup=[Diagnostics.Stopwatch]::StartNew()
    while (-not [IO.File]::Exists((Join-Path $output 'observer-control-active.json'))) {
        Assert-Snapshot ($startup.ElapsedMilliseconds -lt 4000) 'The harmless original parent did not become active.'
        Start-Sleep -Milliseconds 20
    }
    $failed=$false;$timer=[Diagnostics.Stopwatch]::StartNew()
    $live=@{received=$false;original_alive=$false}
    $env:P4KX_SNAPSHOT_FIXTURE_BLOCK='owned-state'
    try {
        Invoke-P4kxMsiObserverSnapshot $observer 1 250 -MaximumMilliseconds 2000 6>&1 | ForEach-Object {
            $text=[string]$_
            if (-not $text.StartsWith('[P4KX-MSI-SNAPSHOT]',[StringComparison]::Ordinal)) { return }
            $marker=$text.Substring('[P4KX-MSI-SNAPSHOT]'.Length) | ConvertFrom-Json -AsHashtable
            if ($marker.stage -ceq 'worker.parentState' -and $marker.status -ceq 'begin') {
                $leafPath=Join-Path $observer 'controlled-leaf.json'
                $leafWait=[Diagnostics.Stopwatch]::StartNew()
                while (-not [IO.File]::Exists($leafPath) -and $leafWait.ElapsedMilliseconds -lt 500) { Start-Sleep -Milliseconds 10 }
                $liveLeaf=Read-P4kxDetachedReceipt $leafPath
                $ownedLeaf=[Diagnostics.Process]::GetProcessById($liveLeaf.pid)
                $original=[Diagnostics.Process]::GetProcessById($handle.pid)
                try {
                    Assert-Snapshot (-not $ownedLeaf.HasExited -and -not $original.HasExited) 'Live progress was replayed after the blocked leaf or original parent ended.'
                    $live.received=$true;$live.original_alive=$true
                } finally { $ownedLeaf.Dispose();$original.Dispose() }
            }
            Write-Host $text
        }
    } catch { $failed=$true }
    finally { $env:P4KX_SNAPSHOT_FIXTURE_BLOCK=$saved.block }
    $elapsed=$timer.ElapsedMilliseconds
    $capture=Join-Path $observer 'snapshots/01'
    $result=Read-P4kxDetachedReceipt (Join-Path $capture 'caller-result.json')
    $leaf=Read-P4kxDetachedReceipt (Join-Path $observer 'controlled-leaf.json')
    $stage=Read-P4kxDetachedReceipt (Join-Path $capture 'parentState-begin.json')
    Assert-Snapshot ($failed -and $elapsed -ge 1900 -and $elapsed -lt 7000 -and $result.snapshot_returned -eq $false -and
        $result.worker.started -eq $true -and $result.worker.exited -eq $true -and $result.worker.pid -eq $leaf.pid -and
        $result.worker.pid -ne $handle.pid -and $leaf.request_sha256 -ceq $handle.request_sha256 -and
        $stage.request_sha256 -ceq $handle.request_sha256 -and $stage.completed_success -eq $false -and
        $stage.native_completion_unproven -eq $true -and $stage.mutation_authority -eq $false -and
        -not [IO.File]::Exists((Join-Path $capture 'parentState-end.json')) -and
        -not [IO.File]::Exists((Join-Path $observer 'checkpoints/01/checkpoint.json')) -and
        -not [IO.File]::Exists($env:GITHUB_ENV)) 'Blocked snapshot lost its bounded failure, exact last stage or unproven verdict.'
    Assert-Snapshot ($live.received -and $live.original_alive) 'The blocked snapshot did not expose its exact stage live before worker return.'
    Assert-Snapshot (-not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) 'Snapshot watchdog signaled the independent original parent.'
    Invoke-P4kxMsiObserverSnapshot $observer 2 250
    $positive=Read-P4kxDetachedReceipt (Join-Path $observer 'snapshots/02/caller-result.json')
    $checkpoint=Read-P4kxDetachedReceipt (Join-Path $observer 'checkpoints/02/checkpoint.json')
    Assert-Snapshot ($positive.snapshot_returned -eq $true -and $positive.completed_success -eq $false -and
        $positive.native_completion_unproven -eq $true -and $checkpoint.request_sha256 -ceq $handle.request_sha256 -and
        $checkpoint.journal.events -eq 2 -and $checkpoint.journal.active.Count -eq 2) 'Unblocked snapshot failed to preserve the active original journal without a native verdict.'
    Write-P4kxDetachedReceipt (Join-Path $output 'observer-control-release.json') @{schema=1;role='harmless-msi-observer-release'}
    $closed=[Diagnostics.Stopwatch]::StartNew()
    while (-not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) {
        Assert-Snapshot ($closed.ElapsedMilliseconds -lt 4000) 'Harmless original parent did not end after release.'
        Start-Sleep -Milliseconds 20
    }
    $journal=Read-P4kxMsiPhaseJournal (Join-Path $output 'diagnostics/phase-journal.jsonl')
    Assert-Snapshot ($journal.events -eq 4 -and $journal.active.Count -eq 0 -and $journal.terminal -ceq 'end') 'Original harmless journal did not close after explicit release.'
    @{role='harmless-snapshot-budget-control';blocked_elapsed_ms=$elapsed;only_snapshot_stopped=$true;unblocked_capture_returned=$true;
        original_closed_after_release=$true;live_parent_state_begin_before_worker_return=$live.received;live_original_parent_alive=$live.original_alive;completed_success=$false;native_completion_unproven=$true;mutation_authority=$false} | ConvertTo-Json -Compress
} finally {
    $env:P4KX_SNAPSHOT_FIXTURE_BLOCK=$saved.block
    if ($null -ne $request -and $null -ne $handle -and -not (Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) {
        $owned=Open-P4kxDetachedOwnedProcess $handle $request -Role 'native-msi-observer-handle'
        try { $owned.Kill($false);[void]$owned.WaitForExit(4000) } finally { $owned.Dispose() }
    }
    $env:GITHUB_ACTIONS=$saved.actions;$env:RUNNER_ENVIRONMENT=$saved.runner;$env:RUNNER_TRACKING_ID=$saved.tracking;$env:P4KX_TARGET=$saved.target
    $env:GITHUB_SHA=$saved.revision;$env:GITHUB_RUN_ID=$saved.run;$env:GITHUB_RUN_ATTEMPT=$saved.attempt
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$saved.deadline;$env:GITHUB_ENV=$saved.workflow_env
}
