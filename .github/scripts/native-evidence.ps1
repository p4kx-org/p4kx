$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macho-image.ps1')
. (Join-Path $PSScriptRoot 'macos-host-evidence.ps1')

function Assert-P4kxBinaryEvidence {
    param([Parameter(Mandatory)]$Metadata, [Parameter(Mandatory)]$Platform)
    $required = @('p4kx', 'p4kx-gui', 'p4kx-mcp')
    $names = @($Metadata.binaries.name | Sort-Object)
    if (($names -join ',') -cne (($required | Sort-Object) -join ',')) {
        throw 'Native evidence must identify every application exactly once.'
    }
    $machine = Get-P4kxNativeMachine -Platform $Platform
    foreach ($binary in $Metadata.binaries) {
        if ($binary.machine -ne $machine -or $binary.sha256 -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Native application machine or digest is invalid.'
        }
        if ($Platform.os -ceq 'macos') { Assert-P4kxMachOIdentity -Image $binary -Platform $Platform }
    }
}

function Get-P4kxNativeMachine {
    param([Parameter(Mandatory)]$Platform)
    switch -CaseSensitive ($Platform.os) {
        windows { return $Platform.pe_machine }
        linux { return $Platform.elf_machine }
        macos { return $Platform.mach_cpu_type }
        default { throw 'Unsupported native image OS.' }
    }
}

function Get-P4kxRuntimeChecks {
    param([Parameter(Mandatory)]$Platform)
    $adapter = if ($Platform.os -ceq 'macos') { 'metal_adapter' } else { 'software_adapter' }
    return @('cli_list', 'cli_extract', 'cli_convert', 'mcp_tools', 'mcp_read', 'gui_frames', $adapter)
}

function Assert-P4kxArchiveEvidence {
    param(
        [Parameter(Mandatory)]$Metadata,
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Revision,
        [Parameter(Mandatory)][string]$Compiler,
        [Parameter(Mandatory)][string]$Nextest,
        [string]$ArchiveHash
    )
    if ($Metadata.schema -ne 1 -or $Metadata.target -cne $Platform.target -or
        $Metadata.os -cne $Platform.os -or $Metadata.arch -cne $Platform.arch -or
        $Metadata.revision -cne $Revision -or $Metadata.compiler -cne $Compiler -or
        $Metadata.nextest -cne $Nextest -or $Metadata.compiler_host -cne $Platform.target -or
        $Metadata.nextest_host -cne $Platform.target -or
        $Metadata.archive_sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        ($ArchiveHash -and $Metadata.archive_sha256 -cne $ArchiveHash)) {
        throw 'Native archive source/target/tool identity or digest mismatch.'
    }
    Assert-P4kxBinaryEvidence -Metadata $Metadata -Platform $Platform
    if ($Platform.os -ceq 'macos') { Assert-P4kxMacosHostEvidence -HostEvidence $Metadata.build_host -Platform $Platform }
}

function Assert-P4kxRuntimeEvidence {
    param(
        [Parameter(Mandatory)]$Metadata,
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Revision
    )
    if ($Metadata.version -cne $Version -or $Metadata.scope -cne 'native-synthetic') {
        throw 'Native runtime evidence has a different version or scope.'
    }
    if ($Metadata.workflow_run -cnotmatch '^https://github\.com/p4kx-org/p4kx/actions/runs/[0-9]+$') {
        throw 'Native runtime evidence has no owning repository CI run.'
    }
    foreach ($name in (Get-P4kxRuntimeChecks -Platform $Platform)) {
        if ($Metadata.runtime.$name -cne 'success') { throw "Required native runtime check '$name' did not succeed." }
    }
    foreach ($name in @('physical_audio', 'clipboard', 'file_dialogs', 'hardware_gpu')) {
        if ($Metadata.device_checks.$name -cnotin @('not_checked', 'success', 'unavailable')) {
            throw "Physical device verification state '$name' is missing or invalid."
        }
    }
    if ($Platform.os -ceq 'macos') { Assert-P4kxMacosHostEvidence -HostEvidence $Metadata.runtime_host -Platform $Platform }
    if ($Metadata.revision -cne $Revision -or $Metadata.target -cne $Platform.target) {
        throw 'Runtime evidence source/target mismatch.'
    }
}
