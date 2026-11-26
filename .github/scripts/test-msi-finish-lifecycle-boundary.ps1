param([Parameter(Mandatory)][string]$Directory, [switch]$HoldWorker)
$ErrorActionPreference = 'Stop'

if ($HoldWorker) {
    [IO.File]::WriteAllText((Join-Path $Directory 'worker-ready'), [string]$PID)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.ElapsedMilliseconds -lt 20000 -and -not [IO.File]::Exists((Join-Path $Directory 'worker-release'))) {
        Start-Sleep -Milliseconds 25
    }
    exit 0
}
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-wait-host.ps1')

function New-P4kxFinishLifecycleFixture {
    param([string]$Root, [ValidateSet('Bound','Foreign','Stale','Hosted')][string]$Case, [string]$ForeignDigest)

    $caseRoot = [IO.Directory]::CreateDirectory((Join-Path $Root $Case)).FullName
    $invocation = Join-Path $caseRoot 'invocation.json'
    $output = Join-Path $caseRoot 'diagnostics'
    Write-P4kxDetachedReceipt $invocation @{
        Target=$env:P4KX_TARGET;Msi=(Join-Path $caseRoot 'nonexistent.msi');OutputDirectory=$output
    }
    $observer = Join-Path $caseRoot 'observer with spaces'
    $request = New-P4kxMsiObserverRequest $observer $invocation Lifecycle 0
    $requestPath = Join-Path $observer 'request.json'
    Write-P4kxDetachedReceipt $requestPath $request
    $digest = Get-P4kxDetachedDigest $requestPath
    $started = [DateTimeOffset]::UtcNow.UtcTicks
    Write-P4kxDetachedReceipt (Join-Path $observer 'handle.json') @{
        schema=1;role='native-msi-observer-handle';pid=$PID;started_utc_ticks=$started;request_sha256=$digest
    }
    $cutoff = Get-P4kxMsiFinishCutoff $request
    $finish = [IO.Directory]::CreateDirectory((Join-Path $observer 'finish-supervision')).FullName
    $launch = @{
        schema=1;role='native-msi-finish-supervisor-launch';request_sha256=$digest
        supervisor_pid=$PID;supervisor_started_utc_ticks=$started;cutoff_utc=$cutoff.ToString('o')
        mutation_authority=$false
    }
    if ($Case -ceq 'Foreign') { $launch.request_sha256 = $ForeignDigest }
    if ($Case -ceq 'Stale') { $launch.cutoff_utc = $cutoff.AddSeconds(-1).ToString('o') }
    Write-P4kxDetachedReceipt (Join-Path $finish 'launch.json') $launch
    $resultPath = Join-Path $finish 'result.json'
    Write-P4kxDetachedReceipt $resultPath @{
        schema=1;role='native-msi-finish-lifecycle-control-response';request_sha256=$digest
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    return @{
        observer=$observer;output=$output;request=$request;digest=$digest;launch=$launch
        collection_cutoff=$cutoff.AddSeconds(3);result_path=$resultPath
        wait_path=(Join-Path $finish 'wait-result.json')
    }
}

function Read-P4kxFinishLifecycleObservation([string[]]$Lines) {
    foreach ($line in $Lines) {
        if ($line -cmatch '^\[P4KX-MSI-OBSERVER\](.+)$') {
            $Matches[1] | ConvertFrom-Json -AsHashtable -DateKind String
        }
    }
}

function Assert-P4kxFinishLifecycleObservation($Fixture, [string[]]$Lines) {
    $records = @(Read-P4kxFinishLifecycleObservation $Lines)
    $expected = @('finishWaitBinding.begin','finishWaitBinding.end','finishWaitPoll.begin','finishWaitPoll.end',
        'finishWaitDigest.begin','finishWaitDigest.end','finishWaitReceipt.begin','finishWaitReceipt.end')
    $observed = @($records | ForEach-Object { $_.stage + '.' + $_.status })
    if (($observed -join ' ') -cne ($expected -join ' ')) { throw 'Original Finish stage observations are missing or out of order.' }
    for ($index = 0; $index -lt $records.Count; $index++) {
        $record = $records[$index]
        $digest = if ($index -gt 0) { $Fixture.digest } else { '' }
        if ($record.schema -ne 1 -or $record.role -cne 'native-msi-observer-progress' -or
            $record.revision -cne $Fixture.request.revision -or $record.run -cne $Fixture.request.run -or
            $record.attempt -cne $Fixture.request.attempt -or $record.target -cne $Fixture.request.target -or
            $record.request_sha256 -cne $digest -or $record.binding_unproven -ne $true -or
            $record.completed_success -ne $false -or $record.native_completion_unproven -ne $true -or
            $record.mutation_authority -ne $false) {
            throw 'Finish progress lost its original observation-only identity.'
        }
    }
    $notices = @($Lines | ForEach-Object {
        if ($_ -cmatch '^::notice title=Native MSI Finish observation::phase=(\S+) status=(begin|end) completion=unproven$') {
            $Matches[1] + '.' + $Matches[2]
        }
    })
    $expected = foreach ($phase in @('predicate-setup','predicate-start','predicate-wait','predicate-cleanup','receipt-digest','wait-receipt')) {
        $phase + '.begin'
        $phase + '.end'
    }
    if (($notices -join ' ') -cne ($expected -join ' ')) { throw 'Original Finish predicate notices are missing or out of order.' }
}

function Assert-P4kxFinishLifecycleWait($Fixture, [string]$ResultDigest) {
    $wait = Read-P4kxDetachedReceipt $Fixture.wait_path
    $probe = $wait.direct_probe
    $observed = ConvertFrom-P4kxMsiBudgetTimestamp $wait.observed_utc
    $started = [DateTimeOffset]::new([long]$Fixture.launch.supervisor_started_utc_ticks, [TimeSpan]::Zero)
    if ($wait.schema -ne 1 -or $wait.role -cne 'native-msi-finish-receipt-wait' -or
        $wait.request_sha256 -cne $Fixture.digest -or $wait.supervisor_pid -ne $Fixture.launch.supervisor_pid -or
        $wait.supervisor_started_utc_ticks -ne $Fixture.launch.supervisor_started_utc_ticks -or
        $wait.deadline_utc -cne $Fixture.collection_cutoff.ToString('o') -or
        $wait.result_sha256 -cne $ResultDigest -or (Get-P4kxDetachedDigest $Fixture.result_path) -cne $ResultDigest -or
        $observed -lt $started -or $observed -gt $Fixture.collection_cutoff -or $wait.mutation_authority -ne $false -or
        $probe.signal_contract -cne 'direct-process-only' -or $probe.pid -le 0 -or $probe.pid -eq $PID -or
        $probe.started -ne $true -or $probe.exited -ne $true -or $probe.exit_code -ne 0 -or $probe.timed_out -ne $false -or
        [IO.Path]::Exists($Fixture.output) -or [IO.Path]::Exists((Join-Path $Fixture.observer 'observer-result.json'))) {
        throw 'The actual Finish wait lost its bound observation-only result.'
    }
}

function Assert-P4kxFinishLaunchRefused($Fixture, [string]$Entrypoint) {
    $lines = [Collections.Generic.List[string]]::new()
    $failure = $null
    try {
        & $Entrypoint -Stage WaitFinish -Directory $Fixture.observer 6>&1 | ForEach-Object { $lines.Add($_.ToString()) }
    } catch { $failure = $_.Exception.Message }
    $records = @(Read-P4kxFinishLifecycleObservation $lines.ToArray())
    if ($failure -cne 'Finish supervisor launch differs from the original observer.' -or
        $records.Count -ne 1 -or $records[0].stage -cne 'finishWaitBinding' -or $records[0].status -cne 'begin' -or
        [IO.Path]::Exists($Fixture.wait_path) -or [IO.Path]::Exists($Fixture.output)) {
        throw 'A foreign or stale Finish launch reached the receipt wait.'
    }
}

function Assert-P4kxFinishWaitHostBinding($Record, [DateTimeOffset]$Cutoff) {
    $started = ConvertFrom-P4kxMsiBudgetTimestamp $Record.started_utc
    $ended = ConvertFrom-P4kxMsiBudgetTimestamp $Record.ended_utc
    if ($Record.schema -ne 1 -or $Record.role -cne 'native-msi-finish-wait-host' -or
        $Record.revision -cne $env:GITHUB_SHA -or $Record.run -cne $env:GITHUB_RUN_ID -or
        $Record.attempt -cne $env:GITHUB_RUN_ATTEMPT -or $Record.target -cne $env:P4KX_TARGET -or
        $Record.pid -ne $PID -or $Record.child_pid -le 0 -or $Record.child_pid -eq $PID -or
        $Record.cutoff_utc -cne $Cutoff.ToString('o') -or $started -ge $Cutoff -or $ended -lt $started -or
        $Record.child_exited -ne $true -or $Record.native_completion_unproven -ne $true -or
        $Record.mutation_authority -ne $false) {
        throw 'The hosted Finish wait record lost its source, cutoff or owned-child observation.'
    }
}

function Assert-P4kxHostedFinishWait($Fixture) {
    $digest = Get-P4kxDetachedDigest $Fixture.result_path
    $cutoff = $Fixture.collection_cutoff
    Invoke-P4kxMsiHostedFinishWait -Directory $Fixture.observer -CollectionCutoffUtc $cutoff.ToString('o') 6>$null
    $diagnostics = Split-Path -Parent $Fixture.wait_path
    $record = Read-P4kxDetachedReceipt (Join-Path $diagnostics 'wait-host-result.json')
    $hostCutoff = $cutoff.AddSeconds(1)
    Assert-P4kxFinishWaitHostBinding $record $hostCutoff
    if ($record.timed_out -ne $false -or $record.exit_code -ne 0 -or
        (ConvertFrom-P4kxMsiBudgetTimestamp $record.ended_utc) -gt $hostCutoff) {
        throw 'The hosted Finish wait did not preserve a timely successful observation.'
    }
    Assert-P4kxFinishLifecycleObservation $Fixture ([IO.File]::ReadAllLines((Join-Path $diagnostics 'wait-host-stdout.log')))
    Assert-P4kxFinishLifecycleWait $Fixture $digest
}

function Get-P4kxFinishHoldArguments([string]$Script, [string]$Directory) {
    @('-NoLogo','-NoProfile','-NonInteractive','-File',('"' + $Script + '"'),
        '-Directory',('"' + $Directory + '"'),'-HoldWorker')
}

function Wait-P4kxOriginalControlWorker($Process, [string]$ReadyPath) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while (-not [IO.File]::Exists($ReadyPath)) {
        if ($Process.HasExited -or $clock.ElapsedMilliseconds -ge 6000) {
            throw 'The independent harmless worker did not reach its hold.'
        }
        Start-Sleep -Milliseconds 25
    }
    if ($Process.HasExited -or [IO.File]::ReadAllText($ReadyPath) -cne [string]$Process.Id) {
        throw 'The independent harmless worker identity differs.'
    }
}

