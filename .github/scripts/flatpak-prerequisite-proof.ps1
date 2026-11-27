$ErrorActionPreference = 'Stop'

function Get-P4kxFlatpakApi4Version {
    <#
    .SYNOPSIS
    Requires a typed unsigned-compatible API4 observation without string or boolean coercion.
    #>
    param($Value)
    if (($Value -isnot [int] -and $Value -isnot [long] -and $Value -isnot [uint32]) -or $Value -ne 4) {
        throw 'API4 negative proof requires the measured integer interface version 4.'
    }
    return [uint32]$Value
}

function Assert-P4kxFlatpakApi4Evidence {
    <#
    .SYNOPSIS
    Binds version-specific app refusals to the recorded stock-session capability owner.
    #>
    param($Negative, $StockSession)
    $version = Get-P4kxFlatpakApi4Version $Negative.interface.interface_version
    $proof = $Negative.prerequisite
    if (($proof.schema -isnot [int] -and $proof.schema -isnot [long]) -or $proof.schema -ne 1 -or
        $proof.outcome -cne 'expected-refusal' -or $proof.runtime_pass -isnot [bool] -or $proof.runtime_pass -or
        $Negative.session -isnot [string] -or -not $Negative.session -or $Negative.session -cne $StockSession.session -or
        $StockSession.initial_session -cne $StockSession.session) {
        throw 'API4 negative proof is unversioned, incomplete or bound to a different session.'
    }
    $owner = $Negative.interface.owner
    if ($owner -isnot [string] -or $owner -cnotmatch '^:[0-9]+\.[0-9]+$') {
        throw 'API4 capability owner was not observed.'
    }
    foreach ($services in @(@($StockSession.initial_services), @($StockSession.final_services))) {
        $documents = @($services | Where-Object service -CEQ 'org.freedesktop.portal.Documents')
        if ($documents.Count -ne 1 -or $documents[0].owner -cne $owner) {
            throw 'API4 capability owner differs from the retained stock-session service identities.'
        }
    }
    $applications = @($proof.applications)
    if ((@($applications.application | Sort-Object) -join ',') -cne 'p4kx,p4kx-gui,p4kx-mcp') {
        throw 'API4 negative does not cover all three real apps.'
    }
    foreach ($application in $applications) {
        $observed = Get-P4kxFlatpakApi4Version $application.interface_version
        if (($application.exit_code -isnot [int] -and $application.exit_code -isnot [long]) -or
            $application.exit_code -ne 1 -or $observed -ne $version -or $application.refusal_reason -cne 'unsupported-interface' -or
            $application.prerequisite -cne 'expected-refusal' -or $application.help -cne 'success' -or
            $application.version -cne 'success') {
            throw 'API4 app evidence lacks a matching unsupported-interface refusal, help or version result.'
        }
    }
}
