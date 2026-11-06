$ErrorActionPreference = 'Stop'

function Assert-P4kxBinaryEvidence {
    param([Parameter(Mandatory)]$Metadata, [Parameter(Mandatory)]$Platform)
    $required = @('p4kx', 'p4kx-gui', 'p4kx-mcp')
    $names = @($Metadata.binaries.name | Sort-Object)
    if (($names -join ',') -cne (($required | Sort-Object) -join ',')) {
        throw 'Native evidence must identify every application exactly once.'
    }
    $machine = if ($Platform.os -eq 'windows') { $Platform.pe_machine } else { $Platform.elf_machine }
    foreach ($binary in $Metadata.binaries) {
        if ($binary.machine -ne $machine -or $binary.sha256 -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Native application machine or digest is invalid.'
        }
    }
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
        $Metadata.revision -cne $Revision -or $Metadata.compiler -cne $Compiler -or
        $Metadata.nextest -cne $Nextest -or $Metadata.compiler_host -cne $Platform.target -or
        $Metadata.nextest_host -cne $Platform.target -or
        $Metadata.archive_sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        ($ArchiveHash -and $Metadata.archive_sha256 -cne $ArchiveHash)) {
        throw 'Native archive source/target/tool identity or digest mismatch.'
    }
    Assert-P4kxBinaryEvidence -Metadata $Metadata -Platform $Platform
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
    foreach ($name in @('cli_list', 'cli_extract', 'cli_convert', 'mcp_tools', 'mcp_read', 'gui_frames', 'software_adapter')) {
        if ($Metadata.runtime.$name -cne 'success') { throw "Required native runtime check '$name' did not succeed." }
    }
    foreach ($name in @('physical_audio', 'clipboard', 'file_dialogs', 'hardware_gpu')) {
        if ($Metadata.device_checks.$name -cnotin @('not_checked', 'success', 'unavailable')) {
            throw "Physical device verification state '$name' is missing or invalid."
        }
    }
    if ($Metadata.revision -cne $Revision -or $Metadata.target -cne $Platform.target) {
        throw 'Runtime evidence source/target mismatch.'
    }
}
