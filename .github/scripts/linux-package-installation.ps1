$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-tools.ps1')
. (Join-Path $PSScriptRoot 'linux-debian.ps1')
. (Join-Path $PSScriptRoot 'flatpak-metadata.ps1')
. (Join-Path $PSScriptRoot 'flatpak-installed-identity.ps1')

function Assert-P4kxDisposableLinuxRunner {
    param($Platform)
    if ($Platform.os -cne 'linux') { throw 'Linux package verification requires a Linux target.' }
    Assert-P4kxNativeHost -Platform $Platform
    if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') {
        throw 'Native package installation verification requires a disposable GitHub-hosted Linux runner.'
    }
}

function Expand-P4kxLinuxTar {
    param([string]$Asset, [string]$Directory)
    $root = [IO.Path]::GetFileName($Asset) -replace '\.tar\.gz$', ''
    $entries = @(Invoke-P4kxLinuxTool tar @('--list', '--gzip', '--file', $Asset))
    foreach ($entry in $entries) {
        if (-not $entry.StartsWith("$root/", [StringComparison]::Ordinal) -or $entry -match '(^|/)\.\.?(/|$)|[\r\n\0\\]') {
            throw 'Portable tar archive contains an unsafe or foreign path.'
        }
    }
    New-Item -ItemType Directory -Path $Directory | Out-Null
    Invoke-P4kxLinuxTool tar @('--extract', '--gzip', '--file', $Asset, '--directory', $Directory, '--no-same-owner') | Out-Null
    return @{binaries=(Join-Path $Directory "$root/bin"); launchers=$null}
}

function Expand-P4kxAppImage {
    param([string]$Asset, [string]$Directory, $Platform)
    if ((Get-ImageMachine -Path $Asset -Os linux) -ne $Platform.elf_machine) { throw 'AppImage runtime has the wrong CPU.' }
    Invoke-P4kxLinuxTool chmod @('0755', $Asset) | Out-Null
    New-Item -ItemType Directory -Path $Directory | Out-Null
    Push-Location $Directory
    try { Invoke-P4kxLinuxTool $Asset @('--appimage-extract') | Out-Null }
    finally { Pop-Location }
    $launchers = @{}
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $prefix = @('--appimage-extract-and-run')
        if ($binary -eq 'p4kx') { $prefix += '--cli' }
        if ($binary -eq 'p4kx-mcp') { $prefix += '--mcp' }
        $launchers[$binary] = @{executable=$Asset; prefix=$prefix}
    }
    return @{binaries=(Join-Path $Directory 'squashfs-root/usr/bin'); launchers=$launchers}
}

function Install-P4kxDeb {
    param([string]$Asset, [string]$Version, $Platform)
    Assert-P4kxDisposableLinuxRunner $Platform
    $fields = @(Invoke-P4kxLinuxTool dpkg-deb @('--field', $Asset, 'Package', 'Version', 'Architecture'))
    $metadata = @{}
    foreach ($field in $fields) { if ($field -match '^([^:]+):\s*(.+)$') { $metadata[$Matches[1]] = $Matches[2] } }
    if ($metadata.Package -cne 'p4kx' -or $metadata.Version -cne (Get-P4kxDebianVersion $Version) -or
        $metadata.Architecture -cne $Platform.deb_arch) { throw 'Debian package metadata does not match the native release.' }
    $PSNativeCommandUseErrorActionPreference = $false
    & dpkg-query '--show' '--showformat=${db:Status-Abbrev}' 'p4kx' 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) { throw 'Debian verification requires no existing P4KX package.' }
    if ($LASTEXITCODE -ne 1) { throw 'Installed-package ownership query failed.' }
    Invoke-P4kxLinuxTool sudo @('apt-get', 'install', '--yes', $Asset) | Out-Null
    $launchers = @{}
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $public = "/usr/bin/$binary"
        $actual = [IO.FileInfo]::new($public).ResolveLinkTarget($true)
        if (-not $actual -or $actual.FullName -cne "/usr/lib/p4kx/$binary") { throw 'Debian public command does not resolve to the package payload.' }
        $launchers[$binary] = @{executable=$public; prefix=@()}
    }
    return @{binaries='/usr/lib/p4kx'; launchers=$launchers}
}

