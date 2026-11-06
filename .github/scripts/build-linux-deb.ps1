param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$StagingDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-payload.ps1')
. (Join-Path $PSScriptRoot 'linux-debian.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
$name = Get-P4kxAssetName -Target $Target -Version $Version -Package deb
$payload = Join-Path $StagingDirectory 'payload'
$original = @(New-P4kxLinuxPayload -Target $Target -Version $Version -Package deb -BinaryDirectory $BinaryDirectory -OutputDirectory $payload)
$packageRoot = Join-Path $StagingDirectory 'package'
$private = Join-Path $packageRoot 'usr/lib/p4kx'
$commands = Join-Path $packageRoot 'usr/bin'
New-Item -ItemType Directory -Force -Path $private, $commands | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'usr/share') | Out-Null
Copy-Item -Path (Join-Path $payload 'bin/*') -Destination $private
foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    Invoke-P4kxLinuxTool strip @('--strip-unneeded', '--remove-section=.comment', '--remove-section=.note', (Join-Path $private $binary)) | Out-Null
    Invoke-P4kxLinuxTool ln @('--symbolic', "../lib/p4kx/$binary", (Join-Path $commands $binary)) | Out-Null
}
foreach ($kind in @('applications', 'icons', 'metainfo', 'man')) {
    Copy-Item -LiteralPath (Join-Path $payload "share/$kind") -Destination (Join-Path $packageRoot "usr/share/$kind") -Recurse
}
$documents = Join-Path $packageRoot 'usr/share/doc/p4kx'
New-Item -ItemType Directory -Force -Path $documents | Out-Null
Copy-Item -LiteralPath (Join-Path $payload 'share/licenses/p4kx/THIRD-PARTY-NOTICES.md') -Destination $documents
Copy-Item -LiteralPath (Join-Path $payload 'share/licenses/p4kx/CHANGELOG.md') -Destination (Join-Path $documents 'changelog')
Write-P4kxDebianCopyright -OutputDirectory $documents
$epoch = [DateTimeOffset]::FromUnixTimeSeconds([long]$env:SOURCE_DATE_EPOCH)
$debianVersion = Get-P4kxDebianVersion $Version
$date = $epoch.UtcDateTime.ToString('ddd, dd MMM yyyy HH:mm:ss', [Globalization.CultureInfo]::InvariantCulture) + ' +0000'
$changelog = "p4kx ($debianVersion) unstable; urgency=medium`n`n  * Upstream release $Version.`n`n -- P4KX contributors <lucas.greuloch@gmail.com>  $date`n"
[IO.File]::WriteAllText((Join-Path $documents 'changelog.Debian'), $changelog, [Text.UTF8Encoding]::new($false))
Invoke-P4kxLinuxTool gzip @('--no-name', '--best', (Join-Path $documents 'changelog.Debian')) | Out-Null
Invoke-P4kxLinuxTool gzip @('--no-name', '--best', (Join-Path $documents 'changelog')) | Out-Null
$debian = Join-Path $StagingDirectory 'debian'
New-Item -ItemType Directory -Force -Path $debian | Out-Null
"Source: p4kx`n`nPackage: p4kx`nArchitecture: any`nDescription: browser for Star Citizen Archives" | Set-Content (Join-Path $debian 'control') -Encoding utf8NoBOM
Push-Location $StagingDirectory
try {
    $scan = @('p4kx', 'p4kx-gui', 'p4kx-mcp') | ForEach-Object { '-e' + [IO.Path]::GetFullPath((Join-Path 'package/usr/lib/p4kx' $_)) }
    $dependencies = Get-P4kxDebianDependencies @(Invoke-P4kxLinuxTool dpkg-shlibdeps (@('-O') + $scan))
}
finally { Pop-Location }
$control = Join-Path $packageRoot 'DEBIAN'
New-Item -ItemType Directory -Force -Path $control | Out-Null
$size = [long][Math]::Ceiling((Get-ChildItem -LiteralPath $packageRoot -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
@"
Package: p4kx
Version: $debianVersion
Section: utils
Priority: optional
Architecture: $($platform.deb_arch)
Maintainer: P4KX contributors <lucas.greuloch@gmail.com>
Installed-Size: $size
Depends: $dependencies
Recommends: pipewire-alsa | libasound2-plugins, xdg-desktop-portal-gtk | xdg-desktop-portal-backend
Homepage: https://github.com/p4kx-org/p4kx
Description: browser and extractor for Star Citizen Archives
 P4KX browses local Archives and exports their Entries in readable formats.
 This package includes the desktop window, command line and MCP server.
 It contains no game files or Archive key.
"@ | Set-Content -LiteralPath (Join-Path $control 'control') -Encoding utf8NoBOM
foreach ($manual in Get-ChildItem -LiteralPath (Join-Path $packageRoot 'usr/share/man/man1') -Filter '*.1') {
    Invoke-P4kxLinuxTool gzip @('--no-name', '--best', $manual.FullName) | Out-Null
}
Write-P4kxDebianMd5Sums -PackageRoot $packageRoot -Output (Join-Path $control 'md5sums')
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$internal = [IO.Path]::GetFullPath((Join-Path $OutputDirectory "p4kx_$($debianVersion)_$($platform.deb_arch).deb"))
Invoke-P4kxLinuxTool dpkg-deb @('--build', '--root-owner-group', $packageRoot, $internal) | Out-Null
Invoke-P4kxLinuxTool lintian @('--fail-on', 'error,warning', $internal) | Out-Null
$asset = Join-Path $OutputDirectory $name
Move-Item -LiteralPath $internal -Destination $asset
Write-P4kxLinuxPackageProducer -Target $Target -Version $Version -Package deb -Asset $asset -BinaryDirectory $private -OriginalImages $original -Transformations @('strip-unneeded') -Output (Join-Path $OutputDirectory "$name.producer.json")
