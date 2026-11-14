$ErrorActionPreference = 'Stop'

function ConvertFrom-P4kxPlatformContract {
    param([Parameter(Mandatory)][string]$Json)
    $contract = $Json | ConvertFrom-Json
    $packageNames = @('msi', 'portable-zip', 'tar-gz', 'app-image', 'deb', 'flatpak', 'dmg')
    if ($contract.version -ne 1 -or $contract.targets.Count -ne 5 -or @($contract.packages.PSObject.Properties).Count -ne $packageNames.Count) {
        throw 'Platform contract must contain version 1, five native targets and seven package formats.'
    }
    $pairs = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    foreach ($row in $contract.targets) {
        if ($row.os -cnotin @('windows', 'linux', 'macos') -or $row.arch -cnotin @('amd64', 'arm64')) {
            throw 'Unsupported OS or CPU in platform contract.'
        }
        if (-not $pairs.Add("$($row.os)/$($row.arch)")) { throw 'Duplicate native platform.' }
        $rustArch = if ($row.arch -eq 'amd64') { 'x86_64' } else { 'aarch64' }
        $abi = switch -CaseSensitive ($row.os) {
            'windows' { 'pc-windows-msvc' }
            'linux' { 'unknown-linux-gnu' }
            'macos' { 'apple-darwin' }
        }
        if ($row.target -cne "$rustArch-$abi" -or $row.runner -cnotmatch '^[A-Za-z0-9_.-]+$') {
            throw 'Platform target/runner identity mismatch.'
        }
        $expected = switch -CaseSensitive ($row.os) {
            'windows' { @('msi', 'portable-zip') }
            'linux' { @('tar-gz', 'app-image', 'deb', 'flatpak') }
            'macos' { 'dmg' }
        }
        if ((($row.packages | Sort-Object) -join ',') -cne (($expected | Sort-Object) -join ',')) {
            throw "Required package formats do not match $($row.target)."
        }
        Assert-P4kxPlatformIdentity $row
    }
    $suffixes = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($property in $contract.packages.PSObject.Properties) {
        if ($property.Name -cnotin $packageNames -or
            $property.Value -cnotmatch '^[.-][A-Za-z0-9.-]+$' -or
            -not $suffixes.Add($property.Value)) {
            throw 'Unknown package format or unsafe asset suffix.'
        }
    }
    return $contract
}

function Assert-P4kxPlatformIdentity {
    param([Parameter(Mandatory)]$Platform)
    $arm = $Platform.arch -ceq 'arm64'
    $runnerArchitecture = if ($arm) { 'ARM64' } else { 'X64' }
    if ($Platform.runner_arch -cne $runnerArchitecture) {
        throw 'Runner CPU does not match the target.'
    }
    if ($Platform.os -ceq 'macos') {
        if (-not $arm -or $Platform.runner_os -cne 'macOS' -or
            $null -ne $Platform.pe_machine -or $null -ne $Platform.elf_machine -or
            $null -ne $Platform.msi_arch -or $null -ne $Platform.deb_arch -or $null -ne $Platform.unix_arch -or
            $Platform.mach_cpu_type -ne 0x0100000c -or $Platform.deployment_target -cne '26.0') {
            throw 'macOS target architecture or deployment identity mismatch.'
        }
        return
    }
    if ($null -ne $Platform.mach_cpu_type -or $null -ne $Platform.deployment_target) {
        throw 'Mach-O identity belongs to a macOS target.'
    }
    if ($Platform.os -ceq 'windows') {
        $msi = if ($arm) { 'arm64' } else { 'x64' }
        $machine = if ($arm) { 0xaa64 } else { 0x8664 }
        if ($Platform.runner_os -cne 'Windows' -or $Platform.msi_arch -cne $msi -or $Platform.pe_machine -ne $machine -or
            $null -ne $Platform.elf_machine -or $null -ne $Platform.deb_arch -or $null -ne $Platform.unix_arch) {
            throw 'Windows target architecture identity mismatch.'
        }
        return
    }
    $deb = if ($arm) { 'arm64' } else { 'amd64' }
    $unix = if ($arm) { 'aarch64' } else { 'x86_64' }
    $machine = if ($arm) { 183 } else { 62 }
    if ($Platform.runner_os -cne 'Linux' -or $Platform.deb_arch -cne $deb -or $Platform.unix_arch -cne $unix -or
        $Platform.elf_machine -ne $machine -or $null -ne $Platform.pe_machine -or $null -ne $Platform.msi_arch) {
        throw 'Linux target architecture identity mismatch.'
    }
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
    $os = if ($IsWindows) { 'windows' } elseif ($IsLinux) { 'linux' } elseif ($IsMacOS) { 'macos' } else { throw 'Unsupported native OS.' }
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
    if ($Platform.os -ceq 'macos' -and [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -ne 'Arm64') {
        throw 'macOS evidence requires a native ARM64 process.'
    }
    if (($env:RUNNER_OS -and $env:RUNNER_OS -cne $Platform.runner_os) -or
        ($env:RUNNER_ARCH -and $env:RUNNER_ARCH -cne $Platform.runner_arch)) {
        throw 'GitHub runner identity differs from the native target.'
    }
}
