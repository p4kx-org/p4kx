param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'msi-observer-finish-wait-host.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-watchdog.ps1')

function Assert-P4kxFinishReceiptDigest([string]$Root) {
    $path = Join-Path $Root 'digest-fixture.json'
    $json = '{"schema":1}'
    $validAtLimit = $json.PadRight(16KB - 1) + [char]10
    foreach ($content in @('', $json, $validAtLimit)) {
        [IO.File]::WriteAllText($path, $content)
        if ((Get-P4kxDetachedReceiptDigest $path) -cne (Get-P4kxDetachedDigest $path)) {
            throw 'The bounded receipt digest changed the SHA-256 of the original bytes.'
        }
    }

    [IO.File]::AppendAllText($path, ' ')
    try {
        $null = Get-P4kxDetachedReceiptDigest $path
        throw 'An oversized Finish receipt was accepted for hashing.'
    } catch {
        if ($_.Exception.Message -cne 'Detached control receipt exceeds its metadata bound.') { throw }
    }
}

function Write-P4kxFinishLifecycleFixtureResult([string]$Path, [string]$Digest) {
    Write-P4kxDetachedReceipt $Path @{
        schema=1;role='native-msi-finish-lifecycle-control-response';request_sha256=$Digest
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
}

function New-P4kxFinishLifecycleFixture {
    param([string]$Root, [ValidateSet('Bound','Foreign','Stale','Hosted','LaunchReadFailure','Absent','Late','Watchdog','WatchdogFailure','AcrossLaunchers')][string]$Case, [string]$ForeignDigest, [switch]$Unpublished)

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
    if (-not $Unpublished) {
        Write-P4kxFinishLifecycleFixtureResult $resultPath $digest
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

function Assert-P4kxFinishLifecycleWait {
    param($Fixture, [string]$ResultDigest, [switch]$Hosted)
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
        [IO.Path]::Exists($Fixture.output) -or [IO.Path]::Exists((Join-Path $Fixture.observer 'observer-result.json'))) {
        throw 'The Finish wait lost its bound observation-only result.'
    }
    if ($Hosted) {
        if ($wait.Contains('direct_probe')) { throw 'Hosted Finish receipt observation launched a direct probe.' }
    } elseif ($probe.signal_contract -cne 'direct-process-only' -or $probe.pid -le 0 -or $probe.pid -eq $PID -or
        $probe.started -ne $true -or $probe.exited -ne $true -or $probe.exit_code -ne 0 -or $probe.timed_out -ne $false) {
        throw 'The direct predicate control lost its process-lifetime evidence.'
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

function Assert-P4kxHostedFinishNotices([string[]]$Lines, [string[]]$Phases, [string]$InterruptedPhase) {
    $observed = @($Lines | ForEach-Object {
        if ($_ -cmatch '^::notice title=Native MSI hosted Finish phase::phase=(\S+) status=(begin|end) completion=unproven$') {
            $Matches[1] + '.' + $Matches[2]
        }
    })
    $expected = @(foreach ($phase in $Phases) { "$phase.begin"; "$phase.end" })
    if ($InterruptedPhase) { $expected += "$InterruptedPhase.begin" }
    if (($observed -join ' ') -cne ($expected -join ' ')) {
        throw 'Hosted Finish observation notices are missing, out of order or claim native completion.'
    }
}

function Assert-P4kxHostedFinishWait($Fixture) {
    $diagnostics = Split-Path -Parent $Fixture.wait_path
    $wrongCutoff = $Fixture.collection_cutoff.AddSeconds(1).ToString('o')
    try {
        Invoke-P4kxMsiHostedFinishWait -Directory $Fixture.observer -CollectionCutoffUtc $wrongCutoff
        throw 'Hosted Finish wait accepted a different collection cutoff.'
    } catch {
        if ($_.Exception.Message -cne 'Hosted Finish wait requires the original Lifecycle collection cutoff.') { throw }
    }

    $lines = @(& {
        function Start-Process { throw 'Hosted receipt observation attempted to launch a process.' }
        function New-P4kxPackageProcess { throw 'Hosted receipt observation constructed an owned process.' }
        function Start-Sleep([int]$Milliseconds) {
            if ($Milliseconds -lt 1 -or $Milliseconds -gt 100 -or [IO.File]::Exists($Fixture.result_path)) {
                throw 'Hosted Finish receipt polling exceeded its interval or repeated a completed read.'
            }
            Write-P4kxFinishLifecycleFixtureResult $Fixture.result_path $Fixture.digest
        }
        Invoke-P4kxMsiHostedFinishWait -Directory $Fixture.observer -CollectionCutoffUtc $Fixture.collection_cutoff.ToString('o')
    } 6>&1 | ForEach-Object { $_.ToString() })
    Assert-P4kxHostedFinishNotices $lines @('context-binding','launch-read','receipt-poll','receipt-digest','wait-receipt')
    $digest = Get-P4kxDetachedDigest $Fixture.result_path
    Assert-P4kxFinishLifecycleWait $Fixture $digest -Hosted
    foreach ($stage in 'finishWaitBinding','finishWaitPoll','finishWaitDigest','finishWaitReceipt') {
        foreach ($status in 'begin','end') {
            $progress = Read-P4kxDetachedReceipt (Join-Path $diagnostics "$stage-$status.json")
            if ($progress.stage -cne $stage -or $progress.status -cne $status -or
                $progress.request_sha256 -cne $Fixture.digest -or $progress.native_completion_unproven -ne $true -or
                $progress.completed_success -ne $false -or $progress.mutation_authority -ne $false) {
                throw 'Hosted Finish lost durable stage and request evidence.'
            }
        }
    }
    [IO.File]::AppendAllText($Fixture.result_path, ' ')
    try {
        Read-P4kxMsiSupervisedFinish $Fixture.observer 6>$null
        throw 'Changed terminal bytes passed the original Finish validation.'
    } catch {
        if ($_.Exception.Message -cne 'Finish requires the exact terminal receipt from its bounded wait.') { throw }
    }
}

function Assert-P4kxHostedFinishLaunchReadFailure($Fixture) {
    $failure = @{message=$null}
    $lines = @(& {
        function Read-P4kxMsiFinishLaunch { throw 'Injected hosted Finish launch read failure.' }
        try {
            Invoke-P4kxMsiHostedFinishWait -Directory $Fixture.observer -CollectionCutoffUtc $Fixture.collection_cutoff.ToString('o')
        } catch { $failure.message = $_.Exception.Message }
    } 6>&1 | ForEach-Object { $_.ToString() })
    Assert-P4kxHostedFinishNotices $lines @('context-binding') -InterruptedPhase 'launch-read'
    if ($failure.message -cne 'Injected hosted Finish launch read failure.' -or
        [IO.File]::Exists($Fixture.wait_path) -or [IO.Path]::Exists($Fixture.output)) {
        throw 'Hosted Finish changed the read failure or wrote a terminal observation.'
    }
}

function Assert-P4kxHostedFinishWaitRefused($Fixture, [DateTimeOffset]$Cutoff) {
    $context = Get-P4kxMsiFinishContext $Fixture.observer
    $launch = Read-P4kxMsiFinishLaunch $context
    $context.collection_cutoff = $Cutoff
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $failure = @{message=$null}
    $lines = @(& {
        try { Wait-P4kxMsiFinishReceipt $context $launch -Hosted }
        catch { $failure.message = $_.Exception.Message }
    } 6>&1 | ForEach-Object { $_.ToString() })
    if ($failure.message -cne 'The bound Finish supervisor produced no result within the original allowance.') {
        throw 'Missing or late terminal bytes passed the hosted wait or emitted a different error.'
    }
    Assert-P4kxHostedFinishNotices $lines @('receipt-poll','wait-receipt')
    $wait = Read-P4kxDetachedReceipt $Fixture.wait_path
    if ($clock.ElapsedMilliseconds -gt 5000 -or $wait.schema -ne 1 -or $wait.role -cne 'native-msi-finish-receipt-wait' -or
        $wait.deadline_utc -cne $Cutoff.ToString('o') -or $wait.request_sha256 -cne $Fixture.digest -or
        $wait.supervisor_pid -ne $launch.supervisor_pid -or $wait.supervisor_started_utc_ticks -ne $launch.supervisor_started_utc_ticks -or
        $null -ne $wait.result_sha256 -or $wait.Contains('direct_probe') -or $wait.mutation_authority -ne $false) {
        throw 'The hosted wait did not preserve bounded negative observation evidence.'
    }
}

function Assert-P4kxHostedFinishWatchdogBinding($Fixture, [switch]$WrongCutoff) {
    $cutoff = if ($WrongCutoff) { $Fixture.collection_cutoff.AddSeconds(1) } else { $Fixture.collection_cutoff }
    $code = Invoke-P4kxMsiHostedFinishWatchdog -Directory $Fixture.observer -CollectionCutoffUtc $cutoff.ToString('o')
    $diagnostics = Split-Path -Parent $Fixture.wait_path
    if ($WrongCutoff) {
        $stderr = [IO.File]::ReadAllText((Join-Path $diagnostics 'host-stderr.log'))
        if ($code -ne 1 -or $stderr -cnotmatch 'Hosted Finish wait requires the original Lifecycle collection cutoff.' -or
            [IO.File]::Exists($Fixture.wait_path)) { throw 'The native watchdog lost the bound child cutoff rejection.' }
    } else {
        $stdout = [IO.File]::ReadAllText((Join-Path $diagnostics 'host-stdout.log'))
        if ($code -ne 0 -or $stdout -cnotmatch 'The bound terminal receipt was observed. Native completion is unproven.') {
            throw 'The native watchdog did not preserve the successful observation-only child exit.'
        }
        Assert-P4kxFinishLifecycleWait $Fixture (Get-P4kxDetachedDigest $Fixture.result_path) -Hosted
    }
}

function Assert-P4kxHostedFinishAcrossLaunchers($Fixture) {
    $PSNativeCommandUseErrorActionPreference = $false
    $observer = Join-Path $PSScriptRoot 'invoke-msi-finish-observation.ps1'
    $shellArguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$observer)
    $cutoff = $Fixture.collection_cutoff.ToString('o')
    $binding = @('-Directory',$Fixture.observer,'-CollectionCutoffUtc',$cutoff)
    $scriptBinding = @{Directory=$Fixture.observer;CollectionCutoffUtc=$cutoff}
    $digest = & ([Environment]::ProcessPath) @shellArguments -Stage Start @binding
    if ($LASTEXITCODE -ne 0 -or $digest -cnotmatch '^[0-9a-f]{64}$') {
        throw 'The first Finish shell did not exit with a single original readiness digest.'
    }
    $directory = Split-Path -Parent $Fixture.wait_path
    $readyPath = Join-Path $directory 'waiter-ready.json'
    $exitPath = Join-Path $directory 'waiter-exit.json'
    $ready = Read-P4kxMsiHostedFinishReady $readyPath $Fixture.collection_cutoff $digest
    $worker = Get-Process -Id $ready.waiter_pid
    try {
        if ($worker.StartTime.ToUniversalTime().Ticks -ne $ready.waiter_started_utc_ticks -or
            $ready.request_sha256 -cne $Fixture.digest -or
            $ready.launch_sha256 -cne (Get-P4kxDetachedReceiptDigest (Join-Path $directory 'launch.json')) -or
            [IO.File]::Exists($exitPath) -or [IO.File]::Exists($Fixture.wait_path)) {
            throw 'The observed waiter did not stay live after its launcher, or readiness claimed completion.'
        }
    } finally { $worker.Dispose() }
    try {
        & $observer -Stage Wait @scriptBinding -ReadyDigest ('b'*64)
        throw 'A foreign readiness digest was accepted.'
    } catch {
        if ($_.Exception.Message -cne 'Finish readiness bytes differ from the original launch stage.') { throw }
    } finally {
        Write-P4kxFinishLifecycleFixtureResult $Fixture.result_path $Fixture.digest
    }
    & ([Environment]::ProcessPath) @shellArguments -Stage Wait @binding -ReadyDigest $digest
    if ($LASTEXITCODE -ne 0) { throw 'The second Finish shell rejected the timely bound child exit.' }
    Assert-P4kxFinishLifecycleWait $Fixture (Get-P4kxDetachedReceiptDigest $Fixture.result_path) -Hosted
    $expired = [DateTimeOffset]::UtcNow.AddSeconds(-1).ToString('o')
    try {
        & $observer -Stage Wait -Directory $Fixture.observer -CollectionCutoffUtc $expired -ReadyDigest $digest
        throw 'An expired original Finish wait was accepted.'
    } catch {
        if ($_.Exception.Message -cne 'Native MSI diagnostic deadline expired or exceeds its job budget.') { throw }
    }
    [IO.File]::AppendAllText($Fixture.wait_path, [char]10)
    try {
        & $observer -Stage Wait @scriptBinding -ReadyDigest $digest
        throw 'Changed wait receipt bytes were accepted.'
    } catch {
        if ($_.Exception.Message -cne 'The original Finish waiter exit proof is late, foreign or unsuccessful.') { throw }
    }
    try {
        & $observer -Stage Start @scriptBinding
        throw 'A duplicate Finish waiter launch was accepted.'
    } catch {
        if ($_.Exception.Message -cne 'A Finish waiter was already admitted for this launch.') { throw }
    }
}

$root = [IO.Path]::GetFullPath($Directory)
if (-not $IsWindows -or [IO.Path]::Exists($root)) { throw 'The Lifecycle Finish control requires a fresh native Windows directory.' }
$null = New-Item -ItemType Directory -Path $root
Assert-P4kxFinishReceiptDigest $root
$budget = New-P4kxMsiStepBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
$savedMinutes = $env:P4KX_MSI_STEP_MINUTES
$savedComputed = $env:P4KX_MSI_STEP_COMPUTED_AT_UTC
$savedActions = $env:GITHUB_ACTIONS
$savedRunner = $env:RUNNER_ENVIRONMENT
try {
    $env:GITHUB_ACTIONS = 'true'
    $env:RUNNER_ENVIRONMENT = 'github-hosted'
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
    Assert-P4kxHostedFinishWait (New-P4kxFinishLifecycleFixture $root Hosted -Unpublished)
    Assert-P4kxHostedFinishLaunchReadFailure (New-P4kxFinishLifecycleFixture $root LaunchReadFailure -Unpublished)
    Assert-P4kxHostedFinishWaitRefused (New-P4kxFinishLifecycleFixture $root Absent -Unpublished) ([DateTimeOffset]::UtcNow.AddMilliseconds(150))
    Assert-P4kxHostedFinishWaitRefused (New-P4kxFinishLifecycleFixture $root Late) ([DateTimeOffset]::UtcNow.AddSeconds(-1))
    Assert-P4kxHostedFinishWatchdogBinding (New-P4kxFinishLifecycleFixture $root Watchdog)
    Assert-P4kxHostedFinishWatchdogBinding (New-P4kxFinishLifecycleFixture $root WatchdogFailure) -WrongCutoff
    Assert-P4kxHostedFinishAcrossLaunchers (New-P4kxFinishLifecycleFixture $root AcrossLaunchers -Unpublished)
    'PASS: Lifecycle WaitFinish verified its bound receipt, refused foreign and stale sources, retained missing or late observations, and exercised the native watchdog child.'
} finally {
    $env:P4KX_MSI_STEP_MINUTES = $savedMinutes
    $env:P4KX_MSI_STEP_COMPUTED_AT_UTC = $savedComputed
    $env:GITHUB_ACTIONS = $savedActions
    $env:RUNNER_ENVIRONMENT = $savedRunner
}
