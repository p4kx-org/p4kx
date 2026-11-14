$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')

function ConvertFrom-P4kxLinuxPackaging {
    param([Parameter(Mandatory)][string]$Json)
    $settings = $Json | ConvertFrom-Json
    if ($settings.schema -ne 1 -or $settings.application_id -cnotmatch '^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*){2,}$' -or
        $settings.maximum_glibc_requirement -cne '2.39' -or $settings.flatpak.runtime_branch -cne '25.08') {
        throw 'Linux package identity or ABI baseline is invalid.'
    }
    $unique = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($permission in $settings.flatpak.finish_args) {
        if (-not $unique.Add($permission)) { throw 'Duplicate Flatpak capability.' }
        if ($permission -match '^--filesystem=' -and $permission -cne '--filesystem=~/Games/star-citizen:ro') {
            throw 'Flatpak static filesystem access exceeds the read-only standard prefix.'
        }
        if ($permission -match '^--(socket=(session|system)-bus|device=all|talk-name=org\.freedesktop\.Flatpak)') {
            throw 'Flatpak package requests an unrestricted host capability.'
        }
    }
    return $settings
}

function Get-P4kxLinuxPackaging {
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    return ConvertFrom-P4kxLinuxPackaging (Get-Content -LiteralPath (Join-Path $root '.config/linux-packaging.json') -Raw)
}

function Get-P4kxLinuxPlatform {
    param([Parameter(Mandatory)][string]$Target)
    $row = Get-P4kxPlatform -Target $Target
    if ($row.os -cne 'linux' -or -not $row.unix_arch -or -not $row.deb_arch -or -not $row.elf_machine) {
        throw 'Linux packaging requires a complete Linux row from the native platform contract.'
    }
    return $row
}

function Invoke-P4kxLinuxTool {
    param([Parameter(Mandatory)][string]$Executable, [string[]]$Arguments)
    $output = & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        foreach ($line in $output) { Write-Host $line }
        throw "Linux package tool failed ($LASTEXITCODE): $Executable"
    }
    return $output
}
