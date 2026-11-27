$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')

function Invoke-P4kxMsiHostedFinishWatchdog {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][string]$CollectionCutoffUtc,
        [string]$WorkerScript = (Join-Path $PSScriptRoot 'msi-observer-finish-hosted-worker.ps1'),
        [long]$MonitorStartedUtcTicks
    )
    $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $budget = Get-P4kxMsiRemainingBudget $CollectionCutoffUtc
    $privateLogs = [IO.Path]::Combine($Directory, 'finish-supervision')
    $start = @{
        FilePath = [Environment]::ProcessPath
        ArgumentList = @('-NoLogo','-NoProfile','-NonInteractive','-File',('"' + $WorkerScript + '"'),
            '-Directory',('"' + $Directory + '"'),'-CollectionCutoffUtc',$CollectionCutoffUtc)
        WindowStyle = 'Hidden'
        PassThru = $true
        RedirectStandardOutput = [IO.Path]::Combine($privateLogs, 'host-stdout.log')
        RedirectStandardError = [IO.Path]::Combine($privateLogs, 'host-stderr.log')
    }
    if ($MonitorStartedUtcTicks -gt 0) {
        $start.ArgumentList += @('-MonitorPid',$PID,'-MonitorStartedUtcTicks',$MonitorStartedUtcTicks)
    }
    $hosted = $MonitorStartedUtcTicks -eq 0 -and $env:GITHUB_ACTIONS -ceq 'true' -and $env:RUNNER_ENVIRONMENT -ceq 'github-hosted'
    if ($hosted) { Write-Host '::notice title=Native MSI Finish watchdog::Starting the bound receipt observer. Native completion is unproven.' }
    $owned = Start-Process @start
    try {
        $remaining = [int][Math]::Max(0,[Math]::Floor([Math]::Min(
            ($cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds, $budget - $timer.Elapsed.TotalMilliseconds)))
        if ($remaining -eq 0 -or -not $owned.WaitForExit($remaining) -or
            [DateTimeOffset]::UtcNow -gt $cutoff -or $timer.Elapsed.TotalMilliseconds -gt $budget) {
            if ($hosted) { Write-Host '::notice title=Native MSI Finish watchdog::The receipt observer reached its original deadline. Native completion is unproven.' }
            $failureSettlementMilliseconds = 750
            [void]$owned.WaitForExit($failureSettlementMilliseconds)
            throw 'Hosted Finish observation exceeded its original collection deadline.'
        }
        $code = $owned.ExitCode
        if ($hosted) { Write-Host "::notice title=Native MSI Finish watchdog::Receipt observer exited $code. Native completion is unproven." }
        return $code
    } finally {
        try {
            if (-not $owned.HasExited) {
                try { $owned.Kill($false) }
                catch [InvalidOperationException], [ComponentModel.Win32Exception] {
                    if (-not $owned.HasExited) { throw }
                }
                if (-not $owned.WaitForExit(1000)) { throw 'The owned hosted Finish observer did not stop after its watchdog.' }
            }
        } finally { $owned.Dispose() }
    }
}
