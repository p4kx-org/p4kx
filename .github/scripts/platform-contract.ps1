$ErrorActionPreference = 'Stop'

function ConvertFrom-P4kxPlatformContract {
    param([Parameter(Mandatory)][string]$Json)
    $contract = $Json | ConvertFrom-Json
    if ($contract.version -ne 1 -or $contract.targets.Count -ne 4 -or @($contract.packages.PSObject.Properties).Count -ne 6) {
        throw 'Platform contract must contain version 1, four native targets and six package formats.'
    }
    $pairs = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    foreach ($row in $contract.targets) {
        if ($row.os -cnotin @('windows', 'linux') -or $row.arch -cnotin @('amd64', 'arm64')) {
            throw 'Unsupported OS or CPU in platform contract.'
        }
        if (-not $pairs.Add("$($row.os)/$($row.arch)")) { throw 'Duplicate native platform.' }
        $rustArch = if ($row.arch -eq 'amd64') { 'x86_64' } else { 'aarch64' }
        $abi = if ($row.os -eq 'windows') { 'pc-windows-msvc' } else { 'unknown-linux-gnu' }
        if ($row.target -cne "$rustArch-$abi" -or $row.runner -cnotmatch '^[A-Za-z0-9_.-]+$') {
            throw 'Platform target/runner identity mismatch.'
        }
        $expected = if ($row.os -eq 'windows') { @('msi', 'portable-zip') } else { @('tar-gz', 'app-image', 'deb', 'flatpak') }
        if ((($row.packages | Sort-Object) -join ',') -cne (($expected | Sort-Object) -join ',')) {
            throw "Required package formats do not match $($row.target)."
        }
    }
    foreach ($property in $contract.packages.PSObject.Properties) {
        if ($property.Name -cnotin @('msi', 'portable-zip', 'tar-gz', 'app-image', 'deb', 'flatpak') -or
            $property.Value -cnotmatch '^[.-][A-Za-z0-9.-]+$') {
            throw 'Unknown package format or unsafe asset suffix.'
        }
    }
    return $contract
}

function Get-P4kxPlatformContract {
    $root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $json = Get-Content -LiteralPath (Join-Path $root '.config/platforms.json') -Raw
    return ConvertFrom-P4kxPlatformContract -Json $json
}

function Get-P4kxPlatform {
    param([Parameter(Mandatory)][string]$Target)
    $contract = Get-P4kxPlatformContract
    $rows = @($contract.targets | Where-Object target -CEQ $Target)
    if ($rows.Count -ne 1) { throw "Unknown native target '$Target'." }
    return $rows[0]
}

function Get-P4kxAssetName {
    param(
        [Parameter(Mandatory)][string]$Target,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Package
    )
    $contract = Get-P4kxPlatformContract
    $row = Get-P4kxPlatform -Target $Target
    if ($Package -cnotin $row.packages -or $Version -cnotmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$') {
        throw 'Unsupported package or invalid release version.'
    }
    $suffix = $contract.packages.$Package
    return "P4KX-$Version-$Target$suffix"
}

function Get-P4kxCurrentPlatform {
    $os = if ($IsWindows) { 'windows' } elseif ($IsLinux) { 'linux' } else { throw 'Unsupported native OS.' }
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    $contract = Get-P4kxPlatformContract
    $rows = @($contract.targets | Where-Object { $_.os -ceq $os -and $_.runner_arch -eq $architecture })
    if ($rows.Count -ne 1) { throw 'No unique native target matches the running OS/CPU.' }
    return $rows[0]
}

function Assert-P4kxNativeHost {
    param([Parameter(Mandatory)]$Platform)
    $current = Get-P4kxCurrentPlatform
    if ($Platform.target -cne $current.target) { throw "Target $($Platform.target) does not match native OS/CPU $($current.target)." }
}
