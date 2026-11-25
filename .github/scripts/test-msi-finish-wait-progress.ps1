param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

function Assert-P4kxWaitProgress([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
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
