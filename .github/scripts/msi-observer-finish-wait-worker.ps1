param(
    [Parameter(Mandatory)][string]$ReceiptPath,
    [Parameter(Mandatory)][string]$CollectionCutoff,
    [switch]$UseExitStatus
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-receipt-poll.ps1')

$cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoff
$budget = ($cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
$clock = [Diagnostics.Stopwatch]::StartNew()
$found = Wait-P4kxMsiFinishReceiptFile $ReceiptPath $cutoff $clock $budget
if ($UseExitStatus) {
    if ($found) { exit 0 }
    exit 3
}
[Console]::WriteLine($found)
