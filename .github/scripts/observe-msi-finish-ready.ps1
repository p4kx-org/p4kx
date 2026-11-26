param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$CollectionCutoffUtc,
    [Parameter(Mandatory)][string]$ReadyDigest
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-ready.ps1')
. (Join-Path $PSScriptRoot 'msi-native-progress-liveness.ps1')

$context = Get-P4kxMsiFinishContext $Directory
$cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
if ($context.request.mode -cne 'Lifecycle' -or $cutoff -ne $context.collection_cutoff -or
    [DateTimeOffset]::UtcNow -ge $cutoff -or $ReadyDigest -cnotmatch '\A[0-9a-f]{64}\z') {
    throw 'A live checkpoint requires original Lifecycle readiness before the original cutoff.'
}

$launch = Read-P4kxMsiFinishLaunch $context
$launchDigest = Get-P4kxDetachedReceiptDigest (Join-Path $context.directory 'launch.json')
$ready = Read-P4kxMsiHostedFinishReady (Join-Path $context.directory 'waiter-ready.json') $cutoff $ReadyDigest
Assert-P4kxMsiProgressReadiness $context $ready $launchDigest

$observed = [DateTimeOffset]::UtcNow
$processes = @{
    supervisor=(Get-P4kxMsiObservedProcess $launch.supervisor_pid $launch.supervisor_started_utc_ticks)
    monitor=(Get-P4kxMsiObservedProcess $ready.monitor_pid $ready.monitor_started_utc_ticks)
    waiter=(Get-P4kxMsiObservedProcess $ready.waiter_pid $ready.waiter_started_utc_ticks)
}
$parent = Get-P4kxMsiObserverParentState $context.request $context.handle
$sequence = 4
$checkpoint = Save-P4kxMsiObserverCheckpoint $context.request $context.handle $sequence $parent
$files = @{}
foreach ($name in @('result.json','wait-result.json','waiter-exit.json')) {
    $files[$name] = Copy-P4kxMsiObserverFile (Join-Path $context.directory $name) (Join-Path $checkpoint "finish-$name") 16KB
}
$runner = @{}
$runnerVariables = @{ image_os='ImageOS';image_version='ImageVersion';runner_version='RUNNER_VERSION' }
foreach ($key in $runnerVariables.Keys) {
    $value = [Environment]::GetEnvironmentVariable($runnerVariables[$key])
    if ($value -cmatch '\A[A-Za-z0-9._-]{1,80}\z') { $runner[$key] = $value }
}
if ([DateTimeOffset]::UtcNow -ge $cutoff) { throw 'The live checkpoint reached the original collection cutoff.' }
Write-P4kxDetachedReceipt (Join-Path $checkpoint 'post-ready.json') @{
    schema=1;role='native-msi-finish-live-checkpoint';revision=$context.request.revision
    run=$context.request.run;attempt=$context.request.attempt;target=$context.request.target
    sequence=$sequence;request_sha256=$context.digest;launch_sha256=$launchDigest
    readiness_sha256=$ReadyDigest;checkpoint_sha256=(Get-P4kxDetachedReceiptDigest (Join-Path $checkpoint 'checkpoint.json'))
    observed_utc=$observed.ToString('o');collection_cutoff_utc=$cutoff.ToString('o')
    processes=$processes;files=$files;runner=$runner
    completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
}
