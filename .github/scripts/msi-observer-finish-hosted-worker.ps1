param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$CollectionCutoffUtc
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-finish-wait-host.ps1')
Invoke-P4kxMsiHostedFinishWait -Directory $Directory -CollectionCutoffUtc $CollectionCutoffUtc
