param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')

$context = Get-P4kxMsiFinishContext $Directory
$self = [Diagnostics.Process]::GetCurrentProcess()
try {
    $result = @{
        schema=1;role='native-msi-finish-supervisor-result';request_sha256=$context.digest
        supervisor_pid=$PID;supervisor_started_utc_ticks=$self.StartTime.ToUniversalTime().Ticks
        caller_pid=$null;caller_exited=$false;caller_exit_code=$null;observed_exit_utc=$null
        caller_result_sha256=$null;timed_out=$false;mutation_authority=$false
    }
} finally { $self.Dispose() }
$owned = $null
try {
    $null = Get-P4kxMsiSnapshotBudget $context.cutoff.ToString('o')
    $owned = Start-P4kxMsiFinishProcess $context 'invoke-msi-observer.ps1' caller @('-Stage','Finish','-WaitMilliseconds','0')
    $result.caller_pid = $owned.Id
    $remaining = [int][Math]::Max(0,[Math]::Ceiling(($context.cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds))
    if ($owned.WaitForExit($remaining)) { $result.observed_exit_utc = [DateTimeOffset]::UtcNow.ToString('o') }
    else { $result.timed_out = $true }
} finally {
    if ($null -ne $owned) {
        try {
            if (-not $owned.HasExited) {
                $owned.Kill($false)
                [void]$owned.WaitForExit(1000)
            }
            $result.caller_exited = $owned.HasExited
            if ($owned.HasExited) { $result.caller_exit_code = $owned.ExitCode }
        } finally { $owned.Dispose() }
    }
    $caller = Join-Path $context.root 'snapshots/09/caller-result.json'
    if ([IO.File]::Exists($caller)) { $result.caller_result_sha256 = Get-P4kxDetachedDigest $caller }
    $staged = Join-Path $context.directory 'result.stage.json'
    Write-P4kxDetachedReceipt $staged $result
    [IO.File]::Move($staged,(Join-Path $context.directory 'result.json'))
}
