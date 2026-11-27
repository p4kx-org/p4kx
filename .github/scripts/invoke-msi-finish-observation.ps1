param(
    [Parameter(Mandatory)][ValidateSet('Start','Monitor','Wait')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$CollectionCutoffUtc,
    [string]$ReadyDigest
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-ready.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-watchdog.ps1')

$cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
$clock = [Diagnostics.Stopwatch]::StartNew()
$budget = Get-P4kxMsiRemainingBudget $CollectionCutoffUtc
$private = Join-Path $Directory 'finish-supervision'
$readyPath = Join-Path $private 'waiter-ready.json'
$exitPath = Join-Path $private 'waiter-exit.json'
$waitPath = Join-Path $private 'wait-result.json'

switch ($Stage) {
    'Start' {
        $context = Get-P4kxMsiFinishContext $Directory
        if ($context.request.mode -cne 'Lifecycle' -or $cutoff -ne $context.collection_cutoff) {
            throw 'Hosted Finish monitor requires the original Lifecycle collection cutoff.'
        }
        $null = Read-P4kxMsiFinishLaunch $context
        $launchDigest = Get-P4kxDetachedReceiptDigest (Join-Path $private 'launch.json')
        $admission = Join-Path $private 'waiter-admission.json'
        if ([IO.File]::Exists($admission)) { throw 'A Finish waiter was already admitted for this launch.' }
        $readyBudget = [Math]::Min(10000, $budget)
        Write-P4kxDetachedReceipt $admission @{
            schema=1;role='native-msi-finish-waiter-admission';request_sha256=$context.digest
            launch_sha256=$launchDigest;deadline_utc=$CollectionCutoffUtc
            native_completion_unproven=$true;mutation_authority=$false
        }
        if ((Get-P4kxMsiFinishProbeMilliseconds $cutoff $clock $readyBudget) -eq 0) {
            throw 'The Finish waiter startup allowance expired before launch.'
        }
        $monitor = Start-P4kxMsiFinishProcess $context 'invoke-msi-finish-observation.ps1' '' @(
            '-Stage','Monitor','-CollectionCutoffUtc',$CollectionCutoffUtc
        )
        try {
            $monitorTicks = $monitor.StartTime.ToUniversalTime().Ticks
            if (-not (Wait-P4kxMsiFinishReceiptFile $readyPath $cutoff $clock $readyBudget)) {
                throw 'The original Finish waiter did not attest readiness within the launch allowance.'
            }
            $ready = Read-P4kxMsiHostedFinishReady $readyPath $cutoff
            if ($ready.monitor_pid -ne $monitor.Id -or $ready.monitor_started_utc_ticks -ne $monitorTicks -or
                $ready.request_sha256 -cne $context.digest -or $ready.launch_sha256 -cne $launchDigest) {
                throw 'Finish waiter readiness belongs to a foreign launch.'
            }
            Get-P4kxDetachedReceiptDigest $readyPath
        } finally { $monitor.Dispose() }
    }
    'Monitor' {
        $self = [Diagnostics.Process]::GetCurrentProcess()
        try {
            $monitorTicks = $self.StartTime.ToUniversalTime().Ticks
            $code = Invoke-P4kxMsiHostedFinishWatchdog -Directory $Directory -CollectionCutoffUtc $CollectionCutoffUtc -MonitorStartedUtcTicks $monitorTicks
            $observed = [DateTimeOffset]::UtcNow
            $ready = Read-P4kxMsiHostedFinishReady $readyPath $cutoff
            if ($ready.monitor_pid -ne $self.Id -or $ready.monitor_started_utc_ticks -ne $monitorTicks) {
                throw 'Finish waiter readiness belongs to a different monitor.'
            }
            $waitDigest = if ($code -eq 0) { Get-P4kxDetachedReceiptDigest $waitPath } else { $null }
            Write-P4kxDetachedReceipt $exitPath @{
                schema=1;role='native-msi-finish-waiter-exit';ready_sha256=(Get-P4kxDetachedReceiptDigest $readyPath)
                deadline_utc=$CollectionCutoffUtc;observed_utc=$observed.ToString('o');exit_code=$code
                wait_sha256=$waitDigest;native_completion_unproven=$true;mutation_authority=$false
            }
        } finally { $self.Dispose() }
    }
    'Wait' {
        if ($ReadyDigest -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Finish wait requires the original waiter readiness digest.'
        }
        $ready = Read-P4kxMsiHostedFinishReady $readyPath $cutoff $ReadyDigest
        if (-not (Wait-P4kxMsiFinishReceiptFile $exitPath $cutoff $clock $budget)) {
            throw 'The bound Finish waiter produced no exit receipt within the original allowance.'
        }
        $exitProof = Read-P4kxDetachedReceipt $exitPath
        $observed = ConvertFrom-P4kxMsiBudgetTimestamp $exitProof.observed_utc
        if ($exitProof.schema -ne 1 -or $exitProof.role -cne 'native-msi-finish-waiter-exit' -or
            $exitProof.ready_sha256 -cne $ReadyDigest -or $exitProof.deadline_utc -cne $CollectionCutoffUtc -or
            $observed -lt (ConvertFrom-P4kxMsiBudgetTimestamp $ready.observed_utc) -or $observed -ge $cutoff -or
            $exitProof.exit_code -ne 0 -or $exitProof.wait_sha256 -cne (Get-P4kxDetachedReceiptDigest $waitPath) -or
            $exitProof.native_completion_unproven -ne $true -or $exitProof.mutation_authority -ne $false -or
            (Get-P4kxMsiFinishProbeMilliseconds $cutoff $clock $budget) -eq 0) {
            throw 'The original Finish waiter exit proof is late, foreign or unsuccessful.'
        }
    }
}
