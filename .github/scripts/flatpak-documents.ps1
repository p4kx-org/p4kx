$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')

function Assert-P4kxFlatpakDocumentMount {
    <#
    .SYNOPSIS
    Requires one kernel mount belonging to the session Documents portal.
    #>
    param([string]$Metadata, [string]$MountPoint)
    $mounts = @(($Metadata | ConvertFrom-Json -Depth 6).filesystems)
    if ($mounts.Count -ne 1 -or $mounts[0].target -cne $MountPoint -or
        $mounts[0].fstype -cne 'fuse.portal' -or $mounts[0].source -cne 'portal') {
        throw "Document portal requires one fuse.portal mount from portal at '$MountPoint'; observed $Metadata"
    }
}

function Export-P4kxFlatpakDocument {
    param([string]$Path, [string]$ApplicationId, [switch]$Writable)
    $access = if ($Writable) { '--allow-write' } else { '--forbid-write' }
    $result = (Invoke-P4kxLinuxTool flatpak @('document-export', '--unique', '--transient', "--app=$ApplicationId", '--allow-read', $access, $Path) | Out-String).Trim()
    $prefix = Join-Path $env:XDG_RUNTIME_DIR 'doc'
    if (-not $result.StartsWith("$prefix/", [StringComparison]::Ordinal) -or
        $result.Substring($prefix.Length + 1) -cnotmatch '\A[A-Za-z0-9_]{1,32}/[^\r\n]+\z') { throw 'Document export did not return an actual portal path.' }
    return $result
}

function Invoke-P4kxFlatpakDocumentCommand {
    param([string[]]$Prefix, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [ValidateSet('None', 'Installation', 'SourceMembership', 'ExistingSource')][string]$Expected = 'None')
    $result = Invoke-P4kxPortalProcess flatpak ($Prefix + $Arguments) $Directory $Name -Milliseconds 120000 -MaximumBytes 2MB
    $result['classification'] = Assert-P4kxFlatpakDocumentOutcome $result $Expected
    return $result
}

function Assert-P4kxFlatpakDocumentOutcome {
    <#
    .SYNOPSIS
    Separates a proved installation refusal from typed identity uncertainty.
    #>
    param($Result, [ValidateSet('None', 'Installation', 'SourceMembership', 'ExistingSource')][string]$Expected)
    if ($Expected -ceq 'None') {
        if ($Result.exit_code -ne 0) { throw 'Flatpak document operation did not succeed.' }
        return 'success'
    }
    if ($Result.exit_code -ne 2) { throw 'Flatpak protected output did not use the invalid-output status.' }
    $known = $Result.stderr.Contains('the tool never writes there', [StringComparison]::Ordinal)
    $codes = @('source-membership', 'source-overwrite', 'target-identity', 'source-identity')
    $observed = @($codes | Where-Object { $Result.stderr.Contains("[p4kx.write.$_]", [StringComparison]::Ordinal) })
    if ($Expected -ceq 'Installation' -and $known -and $observed.Count -eq 0) { return 'installation' }
    if ($known -or $observed.Count -ne 1) { throw 'Protected output has an ambiguous or incorrect refusal category.' }
    if ($Expected -ceq 'SourceMembership' -and $observed[0] -ceq 'source-membership') { return $observed[0] }
    if ($Expected -ceq 'ExistingSource' -and $observed[0] -cin @('source-overwrite', 'target-identity')) { return $observed[0] }
    throw 'Protected output failed for a different typed reason.'
}

function Get-P4kxFlatpakDocumentPrefix {
    param($Platform, [string]$DataDirectory)
    $settings = Get-P4kxLinuxPackaging
    return @('run', '--user', "--arch=$($Platform.unix_arch)", '--command=p4kx',
        "--env=P4KX_FOLDERS_ROOT=$DataDirectory/portal-smoke", '--env=P4KX_GAME_CHECK=off',
        '--env=P4KX_PROTECT_FOLDERS=', $settings.application_id)
}
