param(
    [Parameter(Mandatory)][ValidateSet('Start','Snapshot','Finish')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory,
    [ValidateRange(1,2)][int]$Sequence=1
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')

function Read-P4kxActiveControlCheckpoint {
    param($Request, $Handle, [int]$Number)
    $sequencePath = '{0:d2}' -f $Number
    $checkpoint = Join-Path $Request.directory "checkpoints/$sequencePath"
    $receipt = Read-P4kxDetachedReceipt (Join-Path $checkpoint 'checkpoint.json')
    $expectedWait = if ($Number -eq 1) { 0 } else { 75000 }
    if ($receipt.sequence -ne $Number -or $receipt.mode -cne 'Control' -or
        $receipt.request_sha256 -cne $Handle.request_sha256 -or $receipt.deadline_utc -cne $Request.deadline_utc -or
        $receipt.observer_deadline_utc -cne $Request.observer_deadline_utc -or $receipt.parent.lifetime_ended -ne $false -or
        $receipt.journal.present -ne $true -or $receipt.journal.complete -ne $true -or $receipt.journal.events -ne 2 -or
        $receipt.journal.active -cnotcontains 'fixture/wait' -or $receipt.journal.active -cnotcontains 'worker' -or
        $null -ne $receipt.journal.terminal -or $receipt.completed_success -ne $false -or
        $receipt.native_completion_unproven -ne $true -or $receipt.mutation_authority -ne $false) {
        throw 'Successive control checkpoint lacks the original active Control journal.'
    }
    foreach ($name in @('request.json','handle.json','phase-journal.jsonl')) {
        if ($receipt.files[$name].status -cne 'captured' -or
            $receipt.files[$name].sha256 -cne (Get-P4kxDetachedDigest (Join-Path $checkpoint $name))) {
            throw 'Successive control checkpoint has incomplete original evidence.'
        }
    }
    if ($receipt.files['request.json'].sha256 -cne $Handle.request_sha256 -or
        $receipt.files['handle.json'].sha256 -cne (Get-P4kxDetachedDigest (Join-Path $Request.directory 'handle.json'))) {
        throw 'Successive control checkpoint changed the original process binding.'
    }
    $returned = Read-P4kxDetachedReceipt (Join-Path $Request.directory "snapshots/$sequencePath/caller-result.json")
    if ($returned.role -cne 'native-msi-snapshot-return' -or $returned.sequence -ne $Number -or
        $returned.revision -cne $Request.revision -or $returned.run -cne $Request.run -or
        $returned.attempt -cne $Request.attempt -or $returned.target -cne $Request.target -or
        $returned.deadline_utc -cne $Request.deadline_utc -or $returned.requested_wait_ms -ne $expectedWait -or
        $returned.elapsed_ms -lt $expectedWait -or $returned.snapshot_returned -ne $true -or
        $returned.worker.started -ne $true -or $returned.worker.exited -ne $true -or $returned.worker.exit_code -ne 0 -or
        $returned.worker.pid -le 0 -or $returned.worker.pid -eq $Handle.pid -or $returned.completed_success -ne $false -or
        $returned.native_completion_unproven -ne $true -or $returned.mutation_authority -ne $false) {
        throw 'Successive control snapshot lacks its independent caller return.'
    }
    return $receipt
}

$root = [IO.Path]::GetFullPath($Directory)
$observer = Join-Path $root 'observer'
$original = Join-Path $root 'original diagnostic'
$invocation = Join-Path $root 'invocation.json'
$entry = Join-Path $PSScriptRoot 'invoke-msi-observer.ps1'
if ($Stage -ceq 'Start') {
    if ([IO.Path]::Exists($root) -or [string]::IsNullOrEmpty($env:GITHUB_ENV)) {
        throw 'Successive control requires a fresh task and its workflow environment file.'
    }
    New-Item -ItemType Directory -Path $root | Out-Null
    Write-P4kxDetachedReceipt $invocation @{
        Target=$env:P4KX_TARGET;Msi='observer-wait';FixtureDirectory=$PSScriptRoot;OutputDirectory=$original
    }
    $controlMilliseconds = 100000
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC = [DateTimeOffset]::UtcNow.AddMilliseconds($controlMilliseconds).ToString('o')
    "P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC" >> $env:GITHUB_ENV
    & $entry -Stage Start -Directory $observer -Invocation $invocation -Mode Control -ControlMilliseconds $controlMilliseconds
    return
}

$request = Read-P4kxDetachedReceipt (Join-Path $observer 'request.json')
if ($request.mode -cne 'Control' -or $request.invocation -cne $invocation -or $request.output_directory -cne $original) {
    throw 'Successive control requires the harmless original invocation.'
}
Assert-P4kxMsiObserverBinding $request $observer
$handle = Read-P4kxDetachedReceipt (Join-Path $observer 'handle.json')
if ($Stage -ceq 'Snapshot') {
    if ($Sequence -eq 2) { [void](Read-P4kxActiveControlCheckpoint $request $handle 1) }
    $marker = Join-Path $original 'observer-control-active.json'
    while (-not [IO.File]::Exists($marker)) {
        [void](Get-P4kxMsiRemainingBudget $request.observer_deadline_utc)
        if ((Get-P4kxMsiObserverParentState $request $handle).lifetime_ended) {
            throw 'Successive control parent ended before its active journal.'
        }
        Start-Sleep -Milliseconds 20
    }
    $waitMilliseconds = if ($Sequence -eq 1) { 0 } else { 75000 }
    $remaining = ((ConvertFrom-P4kxMsiBudgetTimestamp $request.observer_deadline_utc)-[DateTimeOffset]::UtcNow).TotalMilliseconds
    $budget = if ($remaining -gt $waitMilliseconds) { Get-P4kxMsiSnapshotBudget $request.deadline_utc }
    $admitted = $remaining -gt $waitMilliseconds -and $null -ne $budget -and $budget.capture_ms -gt $waitMilliseconds
    $budgetPath = Join-Path $root ('snapshot-budget-{0:d2}.json' -f $Sequence)
    Write-P4kxDetachedReceipt $budgetPath @{
        schema=1;role='successive-msi-control-admission';request_sha256=$handle.request_sha256;sequence=$Sequence;
        deadline_utc=$request.deadline_utc;observer_deadline_utc=$request.observer_deadline_utc;requested_wait_ms=$waitMilliseconds;
        remaining_ms=[long][Math]::Floor($remaining);budget=$budget;admitted=$admitted;native_completion_unproven=$true;mutation_authority=$false
    }
    if (-not $admitted) { throw 'Successive control is inconclusive because the full snapshot does not fit its original deadline.' }
    & $entry -Stage Snapshot -Directory $observer -Sequence $Sequence -WaitMilliseconds $waitMilliseconds
    [void](Read-P4kxActiveControlCheckpoint $request $handle $Sequence)
    return
}

$first = Read-P4kxActiveControlCheckpoint $request $handle 1
$second = Read-P4kxActiveControlCheckpoint $request $handle 2
if ($first.files['phase-journal.jsonl'].sha256 -cne $second.files['phase-journal.jsonl'].sha256) {
    throw 'Successive control snapshots did not preserve the same held original worker.'
}
[void](Get-P4kxMsiRemainingBudget $request.observer_deadline_utc)
Write-P4kxDetachedReceipt (Join-Path $original 'observer-control-release.json') @{schema=1;role='harmless-msi-observer-release'}
& $entry -Stage Finish -Directory $observer -WaitMilliseconds 0
$receipt = Read-P4kxDetachedReceipt (Join-Path $observer 'observer-result.json')
if ($receipt.mode -cne 'Control' -or $receipt.request_sha256 -cne $handle.request_sha256 -or
    $receipt.control_passed -ne $true -or $receipt.completed_success -ne $false -or
    $receipt.native_completion_unproven -ne $true -or $receipt.mutation_authority -ne $false) {
    throw 'Successive control did not record its original harmless completion.'
}
