$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Install-P4kxLinuxDesktop {
    param([string]$Prefix, [string]$Version, [string]$ReleaseDate)
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $settings = Get-P4kxLinuxPackaging
    $identity = $settings.application_id
    $desktop = Get-Content -LiteralPath (Join-Path $root 'packaging/linux/p4kx.desktop') -Raw
    foreach ($field in @("Icon=$identity", "StartupWMClass=$identity")) {
        if ($field -cnotin ($desktop -split '\r?\n')) { throw 'Desktop source and canonical Linux application identity differ.' }
    }
    $desktopDirectory = Join-Path $Prefix 'share/applications'
    New-Item -ItemType Directory -Force -Path $desktopDirectory | Out-Null
    [IO.File]::WriteAllText((Join-Path $desktopDirectory "$identity.desktop"), $desktop, [Text.UTF8Encoding]::new($false))
    foreach ($item in @(
        @{source='application.metainfo.xml.in'; directory='share/metainfo'; name="$identity.metainfo.xml"}
    )) {
        $template = Get-Content -LiteralPath (Join-Path $root "packaging/linux/templates/$($item.source)") -Raw
        $content = $template.Replace('@APPLICATION_ID@', $identity).Replace('@VERSION@', $Version).Replace('@RELEASE_DATE@', $ReleaseDate)
        $destination = Join-Path $Prefix $item.directory
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        [IO.File]::WriteAllText((Join-Path $destination $item.name), $content, [Text.UTF8Encoding]::new($false))
    }
    $manuals = Join-Path $Prefix 'share/man/man1'
    New-Item -ItemType Directory -Force -Path $manuals | Out-Null
    Copy-Item -Path (Join-Path $root 'packaging/linux/man/*.1') -Destination $manuals
    $icons = Join-Path $root 'packaging/linux/icons/hicolor'
    foreach ($directory in Get-ChildItem -LiteralPath $icons -Directory) {
        foreach ($icon in Get-ChildItem -LiteralPath (Join-Path $directory.FullName 'apps') -File) {
            $destination = Join-Path $Prefix "share/icons/hicolor/$($directory.Name)/apps"
            New-Item -ItemType Directory -Force -Path $destination | Out-Null
            Copy-Item -LiteralPath $icon.FullName -Destination (Join-Path $destination "$identity$($icon.Extension)")
        }
    }
    Invoke-P4kxLinuxTool desktop-file-validate @((Join-Path $desktopDirectory "$identity.desktop")) | Out-Null
    Invoke-P4kxLinuxTool appstreamcli @('validate', '--no-net', (Join-Path $Prefix "share/metainfo/$identity.metainfo.xml")) | Out-Null
}
