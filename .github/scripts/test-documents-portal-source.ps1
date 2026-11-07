<#
.SYNOPSIS
Checks official service source, closure and recipe contracts without building or starting services.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-source.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Documents source test directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$recipe = Get-P4kxDocumentsPortalRecipe
if ($recipe.version -cne '1.22.1' -or $recipe.bytes -ne 1264992) { throw 'Official reviewed source pin differs.' }
$original = $recipe | ConvertTo-Json -Depth 8
foreach ($mutation in @(
    { param($value) $value.url = 'https://invalid.example/source.tar.xz' },
    { param($value) $value.sha256 = 'missing' },
    { param($value) $value.bytes = 0 },
    { param($value) $value.wraps.gvdb = 'HEAD' },
    { param($value) $value.options += '-Dsandboxed-image-validation=disabled' },
    { param($value) $value.options = @($value.options | Where-Object { $_ -cne '--wrap-mode=nodownload' }) },
    { param($value) $value.apt_packages += $value.apt_packages[0] },
    { param($value) $value.pkg_config += 'name;untrusted' }
)) {
    $value = $original | ConvertFrom-Json -Depth 8
    & $mutation $value
    $rejected = $false
    try { ConvertFrom-P4kxDocumentsPortalRecipe ($value | ConvertTo-Json -Depth 8) | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'A mutable or security-reduced source recipe was accepted.' }
}
$prefix = 'xdg-desktop-portal-1.22.1/'
Assert-P4kxDocumentsArchiveMembers @($prefix, "${prefix}meson.build") $recipe.version
foreach ($members in @(@(), @('/absolute'), @("${prefix}../escape"), @("${prefix}bad\name"), @('another-root/file'))) {
    $rejected = $false
    try { Assert-P4kxDocumentsArchiveMembers $members $recipe.version } catch { $rejected = $true }
    if (-not $rejected) { throw 'A source extraction member left its root or had no inventory.' }
}
$asset = Join-Path $Directory 'synthetic-source.bin'
[IO.File]::WriteAllBytes($asset, [byte[]]@(1, 2, 3))
$synthetic = @{bytes=3; sha256=(Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()}
Assert-P4kxDocumentsSourceArchive $synthetic $asset
$synthetic.bytes = 4
$rejected = $false
try { Assert-P4kxDocumentsSourceArchive $synthetic $asset } catch { $rejected = $true }
if (-not $rejected) { throw 'An incomplete source download was accepted.' }
'PASS: pinned recipe, eight unsafe mutations, extraction roots and exact asset length/hash; no service built or started.'
