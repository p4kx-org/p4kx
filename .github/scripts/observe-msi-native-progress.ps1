$ErrorActionPreference = 'Stop'
$harness = $env:GITHUB_SHA
$env:GITHUB_SHA = $env:P4KX_MSI_CANDIDATE
. .github/scripts/msi-observer-finish-control.ps1
. .github/scripts/msi-observer-finish-ready.ps1
. .github/scripts/msi-observer-evidence.ps1

. (Join-Path $PSScriptRoot 'msi-native-progress-liveness.ps1')

$directory = Join-Path $env:P4KX_PACKAGE_TASK 'installer-check/observer'
$context = Get-P4kxMsiFinishContext $directory
$launch = Read-P4kxMsiFinishLaunch $context
$cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $env:P4KX_MSI_COLLECTION_CUTOFF
if ($context.request.mode -cne 'Lifecycle' -or $cutoff -ne $context.collection_cutoff -or
    [DateTimeOffset]::UtcNow -ge $cutoff -or $env:P4KX_MSI_READY_DIGEST -cnotmatch '^[0-9a-f]{64}$') {
    throw 'A live diagnostic sample requires the original Lifecycle readiness before cutoff.'
}
$readyPath = Join-Path $context.directory 'waiter-ready.json'
$ready = Read-P4kxMsiHostedFinishReady $readyPath $cutoff $env:P4KX_MSI_READY_DIGEST
$launchDigest = Get-P4kxDetachedReceiptDigest (Join-Path $context.directory 'launch.json')
Assert-P4kxMsiProgressReadiness $context $ready $launchDigest
try {
    .github/scripts/invoke-msi-observer.ps1 -Stage ProbeFinish -Directory $directory -Sequence 4
} finally {
    $sample = Join-Path $directory 'finish-probes/04'
    $null = [IO.Directory]::CreateDirectory($sample)
    $observed = [DateTimeOffset]::UtcNow
    $processes = @{
        supervisor=(Get-P4kxMsiObservedProcess $launch.supervisor_pid $launch.supervisor_started_utc_ticks)
        monitor=(Get-P4kxMsiObservedProcess $ready.monitor_pid $ready.monitor_started_utc_ticks)
        waiter=(Get-P4kxMsiObservedProcess $ready.waiter_pid $ready.waiter_started_utc_ticks)
    }
    $files = @{}
    foreach ($name in @('waiter-ready.json','waiter-exit.json','wait-result.json')) {
        $files[$name] = Copy-P4kxMsiObserverFile (Join-Path $context.directory $name) (Join-Path $sample $name) 16KB
    }
    $checkpoint = Join-Path $directory 'checkpoints/04/finish-progress.json'
    $progress = if ([IO.File]::Exists($checkpoint)) { Get-P4kxDetachedReceiptDigest $checkpoint }
    Write-P4kxDetachedReceipt (Join-Path $sample 'post-ready.json') @{
        schema=1; role='native-msi-finish-post-ready'; target=$context.request.target
        candidate_revision=$context.request.revision; harness_revision=$harness
        run=$context.request.run; attempt=$context.request.attempt; request_sha256=$context.digest
        launch_sha256=$launchDigest; readiness_sha256=$env:P4KX_MSI_READY_DIGEST
        progress_sha256=$progress; observed_utc=$observed.ToString('o'); cutoff_utc=$cutoff.ToString('o')
        processes=$processes; files=$files; native_completion_unproven=$true; mutation_authority=$false
    }
}
