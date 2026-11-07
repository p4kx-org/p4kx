function New-P4kxTestEvidence {
    param(
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Revision,
        [Parameter(Mandatory)][string]$Compiler,
        [Parameter(Mandatory)][string]$Nextest
    )
    $machine = if ($Platform.os -eq 'windows') { $Platform.pe_machine } else { $Platform.elf_machine }
    return [pscustomobject]@{
        schema = 1
        version = $Version
        target = $Platform.target
        os = $Platform.os
        arch = $Platform.arch
        revision = $Revision
        compiler = $Compiler
        nextest = $Nextest
        compiler_host = $Platform.target
        nextest_host = $Platform.target
        archive_sha256 = 'b' * 64
        binaries = @(foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
            [pscustomobject]@{ name = $name; machine = $machine; sha256 = 'c' * 64 }
        })
        scope = 'native-synthetic'
        workflow_run = 'https://github.com/p4kx-org/p4kx/actions/runs/1'
        runtime = [pscustomobject]@{
            cli_list = 'success'; cli_extract = 'success'; cli_convert = 'success'
            mcp_tools = 'success'; mcp_read = 'success'; gui_frames = 'success'; software_adapter = 'success'
        }
        device_checks = [pscustomobject]@{
            physical_audio = 'not_checked'; clipboard = 'not_checked'
            file_dialogs = 'not_checked'; hardware_gpu = 'not_checked'
        }
    }
}
