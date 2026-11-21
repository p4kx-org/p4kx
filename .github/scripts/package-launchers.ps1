$ErrorActionPreference = 'Stop'

function Get-P4kxPackageLaunchers {
    param([string]$BinaryDirectory, [string]$Os, [hashtable]$Overrides)
    $names = @('p4kx', 'p4kx-gui', 'p4kx-mcp')
    if ($Overrides -and (($Overrides.Keys | Sort-Object) -join ',') -cne (($names | Sort-Object) -join ',')) {
        throw 'Package launchers must identify all three applications exactly once.'
    }
    $suffix = if ($Os -eq 'windows') { '.exe' } else { '' }
    $launchers = @{}
    foreach ($name in $names) {
        $launcher = if ($Overrides) { $Overrides[$name] } else { @{executable=(Join-Path $BinaryDirectory "$name$suffix"); prefix=@()} }
        if (-not $launcher.executable -or $launcher.executable -isnot [string] -or
            @($launcher.prefix | Where-Object { $_ -isnot [string] }).Count -ne 0) {
            throw "Invalid executable or argument prefix for $name."
        }
        $launchers[$name] = [pscustomobject]@{executable=$launcher.executable; prefix=@($launcher.prefix)}
    }
    return $launchers
}

function Get-P4kxPackageArguments {
    param($Launcher, [string[]]$Arguments)
    return ,(@($Launcher.prefix) + $Arguments)
}
