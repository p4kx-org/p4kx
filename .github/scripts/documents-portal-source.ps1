$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-tools.ps1')

function ConvertFrom-P4kxDocumentsPortalRecipe {
    <#
    .SYNOPSIS
    Validates the immutable official Documents-service build inputs.
    #>
    param([Parameter(Mandatory)][string]$Json)
    $recipe = $Json | ConvertFrom-Json -Depth 8
    if ($recipe.schema -ne 1 -or $recipe.version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+$' -or
        $recipe.source_commit -cnotmatch '^[0-9a-f]{40}$' -or $recipe.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        ($recipe.bytes -isnot [int] -and $recipe.bytes -isnot [long]) -or $recipe.bytes -le 0 -or $recipe.bytes -gt 16MB) { throw 'Documents source identity is invalid.' }
    $url = "https://github.com/flatpak/xdg-desktop-portal/releases/download/$($recipe.version)/xdg-desktop-portal-$($recipe.version).tar.xz"
    if ($recipe.url -cne $url) { throw 'Documents source must be the exact official release asset.' }
    if ((@($recipe.wraps.PSObject.Properties.Name | Sort-Object) -join ',') -cne 'gvdb,libglnx') { throw 'Documents source closure differs.' }
    foreach ($wrap in $recipe.wraps.PSObject.Properties) {
        if ($wrap.Value -cnotmatch '^[0-9a-f]{40}$') { throw 'Documents vendored revision is not immutable.' }
    }
    $required = @('--wrap-mode=nodownload', '--buildtype=release', '-Dflatpak-interfaces=disabled',
        '-Dgeoclue=disabled', '-Dgudev=disabled', '-Dsystemd=disabled', '-Ddocumentation=disabled',
        '-Dtests=disabled', '-Dman-pages=disabled', '-Dsandboxed-image-validation=enabled', '-Dsandboxed-sound-validation=enabled')
    if ((@($recipe.options | Sort-Object) -join '|') -cne (($required | Sort-Object) -join '|')) { throw 'Documents build options exceed the reviewed service-only recipe.' }
    foreach ($list in @($recipe.apt_packages, $recipe.pkg_config)) {
        if (@($list).Count -eq 0 -or @($list | Sort-Object -Unique).Count -ne @($list).Count) { throw 'Documents build dependency list is empty or duplicated.' }
        foreach ($name in $list) { if ($name -cnotmatch '^[a-z0-9][a-z0-9+_.-]+$') { throw 'Documents build dependency name is invalid.' } }
    }
    return $recipe
}

function Get-P4kxDocumentsPortalRecipe {
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    return ConvertFrom-P4kxDocumentsPortalRecipe (Get-Content -LiteralPath (Join-Path $root '.config/documents-portal.json') -Raw)
}

function Assert-P4kxDocumentsSourceArchive {
    param($Recipe, [string]$Path)
    $item = Get-Item -LiteralPath $Path
    if ($item.PSIsContainer -or $item.Length -ne $Recipe.bytes -or
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Recipe.sha256) {
        throw 'Documents source asset length or digest differs.'
    }
}

function Assert-P4kxDocumentsArchiveMembers {
    param([string[]]$Members, [string]$Version)
    $prefix = "xdg-desktop-portal-$Version/"
    if ($Members.Count -eq 0 -or $Members.Count -gt 10000) { throw 'Documents source inventory is empty or exceeds its bound.' }
    foreach ($member in $Members) {
        if (-not $member.StartsWith($prefix, [StringComparison]::Ordinal) -or
            $member -match '[\r\n\0\\]' -or '..' -cin ($member -split '/')) { throw 'Documents source member leaves its owned extraction root.' }
    }
}

function Assert-P4kxDocumentsSourceClosure {
    param($Recipe, [string]$Directory)
    foreach ($wrap in $Recipe.wraps.PSObject.Properties) {
        $text = Get-Content -LiteralPath (Join-Path $Directory "subprojects/$($wrap.Name).wrap") -Raw
        if ($text -cnotmatch "(?m)^revision = $($wrap.Value)\s*$" -or
            -not (Test-Path -LiteralPath (Join-Path $Directory "subprojects/$($wrap.Name)/meson.build"))) {
            throw 'Documents vendored source is missing or has another revision.'
        }
    }
}
