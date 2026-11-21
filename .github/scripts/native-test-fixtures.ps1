function New-P4kxTestEvidence {
    param(
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Revision,
        [Parameter(Mandatory)][string]$Compiler,
        [Parameter(Mandatory)][string]$Nextest
    )
    $machine = switch -CaseSensitive ($Platform.os) {
        windows { $Platform.pe_machine }
        linux { $Platform.elf_machine }
        macos { $Platform.mach_cpu_type }
    }
    $result = [pscustomobject]@{
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
    if ($Platform.os -ceq 'macos') {
        $hostFixture = @{ os_version = '26.1.0'; os_build = '25B78'; architecture = 'arm64' }
        $result | Add-Member -NotePropertyName build_host -NotePropertyValue $hostFixture
        $result | Add-Member -NotePropertyName runtime_host -NotePropertyValue ($hostFixture.Clone())
        $result.runtime.PSObject.Properties.Remove('software_adapter')
        $result.runtime | Add-Member -NotePropertyName metal_adapter -NotePropertyValue 'success'
        foreach ($binary in $result.binaries) {
            $binary | Add-Member -NotePropertyMembers @{
                cpu_subtype = 0
                file_type = 2
                platform = 1
                min_os = "$($Platform.deployment_target).0"
                sdk = '26.0.0'
            }
        }
    }
    return $result
}
