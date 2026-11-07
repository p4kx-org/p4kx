$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-transport-contract.ps1')

<#
.SYNOPSIS
Checks actual compiled helper transport without installing the synthetic MSI.
#>
function Test-P4kxMsiPathAuthoring {
    param($Metadata)
    Assert-P4kxMsiTransport $Metadata
    foreach ($name in @('P4kxPathPrepare','P4kxPathApply','P4kxPathCommit','P4kxPathRollback')) {
        $actions = @($Metadata.custom_actions | Where-Object { $_[0] -ceq $name })
        if ($actions.Count -ne 1 -or $actions[0][3] -notmatch '\[P4kxPathCapturedBinding\]' -or
            $actions[0][3] -match '\[P4KX_PATH_BINDING\]') { throw 'An MSI phase does not use the one captured private binding.' }
    }
}
