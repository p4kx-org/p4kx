param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$AppDir,
    [Parameter(Mandatory)][string]$RuntimeDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-tools.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxNativeHost $platform
$libraries = Get-Content -LiteralPath (Join-Path $AppDir 'usr/share/licenses/p4kx-system/inventory.json') -Raw | ConvertFrom-Json
$runtime = Get-Content -LiteralPath (Join-Path $RuntimeDirectory 'runtime-inventory.json') -Raw | ConvertFrom-Json
if ($libraries.target -cne $Target -or -not $libraries.libraries -or -not $libraries.sources -or -not $runtime.dependencies) { throw 'Linux runtime/source SBOM inventory is incomplete.' }
$sourceName = "P4KX-$Version-$Target-appimage-corresponding-source.tar.gz"
$sourceDigest = (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $sourceName) -Algorithm SHA256).Hash.ToLowerInvariant()
$components = @(foreach ($library in $libraries.libraries) {
    $path = Join-Path $AppDir $library.path
    if ($library.machine -ne $platform.elf_machine -or
        $library.sha256 -cne (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'SBOM shared library payload CPU/hash differs.' }
    [ordered]@{
        type='file'; name=$library.soname; version=$library.version; 'bom-ref'="linux-appimage:$Target/$($library.path)"
        hashes=@(@{alg='SHA-256'; content=$library.sha256})
        licenses=@(@{license=@{name="Original Ubuntu copyright and licence notices for $($library.package)"}})
        properties=@(
            @{name='p4kx:package-format'; value='app-image'}, @{name='p4kx:source-package'; value=$library.source_package},
            @{name='p4kx:source-version'; value=$library.source_version}, @{name='p4kx:corresponding-source'; value=$sourceName}
        )
    }
})
$components += [pscustomobject]@{
    type='file'; name='AppImage type2 runtime'; version=$runtime.runtime_source_commit; 'bom-ref'="appimage-runtime:$Target"
    hashes=@(@{alg='SHA-256'; content=$runtime.runtime_sha256})
    licenses=@(@{license=@{id='MIT'}})
    properties=@(@{name='p4kx:corresponding-source'; value=$sourceName}, @{name='p4kx:builder-image'; value=$runtime.builder_image})
}
foreach ($dependency in $runtime.dependencies) {
    if (-not $dependency.name -or -not $dependency.version -or -not $dependency.licences) { throw 'Runtime dependency has no source version or original licence evidence.' }
    $properties = @(@{name='p4kx:corresponding-source'; value=$sourceName}, @{name='p4kx:linkage'; value='static AppImage runtime'})
    if ($dependency.recipe_commit) { $properties += @{name='p4kx:source-recipe'; value=$dependency.recipe_commit} }
    $components += [pscustomobject]@{
        type='library'; name=$dependency.name; version=$dependency.version; 'bom-ref'="appimage-static:$Target/$($dependency.name)"
        licenses=@(@{license=@{name="Original $($dependency.name) licence notices in corresponding source and AppImage"}})
        properties=$properties
    }
}
$components += [pscustomobject]@{
    type='file'; name=$sourceName; 'bom-ref'="appimage-sources:$Target"
    hashes=@(@{alg='SHA-256'; content=$sourceDigest})
}
foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $path = Join-Path $AssetDirectory "P4KX-$Version-$Target-$binary.cdx.json"
    $sbom = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $sbom.components = @($sbom.components) + $components
    $sbom | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
}
