param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Verify')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-snapshot-delay.ps1')

$row = Get-P4kxPlatform -Target $env:P4KX_TARGET
Assert-P4kxNativeHost $row
$tracking = Get-P4kxDetachedTrackingDigest
if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
    [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString() -ine $row.runner_arch -or
    $null -eq $tracking) {
    throw 'Split delay control requires original native hosted-runner identity.'
}
$root = [IO.Path]::GetFullPath($Directory)
$path = Join-Path $root 'request.json'
$deadline = $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
$sources = @('invoke-msi-observer.ps1','msi-observer-snapshot-delay.ps1','msi-observer-progress.ps1','msi-detached-control.ps1')
$hashes = @{}
foreach ($name in $sources) { $hashes[$name] = Get-P4kxDetachedDigest (Join-Path $PSScriptRoot $name) }

if ($Stage -ceq 'Prepare') {
    [void](Get-P4kxMsiRemainingBudget $deadline)
    if ([IO.Path]::Exists($root)) { throw 'Split delay control requires a fresh observation directory.' }
    [void][IO.Directory]::CreateDirectory($root)
    Write-P4kxDetachedReceipt $path @{
        schema=1;role='native-msi-observer-request';mode='Lifecycle';purpose='synthetic-split-delay-control'
        directory=$root;deadline_utc=$deadline;observer_deadline_utc=$deadline
        revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$row.target
        tracking_sha256=$tracking;source_sha256=$hashes
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    return
}

$request = Read-P4kxDetachedReceipt $path
if ($request.purpose -cne 'synthetic-split-delay-control' -or $request.tracking_sha256 -cne $tracking -or
    $request.completed_success -ne $false -or $request.native_completion_unproven -ne $true -or
    $request.mutation_authority -ne $false -or $request.Contains('invocation') -or $request.Contains('script') -or
    [IO.Path]::Exists((Join-Path $root 'handle.json')) -or $request.source_sha256.Count -ne $sources.Count) {
    throw 'Split delay control requires its original observation-only request.'
}
foreach ($name in $sources) {
    if ($request.source_sha256[$name] -cne $hashes[$name]) { throw 'Split delay control source bytes differ.' }
}
[void](Read-P4kxMsiCheckpoint2Delay $root)
$resultPath = Join-Path $root 'delays/02/delay-result.json'
$result = Read-P4kxDetachedReceipt $resultPath
Write-P4kxDetachedReceipt (Join-Path $root 'split-delay-control-result.json') @{
    schema=1;role='synthetic-native-msi-split-delay-control-result'
    revision=$result.revision;run=$result.run;attempt=$result.attempt;target=$row.target;tracking_sha256=$tracking
    request_sha256=$result.request_sha256;delay_result_sha256=(Get-P4kxDetachedDigest $resultPath)
    source_sha256=$hashes;deadline_utc=$result.deadline_utc
    requested_wait_ms=$result.requested_wait_ms;elapsed_ms=$result.elapsed_ms;control_passed=$true
    completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
}
