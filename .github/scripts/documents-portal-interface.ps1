$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-identity.ps1')

function Get-P4kxDocumentsInterfaceEvidence {
    param([string]$Directory, [ValidateSet(4, 5)][int]$Expected)
    $service = 'org.freedesktop.portal.Documents'
    $object = '/org/freedesktop/portal/documents'
    $null = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'get-property', $service, $object, $service, 'version') $Directory 'documents-activation'
    $owner = Get-P4kxPortalNameOwner $service $Directory 'documents-query-owner'
    $version = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'get-property', $owner, $object, $service, 'version') $Directory 'documents-version'
    $xml = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', '--xml-interface', 'introspect', $owner, $object, $service) $Directory 'documents-introspection'
    $observed = ConvertFrom-P4kxDocumentsInterface $version $xml
    if ($observed.interface_version -ne $Expected) { throw 'Native session has a different Documents interface version.' }
    if ($Expected -eq 5) {
        if (-not $observed.get_host_paths) { throw 'Native API5 session lacks GetHostPaths.' }
        $empty = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', $owner, $object, $service, 'GetHostPaths', 'as', '0') $Directory 'documents-empty-host-paths'
        if ($empty -cnotmatch '^a\{say\} 0\s*$') { throw 'Native API5 GetHostPaths did not return the empty typed dictionary.' }
        $observed['empty_host_paths'] = 'success'
    }
    if ((Get-P4kxPortalNameOwner $service $Directory 'documents-query-owner-after') -cne $owner) { throw 'Native Documents owner changed during capability measurement.' }
    $observed['owner'] = $owner
    $observed | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Directory 'documents-interface.json') -Encoding utf8NoBOM
    return $observed
}
