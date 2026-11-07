param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$StagingDirectory,
    [Parameter(Mandatory)][string]$ToolsDirectory,
    [Parameter(Mandatory)][string]$RuntimeDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-payload.ps1')
. (Join-Path $PSScriptRoot 'linux-shared-libraries.ps1')
. (Join-Path $PSScriptRoot 'linux-source-tree.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
$settings = Get-P4kxLinuxPackaging
$name = Get-P4kxAssetName -Target $Target -Version $Version -Package app-image
$appDir = [IO.Path]::GetFullPath((Join-Path $StagingDirectory 'AppDir'))
$original = @(New-P4kxLinuxPayload -Target $Target -Version $Version -Package app-image -BinaryDirectory $BinaryDirectory -OutputDirectory (Join-Path $appDir 'usr'))
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Copy-Item -LiteralPath (Join-Path $root 'packaging/linux/AppRun') -Destination (Join-Path $appDir 'AppRun')
Invoke-P4kxLinuxTool chmod @('0755', (Join-Path $appDir 'AppRun')) | Out-Null
$desktop = Join-Path $appDir "usr/share/applications/$($settings.application_id).desktop"
$icon = Join-Path $appDir "usr/share/icons/hicolor/512x512/apps/$($settings.application_id).png"
$deploy = [Collections.Generic.List[string]]::new()
$deploy.Add('--appimage-extract-and-run')
$deploy.Add('--appdir'); $deploy.Add($appDir)
foreach ($binary in @('p4kx-gui', 'p4kx', 'p4kx-mcp')) { $deploy.Add('--executable'); $deploy.Add((Join-Path $appDir "usr/bin/$binary")) }
$deploy.Add('--desktop-file'); $deploy.Add($desktop)
$deploy.Add('--icon-file'); $deploy.Add($icon)
foreach ($library in @(
    'libasound.so.2', 'libX11.so.6', 'libXcursor.so.1', 'libXi.so.6', 'libXrandr.so.2',
    'libxkbcommon.so.0', 'libxkbcommon-x11.so.0', 'libwayland-client.so.0', 'libwayland-cursor.so.0',
    'libwayland-egl.so.1', 'libdbus-1.so.3', 'libvulkan.so.1', 'libEGL.so.1', 'libGL.so.1'
)) {
    $deploy.Add('--library'); $deploy.Add((Find-P4kxNativeLibrary $library $platform))
}
Invoke-P4kxLinuxTool (Join-Path $ToolsDirectory "linuxdeploy-$($platform.unix_arch).AppImage") $deploy.ToArray() | Out-Null
$sourceRoot = Join-Path $StagingDirectory 'corresponding-source'
New-Item -ItemType Directory -Force -Path $sourceRoot | Out-Null
Copy-P4kxLinuxSourceTree -Source (Join-Path $RuntimeDirectory 'sources') -Destination (Join-Path $sourceRoot 'runtime')
Copy-Item -LiteralPath (Join-Path $RuntimeDirectory 'runtime-inventory.json') -Destination $sourceRoot
Copy-Item -LiteralPath (Join-Path $RuntimeDirectory 'licences') -Destination (Join-Path $sourceRoot 'licences') -Recurse
Copy-Item -LiteralPath (Join-Path $RuntimeDirectory 'licences') -Destination (Join-Path $appDir 'usr/share/licenses/appimage-runtime') -Recurse
Copy-Item -LiteralPath (Join-Path $RuntimeDirectory 'runtime-inventory.json') -Destination (Join-Path $appDir 'usr/share/licenses/appimage-runtime/inventory.json')
Write-P4kxAppImageLibraryInventory -AppDir $appDir -SourceDirectory (Join-Path $sourceRoot 'shared-libraries') -Platform $platform
Copy-Item -LiteralPath $desktop -Destination (Join-Path $appDir "$($settings.application_id).desktop")
Copy-Item -LiteralPath $icon -Destination (Join-Path $appDir "$($settings.application_id).png")
Invoke-P4kxLinuxTool ln @('--symbolic', "$($settings.application_id).png", (Join-Path $appDir '.DirIcon')) | Out-Null
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$asset = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $name))
Invoke-P4kxLinuxTool (Join-Path $ToolsDirectory "appimagetool-$($platform.unix_arch).AppImage") @(
    '--appimage-extract-and-run', '--runtime-file', (Join-Path $RuntimeDirectory 'runtime'), $appDir, $asset
) | Out-Null
Write-P4kxLinuxPackageProducer -Target $Target -Version $Version -Package app-image -Asset $asset -BinaryDirectory (Join-Path $appDir 'usr/bin') -OriginalImages $original -Transformations @('linuxdeploy-rpath-and-strip', 'bundle-shared-libraries', 'native-static-appimage-runtime') -Output (Join-Path $OutputDirectory "$name.producer.json")
$sourceAsset = [IO.Path]::GetFullPath((Join-Path $OutputDirectory "P4KX-$Version-$Target-appimage-corresponding-source.tar.gz"))
Invoke-P4kxLinuxTool tar @('--sort=name', "--mtime=@$env:SOURCE_DATE_EPOCH", '--owner=0', '--group=0', '--numeric-owner', '--gzip', '--create', '--file', $sourceAsset, '--directory', $sourceRoot, '.') | Out-Null
$producerPath = Join-Path $OutputDirectory "$name.producer.json"
$producer = Get-Content -LiteralPath $producerPath -Raw | ConvertFrom-Json
$producer | Add-Member -NotePropertyName corresponding_source -NotePropertyValue @{
    name=[IO.Path]::GetFileName($sourceAsset)
    sha256=(Get-FileHash -LiteralPath $sourceAsset -Algorithm SHA256).Hash.ToLowerInvariant()
}
$producer | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $producerPath -Encoding utf8NoBOM
