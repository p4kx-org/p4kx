$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')

function Assert-P4kxLinuxPackageProducer {
    param($Producer, $Platform, $Build, [string]$AssetDirectory, [string]$Version)
    $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package $Producer.package
    if ($Producer.schema -ne 1 -or $Producer.target -cne $Platform.target -or $Producer.version -cne $Version -or
        $Producer.revision -cne $Build.revision -or $Producer.compiler -cne $Build.compiler -or $Producer.nextest -cne $Build.nextest -or
        $Producer.minimum_distribution -cne (Get-P4kxLinuxPackaging).minimum_distribution -or
        $Producer.archive_sha256 -cne (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()) {
        throw 'Linux package producer source/target/tool/ABI/digest identity differs.'
    }
    Assert-P4kxBinaryEvidence -Metadata $Producer -Platform $Platform
    Assert-P4kxBinaryEvidence -Metadata @{binaries=$Producer.source_binaries} -Platform $Platform
    foreach ($binary in $Producer.binaries) {
        $source = @($Producer.source_binaries | Where-Object name -CEQ $binary.name)
        $built = @($Build.binaries | Where-Object name -CEQ $binary.name)
        if ($source.Count -ne 1 -or $built.Count -ne 1 -or $source[0].sha256 -cne $built[0].sha256 -or
            $binary.text_sha256 -cnotmatch '^[0-9a-f]{64}$' -or $binary.text_sha256 -cne $source[0].text_sha256) {
            throw 'Packaged ELF code is not the original source-bound native release application.'
        }
    }
    $allowed = @('strip-unneeded', 'flatpak-sdk-install', 'linuxdeploy-rpath-and-strip', 'bundle-shared-libraries', 'native-static-appimage-runtime')
    if (@($Producer.transformations | Where-Object { $_ -cnotin $allowed }).Count -ne 0) { throw 'Unknown ELF packaging transformation.' }
    if ($Producer.package -eq 'app-image') {
        $name = "P4KX-$Version-$($Platform.target)-appimage-corresponding-source.tar.gz"
        if ($Producer.corresponding_source.name -cne $name -or $Producer.corresponding_source.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            $Producer.corresponding_source.sha256 -cne (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()) {
            throw 'AppImage corresponding-source artifact is missing or differs from its producer.'
        }
    }
}

function Assert-P4kxLinuxPackageExecution {
    param($Execution, $Producer, $Platform)
    if ($Execution.package -cne $Producer.package -or $Execution.target -cne $Producer.target) {
        throw 'Executed Linux format/target differs from its producer.'
    }
    Assert-P4kxBinaryEvidence -Metadata $Execution -Platform $Platform
    foreach ($binary in $Execution.binaries) {
        $produced = @($Producer.binaries | Where-Object name -CEQ $binary.name)
        if ($produced.Count -ne 1 -or $produced[0].sha256 -cne $binary.sha256) {
            throw 'Executed Linux package ELF differs from the produced package payload.'
        }
    }
}
