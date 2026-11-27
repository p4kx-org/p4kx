param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')

function Assert-Delay([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Test-DelayExecutionMarkers {
    param([string]$Root, [hashtable]$Prototype)
    $directory = Join-Path $Root 'executed'
    [void][IO.Directory]::CreateDirectory($directory)
    $now = [DateTimeOffset]::UtcNow
    $request = $Prototype.Clone()
    $request.directory = $directory
    $request.deadline_utc = $now.AddSeconds(180).ToString('o')
    $request.observer_deadline_utc = $now.AddSeconds(130).ToString('o')
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC = $request.deadline_utc
    Write-P4kxDetachedReceipt (Join-Path $directory 'request.json') $request

    function Start-Sleep {
        param([int]$Milliseconds)
        Assert-Delay ($Milliseconds -eq 75000) 'The executed observation requests the original 75 seconds.'
    }
    $prefix = '[P4KX-MSI-OBSERVER]'
    $observed = [Collections.Generic.List[string]]::new()
    $failure = $null
    try {
        & (Join-Path $PSScriptRoot 'invoke-msi-observer.ps1') -Stage Delay -Directory $directory -Sequence 2 -WaitMilliseconds 75000 6>&1 |
            ForEach-Object {
                $message = $_.ToString()
                if ($message.StartsWith($prefix, [StringComparison]::Ordinal)) {
                    $event = $message.Substring($prefix.Length) | ConvertFrom-Json -AsHashtable -DateKind String
                    $observed.Add($event.stage + ':' + $event.status)
                }
            }
    } catch { $failure = $_.Exception.Message }
    $expected = @(
        'delayCaller:begin','delayBinding:begin','delayReadRequest:begin','delayReadRequest:end','delayBinding:end',
        'delayStartReceipt:begin','delayStartReceipt:end','delayWait:begin','delayWait:end',
        'delayResultReceipt:begin','delayResultReceipt:end','delayCaller:end'
    )
    Assert-Delay (($observed -join ',') -ceq ($expected -join ',')) 'Delay markers bracket each operation through failure and return.'
    Assert-Delay ($failure -ceq 'Checkpoint 2 delay ended early.') 'Markers cannot accept an early wait.'
    $result = Read-P4kxDetachedReceipt (Join-Path $directory 'delays/02/delay-result.json')
    Assert-Delay ($result.delay_complete -eq $false -and $result.native_completion_unproven -eq $true) 'The exercised early delay remains unproven.'
}

$root = [IO.Path]::GetFullPath($Directory)
$path = Join-Path $root 'delays/02'
if ([IO.Path]::Exists($root)) { throw 'Delay budget fixture directory must be new.' }
New-Item -ItemType Directory -Path $path -Force | Out-Null
$started = [DateTimeOffset]::UtcNow.AddSeconds(-101)
$request = @{
    schema=1;role='native-msi-observer-request';mode='Lifecycle';directory=$root;revision=$env:GITHUB_SHA
    run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET
    deadline_utc=$started.AddSeconds(180).ToString('o');observer_deadline_utc=$started.AddSeconds(130).ToString('o')
}
$saved = $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
try {
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC = $request.deadline_utc
    Write-P4kxDetachedReceipt (Join-Path $root 'request.json') $request
    $binding = Get-P4kxMsiCheckpoint2Binding $root
    $window = New-P4kxMsiCheckpoint2Window $binding $started
    $receipt = $binding + $window + @{
        schema=1;role='native-msi-snapshot-delay-start';sequence=2;started_utc=$started.ToString('o')
        requested_wait_ms=75000;delay_complete=$false
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    Write-P4kxDetachedReceipt (Join-Path $path 'delay-start.json') $receipt
    $result = $receipt.Clone()
    $result.role='native-msi-snapshot-delay-result'
    $result.delay_complete=$true
    $result.elapsed_ms=75000
    $result.finished_utc=$started.AddSeconds(75).ToString('o')
    $result.failure_type=$null
    Write-P4kxDetachedReceipt (Join-Path $path 'delay-result.json') $result
    $valid = Read-P4kxMsiCheckpoint2Delay $root $started.AddSeconds(80)
    $budget = Get-P4kxMsiCheckpoint2CaptureBudget $valid 100000 $started.AddSeconds(80)
    Assert-Delay ($budget.capture_ms -eq 20000 -and $budget.cleanup_ms -eq 4000) 'Delay and artifact transport must debit the original capture allowance.'
    $budget = Get-P4kxMsiCheckpoint2CaptureBudget $valid 3000 $started.AddSeconds(85)
    Assert-Delay ($budget.capture_ms -eq 3000) 'The scheduled observation cannot increase a previous admission.'
    $binding.observer_deadline_utc=$started.AddSeconds(93).ToString('o')
    $short = New-P4kxMsiCheckpoint2Window $binding $started
    $budget = Get-P4kxMsiCheckpoint2CaptureBudget $short 100000 $started.AddSeconds(80)
    Assert-Delay ($budget.capture_ms+$budget.cleanup_ms -eq 13000) 'Cleanup must fit within the shorter original observer deadline.'
    $expired=$false
    try { Get-P4kxMsiCheckpoint2CaptureBudget $valid 100000 $started.AddSeconds(100) | Out-Null } catch { $expired=$true }
    Assert-Delay $expired 'Exhausted sequence capture time must refuse observation.'
    function Invoke-P4kxPortalProcess { throw 'The expired fixture must not enter Portal capture.' }
    $refused=$false
    try { Invoke-P4kxMsiObserverSnapshot $root 2 0 -ScheduledDelay } catch { $refused=$true }
    $refusal = Read-P4kxDetachedReceipt (Join-Path $root 'snapshots/02/caller-result.json')
    Assert-Delay ($refused -and $refusal.scheduled_delay -eq $true -and $refusal.capture_admitted -eq $false -and
        $refusal.refusal -ceq 'remaining-budget' -and $refusal.snapshot_returned -eq $false -and
        $refusal.completed_success -eq $false -and $refusal.native_completion_unproven -eq $true) 'The refused remaining allowance must leave a durable observation-only receipt.'
    $mismatched=$false
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$started.AddSeconds(179).ToString('o')
    try { Read-P4kxMsiCheckpoint2Delay $root $started.AddSeconds(80) | Out-Null } catch { $mismatched=$true }
    Assert-Delay $mismatched 'A different deadline must refuse the original delay receipt.'
    Test-DelayExecutionMarkers $root $request
} finally {
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC = $saved
}
