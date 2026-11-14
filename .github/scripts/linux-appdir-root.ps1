$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Resolve-P4kxAppDirFile {
    param([string]$Root, [string]$Path)
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.PSIsContainer) { throw "AppDir root entry must resolve to a file: $Path" }
    if ($item.LinkType -and ([IO.Path]::IsPathRooted($item.LinkTarget) -or '..' -cin ($item.LinkTarget -split '/'))) {
        throw "AppDir root link must stay relative to its package: $Path"
    }
    $resolved = Invoke-P4kxLinuxTool '/usr/bin/realpath' @('--canonicalize-existing', '--', $item.FullName)
    if (-not $resolved.StartsWith("$Root/", [StringComparison]::Ordinal)) {
        throw "AppDir root entry resolves outside the package: $Path"
    }
    return $resolved
}

function Get-P4kxAppDirRootIcon {
    param([string]$Root, [string]$Identity, [string]$Repository)
    $icons = @(Get-ChildItem -LiteralPath $Root -Force | Where-Object Name -CIn @("$Identity.png", "$Identity.svg"))
    if ($icons.Count -ne 1) { throw 'AppDir must have exactly one canonical root icon.' }
    $resolved = Resolve-P4kxAppDirFile -Root $Root -Path $icons[0].FullName
    foreach ($directory in Get-ChildItem -LiteralPath (Join-Path $Repository 'packaging/linux/icons/hicolor') -Directory) {
        $installed = Join-Path $Root "usr/share/icons/hicolor/$($directory.Name)/apps/$($icons[0].Name)"
        if ($resolved -cne $installed) { continue }
        $original = Join-Path $directory.FullName "apps/p4kx$($icons[0].Extension)"
        if ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash) {
            throw 'AppDir root icon bytes differ from the application icon.'
        }
        return $icons[0]
    }
    throw 'AppDir root icon does not select an installed application icon.'
}

function Complete-P4kxAppDirRoot {
    param([Parameter(Mandatory)][string]$AppDir)
    if (-not $IsLinux) { throw 'AppDir root verification requires native Linux.' }
    $root = Invoke-P4kxLinuxTool '/usr/bin/realpath' @('--canonicalize-existing', '--', $AppDir)
    $repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $identity = (Get-P4kxLinuxPackaging).application_id
    $desktops = @(Get-ChildItem -LiteralPath $root -Filter '*.desktop' -Force)
    if ($desktops.Count -ne 1 -or $desktops[0].Name -cne "$identity.desktop") {
        throw 'AppDir must have exactly one canonical root desktop file.'
    }
    $desktop = Resolve-P4kxAppDirFile -Root $root -Path $desktops[0].FullName
    if ($desktop -cne (Join-Path $root "usr/share/applications/$identity.desktop") -or
        (Get-FileHash -LiteralPath $desktop -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath (Join-Path $repository 'packaging/linux/p4kx.desktop') -Algorithm SHA256).Hash) {
        throw 'AppDir root desktop differs from the installed application desktop.'
    }
    $icon = Get-P4kxAppDirRootIcon -Root $root -Identity $identity -Repository $repository
    $appRun = Get-Item -LiteralPath (Join-Path $root 'AppRun') -Force
    if ($appRun.PSIsContainer -or $appRun.LinkType -or
        (Get-FileHash -LiteralPath $appRun.FullName -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath (Join-Path $repository 'packaging/linux/AppRun') -Algorithm SHA256).Hash -or
        ([IO.File]::GetUnixFileMode($appRun.FullName) -band [IO.UnixFileMode]::UserExecute) -eq 0) {
        throw 'AppDir AppRun must retain the executable application dispatch script.'
    }
    $dirIcon = Join-Path $root '.DirIcon'
    if (-not [IO.Path]::Exists($dirIcon) -and -not [IO.FileInfo]::new($dirIcon).LinkTarget) {
        Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $icon.Name, $dirIcon) | Out-Null
    }
    if ((Resolve-P4kxAppDirFile -Root $root -Path $dirIcon) -cne (Resolve-P4kxAppDirFile -Root $root -Path $icon.FullName)) {
        throw 'AppDir .DirIcon does not select the canonical root icon.'
    }
}
