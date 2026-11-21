$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-bundle.ps1')
. (Join-Path $PSScriptRoot 'macos-resources.ps1')

function Copy-P4kxMacMountedBundle {
    param(
        [Parameter(Mandatory)][string]$Dmg,
        [Parameter(Mandatory)][string]$TaskDirectory
    )
    $mount = Join-Path $TaskDirectory 'mount'
    $install = Join-Path $TaskDirectory 'relocated package'
    $null = New-Item -ItemType Directory -Path $mount, $install
    $source = Join-Path $mount 'P4KX.app'
    $destination = Join-Path $install 'P4KX.app'
    $attached = $false
    try {
        $mountResult = @(Invoke-P4kxMacTool /usr/bin/hdiutil @('attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', $mount, $Dmg))
        $attached = $true
        $mountResult | Set-Content -LiteralPath (Join-Path $TaskDirectory 'attach.log') -Encoding utf8NoBOM
        $app = Get-Item -LiteralPath $source
        $link = Get-Item -LiteralPath (Join-Path $mount 'Applications')
        if ($app -isnot [IO.DirectoryInfo] -or $app.LinkType -or $link.LinkType -cne 'SymbolicLink' -or $link.LinkTarget -cne '/Applications') {
            throw 'Mounted DMG is missing its regular app bundle or Applications link.'
        }
        Invoke-P4kxMacTool /usr/bin/ditto @($source, $destination) | Out-Host
    }
    finally {
        if ($attached -or (Test-Path -LiteralPath $source -PathType Container)) {
            Invoke-P4kxMacTool /usr/bin/hdiutil @('detach', $mount) |
                Set-Content -LiteralPath (Join-Path $TaskDirectory 'detach.log') -Encoding utf8NoBOM
        }
    }
    return $destination
}

function Assert-P4kxMacRelocatedBundle {
    param(
        [Parameter(Mandatory)][string]$Bundle,
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)]$Plan
    )
    $paths = Get-P4kxMacBundlePaths -Bundle $Bundle
    $core = ($Version -split '[-+]', 2)[0]
    $expected = [ordered]@{
        CFBundleExecutable = 'p4kx-gui'
        CFBundleIdentifier = 'io.github.p4kx-org.p4kx'
        CFBundleShortVersionString = $core
        CFBundleVersion = $core
        CFBundlePackageType = 'APPL'
        LSMinimumSystemVersion = $Platform.deployment_target
    }
    foreach ($key in $expected.Keys) {
        $value = @(Invoke-P4kxMacTool /usr/bin/plutil @('-extract', $key, 'raw', '-o', '-', $paths.plist))
        if ($value.Count -ne 1 -or $value[0] -cne $expected[$key]) {
            throw "Relocated app bundle has inconsistent $key."
        }
    }
    Assert-P4kxMacSignature -Path $Bundle -Plan $Plan -Bundle -ExecutableCode
    foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $binary = Join-Path $paths.binaries $name
        Assert-P4kxMacSignature -Path $binary -Plan $Plan -ExecutableCode
        Assert-P4kxMacSystemLibraries -Binary $binary
    }
    return $paths
}
