param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateSet('aarch64-pc-windows-msvc','x86_64-pc-windows-msvc')][string]$Target,
    [ValidateSet('inherit','hold','hang','invalid')][string]$Fixture
)
$ErrorActionPreference='Stop'

if ($Fixture -ceq 'hold') {
    [Console]::WriteLine('inherited stdout open')
    [IO.File]::WriteAllText((Join-Path $Directory 'ready'),[string]$PID)
    $clock=[Diagnostics.Stopwatch]::StartNew()
    while ($clock.ElapsedMilliseconds -lt 10000 -and -not [IO.File]::Exists((Join-Path $Directory 'release'))) {
        Start-Sleep -Milliseconds 25
    }
    exit 0
}
if ($Fixture -ceq 'inherit') {
    $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',('"' + $PSCommandPath + '"'),
        '-Directory',('"' + $Directory + '"'),'-Fixture','hold')
    $holder=Start-Process -FilePath ([Environment]::ProcessPath) -ArgumentList $arguments -NoNewWindow -PassThru
    try {
        $clock=[Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists((Join-Path $Directory 'ready'))) {
            if ($clock.ElapsedMilliseconds -ge 3000 -or $holder.HasExited) { exit 9 }
            Start-Sleep -Milliseconds 25
        }
        [Console]::WriteLine($true)
        exit 0
    } finally { $holder.Dispose() }
}
if ($Fixture -ceq 'hang') {
    [IO.File]::WriteAllText((Join-Path $Directory 'ready'),[string]$PID)
    Start-Sleep -Seconds 10
    exit 9
}
if ($Fixture -ceq 'invalid') { exit 9 }

. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Invoke-P4kxFinishProbeFixture([string]$Root, [string]$Target, [string]$Case, [int]$Allowance) {
    $path=[IO.Directory]::CreateDirectory((Join-Path $Root $Case)).FullName
    $realFactory=${function:New-P4kxPackageProcess}
    $fixtureScript=$PSCommandPath
    function New-P4kxPackageProcess {
        param([string]$Executable,[string[]]$Arguments,[string]$Directory)
        $fixtureArgs=@('-NoLogo','-NoProfile','-NonInteractive','-File',$fixtureScript,
            '-Directory',$Directory,'-Fixture',$Case)
        & $realFactory $Executable $fixtureArgs $Directory
    }

    $result=Join-Path $path 'result.json'
    Write-P4kxDetachedReceipt $result @{fixture='bound'}
    $cutoff=[DateTimeOffset]::UtcNow.AddMilliseconds($Allowance)
    $context=@{
        request=@{target=$Target;mode='Lifecycle'}
        directory=$path;digest=('a'*64);collection_cutoff=$cutoff
    }
    $launch=@{supervisor_pid=123;supervisor_started_utc_ticks=456}
    $failure=$null
    $clock=[Diagnostics.Stopwatch]::StartNew()
    try {
        try { Wait-P4kxMsiFinishReceipt $context $launch 6>$null }
        catch { $failure=$_.Exception.Message }
        $elapsed=$clock.ElapsedMilliseconds
        $ready=Join-Path $path 'ready'
        $ownedPid=if ([IO.File]::Exists($ready)) { [int][IO.File]::ReadAllText($ready) } else { $null }
        $owned=if ($null -ne $ownedPid) { Get-Process -Id $ownedPid -ErrorAction SilentlyContinue } else { $null }
        try {
            $alive=$null -ne $owned -and -not $owned.HasExited
            $waitPath=Join-Path $path 'wait-result.json'
            $wait=if ([IO.File]::Exists($waitPath)) { Read-P4kxDetachedReceipt $waitPath } else { $null }
            return @{failure=$failure;wait=$wait;result_sha256=(Get-P4kxDetachedDigest $result);
                cutoff=$cutoff.ToString('o');elapsed=$elapsed;alive=$alive;pid=$ownedPid}
        } finally { if ($null -ne $owned) { $owned.Dispose() } }
    } finally { [IO.File]::WriteAllText((Join-Path $path 'release'),'') }
}

if (-not $Target) { throw 'Specify the Windows lifecycle target for the owned probe fixture.' }
if ([IO.Directory]::Exists($Directory)) { throw 'Use a fresh owned-probe fixture directory.' }
$root=[IO.Directory]::CreateDirectory($Directory).FullName
$inheritance=Invoke-P4kxFinishProbeFixture $root $Target 'inherit' 4500
if (!$inheritance.alive -or $null -eq $inheritance.wait -or
    $inheritance.wait.result_sha256 -cne $inheritance.result_sha256 -or $inheritance.failure -or
    $inheritance.wait.direct_probe.signal_contract -cne 'direct-process-only' -or
    $inheritance.wait.direct_probe.exited -ne $true -or $inheritance.wait.direct_probe.exit_code -ne 0) {
    throw "Direct child exit did not preserve its bound positive result with the inherited stdout writer still open ($($inheritance.failure))."
}
$timeout=Invoke-P4kxFinishProbeFixture $root $Target 'hang' 2000
if ($null -eq $timeout.pid -or $timeout.alive -or $timeout.elapsed -gt 5000 -or
    $timeout.failure -cne 'The bound Finish supervisor produced no result within the original allowance.' -or
    $null -eq $timeout.wait -or $null -ne $timeout.wait.result_sha256 -or
    $timeout.wait.deadline_utc -cne $timeout.cutoff -or $timeout.wait.mutation_authority -ne $false -or
    $timeout.wait.direct_probe.pid -ne $timeout.pid -or $timeout.wait.direct_probe.timed_out -ne $true -or
    $timeout.wait.direct_probe.exited -ne $true -or $null -eq $timeout.wait.direct_probe.exit_code) {
    throw 'The hung presence probe did not terminate with the original negative, bound cutoff evidence.'
}
$invalid=Invoke-P4kxFinishProbeFixture $root $Target 'invalid' 2500
if ($null -ne $invalid.wait -or $invalid.failure -cne 'The bounded Finish presence probe did not report a valid outcome.') {
    throw 'An unexpected probe exit code was not refused.'
}
"PASS: $Target lifecycle does not wait on the inherited stdout writer and preserves the bound negative timeout receipt."
