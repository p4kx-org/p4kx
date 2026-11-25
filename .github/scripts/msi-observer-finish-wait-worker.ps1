param(
    [Parameter(Mandatory)][string]$ReceiptPath,
    [Parameter(Mandatory)][string]$CollectionCutoff,
    [switch]$UseExitStatus
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')

$cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoff
$budget = ($cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
$clock = [Diagnostics.Stopwatch]::StartNew()
while ($true) {
    $wallRemaining = ($cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
    $remaining = [Math]::Min($wallRemaining, $budget - $clock.Elapsed.TotalMilliseconds)
    if ($remaining -le 0) { break }
    if ([IO.File]::Exists($ReceiptPath)) {
        if ($UseExitStatus) { exit 0 }
        [Console]::WriteLine($true)
        return
    }
    Start-Sleep -Milliseconds ([int][Math]::Min(100, [Math]::Ceiling($remaining)))
}
if ($UseExitStatus) { exit 3 }
[Console]::WriteLine($false)
