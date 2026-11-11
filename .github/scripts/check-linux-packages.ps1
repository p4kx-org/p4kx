param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$LifecycleFixtureDirectory,
    [Parameter(Mandatory)][string]$BuildMetadata,
    [Parameter(Mandatory)][string]$AdapterEvidence,
    [Parameter(Mandatory)][string]$ServiceBuildMetadata,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-orchestration.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-evidence.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
$assets = (Resolve-Path -LiteralPath $AssetDirectory).Path
$fixture = (Resolve-Path -LiteralPath $FixtureDirectory).Path
$lifecycleFixtures = (Resolve-Path -LiteralPath $LifecycleFixtureDirectory).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Linux verification requires a fresh output directory.' }
if ($env:XDG_RUNTIME_DIR -cne "/run/user/$(Get-P4kxPortalUserId)" -or -not (Test-Path -LiteralPath $env:XDG_RUNTIME_DIR -PathType Container)) { throw 'Native sessions require the normal existing per-user runtime directory.' }
New-Item -ItemType Directory -Path $output | Out-Null
$env:FLATPAK_USER_DIR = Join-Path $output 'flatpak-installation'
$base = Get-Content -LiteralPath $BuildMetadata -Raw | ConvertFrom-Json
$tarName = Get-P4kxAssetName -Target $Target -Version $Version -Package tar-gz
& (Join-Path $PSScriptRoot 'check-native-platform.ps1') -Target $Target -ArchivePath (Join-Path $assets $tarName) -MetadataPath $BuildMetadata -ValidateArchive
$adapter = Get-Content -LiteralPath $AdapterEvidence -Raw | ConvertFrom-Json
if ($adapter.target -cne $Target -or $adapter.revision -cne $env:GITHUB_SHA -or $adapter.outcome -cne 'success') { throw 'Native software adapter evidence source/CPU/outcome differs.' }
$libc = (Invoke-P4kxLinuxTool getconf @('GNU_LIBC_VERSION') | Out-String).Trim()
if ($libc -cne "glibc $((Get-P4kxLinuxPackaging).maximum_glibc_requirement)") { throw 'Native verification does not use the declared minimum glibc baseline.' }
$services = Get-Content -LiteralPath $ServiceBuildMetadata -Raw | ConvertFrom-Json
Assert-P4kxDocumentsServiceBuild $services $Target $env:GITHUB_SHA
$context = @{target=$Target; version=$Version; assets=$assets; fixture=$fixture; lifecycle_fixtures=$lifecycleFixtures; output=$output;
    metadata=(Resolve-Path -LiteralPath $BuildMetadata).Path; services=(Resolve-Path -LiteralPath $ServiceBuildMetadata).Path}
$phases = Invoke-P4kxLinuxDocumentsPhases $context $services
$smokes = @(Get-Content -LiteralPath (Join-Path $phases.stock 'packages.json') -Raw | ConvertFrom-Json)
$state = Get-Content -LiteralPath (Join-Path $phases.stock 'flatpak-state.json') -Raw | ConvertFrom-Json
$flatpak = Get-P4kxDocumentsCombinedEvidence $phases $services $state.producer.archive_sha256
Assert-P4kxFlatpakDocumentsEvidence $flatpak.documents $platform $base.revision $state.producer.archive_sha256
$smokes += $flatpak
$lifecycle = @(Get-Content -LiteralPath (Join-Path $phases.stock 'linux-lifecycle.json') -Raw | ConvertFrom-Json)
$base | Add-Member -NotePropertyName linux_lifecycle -NotePropertyValue $lifecycle
$base | Add-Member -NotePropertyName package_smokes -NotePropertyValue $smokes
$base | Add-Member -NotePropertyName minimum_distribution -NotePropertyValue (Get-P4kxLinuxPackaging).minimum_distribution
$base | Add-Member -NotePropertyName observed_glibc -NotePropertyValue $libc
$metadata = Join-Path $output 'executed-packages.json'
$base | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $metadata -Encoding utf8NoBOM
foreach ($variable in @('CLI_LIST_OUTCOME', 'CLI_EXTRACT_OUTCOME', 'CLI_CONVERT_OUTCOME', 'MCP_TOOLS_OUTCOME', 'MCP_READ_OUTCOME', 'GUI_FRAMES_OUTCOME')) {
    [Environment]::SetEnvironmentVariable($variable, 'success')
}
$env:SOFTWARE_ADAPTER_OUTCOME = $adapter.outcome
$runtimePath = Join-Path $assets "P4KX-$Version-$Target-runtime.json"
& (Join-Path $PSScriptRoot 'record-native-runtime.ps1') -Target $Target -MetadataPath $metadata -OutputPath $runtimePath
$runtime = Get-Content -LiteralPath $runtimePath -Raw | ConvertFrom-Json
Assert-P4kxPackageRuntime $runtime $platform $assets $Version
