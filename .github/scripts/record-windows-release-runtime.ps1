param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$PortableResult,
    [Parameter(Mandatory)][string]$InstalledResult,
    [Parameter(Mandatory)][string]$InstallerResult,
    [Parameter(Mandatory)][string]$BuildMetadata,
    [Parameter(Mandatory)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
$installer = Get-Content -LiteralPath $InstallerResult -Raw | ConvertFrom-Json
if ($installer.target -cne $Target -or $installer.lifecycle -cne 'success' -or
    $installer.rollback -cne 'success' -or $installer.repair -cne 'success' -or -not $installer.profile_preserved) { throw 'Installer lifecycle verification did not succeed.' }
$metadata = Get-Content -LiteralPath $BuildMetadata -Raw | ConvertFrom-Json
$packages = @(
    (Get-Content -LiteralPath $PortableResult -Raw | ConvertFrom-Json),
    (Get-Content -LiteralPath $InstalledResult -Raw | ConvertFrom-Json)
)
foreach ($package in $packages) {
    $name = Get-P4kxAssetName -Target $Target -Version $Version -Package $package.package
    $package | Add-Member -NotePropertyName archive_sha256 -NotePropertyValue (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
}
$metadata | Add-Member -NotePropertyName package_smokes -NotePropertyValue $packages
$metadata | Add-Member -NotePropertyName installer -NotePropertyValue $installer
$metadata | Add-Member -NotePropertyName os_version -NotePropertyValue ([Environment]::OSVersion.VersionString)
Assert-P4kxPackageRuntime $metadata $platform $AssetDirectory $Version
$intermediate = Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) 'release-metadata.json'
$metadata | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $intermediate -Encoding utf8NoBOM
foreach ($name in @('CLI_LIST_OUTCOME', 'CLI_EXTRACT_OUTCOME', 'CLI_CONVERT_OUTCOME', 'MCP_TOOLS_OUTCOME', 'MCP_READ_OUTCOME', 'GUI_FRAMES_OUTCOME')) {
    [Environment]::SetEnvironmentVariable($name, 'success')
}
& (Join-Path $PSScriptRoot 'record-native-runtime.ps1') -Target $Target -MetadataPath $intermediate -OutputPath $OutputPath
