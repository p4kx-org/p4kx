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
    $null = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', $owner, $object, $service, 'GetMountPoint') $Directory 'documents-mount-point' -MaximumBytes 16384
    if ((Get-P4kxPortalNameOwner $service $Directory 'documents-query-owner-after') -cne $owner) { throw 'Native Documents owner changed during capability measurement.' }
    $observed['owner'] = $owner
    $observed | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Directory 'documents-interface.json') -Encoding utf8NoBOM
    return $observed
}

function ConvertFrom-P4kxSandboxRuntime {
    <#
    .SYNOPSIS
    Validates one measured absolute sandbox runtime path without rewriting it.
    #>
    param([string]$Output)
    if ([Text.Encoding]::UTF8.GetByteCount($Output) -gt 16384) { throw 'Sandbox runtime output exceeds its limit.' }
    $path = if ($Output.EndsWith("`r`n", [StringComparison]::Ordinal)) { $Output.Substring(0, $Output.Length - 2) }
        elseif ($Output.EndsWith("`n", [StringComparison]::Ordinal)) { $Output.Substring(0, $Output.Length - 1) } else { $Output }
    if (-not $path.StartsWith('/', [StringComparison]::Ordinal) -or $path -ceq '/' -or $path -match '[\r\n\0]' -or
        $path.Contains('//') -or $path.EndsWith('/', [StringComparison]::Ordinal) -or
        @($path -split '/' | Where-Object { $_ -ceq '.' -or $_ -ceq '..' }).Count -ne 0) { throw 'Sandbox runtime output is not one absolute directory path.' }
    return $path
}

function Save-P4kxSandboxMountEvidence {
    <#
    .SYNOPSIS
    Measures the installed runtime's actual document alias with bounded native probes.
    #>
    param($Platform, [string]$Directory)
    $application = (Get-P4kxLinuxPackaging).application_id
    $sandbox = @('run', '--user', "--arch=$($Platform.unix_arch)")
    $null = Invoke-P4kxPortalMeasurement flatpak @('--version') $Directory 'host-flatpak-version' -MaximumBytes 1024
    $runtime = ConvertFrom-P4kxSandboxRuntime (Invoke-P4kxPortalMeasurement flatpak ($sandbox + @('--command=printenv', $application, 'XDG_RUNTIME_DIR')) $Directory 'sandbox-runtime-directory' -MaximumBytes 16384)
    $null = Invoke-P4kxPortalMeasurement flatpak ($sandbox + @('--command=readlink', $application, '-e', '--', "$runtime/doc")) $Directory 'sandbox-doc-canonical' -MaximumBytes 16384
    $mounts = Invoke-P4kxPortalMeasurement flatpak ($sandbox + @('--command=head', $application, '--bytes=524289', '--', '/proc/self/mountinfo')) $Directory 'sandbox-mountinfo' -MaximumBytes 524289
    if ([Text.Encoding]::UTF8.GetByteCount($mounts) -gt 524288) { throw 'Sandbox mount inventory exceeds the product inventory bound.' }
}
