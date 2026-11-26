function Get-P4kxMsiObservedProcess([int]$ProcessId, [long]$StartedTicks) {
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    $state = 'absent'
    if ($null -ne $process) {
        try {
            $state = if ($process.StartTime.ToUniversalTime().Ticks -ne $StartedTicks) { 'pidReused' }
                elseif ($process.HasExited) { 'exited' } else { 'running' }
        } catch { $state = 'unavailable' }
        finally { $process.Dispose() }
    }
    return @{ pid=$ProcessId; started_utc_ticks=$StartedTicks; state=$state }
}

function Assert-P4kxMsiProgressReadiness($Context, $Ready, [string]$LaunchDigest) {
    if ($Ready.request_sha256 -cne $Context.digest -or $Ready.launch_sha256 -cne $LaunchDigest) {
        throw 'Finish readiness belongs to a foreign MSI launch.'
    }
}
