$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-session-bus.ps1')

function Wait-P4kxDocumentsWorker {
    param([hashtable]$Bus, [hashtable]$Worker, [string]$Address, [Diagnostics.Stopwatch]$Timer,
        [int]$Milliseconds, [int]$MaximumBytes)
    while ($true) {
        if ($Timer.ElapsedMilliseconds -ge $Milliseconds) { throw 'Owned session worker timed out.' }
        Update-P4kxPortalChannels $Bus.channels 131072
        Update-P4kxPortalChannels $Worker.channels $MaximumBytes
        if ($Bus.process.HasExited) { throw 'Owned session bus exited while its worker was running.' }
        if ((ConvertFrom-P4kxDocumentsBusAddress $Bus.channels[0].text.ToString() '') -cne $Address) {
            throw 'Owned session bus address changed during execution.'
        }
        if ($Worker.process.HasExited -and @($Worker.channels | Where-Object { $null -ne $_.pending }).Count -eq 0) {
            return @{exit_code=$Worker.process.ExitCode; stdout=$Worker.channels[0].text.ToString(); stderr=$Worker.channels[1].text.ToString()}
        }
        Wait-P4kxPortalChannels ($Bus.channels + $Worker.channels)
    }
}

function Close-P4kxDocumentsProcess {
    param([hashtable]$Owned, [string]$Directory, [string]$Name, [Collections.Generic.List[Exception]]$Failures)
    if ($null -eq $Owned) { return }
    Close-P4kxPortalProcess $Owned.process $Owned.started $Owned.channels $Directory $Name $Failures $Owned.proof
}

function Save-P4kxDocumentsController {
    param([hashtable]$Bus, [hashtable]$Worker, [string]$Address, [string]$Directory, [string]$Name)
    $busProof = if ($Bus) { $Bus.proof } else { @{started=$false; exited=$false} }
    $workerProof = if ($Worker) { $Worker.proof } else { @{started=$false; exited=$false} }
    @{schema=1; role=$Name; session=$Address; signal_contract='direct-process-only'; bus=$busProof; worker=$workerProof;
        direct_processes_exited=($busProof.started -eq $true -and $busProof.exited -eq $true -and $workerProof.started -eq $true -and $workerProof.exited -eq $true)} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Directory 'session-controller.json') -Encoding utf8NoBOM
}

function Invoke-P4kxOwnedDocumentsProcesses {
    <#
    .SYNOPSIS
    Holds one direct bus and one direct worker until their bounded output and teardown finish.
    #>
    param([string]$Script, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [hashtable]$Environment, [string]$Config, [int]$Milliseconds = 1800000,
        [int]$ReadinessMilliseconds = 5000, [int]$MaximumBytes = 16777216)
    if ($Milliseconds -le 0 -or $ReadinessMilliseconds -le 0 -or $ReadinessMilliseconds -gt 5000 -or $MaximumBytes -le 0 -or
        $Name -cnotin @('stock', 'api5', 'cleanup-only')) { throw 'Owned session bounds or role are invalid.' }
    if (Test-Path -LiteralPath (Join-Path $Directory 'session-controller.json')) { throw 'Owned session controller requires fresh evidence.' }
    $launch = Get-P4kxDocumentsBusLaunch $Config
    $owned = $Environment.Clone()
    $owned['P4KX_PORTAL_SESSION_ROLE'] = $Name
    $owned['P4KX_PORTAL_PARENT_BUS'] = [string]$env:DBUS_SESSION_BUS_ADDRESS
    $bus = $null; $worker = $null; $address = ''; $result = $null
    $failures = [Collections.Generic.List[Exception]]::new()
    try {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $bus = New-P4kxDocumentsProcess 'dbus-daemon' $launch.arguments $Directory $owned ''
        Start-P4kxDocumentsProcess $bus
        $address = Wait-P4kxDocumentsBusAddress $bus $launch.listener $ReadinessMilliseconds
        if ($address -ceq $owned['P4KX_PORTAL_PARENT_BUS']) { throw 'Owned session reused the parent bus address.' }
        $worker = New-P4kxDocumentsProcess 'pwsh' (@('-NoProfile', '-File', $Script) + $Arguments) $Directory $owned $address
        Start-P4kxDocumentsProcess $worker
        $result = Wait-P4kxDocumentsWorker $bus $worker $address $timer $Milliseconds $MaximumBytes
    }
    catch { $failures.Add($_.Exception) }
    finally {
        Close-P4kxDocumentsProcess $worker $Directory "$Name-worker" $failures
        Close-P4kxDocumentsProcess $bus $Directory "$Name-bus" $failures
        try { Save-P4kxDocumentsController $bus $worker $address $Directory $Name } catch { $failures.Add($_.Exception) }
    }
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Owned session execution or direct-process cleanup failed.', $failures) }
    return $result
}