$ErrorActionPreference = 'Stop'

function Assert-P4kxDocumentsControllerEvidence {
    <#
    .SYNOPSIS
    Requires both directly held session processes to have exited on the measured bus.
    #>
    param($Controller, [string]$Session, [string]$Role)
    if ($Controller.schema -ne 1 -or $Controller.session -cne $Session -or $Controller.role -cne $Role -or
        $Controller.direct_processes_exited -ne $true -or $Controller.signal_contract -cne 'direct-process-only') {
        throw 'Owned session controller identity or direct-process teardown is incomplete.'
    }
    foreach ($process in @($Controller.bus, $Controller.worker)) {
        if ($process.started -ne $true -or $process.exited -ne $true -or $process.signal_contract -cne 'direct-process-only' -or
            ($process.pid -isnot [int] -and $process.pid -isnot [long]) -or $process.pid -le 0 -or
            ($process.exit_code -isnot [int] -and $process.exit_code -isnot [long])) {
            throw 'Owned session process has no complete direct-start/exit evidence.'
        }
    }
    if ($Controller.bus.pid -eq $Controller.worker.pid) { throw 'Owned bus and worker process identities are not distinct.' }
}