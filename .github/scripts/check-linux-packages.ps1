param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$BuildMetadata,
    [Parameter(Mandatory)][string]$AdapterEvidence,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'linux-package-evidence.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
. (Join-Path $PSScriptRoot 'linux-elf.ps1')
. (Join-Path $PSScriptRoot 'linux-profile-preservation.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
Assert-P4kxDisposableLinuxRunner $platform
$assets = (Resolve-Path -LiteralPath $AssetDirectory).Path
$fixture = (Resolve-Path -LiteralPath $FixtureDirectory).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Linux verification requires a fresh output directory.' }
New-Item -ItemType Directory -Path $output | Out-Null
$env:FLATPAK_USER_DIR = Join-Path $output 'flatpak-installation'
$base = Get-Content -LiteralPath $BuildMetadata -Raw | ConvertFrom-Json
$tarName = Get-P4kxAssetName -Target $Target -Version $Version -Package tar-gz
& (Join-Path $PSScriptRoot 'check-native-platform.ps1') -Target $Target -ArchivePath (Join-Path $assets $tarName) -MetadataPath $BuildMetadata -ValidateArchive
$adapter = Get-Content -LiteralPath $AdapterEvidence -Raw | ConvertFrom-Json
if ($adapter.target -cne $Target -or $adapter.revision -cne $env:GITHUB_SHA -or $adapter.outcome -cne 'success') {
    throw 'Native software adapter evidence source/CPU/outcome differs.'
}
$libc = (Invoke-P4kxLinuxTool getconf @('GNU_LIBC_VERSION') | Out-String).Trim()
if ($libc -cne "glibc $((Get-P4kxLinuxPackaging).maximum_glibc_requirement)") { throw 'Native verification does not use the declared minimum glibc baseline.' }
Invoke-P4kxLinuxTool dbus-update-activation-environment @('DISPLAY', 'XAUTHORITY', 'XDG_CURRENT_DESKTOP') | Out-Null
$smokes = @(foreach ($package in $platform.packages) {
    $name = Get-P4kxAssetName -Target $Target -Version $Version -Package $package
    $asset = Join-Path $assets $name
    $producer = Get-Content -LiteralPath (Join-Path $assets "$name.producer.json") -Raw | ConvertFrom-Json
    Assert-P4kxLinuxPackageProducer $producer $platform $base $assets $Version
    $check = Join-Path $output $package
    New-Item -ItemType Directory -Path $check | Out-Null
    $preserved = @(New-P4kxProfileSentinels (Join-Path $check 'preserved-profile'))
    $state = $null
    try {
        $state = switch ($package) {
            'tar-gz' { Expand-P4kxLinuxTar $asset (Join-Path $check 'payload') }
            'app-image' { Expand-P4kxAppImage $asset (Join-Path $check 'payload') $platform }
            'deb' { Install-P4kxDeb $asset $Version $platform }
            'flatpak' { Install-P4kxFlatpak $asset $producer $platform $fixture $check }
            default { throw 'Unknown required Linux package format.' }
        }
        $images = @(Get-P4kxLinuxApplicationEvidence -Directory $state.binaries -Platform $platform)
        Assert-P4kxLinuxPackageExecution @{target=$Target; package=$package; binaries=$images} $producer $platform
        & (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Target -BinaryDirectory $state.binaries -FixtureDirectory $fixture -OutputDirectory (Join-Path $check 'smoke') -Package $package -Launchers $state.launchers -ExpectedVersion $Version
        $result = Get-Content -LiteralPath (Join-Path $check 'smoke/runtime.json') -Raw | ConvertFrom-Json
        Assert-P4kxLinuxPackageExecution $result $producer $platform
        if ($package -eq 'flatpak') {
            $flatpakPreserved = @(New-P4kxProfileSentinels (Join-Path $state.data_directory 'preserved-profile'))
            $preserved += $flatpakPreserved
            & (Join-Path $PSScriptRoot 'check-flatpak-documents.ps1') -Target $Target -FixtureDirectory $fixture -OutputDirectory (Join-Path $check 'documents') | Out-Host
            $result | Add-Member -NotePropertyName documents -NotePropertyValue (Get-Content -LiteralPath (Join-Path $check 'documents/documents.json') -Raw | ConvertFrom-Json)
        }
        $result | Add-Member -NotePropertyName producer -NotePropertyValue $producer
        $result | Add-Member -NotePropertyName archive_sha256 -NotePropertyValue $producer.archive_sha256
        $result
    }
    finally {
        if ($state) { Remove-P4kxLinuxPackage $package $platform }
        Assert-P4kxProfileSentinels $preserved
    }
})
$base | Add-Member -NotePropertyName package_smokes -NotePropertyValue $smokes
$base | Add-Member -NotePropertyName minimum_distribution -NotePropertyValue (Get-P4kxLinuxPackaging).minimum_distribution
$base | Add-Member -NotePropertyName observed_glibc -NotePropertyValue $libc
$metadata = Join-Path $output 'executed-packages.json'
$base | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $metadata -Encoding utf8NoBOM
foreach ($variable in @('CLI_LIST_OUTCOME', 'CLI_EXTRACT_OUTCOME', 'CLI_CONVERT_OUTCOME', 'MCP_TOOLS_OUTCOME', 'MCP_READ_OUTCOME', 'GUI_FRAMES_OUTCOME')) {
    [Environment]::SetEnvironmentVariable($variable, 'success')
}
$env:SOFTWARE_ADAPTER_OUTCOME = $adapter.outcome
$runtimePath = Join-Path $assets "P4KX-$Version-$Target-runtime.json"
& (Join-Path $PSScriptRoot 'record-native-runtime.ps1') -Target $Target -MetadataPath $metadata -OutputPath $runtimePath
$runtime = Get-Content -LiteralPath $runtimePath -Raw | ConvertFrom-Json
Assert-P4kxPackageRuntime $runtime $platform $assets $Version
