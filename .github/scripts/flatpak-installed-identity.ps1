$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Get-P4kxLiveFlatpakIdentity {
    param($Platform, $Producer)
    $settings = Get-P4kxLinuxPackaging
    $arguments = @('info', '--user', "--arch=$($Platform.unix_arch)")
    $reference = (Invoke-P4kxLinuxTool flatpak ($arguments + @('--show-ref', $settings.application_id)) | Out-String).Trim()
    $runtime = (Invoke-P4kxLinuxTool flatpak ($arguments + @('--show-runtime', $settings.application_id)) | Out-String).Trim()
    $appCommit = (Invoke-P4kxLinuxTool flatpak ($arguments + @('--show-commit', $settings.application_id)) | Out-String).Trim()
    $runtimeCommit = (Invoke-P4kxLinuxTool flatpak ($arguments + @('--show-commit', $Producer.flatpak.runtime_ref)) | Out-String).Trim()
    if ($reference -cne $Producer.flatpak.app_ref -or $runtime -cne ($Producer.flatpak.runtime_ref -creplace '^runtime/', '') -or
        $appCommit -cne $Producer.flatpak.app_commit -or $runtimeCommit -cne $Producer.flatpak.runtime_commit) {
        throw 'Flatpak session changed the installed application/runtime source identity.'
    }
    return @{reference=$reference; runtime=$runtime; app_commit=$appCommit; runtime_commit=$runtimeCommit}
}
