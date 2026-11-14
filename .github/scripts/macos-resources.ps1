$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-signing.ps1')

function New-P4kxMacIcon {
    param([Parameter(Mandatory)][string]$Repository, [Parameter(Mandatory)][string]$Directory)
    $iconset = Join-Path $Directory 'P4KX.iconset'
    $output = Join-Path $Directory 'P4KX.icns'
    $null = New-Item -ItemType Directory -Path $iconset
    $assets = Join-Path $Repository 'packaging/linux/icons/hicolor'
    foreach ($size in @(16, 32, 128, 256, 512)) {
        $image = Join-Path $assets "${size}x${size}/apps/p4kx.png"
        Copy-Item -LiteralPath $image -Destination (Join-Path $iconset "icon_${size}x${size}.png")
        $largeSize = $size * 2
        $largeImage = Join-Path $assets "${largeSize}x${largeSize}/apps/p4kx.png"
        $retina = Join-Path $iconset "icon_${size}x${size}@2x.png"
        if (Test-Path -LiteralPath $largeImage -PathType Leaf) {
            Copy-Item -LiteralPath $largeImage -Destination $retina
        } else {
            Invoke-P4kxMacTool /usr/bin/sips @('-z', "$largeSize", "$largeSize", $image, '--out', $retina) | Out-Host
        }
    }
    Invoke-P4kxMacTool /usr/bin/iconutil @('--convert', 'icns', '--output', $output, $iconset) | Out-Host
    $output
}

function Assert-P4kxMacSystemLibraries {
    param([Parameter(Mandatory)][string]$Binary)
    $output = @(Invoke-P4kxMacTool /usr/bin/otool @('-L', $Binary))
    if ($output.Count -lt 2) { throw 'Missing native library dependency inventory.' }
    foreach ($line in $output[1..($output.Count - 1)]) {
        if ($line -cnotmatch '^\s+(?<path>.+) \(compatibility version .+\)$') {
            throw 'Unrecognized native library dependency.'
        }
        $dependency = $Matches.path
        if (-not ($dependency.StartsWith('/System/Library/Frameworks/', [StringComparison]::Ordinal) -or
            $dependency.StartsWith('/usr/lib/', [StringComparison]::Ordinal))) {
            throw "Unbundled non-system library dependency: $dependency"
        }
    }
}