function Assert-P4kxFinishWaitHostTimeout([string]$Root, [string]$Script) {
    $original = [IO.Directory]::CreateDirectory((Join-Path $Root 'independent worker')).FullName
    $timeout = [IO.Directory]::CreateDirectory((Join-Path $Root 'uncooperative wait')).FullName
    $workerArguments = Get-P4kxFinishHoldArguments $Script $original
    $worker = Start-Process -FilePath ([Environment]::ProcessPath) -ArgumentList $workerArguments -WindowStyle Hidden -PassThru
    try {
        Wait-P4kxOriginalControlWorker $worker (Join-Path $original 'worker-ready')
        $arguments = Get-P4kxFinishHoldArguments $Script $timeout
        $cutoff = [DateTimeOffset]::UtcNow.AddSeconds(4)
        $failure = $null
        try {
            $null = Invoke-P4kxMsiFinishWaitProcess -Arguments $arguments -Directory $timeout -Cutoff $cutoff 6>$null
        } catch { $failure = $_.Exception.Message }
        $record = Read-P4kxDetachedReceipt (Join-Path $timeout 'wait-host-result.json')
        Assert-P4kxFinishWaitHostBinding $record $cutoff
        $readyPid = [IO.File]::ReadAllText((Join-Path $timeout 'worker-ready'))
        if ($failure -cne 'The owned MSI Finish wait exceeded the original collection cutoff.' -or
            $record.timed_out -ne $true -or $null -eq $record.exit_code -or $record.exit_code -eq 0 -or
            (ConvertFrom-P4kxMsiBudgetTimestamp $record.ended_utc) -lt $cutoff -or
            $readyPid -cne [string]$record.child_pid -or $record.child_pid -eq $worker.Id -or $worker.HasExited -or
            [IO.Path]::Exists((Join-Path $timeout 'wait-result.json'))) {
            throw 'The expired wait did not stop only its owned child and retain a failure observation.'
        }
        $child = Get-Process -Id $record.child_pid -ErrorAction SilentlyContinue
        try {
            if ($null -ne $child -and -not $child.HasExited) { throw 'The owned wait child is still alive after its recorded exit.' }
        } finally {
            if ($null -ne $child) { $child.Dispose() }
        }
    } finally {
        try {
            [IO.File]::WriteAllText((Join-Path $original 'worker-release'), '')
            if (-not $worker.WaitForExit(3000)) {
                $worker.Kill($false)
                if (-not $worker.WaitForExit(1000)) { throw 'The owned harmless fixture worker did not stop.' }
            }
        } finally { $worker.Dispose() }
    }
}

