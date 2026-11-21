$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function ConvertFrom-P4kxFlatpakMetadata {
    param([string[]]$Lines)
    $sections = @{}
    $current = $null
    foreach ($line in $Lines) {
        if (-not $line -or $line.StartsWith('#')) { continue }
        if ($line -cmatch '^\[([^\]]+)\]$') {
            $current = $Matches[1]
            if ($sections.ContainsKey($current)) { throw 'Duplicate Flatpak metadata section.' }
            $sections[$current] = @{}
        }
        elseif ($current -and $line -cmatch '^([^=]+)=(.*)$') {
            if ($sections[$current].ContainsKey($Matches[1])) { throw 'Duplicate Flatpak metadata value.' }
            $sections[$current][$Matches[1]] = $Matches[2]
        }
        else { throw 'Malformed Flatpak metadata.' }
    }
    return $sections
}

function Assert-P4kxFlatpakMetadata {
    param([string[]]$Lines, $Platform)
    $settings = Get-P4kxLinuxPackaging
    $metadata = ConvertFrom-P4kxFlatpakMetadata $Lines
    $runtime = "$($settings.flatpak.runtime)/$($Platform.unix_arch)/$($settings.flatpak.runtime_branch)"
    if ($metadata.Application.name -cne $settings.application_id -or $metadata.Application.runtime -cne $runtime -or
        $metadata.Application.command -cne 'p4kx-gui') { throw 'Flatpak metadata application/runtime/command differs.' }
    $expected = @{}
    $categories = @{share='shared'; socket='sockets'; device='devices'; filesystem='filesystems'}
    foreach ($permission in $settings.flatpak.finish_args) {
        if ($permission -cnotmatch '^--(share|socket|device|filesystem)=(.+)$') { throw 'Flatpak permission has no metadata validator.' }
        $category = $categories[$Matches[1]]
        $expected[$category] = @($expected[$category]) + $Matches[2]
        if ($permission -ceq '--socket=fallback-x11') {
            # Flatpak serializes fallback-X11 with its X11 socket bit.
            $expected[$category] += 'x11'
        }
    }
    if ((($metadata.Context.Keys | Sort-Object) -join ',') -cne (($expected.Keys | Sort-Object) -join ',')) { throw 'Flatpak has an undeclared capability category.' }
    foreach ($key in $expected.Keys) {
        $actual = @($metadata.Context[$key] -split ';' | Where-Object { $_ } | Sort-Object)
        $required = @($expected[$key] | Where-Object { $_ } | Sort-Object)
        if (($actual -join ';') -cne ($required -join ';')) {
            throw "Flatpak effective $key permissions differ: expected '$($required -join ';')', observed '$($actual -join ';')'."
        }
    }
    foreach ($section in @('Session Bus Policy', 'System Bus Policy')) {
        if ($metadata.ContainsKey($section) -and $metadata[$section].Count -ne 0) { throw 'Flatpak requests additional bus policy.' }
    }
}
