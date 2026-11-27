$ErrorActionPreference = 'Stop'

function Write-P4kxMsiHostedFinishReady {
    param($Context, [int]$MonitorPid, [long]$MonitorStartedUtcTicks)
    $self = [Diagnostics.Process]::GetCurrentProcess()
    try {
        $observed = [DateTimeOffset]::UtcNow
        $started = $self.StartTime.ToUniversalTime().Ticks
        if ($MonitorStartedUtcTicks -le 0 -or $started -lt $MonitorStartedUtcTicks -or
            $observed -ge $Context.collection_cutoff -or $MonitorPid -eq $self.Id) {
            throw 'Finish observer readiness missed its original collection allowance or process binding.'
        }
        Write-P4kxDetachedReceipt (Join-Path $Context.directory 'waiter-ready.json') @{
            schema=1;role='native-msi-finish-waiter-ready';request_sha256=$Context.digest
            launch_sha256=(Get-P4kxDetachedReceiptDigest (Join-Path $Context.directory 'launch.json'))
            monitor_pid=$MonitorPid;monitor_started_utc_ticks=$MonitorStartedUtcTicks
            waiter_pid=$self.Id;waiter_started_utc_ticks=$started
            deadline_utc=$Context.collection_cutoff.ToString('o');observed_utc=$observed.ToString('o')
            native_completion_unproven=$true;mutation_authority=$false
        }
    } finally { $self.Dispose() }
}

function Read-P4kxMsiHostedFinishReady {
    param([string]$Path, [DateTimeOffset]$Cutoff, [string]$ExpectedDigest)
    if ($ExpectedDigest -and $ExpectedDigest -cne (Get-P4kxDetachedReceiptDigest $Path)) {
        throw 'Finish readiness bytes differ from the original launch stage.'
    }
    $ready = Read-P4kxDetachedReceipt $Path
    $observed = ConvertFrom-P4kxMsiBudgetTimestamp $ready.observed_utc
    if ($ready.schema -ne 1 -or $ready.role -cne 'native-msi-finish-waiter-ready' -or
        $ready.monitor_pid -le 0 -or $ready.waiter_pid -le 0 -or $ready.monitor_pid -eq $ready.waiter_pid -or
        $ready.monitor_started_utc_ticks -le 0 -or $ready.waiter_started_utc_ticks -lt $ready.monitor_started_utc_ticks -or
        $ready.waiter_started_utc_ticks -gt $observed.UtcTicks -or $observed -ge $Cutoff -or
        $ready.deadline_utc -cne $Cutoff.ToString('o') -or
        $ready.request_sha256 -cnotmatch '^[0-9a-f]{64}$' -or $ready.launch_sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        $ready.native_completion_unproven -ne $true -or $ready.mutation_authority -ne $false) {
        throw 'Finish readiness does not bind a timely original observer and monitor.'
    }
    return $ready
}
