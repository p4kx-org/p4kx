$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-finish-predicate.ps1')

function Invoke-P4kxMsiFinishWaitProcess {
    param([string[]]$Arguments, [string]$Directory, [DateTimeOffset]$Cutoff)
    $output = Join-Path $Directory 'wait-host-stdout.log'
    $errors = Join-Path $Directory 'wait-host-stderr.log'
    $inputFile = Join-Path $Directory 'wait-host-empty-input'
    $receipt = Join-Path $Directory 'wait-host-result.json'
    if (@($output,$errors,$inputFile,$receipt | Where-Object { [IO.Path]::Exists($_) }).Count -ne 0) {
        throw 'MSI Finish wait host requires fresh diagnostics.'
    }
    $startedAt = [DateTimeOffset]::UtcNow
    $allowance = ($Cutoff - $startedAt).TotalMilliseconds
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $record = @{
        schema=1; role='native-msi-finish-wait-host'; revision=$env:GITHUB_SHA; run=$env:GITHUB_RUN_ID
        attempt=$env:GITHUB_RUN_ATTEMPT; target=$env:P4KX_TARGET; pid=$PID; child_pid=$null
        started_utc=$startedAt.ToString('o'); cutoff_utc=$Cutoff.ToString('o'); timed_out=$false
        child_exited=$false; exit_code=$null; native_completion_unproven=$true; mutation_authority=$false
    }
    $process = $null
    try {
        [IO.File]::WriteAllBytes($inputFile, [byte[]]::new(0))
        $start = @{
            FilePath=[Environment]::ProcessPath; ArgumentList=$Arguments; WorkingDirectory=$Directory
            WindowStyle='Hidden'; PassThru=$true; RedirectStandardInput=$inputFile
            RedirectStandardOutput=$output; RedirectStandardError=$errors
        }
        $remaining = Get-P4kxMsiFinishProbeMilliseconds $Cutoff $clock $allowance
        if ($remaining -eq 0) {
            $record.timed_out = $true
            throw 'The original MSI Finish collection allowance expired before wait launch.'
        }
        Write-Host '::notice title=Native MSI Finish wait host::phase=start completion=unproven'
        $process = Start-Process @start
        $record.child_pid = $process.Id
        $remaining = Get-P4kxMsiFinishProbeMilliseconds $Cutoff $clock $allowance
        if (-not $process.WaitForExit($remaining)) {
            $record.timed_out = $true
            throw 'The owned MSI Finish wait exceeded the original collection cutoff.'
        }
        if ([DateTimeOffset]::UtcNow -gt $Cutoff -or $clock.Elapsed.TotalMilliseconds -gt $allowance) {
            $record.timed_out = $true
            throw 'The owned MSI Finish wait exited after the original collection cutoff.'
        }
    } finally {
        try {
            if ($null -ne $process) {
                $record.child_exited = $process.HasExited
                if (-not $record.child_exited) {
                    try { $process.Kill($false) }
                    catch [InvalidOperationException], [ComponentModel.Win32Exception] {
                        if (-not $process.HasExited) { throw }
                    }
                    $record.child_exited = $process.WaitForExit(1000)
                    if (-not $record.child_exited) { throw 'The owned MSI Finish wait did not stop after its watchdog.' }
                }
                $record.exit_code = $process.ExitCode
            }
        } finally {
            if ($null -ne $process) { $process.Dispose() }
            $record.ended_utc = [DateTimeOffset]::UtcNow.ToString('o')
            if ($record.timed_out) {
                Write-Host '::error title=Native MSI Finish wait host::Owned wait exceeded the original collection cutoff.'
            }
            $record | ConvertTo-Json -Depth 4 -Compress | Set-Content -LiteralPath $receipt -Encoding utf8NoBOM
        }
    }
    Write-Host '::notice title=Native MSI Finish wait host::phase=exit completion=unproven'
    return $record
}

function Invoke-P4kxMsiHostedFinishWait {
    param([string]$Directory, [string]$CollectionCutoffUtc)
    $collectionCutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
    # The hosted reserve covers the wait process exit after an on-time observation.
    $cutoff = $collectionCutoff.AddSeconds(1)
    $root = [IO.Path]::GetFullPath($Directory)
    $diagnostics = Join-Path $root 'finish-supervision'
    $entry = Join-Path $PSScriptRoot 'invoke-msi-observer.ps1'
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',('"' + $entry + '"'),
        '-Stage','WaitFinish','-Directory',('"' + $root + '"'))
    $record = Invoke-P4kxMsiFinishWaitProcess -Arguments $arguments -Directory $diagnostics -Cutoff $cutoff
    if ($record.exit_code -ne 0 -or -not [IO.File]::Exists((Join-Path $diagnostics 'wait-result.json'))) {
        throw 'The original MSI Finish wait did not provide a successful observation. See the wait host diagnostics.'
    }
}
