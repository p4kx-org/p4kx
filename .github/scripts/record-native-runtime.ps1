param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][string]$OutputPath,
    [System.Collections.IDictionary]$SmokeOutcomes
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row
$metadata = Get-Content -LiteralPath $MetadataPath -Raw | ConvertFrom-Json
if ($metadata.target -cne $Target) { throw 'Native test archive target changed.' }
if ($env:GITHUB_SERVER_URL -cne 'https://github.com' -or
    $env:GITHUB_REPOSITORY -cne 'p4kx-org/p4kx' -or $env:GITHUB_RUN_ID -cnotmatch '^[0-9]+$' -or
    $metadata.revision -cne $env:GITHUB_SHA) {
    throw 'Runtime evidence requires the matching repository CI run and source commit.'
}
$runtime = if ($null -ne $SmokeOutcomes) {
    $SmokeOutcomes
} else {
    $checks = [ordered]@{
        cli_list = $env:CLI_LIST_OUTCOME
        cli_extract = $env:CLI_EXTRACT_OUTCOME
        cli_convert = $env:CLI_CONVERT_OUTCOME
        mcp_tools = $env:MCP_TOOLS_OUTCOME
        mcp_read = $env:MCP_READ_OUTCOME
        gui_frames = $env:GUI_FRAMES_OUTCOME
    }
    if ($row.os -ceq 'macos') {
        $checks.metal_adapter = $env:METAL_ADAPTER_OUTCOME
    } else {
        $checks.software_adapter = $env:SOFTWARE_ADAPTER_OUTCOME
    }
    $checks
}
if ($row.os -ceq 'macos') {
    $metadata | Add-Member -NotePropertyName runtime_host -NotePropertyValue (Get-P4kxMacosHostEvidence -Platform $row)
}
foreach ($name in (Get-P4kxRuntimeChecks -Platform $row)) {
    if ($runtime[$name] -cne 'success') { throw "Required native smoke step '$name' was missing, skipped or unsuccessful." }
}
$metadata | Add-Member -NotePropertyName scope -NotePropertyValue 'native-synthetic'
$metadata | Add-Member -NotePropertyName runtime -NotePropertyValue $runtime
$metadata | Add-Member -NotePropertyName device_checks -NotePropertyValue ([ordered]@{
    physical_audio = 'not_checked'
    clipboard = 'not_checked'
    file_dialogs = 'not_checked'
    hardware_gpu = 'not_checked'
})
$metadata | Add-Member -NotePropertyName workflow_run -NotePropertyValue "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
$metadata | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
Write-Output "Recorded successful native synthetic runtime checks for $Target; physical device checks remain explicit."
