$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'linux-package-evidence.ps1')
. (Join-Path $PSScriptRoot 'linux-package-lifecycle-evidence.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-path-verification.ps1')

function Assert-P4kxPackageRuntime {
    param($Runtime, $Platform, [string]$AssetDirectory, [string]$Version)
    if ($Platform.os -ceq 'windows') {
        $msi = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package msi
        $hash = (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $msi) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($Runtime.installer.lifecycle -cne 'success' -or $Runtime.installer.repair -cne 'success' -or $Runtime.installer.rollback -cne 'success') { throw 'MSI lifecycle, repair or rollback failed.' }
        Assert-P4kxMsiPathVerification $Runtime.installer.path_contract $Platform $Runtime.revision $hash
    }
    $packages = @($Runtime.package_smokes)
    if ((($packages.package | Sort-Object) -join ',') -cne (($Platform.packages | Sort-Object) -join ',')) {
        throw 'Release evidence does not cover every target package exactly once.'
    }
    $sourcePackage = if ($Platform.os -ceq 'windows') { 'portable-zip' } else { 'tar-gz' }
    foreach ($package in $packages) {
        $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package $package.package
        $hash = (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($package.target -cne $Platform.target -or $package.archive_sha256 -cne $hash) { throw 'Executed package source/target/digest differs.' }
        if ($package.package -ceq $sourcePackage -and $Runtime.archive_sha256 -cne $hash) {
            throw "Native release archive digest differs from the executed $sourcePackage."
        }
        Assert-P4kxBinaryEvidence -Metadata $package -Platform $Platform
        if ($Platform.os -eq 'linux') {
            Assert-P4kxLinuxPackageProducer $package.producer $Platform $Runtime $AssetDirectory $Version
            Assert-P4kxLinuxPackageExecution $package $package.producer $Platform
            if ($package.package -eq 'flatpak') {
                Assert-P4kxFlatpakDocumentsEvidence $package.documents $Platform $Runtime.revision $hash
            }
        }
        foreach ($binary in $package.binaries) {
            $built = @($Runtime.binaries | Where-Object name -CEQ $binary.name)
            if ($built.Count -ne 1 -or ($Platform.os -eq 'windows' -and $built[0].sha256 -cne $binary.sha256)) {
                throw 'Executed package binary differs from release build.'
            }
        }
        foreach ($check in @('cli_list', 'cli_extract', 'cli_convert', 'mcp_tools', 'mcp_read', 'gui_frames')) {
            if ($package.runtime.$check -cne 'success') { throw "Packaged runtime check '$check' did not succeed." }
        }
    }
    if ($Platform.os -ceq 'linux') {
        Assert-P4kxLinuxLifecycleEvidence $Runtime.linux_lifecycle $packages $Platform $Version $Runtime.revision
    }
}