function Install-P4kxFlatpak {
    param([string]$Asset, $Producer, $Platform, [string]$FixtureDirectory, [string]$CheckDirectory)
    Assert-P4kxDisposableLinuxRunner $Platform
    $settings = Get-P4kxLinuxPackaging
    $runtime = $Producer.flatpak.runtime_ref
    $expected = "runtime/$($settings.flatpak.runtime)/$($Platform.unix_arch)/$($settings.flatpak.runtime_branch)"
    if ($runtime -cne $expected -or $Producer.flatpak.runtime_commit -cnotmatch '^[0-9a-f]{64}$') {
        throw 'Flatpak producer has no target-correct runtime identity.'
    }
    Invoke-P4kxLinuxTool flatpak @('remote-add', '--user', '--if-not-exists', 'flathub', $settings.flatpak.runtime_repository) | Out-Null
    Invoke-P4kxLinuxTool flatpak @('install', '--user', '--assumeyes', '--noninteractive', "--arch=$($Platform.unix_arch)", 'flathub', $runtime) | Out-Null
    Invoke-P4kxLinuxTool flatpak @('update', '--user', '--assumeyes', '--noninteractive', "--commit=$($Producer.flatpak.runtime_commit)", $runtime) | Out-Null
    Invoke-P4kxLinuxTool flatpak @('install', '--user', '--assumeyes', '--noninteractive', $Asset) | Out-Null
    $identity = Get-P4kxLiveFlatpakIdentity $Platform $Producer
    $location = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($Platform.unix_arch)", '--show-location', $settings.application_id) | Out-String).Trim()
    $metadata = @(Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($Platform.unix_arch)", '--show-metadata', $settings.application_id))
    $metadata | Set-Content -LiteralPath (Join-Path $CheckDirectory 'flatpak-effective-metadata.log') -Encoding utf8NoBOM
    Assert-P4kxFlatpakMetadata $metadata $Platform
    $data = (Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($Platform.unix_arch)", '--command=printenv', $settings.application_id, 'XDG_DATA_HOME') | Out-String).Trim()
    if (-not [IO.Path]::IsPathFullyQualified($data)) { throw 'Flatpak did not provide an absolute writable application data directory.' }
    $drivers = @(Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($Platform.unix_arch)", '--command=find', $settings.application_id, '/usr/lib', '-name', 'lvp_icd*.json', '-type', 'f'))
    if ($drivers.Count -ne 1 -or -not [IO.Path]::IsPathFullyQualified($drivers[0]) -or $drivers[0] -match '[\r\n\0]') {
        throw 'Flatpak native Mesa software driver is unavailable or ambiguous.'
    }
    $launchers = @{}
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $prefix = @(
            'run', '--user', "--arch=$($Platform.unix_arch)", "--command=$binary",
            "--env=P4KX_FOLDERS_ROOT=$data/native-package-smoke", '--env=P4KX_GAME_CHECK=off',
            "--filesystem=$($FixtureDirectory):ro", "--filesystem=$($CheckDirectory):rw"
        )
        if ($binary -eq 'p4kx-gui') { $prefix += @('--env=P4KX_SMOKE_FRAMES=3', "--env=VK_DRIVER_FILES=$($drivers[0])", '--env=LIBGL_ALWAYS_SOFTWARE=1') }
        $prefix += $settings.application_id
        $launchers[$binary] = @{executable='flatpak'; prefix=$prefix}
    }
    return @{binaries=(Join-Path $location 'files/bin'); launchers=$launchers; installed_ref=$identity.reference; runtime_ref=$identity.runtime; data_directory=$data}
}

function Remove-P4kxLinuxPackage {
    param([string]$Package, $Platform)
    if ($Package -eq 'deb') {
        Invoke-P4kxLinuxTool sudo @('apt-get', 'remove', '--yes', 'p4kx') | Out-Null
        foreach ($path in @('/usr/bin/p4kx', '/usr/bin/p4kx-gui', '/usr/bin/p4kx-mcp', '/usr/lib/p4kx')) {
            if (Test-Path -LiteralPath $path) { throw "Debian uninstall left package ownership: $path" }
        }
    }
    if ($Package -eq 'flatpak') {
        $identity = (Get-P4kxLinuxPackaging).application_id
        Invoke-P4kxLinuxTool flatpak @('uninstall', '--user', '--assumeyes', '--noninteractive', "--arch=$($Platform.unix_arch)", $identity) | Out-Null
    }
}
