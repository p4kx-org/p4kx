$ErrorActionPreference = 'Stop'

function Get-P4kxMacBundlePaths {
    param([Parameter(Mandatory)][string]$Bundle)
    $contents = Join-Path $Bundle 'Contents'
    $resources = Join-Path $contents 'Resources'
    [pscustomobject]@{
        binaries = Join-Path $contents 'MacOS'
        resources = $resources
        notices = Join-Path $resources 'Notices'
        identity = Join-Path $resources 'p4kx-distribution.json'
        plist = Join-Path $contents 'Info.plist'
        icon = Join-Path $resources 'P4KX.icns'
    }
}

function New-P4kxMacBundle {
    param(
        [Parameter(Mandatory)][string]$Destination,
        [Parameter(Mandatory)][string]$BinaryDirectory,
        [Parameter(Mandatory)][string]$Icon,
        [Parameter(Mandatory)][string]$Repository,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)]$Platform
    )

    if ($Platform.os -cne 'macos' -or $Platform.target -cne 'aarch64-apple-darwin' -or
        $Platform.deployment_target -cnotmatch '^[0-9]+\.[0-9]+$' -or
        @($Platform.packages).Count -ne 1 -or $Platform.packages[0] -cne 'dmg') {
        throw 'The bundle requires the Apple Silicon DMG platform contract.'
    }
    if ($Version -cnotmatch '^(?<core>(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') {
        throw 'The bundle version must be a semantic version.'
    }
    $core = $Matches.core
    if ((Split-Path -Leaf $Destination) -cne 'P4KX.app' -or (Test-Path -LiteralPath $Destination)) {
        throw 'The destination must be a fresh P4KX.app.'
    }
    $paths = Get-P4kxMacBundlePaths $Destination
    $copies = @(
        foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
            @{ source = Join-Path $BinaryDirectory $name; destination = Join-Path $paths.binaries $name }
        }
        foreach ($name in @('LICENSE-MIT', 'LICENSE-MIT.md', 'LICENSE-APACHE', 'LICENSE-APACHE.md', 'THIRD-PARTY-NOTICES.md', 'CHANGELOG.md')) {
            @{ source = Join-Path $Repository $name; destination = Join-Path $paths.notices $name }
        }
        @{ source = $Icon; destination = $paths.icon }
    )
    foreach ($copy in $copies) {
        $file = Get-Item -LiteralPath $copy.source
        if ($file -isnot [IO.FileInfo] -or $file.LinkType -or $file.Length -eq 0) {
            throw "Expected a nonempty regular bundle input: $($copy.source)"
        }
    }
    $template = Get-Content -LiteralPath (Join-Path $Repository 'packaging/macos/Info.plist') -Raw
    if (-not $template.Contains('@BUNDLE_VERSION@') -or -not $template.Contains('@MINIMUM_OS@')) {
        throw 'Bundle metadata is missing its required version placeholders.'
    }
    $null = New-Item -ItemType Directory -Path $paths.binaries, $paths.notices
    foreach ($copy in $copies) {
        Copy-Item -LiteralPath $copy.source -Destination $copy.destination
    }
    $template.Replace('@BUNDLE_VERSION@', $core).Replace('@MINIMUM_OS@', $Platform.deployment_target) |
        Set-Content -LiteralPath $paths.plist -Encoding utf8NoBOM
    [ordered]@{ target = $Platform.target; package = 'dmg' } | ConvertTo-Json -Compress |
        Set-Content -LiteralPath $paths.identity -Encoding utf8NoBOM
    $paths
}