$root = [IO.Path]::GetFullPath($Directory)
if (-not $IsWindows -or [IO.Path]::Exists($root)) { throw 'The Lifecycle Finish control requires a fresh native Windows directory.' }
$null = New-Item -ItemType Directory -Path $root
$budget = New-P4kxMsiStepBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
$savedMinutes = $env:P4KX_MSI_STEP_MINUTES
$savedComputed = $env:P4KX_MSI_STEP_COMPUTED_AT_UTC
try {
    $env:P4KX_MSI_STEP_MINUTES = [string]$budget.minutes
    $env:P4KX_MSI_STEP_COMPUTED_AT_UTC = $budget.computed_at_utc
    $entry = Join-Path $PSScriptRoot 'invoke-msi-observer.ps1'
    $bound = New-P4kxFinishLifecycleFixture $root Bound
    $digest = Get-P4kxDetachedDigest $bound.result_path
    $lines = @(& $entry -Stage WaitFinish -Directory $bound.observer 6>&1 | ForEach-Object { $_.ToString() })
    Assert-P4kxFinishLifecycleObservation $bound $lines
    Assert-P4kxFinishLifecycleWait $bound $digest
    Assert-P4kxFinishLaunchRefused (New-P4kxFinishLifecycleFixture $root Foreign -ForeignDigest $bound.digest) $entry
    Assert-P4kxFinishLaunchRefused (New-P4kxFinishLifecycleFixture $root Stale) $entry
    Assert-P4kxHostedFinishWait (New-P4kxFinishLifecycleFixture $root Hosted)
    Assert-P4kxFinishWaitHostTimeout $root $PSCommandPath
    'PASS: Lifecycle WaitFinish verified bound observations, refused foreign and stale launches, and timed out only its owned wait child.'
} finally {
    $env:P4KX_MSI_STEP_MINUTES = $savedMinutes
    $env:P4KX_MSI_STEP_COMPUTED_AT_UTC = $savedComputed
}
