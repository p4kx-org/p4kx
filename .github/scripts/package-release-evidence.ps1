$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')

function Assert-P4kxPackageRuntime {
    param($Runtime, $Platform, [string]$AssetDirectory, [string]$Version)
    $packages = @($Runtime.package_smokes)
    if ((($packages.package | Sort-Object) -join ',') -cne (($Platform.packages | Sort-Object) -join ',')) {
        throw 'Release evidence does not cover every target package exactly once.'
    }
    foreach ($package in $packages) {
        $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package $package.package
        $hash = (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($package.target -cne $Platform.target -or $package.archive_sha256 -cne $hash) { throw 'Executed package source/target/digest differs.' }
        Assert-P4kxBinaryEvidence -Metadata $package -Platform $Platform
        foreach ($binary in $package.binaries) {
            $built = @($Runtime.binaries | Where-Object name -CEQ $binary.name)
            if ($built.Count -ne 1 -or $built[0].sha256 -cne $binary.sha256) { throw 'Executed package binary differs from release build.' }
        }
        foreach ($check in @('cli_list', 'cli_extract', 'cli_convert', 'mcp_tools', 'mcp_read', 'gui_frames')) {
            if ($package.runtime.$check -cne 'success') { throw "Packaged runtime check '$check' did not succeed." }
        }
    }
}
