param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$StagingDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-tools.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
$tools = Get-P4kxLinuxTools
$runtime = $tools.runtime
if ($runtime.builder_image -cnotmatch '^docker\.io/library/alpine@sha256:[0-9a-f]{64}$') { throw 'Runtime builder image is not immutable.' }
$stage = [IO.Path]::GetFullPath($StagingDirectory)
if (Test-Path -LiteralPath $stage) { throw 'Native runtime build staging directory must be new.' }
$context = Join-Path $stage 'context'
New-Item -ItemType Directory -Force -Path $context | Out-Null
$sourceArchive = Join-Path $stage 'upstream.tar.gz'
Save-P4kxPinnedDownload $runtime.url $runtime.sha256 $sourceArchive
Invoke-P4kxLinuxTool tar @('--extract', '--gzip', '--file', $sourceArchive, '--directory', $context) | Out-Null
$sources = @(Get-ChildItem -LiteralPath $context -Directory)
if ($sources.Count -ne 1 -or $sources[0].Name -cne "type2-runtime-$($runtime.source_commit)") { throw 'Pinned runtime archive root differs.' }
Move-Item -LiteralPath $sources[0].FullName -Destination (Join-Path $context 'upstream')
$packages = Join-Path $context 'packages'
New-Item -ItemType Directory -Force -Path $packages | Out-Null
$locked = @($runtime.libraries.($platform.unix_arch))
foreach ($package in $locked) {
    Save-P4kxPinnedDownload $package.url $package.sha256 (Join-Path $packages "$($package.name)-$($package.version).apk")
}
$runtime | Add-Member -NotePropertyName static_packages -NotePropertyValue $locked
$runtime.PSObject.Properties.Remove('libraries')
$runtime | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $context 'inputs.json') -Encoding utf8NoBOM
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
foreach ($name in @('Dockerfile', 'collect-sources.py', 'build_inputs.py', 'RELINK.md')) {
    Copy-Item -LiteralPath (Join-Path $root "packaging/linux/runtime/$name") -Destination $context
}
$tag = "p4kx-appimage-runtime-$($platform.arch)"
Invoke-P4kxLinuxTool docker @(
    'build', "--platform=linux/$($platform.arch)", '--build-arg', "BUILDER_IMAGE=$($runtime.builder_image)",
    '--build-arg', "RUNTIME_COMMIT=$($runtime.source_commit)", '--tag', $tag, $context
) | Out-Null
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
Invoke-P4kxLinuxTool docker @('run', '--rm', '--network=none', '--mount', "type=bind,source=$output,destination=/output", $tag) | Out-Null
if ((Get-ImageMachine -Path (Join-Path $output 'runtime') -Os linux) -ne $platform.elf_machine) { throw 'Built AppImage runtime does not match native CPU.' }
$dynamic = (Invoke-P4kxLinuxTool readelf @('--dynamic', '--wide', (Join-Path $output 'runtime'))) -join "`n"
$segments = (Invoke-P4kxLinuxTool readelf @('--program-headers', '--wide', (Join-Path $output 'runtime'))) -join "`n"
if ($dynamic -match '\(NEEDED\)' -or $segments -match '\bINTERP\b') { throw 'AppImage runtime unexpectedly requires a dynamic host loader/library.' }
$inventory = Get-Content -LiteralPath (Join-Path $output 'runtime-inventory.json') -Raw | ConvertFrom-Json
if (-not $inventory.linked_archives -or $inventory.runtime_source_commit -cne $runtime.source_commit -or
    $inventory.runtime_sha256 -cne (Get-FileHash -LiteralPath (Join-Path $output 'runtime') -Algorithm SHA256).Hash.ToLowerInvariant()) {
    throw 'Built runtime and corresponding-source inventory differ.'
}
