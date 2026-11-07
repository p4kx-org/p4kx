param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-appdir-root.ps1')
if (-not $IsLinux) { throw 'AppDir root and dispatch test requires native Linux.' }
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root) -or [IO.FileInfo]::new($root).LinkTarget) { throw 'AppDir test directory must be new.' }
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$appDir = Join-Path $root 'original.AppDir'
$identity = (Get-P4kxLinuxPackaging).application_id
$desktopTarget = "usr/share/applications/$identity.desktop"
$iconTarget = "usr/share/icons/hicolor/64x64/apps/$identity.png"
$icon = Join-Path $appDir "$identity.png"
$desktop = Join-Path $appDir "$identity.desktop"
New-Item -ItemType Directory -Path (Join-Path $appDir 'usr/share/applications'), (Join-Path $appDir 'usr/share/icons/hicolor/64x64/apps'), (Join-Path $appDir 'usr/bin') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/p4kx.desktop') -Destination (Join-Path $appDir $desktopTarget)
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/icons/hicolor/64x64/apps/p4kx.png') -Destination (Join-Path $appDir $iconTarget)
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/AppRun') -Destination (Join-Path $appDir 'AppRun')
Invoke-P4kxLinuxTool '/usr/bin/chmod' @('0755', (Join-Path $appDir 'AppRun')) | Out-Null
Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $desktopTarget, $desktop) | Out-Null
Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $iconTarget, $icon) | Out-Null

function Assert-P4kxAppDirRejected {
    param([scriptblock]$Action)
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Invalid AppDir root layout was accepted.' }
}

function Set-P4kxTestLink {
    param([string]$Path, [string]$Target)
    Remove-Item -LiteralPath $Path -Force
    Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $Target, $Path) | Out-Null
}

Complete-P4kxAppDirRoot -AppDir $appDir
Complete-P4kxAppDirRoot -AppDir $appDir
if ((Get-Item -LiteralPath $desktop -Force).LinkTarget -cne $desktopTarget -or
    (Get-Item -LiteralPath $icon -Force).LinkTarget -cne $iconTarget -or
    (Get-Item -LiteralPath (Join-Path $appDir '.DirIcon') -Force).LinkTarget -cne "$identity.png") {
    throw 'AppDir completion replaced linuxdeploy root links.'
}
$external = Join-Path $root 'external.png'
Copy-Item -LiteralPath (Join-Path $appDir $iconTarget) -Destination $external
foreach ($target in @($external, (Join-Path $appDir $iconTarget), "../original.AppDir/$iconTarget", 'usr/share/icons/missing.png')) {
    Set-P4kxTestLink -Path $icon -Target $target
    Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
    if ((Get-Item -LiteralPath $icon -Force).LinkTarget -cne $target) { throw 'Invalid root icon was overwritten.' }
}
Set-P4kxTestLink -Path $icon -Target $iconTarget
$externalDesktop = Join-Path $root 'external.desktop'
Copy-Item -LiteralPath (Join-Path $appDir $desktopTarget) -Destination $externalDesktop
Set-P4kxTestLink -Path $desktop -Target $externalDesktop
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
Set-P4kxTestLink -Path $desktop -Target $desktopTarget
$extraDesktop = Join-Path $appDir 'unexpected.desktop'
Copy-Item -LiteralPath (Join-Path $appDir $desktopTarget) -Destination $extraDesktop
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
Remove-Item -LiteralPath $extraDesktop
Set-P4kxTestLink -Path (Join-Path $appDir '.DirIcon') -Target $desktopTarget
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
if ((Get-Item -LiteralPath (Join-Path $appDir '.DirIcon') -Force).LinkTarget -cne $desktopTarget) { throw 'Invalid .DirIcon was overwritten.' }
Set-P4kxTestLink -Path (Join-Path $appDir '.DirIcon') -Target "$identity.png"
[IO.File]::WriteAllText((Join-Path $appDir $iconTarget), 'modified icon')
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
Copy-Item -LiteralPath $external -Destination (Join-Path $appDir $iconTarget)
[IO.File]::AppendAllText((Join-Path $appDir 'AppRun'), "`nexit 1`n")
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/AppRun') -Destination (Join-Path $appDir 'AppRun')
Invoke-P4kxLinuxTool '/usr/bin/chmod' @('0644', (Join-Path $appDir 'AppRun')) | Out-Null
Assert-P4kxAppDirRejected { Complete-P4kxAppDirRoot -AppDir $appDir }
Invoke-P4kxLinuxTool '/usr/bin/chmod' @('0755', (Join-Path $appDir 'AppRun')) | Out-Null

$svgTarget = "usr/share/icons/hicolor/scalable/apps/$identity.svg"
New-Item -ItemType Directory -Path (Join-Path $appDir 'usr/share/icons/hicolor/scalable/apps') | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/icons/hicolor/scalable/apps/p4kx.svg') -Destination (Join-Path $appDir $svgTarget)
Remove-Item -LiteralPath $icon
Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $svgTarget, (Join-Path $appDir "$identity.svg")) | Out-Null
Set-P4kxTestLink -Path (Join-Path $appDir '.DirIcon') -Target "$identity.svg"
Complete-P4kxAppDirRoot -AppDir $appDir

$command = @'
#!/bin/sh
set -eu
printf '%s\n' "$0" "$@"
'@
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $path = Join-Path $appDir "usr/bin/$name"
    [IO.File]::WriteAllText($path, $command.Replace("`r`n", "`n") + "`n", [Text.UTF8Encoding]::new($false))
    Invoke-P4kxLinuxTool '/usr/bin/chmod' @('0755', $path) | Out-Null
}
$moved = Join-Path $root 'relocated package.AppDir'
Move-Item -LiteralPath $appDir -Destination $moved
Complete-P4kxAppDirRoot -AppDir $moved
$oldAppDir = $env:APPDIR
try {
    $env:APPDIR = $moved
    foreach ($case in @(@{option=@(); name='p4kx-gui'}, @{option=@('--cli'); name='p4kx'}, @{option=@('--mcp'); name='p4kx-mcp'})) {
        $arguments = @($case.option) + @('argument with spaces', 'Ü雪🚀')
        $output = @(Invoke-P4kxLinuxTool (Join-Path $moved 'AppRun') $arguments)
        $expected = @((Join-Path $moved "usr/bin/$($case.name)"), 'argument with spaces', 'Ü雪🚀')
        if (($output -join "`n") -cne ($expected -join "`n")) { throw 'Relocated AppRun changed dispatch or application arguments.' }
    }
}
finally { $env:APPDIR = $oldAppDir }
'PASS: existing linuxdeploy root links, icon identity, relocation, three real AppRun fixture dispatches and invalid-layout refusal.'
