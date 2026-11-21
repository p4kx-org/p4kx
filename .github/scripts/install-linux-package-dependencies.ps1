param([switch]$Build)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
$platform = Get-P4kxCurrentPlatform
Assert-P4kxDisposableLinuxRunner $platform
$distribution = Get-Content -LiteralPath '/etc/os-release' -Raw
if ($distribution -notmatch '(?m)^ID=ubuntu$' -or $distribution -notmatch '(?m)^VERSION_ID="24\.04"$') {
    throw 'Linux release packaging requires the declared Ubuntu 24.04 baseline.'
}
$packages = @(
    'binutils', 'dbus-x11', 'flatpak', 'fuse3', 'libasound2t64', 'libegl1', 'libgl1',
    'libvulkan1', 'libx11-6', 'libxcursor1', 'libxi6', 'libxrandr2', 'libxkbcommon0',
    'libxkbcommon-x11-0', 'libwayland-client0', 'libwayland-cursor0', 'libwayland-egl1',
    'mesa-vulkan-drivers', 'ostree', 'xauth', 'xdg-desktop-portal', 'xdg-desktop-portal-gtk', 'xvfb'
)
if ($Build) {
    $sources = Get-Content -LiteralPath '/etc/apt/sources.list.d/ubuntu.sources' -Raw
    if ($sources -notmatch '(?m)^Types: deb(?: deb-src)?\s*$') { throw 'Unexpected native Ubuntu source repository configuration.' }
    $sources = $sources -replace '(?m)^Types: deb\s*$', 'Types: deb deb-src'
    $temporary = Join-Path $env:RUNNER_TEMP 'p4kx-ubuntu.sources'
    $sources | Set-Content -LiteralPath $temporary -Encoding utf8NoBOM
    Invoke-P4kxLinuxTool sudo @('install', '--mode=0644', $temporary, '/etc/apt/sources.list.d/ubuntu.sources') | Out-Null
    $packages += @('appstream', 'desktop-file-utils', 'dpkg-dev', 'flatpak-builder', 'gzip', 'libasound2-dev', 'librsvg2-common', 'lintian', 'patchelf', 'zstd')
}
Invoke-P4kxLinuxTool sudo @('apt-get', 'update') | Out-Null
Invoke-P4kxLinuxTool sudo (@('apt-get', 'install', '--yes', '--no-install-recommends') + $packages) | Out-Null
$drivers = @(Get-ChildItem -LiteralPath '/usr/share/vulkan/icd.d' -Filter 'lvp_icd*.json' -File)
if ($drivers.Count -ne 1) { throw 'Native Mesa software adapter ICD is unavailable or ambiguous.' }
if ($env:GITHUB_ENV) {
    "VK_DRIVER_FILES=$($drivers[0].FullName)" >> $env:GITHUB_ENV
    'XDG_CURRENT_DESKTOP=GNOME' >> $env:GITHUB_ENV
}
