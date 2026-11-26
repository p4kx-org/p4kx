param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Assert-P4kxWaitProgress([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Read-P4kxFinishWaitNotice([string]$Console) {
    $prefix = '::notice title=Native MSI Finish observation::'
    foreach ($line in $Console.Split([Environment]::NewLine)) {
        if ($line.StartsWith($prefix,[StringComparison]::Ordinal)) { $line.Substring($prefix.Length) }
    }
}

class P4kxFinishTestCaller {
    [int]$Id
    [bool]$HasExited
    [int]$ExitCode
    [string]$Failure
    [bool]$Disposed

    [bool] WaitForExit([int]$Milliseconds) { return $this.HasExited }
    [void] Kill([bool]$EntireProcessTree) {
        if ($EntireProcessTree -or $this.Failure -ceq 'kill') { throw 'synthetic owned cleanup failure' }
        $this.HasExited = $true
    }
    [void] Dispose() {
        $this.Disposed = $true
        if ($this.Failure -ceq 'dispose') { throw 'synthetic owned disposal failure' }
    }
}

function Assert-P4kxFinishSupervisorNegativeReceipt([string]$Root) {
    $scriptDirectory = [IO.Directory]::CreateDirectory((Join-Path $Root 'scripts')).FullName
    $supervisorName = 'msi-observer-finish-supervisor.ps1'
    $entry = Join-Path $scriptDirectory $supervisorName
    [IO.File]::Copy((Join-Path $PSScriptRoot $supervisorName), $entry)
    [IO.File]::WriteAllText((Join-Path $scriptDirectory 'msi-observer-finish-control.ps1'), '')

    function Get-P4kxMsiFinishContext([string]$Directory) { return $fixtureContext }
    function Start-P4kxMsiFinishProcess { return $fixtureCaller }
    function Get-P4kxDetachedDigest([string]$Path) {
        if ($failDigest -and $Path -ceq $callerPath) { throw 'synthetic caller digest failure' }
        return (Get-FileHash -LiteralPath $Path).Hash.ToLowerInvariant()
    }

    foreach ($case in @(
        @{name='kill';exit=$null;reason='synthetic owned cleanup failure';refusal='The bound Finish caller did not complete successfully within the original allowance.'},
        @{name='dispose';exit=1603;reason='synthetic owned disposal failure';refusal='The bound Finish caller did not complete successfully within the original allowance.'},
        @{name='digest';exit=0;reason='synthetic caller digest failure';refusal='The original Finish caller receipt differs.'}
    )) {
        $path = [IO.Directory]::CreateDirectory((Join-Path $Root $case.name)).FullName
        $supervision = [IO.Directory]::CreateDirectory((Join-Path $path 'finish-supervision')).FullName
        $callerDirectory = [IO.Directory]::CreateDirectory((Join-Path $path 'snapshots/09')).FullName
        $callerPath = Join-Path $callerDirectory 'caller-result.json'
        Write-P4kxDetachedReceipt $callerPath @{fixture='candidate'}
        $cutoff = [DateTimeOffset]::UtcNow.AddSeconds(10)
        $fixtureContext = @{
            root=$path;directory=$supervision;digest=('b'*64);handle=@{pid=($PID+2)}
            cutoff=$cutoff;collection_cutoff=$cutoff.AddSeconds(3)
        }
        $fixtureCaller = [P4kxFinishTestCaller]::new()
        $fixtureCaller.Id = $PID + 1
        $fixtureCaller.Failure = $case.name
        $fixtureCaller.HasExited = $null -ne $case.exit
        $fixtureCaller.ExitCode = [int]$case.exit
        $failDigest = $case.name -ceq 'digest'
        $failure = $null
        try { & $entry -Directory $path }
        catch { $failure = $_.Exception.Message }
        Assert-P4kxWaitProgress ($null -ne $failure -and $failure.Contains($case.reason) -and $fixtureCaller.Disposed) 'Supervisor did not propagate the fixture failure and dispose the owned caller.'

        $terminal = Join-Path $supervision 'result.json'
        $result = Read-P4kxDetachedReceipt $terminal
        Assert-P4kxWaitProgress ($result.schema -eq 1 -and $result.role -ceq 'native-msi-finish-supervisor-result' -and
            $result.request_sha256 -ceq $fixtureContext.digest -and $result.supervisor_pid -eq $PID -and
            $result.caller_pid -eq $fixtureCaller.Id -and $result.caller_exited -eq ($null -ne $case.exit) -and
            $case.exit -eq $result.caller_exit_code -and $result.timed_out -eq ($null -eq $case.exit) -and
            $null -eq $result.caller_result_sha256 -and $result.mutation_authority -eq $false) 'Supervisor did not persist the bound negative result and the actual exit observation.'

        $failDigest = $false
        Write-P4kxDetachedReceipt (Join-Path $supervision 'launch.json') @{
            schema=1;role='native-msi-finish-supervisor-launch';request_sha256=$fixtureContext.digest
            supervisor_pid=$PID;supervisor_started_utc_ticks=$result.supervisor_started_utc_ticks
            cutoff_utc=$cutoff.ToString('o');mutation_authority=$false
        }
        Write-P4kxDetachedReceipt (Join-Path $supervision 'wait-result.json') @{
            schema=1;role='native-msi-finish-receipt-wait';request_sha256=$fixtureContext.digest
            supervisor_pid=$PID;supervisor_started_utc_ticks=$result.supervisor_started_utc_ticks
            deadline_utc=$fixtureContext.collection_cutoff.ToString('o');observed_utc=[DateTimeOffset]::UtcNow.ToString('o')
            result_sha256=(Get-P4kxDetachedDigest $terminal);mutation_authority=$false
        }
        $refusal = $null
        try { $null = Read-P4kxMsiSupervisedFinish $path 6>$null }
        catch { $refusal = $_.Exception.Message }
        Assert-P4kxWaitProgress ($refusal -ceq $case.refusal) "Collector did not refuse the $($case.name) receipt at the expected gate ($refusal)."
    }
    'PASS: supervisor failures preserve bound negative results and cannot satisfy the native Finish collector.'
}
function Assert-P4kxFinishWaitSetupBudget([string]$Root) {
    $realFactory = ${function:New-P4kxPackageProcess}
    $factory = @{resume_utc=$null;returned_utc=$null}
    function New-P4kxPackageProcess {
        param([string]$Executable, [string[]]$Arguments, [string]$Directory)
        $delay = [int][Math]::Ceiling(($factory.resume_utc - [DateTimeOffset]::UtcNow).TotalMilliseconds)
        if ($delay -gt 0) { Start-Sleep -Milliseconds $delay }
        $factory.returned_utc = [DateTimeOffset]::UtcNow
        & $realFactory @PSBoundParameters
    }

    $path = [IO.Directory]::CreateDirectory((Join-Path $Root 'slow-wait-setup')).FullName
    $cutoff = [DateTimeOffset]::UtcNow.AddMilliseconds(1500)
    $factory.resume_utc = $cutoff.AddMilliseconds(50)
    $context = @{request=@{target='aarch64-pc-windows-msvc';mode='Lifecycle'};directory=$path;
        digest=('c'*64);collection_cutoff=$cutoff}
    $launch = @{supervisor_pid=123;supervisor_started_utc_ticks=456}
    $failure = $null
    try { Wait-P4kxMsiFinishReceipt $context $launch 6>$null }
    catch { $failure = $_.Exception.Message }
    $receipt = Read-P4kxDetachedReceipt (Join-Path $path 'wait-result.json')
    Assert-P4kxWaitProgress ($failure -ceq 'The bound Finish supervisor produced no result within the original allowance.' -and
        $factory.returned_utc -ge $cutoff -and $receipt.deadline_utc -ceq $cutoff.ToString('o') -and
        $null -eq $receipt.result_sha256) 'Late setup lost the original negative wait and deadline.'
    Assert-P4kxWaitProgress ($receipt.direct_probe.started -eq $false -and $receipt.direct_probe.timed_out -eq $true -and
        -not [IO.File]::Exists((Join-Path $path 'wait-probe-stdout.log'))) 'The owned wait probe launched after its original allowance.'

    $factory.resume_utc = [DateTimeOffset]::UtcNow.AddMilliseconds(3100)
    $args = @('-NoLogo','-NoProfile','-NonInteractive','-Command',"[Console]::WriteLine('fixture')")
    $result = Invoke-P4kxPortalProcess ([Environment]::ProcessPath) $args $path 'unbounded-setup' -Milliseconds 3000
    Assert-P4kxWaitProgress ($result.exit_code -eq 0 -and $result.stdout.Trim() -ceq 'fixture') 'The existing Portal capture allowance changed for a caller without the deadline.'
    'PASS: late WaitFinish setup launches no probe, with original negative receipt and unchanged Portal defaults.'
}
if ([IO.Directory]::Exists($Directory)) { throw 'Wait progress requires a fresh synthetic directory.' }
$root = [IO.Directory]::CreateDirectory($Directory).FullName
$oldActions = $env:GITHUB_ACTIONS
$oldRunnerEnvironment = $env:RUNNER_ENVIRONMENT
$env:GITHUB_ACTIONS = 'true'
$env:RUNNER_ENVIRONMENT = 'github-hosted'
$receiptNotices = @('phase=wait-receipt status=begin completion=unproven','phase=wait-receipt status=end completion=unproven')
$expectedNotices = @(
    foreach ($phase in @('predicate-setup','predicate-start','predicate-wait','predicate-cleanup','receipt-digest')) {
        "phase=$phase status=begin completion=unproven"
        "phase=$phase status=end completion=unproven"
    }
) + $receiptNotices
try {
    foreach ($case in @(
        @{name='native-arm-lifecycle';target='aarch64-pc-windows-msvc';mode='Lifecycle';direct=$true},
        @{name='native-arm-control';target='aarch64-pc-windows-msvc';mode='Control';direct=$false},
        @{name='native-amd-lifecycle';target='x86_64-pc-windows-msvc';mode='Lifecycle';direct=$true},
        @{name='native-amd-control';target='x86_64-pc-windows-msvc';mode='Control';direct=$false}
    )) {
        $path = [IO.Directory]::CreateDirectory((Join-Path $root $case.name)).FullName
        $terminal = Join-Path $path 'result.json'
        Write-P4kxDetachedReceipt $terminal @{fixture='present'}
        $context = @{
            request=@{target=$case.target;mode=$case.mode}
            directory=$path;digest=('a'*64);collection_cutoff=[DateTimeOffset]::UtcNow.AddSeconds(10)
        }
        $launch = @{supervisor_pid=123;supervisor_started_utc_ticks=456}
        $console = @(Wait-P4kxMsiFinishReceipt $context $launch 6>&1) -join [Environment]::NewLine
        $wait = Read-P4kxDetachedReceipt (Join-Path $path 'wait-result.json')
        Assert-P4kxWaitProgress ($wait.deadline_utc -ceq $context.collection_cutoff.ToString('o') -and
            $wait.result_sha256 -ceq (Get-P4kxDetachedDigest $terminal) -and
            $wait.request_sha256 -ceq $context.digest -and $wait.supervisor_pid -eq $launch.supervisor_pid -and
            $wait.supervisor_started_utc_ticks -eq $launch.supervisor_started_utc_ticks -and
            $wait.mutation_authority -eq $false) "$($case.name) changed the terminal binding or deadline."
        $notices = @(Read-P4kxFinishWaitNotice $console)
        if ($case.direct) {
            Assert-P4kxWaitProgress (($notices -join ',') -ceq ($expectedNotices -join ',')) "$($case.name) did not emit the exact observation-only stages in order."
            Assert-P4kxWaitProgress (-not $console.Contains('finishWaitPortal') -and
                -not $console.Contains('WaitFinish owned probe entered capture.')) 'The direct probe wrote Portal progress.'
            Assert-P4kxWaitProgress ($wait.direct_probe.signal_contract -ceq 'direct-process-only' -and
                $wait.direct_probe.pid -gt 0 -and $wait.direct_probe.started -eq $true -and
                $wait.direct_probe.exited -eq $true -and $wait.direct_probe.exit_code -eq 0 -and
                $wait.direct_probe.timed_out -eq $false) 'The wait lacks its owned process exit observation.'
        } else {
            Assert-P4kxWaitProgress ($notices.Count -eq 0) "$($case.name) emitted Lifecycle-only notice stages."
            Assert-P4kxWaitProgress (-not $wait.Contains('direct_probe') -and $console.Contains('finishWaitPortalCapture') -and
                $console.Contains('WaitFinish owned probe entered capture.')) "$($case.name) changed its console contract."
        }
    }

    $path = [IO.Directory]::CreateDirectory((Join-Path $root 'expired')).FullName
    $context.directory = $path
    $context.request = @{target='aarch64-pc-windows-msvc';mode='Lifecycle'}
    $context.collection_cutoff = [DateTimeOffset]::UtcNow.AddMilliseconds(-1)
    $failure = @{message=$null}
    $console = @(& {
        try { Wait-P4kxMsiFinishReceipt $context $launch }
        catch { $failure.message = $_.Exception.Message }
    } 6>&1) -join [Environment]::NewLine
    $notices = @(Read-P4kxFinishWaitNotice $console)
    Assert-P4kxWaitProgress (($notices -join ',') -ceq ($receiptNotices -join ',')) 'Expired wait reported a probe or digest phase instead of its observation-only negative receipt.'
    $expired = Read-P4kxDetachedReceipt (Join-Path $path 'wait-result.json')
    Assert-P4kxWaitProgress ($failure.message -ceq 'The bound Finish supervisor produced no result within the original allowance.' -and
        $expired.deadline_utc -ceq $context.collection_cutoff.ToString('o') -and
        $null -eq $expired.result_sha256 -and $expired.direct_probe.started -eq $false -and
        $expired.direct_probe.timed_out -eq $true -and
        -not [IO.File]::Exists((Join-Path $path 'wait-probe-stdout.log'))) 'An expired wait launched a probe or accepted a result.'

    $env:RUNNER_ENVIRONMENT = 'self-hosted'
    $context.directory = [IO.Directory]::CreateDirectory((Join-Path $root 'nonhost')).FullName
    $context.collection_cutoff = [DateTimeOffset]::UtcNow.AddSeconds(10)
    $terminal = Join-Path $context.directory 'result.json'
    Write-P4kxDetachedReceipt $terminal @{fixture='present'}
    $console = @(Wait-P4kxMsiFinishReceipt $context $launch 6>&1) -join [Environment]::NewLine
    $wait = Read-P4kxDetachedReceipt (Join-Path $context.directory 'wait-result.json')
    Assert-P4kxWaitProgress (@(Read-P4kxFinishWaitNotice $console).Count -eq 0 -and
        $wait.result_sha256 -ceq (Get-P4kxDetachedDigest $terminal)) 'The non-hosted wait changed its success receipt or emitted a hosted annotation.'
} finally {
    $env:GITHUB_ACTIONS = $oldActions
    $env:RUNNER_ENVIRONMENT = $oldRunnerEnvironment
}
'PASS: both Windows lifecycle targets use the direct process result, and both Control targets use Portal.'
Assert-P4kxFinishSupervisorNegativeReceipt $root
Assert-P4kxFinishWaitSetupBudget $root
foreach ($target in @('aarch64-pc-windows-msvc','x86_64-pc-windows-msvc')) {
    & (Join-Path $PSScriptRoot 'test-msi-finish-owned-probe.ps1') -Target $target -Directory (Join-Path $root $target)
}
