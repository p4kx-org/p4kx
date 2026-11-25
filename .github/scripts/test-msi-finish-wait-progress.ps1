param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Assert-P4kxWaitProgress([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
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
    Assert-P4kxWaitProgress ($receipt.portal_stages.stage -cnotcontains 'portalStart.end' -and
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
$env:GITHUB_ACTIONS = 'true'
$expected = @('portalStart.begin','portalStart.end','portalInputClose.begin','portalInputClose.end',
    'portalCapture.begin','portalCapture.end','portalCleanup.begin','portalCleanup.end')
try {
    foreach ($case in @(
        @{name='native-arm-lifecycle';target='aarch64-pc-windows-msvc';mode='Lifecycle';buffer=$true},
        @{name='native-arm-control';target='aarch64-pc-windows-msvc';mode='Control';buffer=$false},
        @{name='native-amd-lifecycle';target='x86_64-pc-windows-msvc';mode='Lifecycle';buffer=$false}
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
        if ($case.buffer) {
            Assert-P4kxWaitProgress (-not $console.Contains('finishWaitPortal') -and
                -not $console.Contains('WaitFinish owned probe entered capture.')) 'The buffered callback reached the host.'
            Assert-P4kxWaitProgress (($wait.portal_stages.stage -join ' ') -ceq ($expected -join ' ')) 'The wait did not retain all eight fixed transitions.'
            $previous = [long]0
            foreach ($stage in $wait.portal_stages) {
                Assert-P4kxWaitProgress ($stage.elapsed_ms -ge $previous) 'Portal transitions are outside monotonic order.'
                $previous = $stage.elapsed_ms
            }
        } else {
            Assert-P4kxWaitProgress (-not $wait.Contains('portal_stages') -and $console.Contains('finishWaitPortalCapture') -and
                $console.Contains('WaitFinish owned probe entered capture.')) "$($case.name) changed its console contract."
        }
    }

    $path = [IO.Directory]::CreateDirectory((Join-Path $root 'expired')).FullName
    $context.directory = $path
    $context.request = @{target='aarch64-pc-windows-msvc';mode='Lifecycle'}
    $context.collection_cutoff = [DateTimeOffset]::UtcNow.AddMilliseconds(-1)
    $failure = $null
    try { Wait-P4kxMsiFinishReceipt $context $launch 6>$null }
    catch { $failure = $_.Exception.Message }
    $expired = Read-P4kxDetachedReceipt (Join-Path $path 'wait-result.json')
    Assert-P4kxWaitProgress ($failure -ceq 'The bound Finish supervisor produced no result within the original allowance.' -and
        $expired.deadline_utc -ceq $context.collection_cutoff.ToString('o') -and
        $null -eq $expired.result_sha256 -and $expired.portal_stages.Count -eq 0 -and
        -not [IO.File]::Exists((Join-Path $path 'wait-probe-stdout.log'))) 'An expired wait launched a probe or accepted a result.'
} finally {
    $env:GITHUB_ACTIONS = $oldActions
}
'PASS: ARM64 lifecycle callback isolation, unchanged other routes and original deadline.'
Assert-P4kxFinishSupervisorNegativeReceipt $root
Assert-P4kxFinishWaitSetupBudget $root
