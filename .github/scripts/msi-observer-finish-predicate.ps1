$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'package-process.ps1')

function Get-P4kxMsiFinishProbeMilliseconds {
    param([DateTimeOffset]$Deadline, [Diagnostics.Stopwatch]$Clock, [double]$Budget)
    $wall=($Deadline - [DateTimeOffset]::UtcNow).TotalMilliseconds
    return [int][Math]::Max(0,[Math]::Floor([Math]::Min($wall,$Budget - $Clock.Elapsed.TotalMilliseconds)))
}

function Test-P4kxMsiFinishDirectReceiptPresence {
    param($Context, [string]$ReceiptPath, [Diagnostics.Stopwatch]$Clock, [double]$Budget, [hashtable]$Observation)
    $Observation.signal_contract='direct-process-only'
    $Observation.started=$false
    $Observation.exited=$false
    $Observation.exit_code=$null
    $Observation.timed_out=$false
    if ((Get-P4kxMsiFinishProbeMilliseconds $Context.collection_cutoff $Clock $Budget) -eq 0) {
        $Observation.timed_out=$true
        return $false
    }

    $script=Join-Path $PSScriptRoot 'msi-observer-finish-wait-worker.ps1'
    $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-File',$script,
        '-ReceiptPath',$ReceiptPath,'-CollectionCutoff',$Context.collection_cutoff.ToString('o'),'-UseExitStatus')
    $process=New-P4kxPackageProcess ([Environment]::ProcessPath) $arguments $Context.directory
    try {
        if ((Get-P4kxMsiFinishProbeMilliseconds $Context.collection_cutoff $Clock $Budget) -eq 0) {
            $Observation.timed_out=$true
            return $false
        }
        if (-not $process.Start()) { throw 'The owned Finish presence probe did not start.' }
        $Observation.started=$true
        $Observation.pid=$process.Id
        # An inherited pipe writer can outlive the predicate process.
        $remaining=Get-P4kxMsiFinishProbeMilliseconds $Context.collection_cutoff $Clock $Budget
        if ($remaining -eq 0 -or -not $process.WaitForExit($remaining)) {
            $Observation.timed_out=$true
            return $false
        }
        $Observation.exited=$true
        $Observation.exit_code=$process.ExitCode
        switch ($Observation.exit_code) {
            0 { return $true }
            3 { return $false }
            default { throw 'The bounded Finish presence probe did not report a valid outcome.' }
        }
    } finally {
        try {
            if ($Observation.started -and -not $Observation.exited) {
                if (-not $process.HasExited) {
                    $process.Kill($false)
                    if (-not $process.WaitForExit(1000)) { throw 'The owned Finish presence probe did not terminate after its watchdog.' }
                }
                $Observation.exited=$true
                $Observation.exit_code=$process.ExitCode
            }
        } finally { $process.Dispose() }
    }
}
