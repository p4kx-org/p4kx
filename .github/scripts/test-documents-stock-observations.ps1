<#
.SYNOPSIS
Separates current service observations from historical teardown ownership with pure mocks.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-identity.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Fixture root must be fresh.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$old = @{service='org.freedesktop.portal.Documents'; owner=':1.4'; pid=100; start='100'; sha256=('a' * 64)}
$script:ServicePresent = $false
@{session=$env:DBUS_SESSION_BUS_ADDRESS; services=@($old)} | ConvertTo-Json -Depth 6 |
    Set-Content -LiteralPath (Join-Path $Directory 'session-services-initial.json') -Encoding utf8NoBOM
function Invoke-P4kxPortalMeasurement {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name)
    if ($Executable -cne 'busctl' -or $Arguments[-3] -cne 'NameHasOwner' -or $Arguments.Count -ne 9) {
        throw 'Unexpected measurement in the pure observation test.'
    }
    if ($script:ServicePresent -and $Arguments[-1] -ceq 'org.freedesktop.portal.Documents') { return 'b true' }
    return 'b false'
}
function Get-P4kxPortalUserId { return 1000 }
function Get-P4kxPortalServiceIdentity {
    if (-not $script:ServicePresent) { throw 'Absent services must not be opened or queried.' }
    return @{service='org.freedesktop.portal.Documents'; owner=':1.8'; pid=200; start='200'; sha256=('a' * 64)}
}
Save-P4kxDocumentsSessionIdentity $Directory $null 'final'
$saved = Get-Content -LiteralPath (Join-Path $Directory 'session-services-final.json') -Raw | ConvertFrom-Json
if ($null -eq $saved.PSObject.Properties['observed_services'] -or @($saved.observed_services).Count -ne 0) {
    throw 'An absent current service was promoted from the historical teardown ledger.'
}
if (@($saved.services).Count -ne 1 -or $saved.services[0].owner -cne ':1.4') {
    throw 'Historical teardown ownership was lost while retaining the empty current observation.'
}
$restarted = Join-Path $Directory 'restarted'
New-Item -ItemType Directory -Path $restarted | Out-Null
@{session=$env:DBUS_SESSION_BUS_ADDRESS; services=@($old)} | ConvertTo-Json -Depth 6 |
    Set-Content -LiteralPath (Join-Path $restarted 'session-services-initial.json') -Encoding utf8NoBOM
$script:ServicePresent = $true
$build = @{binaries=@(@{name='xdg-document-portal'; path='/synthetic/portal'; sha256=('a' * 64)})}
Save-P4kxDocumentsSessionIdentity $restarted $build 'final'
$saved = Get-Content -LiteralPath (Join-Path $restarted 'session-services-final.json') -Raw | ConvertFrom-Json
if (@($saved.observed_services).Count -ne 1 -or $saved.observed_services[0].owner -cne ':1.8' -or
    @($saved.services).Count -ne 2 -or (@($saved.services.owner | Sort-Object) -join ',') -cne ':1.4,:1.8') {
    throw 'Current restarted owner and historical cleanup owner were conflated.'
}
'PASS: disappeared and restarted service observations remain separate from historical cleanup ownership; no native calls.'
