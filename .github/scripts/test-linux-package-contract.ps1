param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-elf.ps1')
. (Join-Path $PSScriptRoot 'flatpak-metadata.ps1')
. (Join-Path $PSScriptRoot 'linux-profile-preservation.ps1')
. (Join-Path $PSScriptRoot 'linux-debian.ps1')
New-Item -ItemType Directory -Force -Path $Directory | Out-Null
$settings = Get-P4kxLinuxPackaging
foreach ($mutation in @(
    { param($value) $value.flatpak.finish_args += '--filesystem=host' },
    { param($value) $value.flatpak.finish_args += '--filesystem=home:rw' },
    { param($value) $value.flatpak.finish_args += '--talk-name=org.freedesktop.Flatpak' },
    { param($value) $value.flatpak.finish_args += '--device=all' },
    { param($value) $value.flatpak.finish_args += $value.flatpak.finish_args[0] }
)) {
    $changed = $settings | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    & $mutation $changed
    $rejected = $false
    try { ConvertFrom-P4kxLinuxPackaging ($changed | ConvertTo-Json -Depth 8) | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Broad or duplicate sandbox permission was accepted.' }
}
$rows = @((Get-P4kxPlatformContract).targets | Where-Object os -EQ linux)
foreach ($row in $rows) { Get-P4kxLinuxPlatform $row.target | Out-Null }
$rejected = $false
try { Get-P4kxLinuxPlatform 'x86_64-pc-windows-msvc' | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'Windows target was accepted for Linux packaging.' }
$section = '  [15] .text PROGBITS 0000000000000040 000040 000003 00 AX 0 0 16'
$range = Get-P4kxElfTextRange -SectionHeaders @($section)
$path = Join-Path $Directory 'section-range.fixture'
$bytes = [byte[]]::new(68)
[Text.Encoding]::ASCII.GetBytes('abc').CopyTo($bytes, 64)
[IO.File]::WriteAllBytes($path, $bytes)
$expected = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
if ((Get-P4kxFileRangeHash $path $range.offset $range.length) -cne $expected) { throw 'ELF section range hash differs.' }
foreach ($headers in @(@($section, $section), @('unrelated section table'), @($section.Replace('000003', '000000')))) {
    $rejected = $false
    try { Get-P4kxElfTextRange -SectionHeaders $headers | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Invalid or ambiguous executable section was accepted.' }
}
$rejected = $false
try { Get-P4kxFileRangeHash $path 67 2 | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'A section beyond EOF was accepted.' }
$requirements = @(Get-P4kxGlibcRequirements @('Name: GLIBC_2.39 Flags: none', 'Name: GLIBC_2.2.5 Flags: none', 'Name: GLIBC_2.39 Flags: none'))
if (($requirements -join ',') -cne '2.39,2.2.5') { throw 'glibc versions were not ordered numerically and deduplicated.' }
Assert-P4kxGlibcBaseline $requirements '2.39'
$rejected = $false
try { Assert-P4kxGlibcBaseline @('2.40') '2.39' } catch { $rejected = $true }
if (-not $rejected) { throw 'A newer glibc ABI requirement was accepted.' }
$metadata = @(
    '[Application]', "name=$($settings.application_id)", "runtime=$($settings.flatpak.runtime)/$($rows[0].unix_arch)/25.08", 'command=p4kx-gui',
    '[Context]', 'shared=network;ipc;', 'sockets=pulseaudio;wayland;fallback-x11;', 'devices=dri;', 'filesystems=~/Games/star-citizen:ro;'
)
Assert-P4kxFlatpakMetadata $metadata $rows[0]
foreach ($changed in @(
    ($metadata + @('[Session Bus Policy]', 'org.freedesktop.Flatpak=talk')),
    @($metadata | ForEach-Object { $_ -replace '^filesystems=.*', 'filesystems=host;' }),
    @($metadata | ForEach-Object { $_ -replace '^runtime=.*', 'runtime=org.freedesktop.Platform/foreign/25.08' }),
    ($metadata + 'devices=all;')
)) {
    $rejected = $false
    try { Assert-P4kxFlatpakMetadata $changed $rows[0] } catch { $rejected = $true }
    if (-not $rejected) { throw 'Wrong runtime, host capability or duplicate Flatpak metadata was accepted.' }
}
$sentinels = @(New-P4kxProfileSentinels (Join-Path $Directory 'profile'))
Assert-P4kxProfileSentinels $sentinels
[IO.File]::AppendAllText($sentinels[0].path, 'changed')
$rejected = $false
try { Assert-P4kxProfileSentinels $sentinels } catch { $rejected = $true }
if (-not $rejected) { throw 'Changed profile/key preservation evidence was accepted.' }
if ((Get-P4kxDebianVersion '0.5.0-rc.1+build.2') -cne '0.5.0~rc.1+build.2-1') { throw 'Debian prerelease ordering or packaging revision differs.' }
$dependencies = Get-P4kxDebianDependencies @('shlibs:Depends=libc6 (>= 2.38), libasound2t64 (>= 1.0), libx11-6 (>= 1.8)')
if ($dependencies -notmatch 'libc6 \(>= 2\.39\)' -or ([regex]::Matches($dependencies, 'libx11-6')).Count -ne 1 -or
    $dependencies -notmatch 'libasound2t64 \(>= 1\.0\)' -or $dependencies -notmatch 'xdg-desktop-portal') { throw 'Debian native/dlopen dependencies differ.' }
'PASS: Linux target/ELF/glibc contracts, effective Flatpak policy and runtime identity, profile preservation and Debian dependencies. No Linux application executed.'
