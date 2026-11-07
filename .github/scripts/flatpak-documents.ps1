$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'package-process.ps1')

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
        $result.Substring($prefix.Length + 1) -cnotmatch '^[0-9a-f]+/[^\r\n]+$') { throw 'Document export did not return an actual portal path.' }
    return $result
}

function Invoke-P4kxFlatpakDocumentCommand {
    param([string[]]$Prefix, [string[]]$Arguments, [string]$Directory, [string]$Log, [string]$ProtectedFolder, [switch]$Refused)
    $process = New-P4kxPackageProcess 'flatpak' ($Prefix + $Arguments) $Directory
    if ($ProtectedFolder) { $process.StartInfo.Environment['P4KX_PROTECT_FOLDERS'] = $ProtectedFolder }
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(120000)) { $process.Kill($true); $process.WaitForExit(); throw 'Flatpak document operation timed out.' }
        $text = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult()
        $text | Set-Content -LiteralPath $Log -Encoding utf8NoBOM
        if ($Refused) {
            if ($process.ExitCode -eq 0 -or $text -cnotmatch 'the tool never writes there') { throw 'Game-folder portal alias was not refused by the installation guard.' }
        }
        elseif ($process.ExitCode -ne 0) { throw "Flatpak document operation exited $($process.ExitCode); see $Log" }
        return $text
    }
    finally { $process.Dispose() }
}
