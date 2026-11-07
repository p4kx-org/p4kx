param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$StagingDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-source.ps1')
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'linux-elf.ps1')
. (Join-Path $PSScriptRoot 'portal-measurement.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
$recipe = Get-P4kxDocumentsPortalRecipe
$stage = [IO.Path]::GetFullPath($StagingDirectory)
$output = [IO.Path]::GetFullPath($OutputDirectory)
if ((Test-Path -LiteralPath $stage) -or (Test-Path -LiteralPath $output)) { throw 'Documents service build requires fresh task directories.' }
New-Item -ItemType Directory -Path $stage, $output | Out-Null
$null = Invoke-P4kxPortalMeasurement sudo (@('apt-get', 'install', '--yes', '--no-install-recommends') + $recipe.apt_packages) $stage 'dependencies' -Milliseconds 600000 -MaximumBytes 4MB
$archive = Join-Path $stage 'official-source.tar.xz'
Save-P4kxPinnedDownload $recipe.url $recipe.sha256 $archive
Assert-P4kxDocumentsSourceArchive $recipe $archive
$members = @(Invoke-P4kxLinuxTool tar @('--list', '--xz', '--file', $archive))
Assert-P4kxDocumentsArchiveMembers $members $recipe.version
Invoke-P4kxLinuxTool tar @('--extract', '--xz', '--file', $archive, '--directory', $stage) | Out-Null
$source = Join-Path $stage "xdg-desktop-portal-$($recipe.version)"
Assert-P4kxDocumentsSourceClosure $recipe $source
$build = Join-Path $stage 'build'
$null = Invoke-P4kxPortalMeasurement meson (@('setup', $build, $source, "--prefix=$output") + $recipe.options) $stage 'meson-setup' -Milliseconds 180000 -MaximumBytes 2MB
$null = Invoke-P4kxPortalMeasurement meson @('compile', '--verbose', '-C', $build, 'xdg-document-portal', 'xdg-permission-store') $stage 'meson-compile' -Milliseconds 600000 -MaximumBytes 8MB
$binaries = @(foreach ($name in @('xdg-document-portal', 'xdg-permission-store')) {
    $path = Join-Path $output $name
    Copy-Item -LiteralPath (Join-Path $build "document-portal/$name") -Destination $path
    $elf = Get-P4kxElfEvidence $path $platform
    [ordered]@{name=$name; path=$path; machine=$elf.machine; sha256=$elf.sha256; needed=$elf.needed}
})
$dependencies = @(foreach ($name in $recipe.pkg_config) {
    @{name=$name; version=(Invoke-P4kxLinuxTool pkg-config @('--modversion', $name) | Out-String).Trim()}
})
$tools = @(foreach ($tool in @('cc', 'meson', 'ninja', 'pkg-config')) {
    @{name=$tool; version=(Invoke-P4kxPortalMeasurement $tool @('--version') $stage "version-$tool").Trim()}
})
$packages = (Invoke-P4kxLinuxTool dpkg-query (@('--show', '--showformat=${binary:Package}\t${Version}\n') + $recipe.apt_packages) | Out-String).Trim()
$licenses = @(Get-ChildItem -LiteralPath $source -File -Recurse | Where-Object { $_.Name -cmatch '^(COPYING|LICENSE)(\..*)?$' } | ForEach-Object {
    @{path=[IO.Path]::GetRelativePath($source, $_.FullName); sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
if ($licenses.Count -lt 3) { throw 'Documents source closure license inventory is incomplete.' }
Copy-Item -LiteralPath $archive -Destination (Join-Path $output 'official-source.tar.xz')
$proof = [ordered]@{schema=1; target=$Target; revision=$env:GITHUB_SHA; source=$recipe; binaries=$binaries; tools=$tools; dependencies=$dependencies; apt_packages=$packages; licenses=$licenses; role='disposable CI services only'}
$proof | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $output 'services.json') -Encoding utf8NoBOM
