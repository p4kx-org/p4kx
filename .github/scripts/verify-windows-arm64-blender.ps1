# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$ArtifactDirectory,
    [Parameter(Mandatory)][string]$BuildDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'pinned-download.ps1')

$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'windows' -or $platform.arch -cne 'arm64') {
    throw 'The portable Blender consumer requires native Windows ARM64.'
}
if (-not [IO.Path]::IsPathFullyQualified($ArtifactDirectory) -or
    -not (Test-Path -LiteralPath $ArtifactDirectory -PathType Container) -or
    -not [IO.Path]::IsPathFullyQualified($BuildDirectory) -or (Test-Path -LiteralPath $BuildDirectory)) {
    throw 'The Blender consumer requires the downloaded packages and a new absolute build directory.'
}

$version = '5.3.0'
$release = "blender-$version-windows-arm64"
$digest = 'c2e2ec602356fe2a33325538dc58c5a0c0ae068dfd0b3874d65de933df918452'
$stage = [IO.Directory]::CreateDirectory($BuildDirectory).FullName
$archive = Join-Path $stage "$release.zip"
Save-P4kxPinnedDownload "https://download.blender.org/release/Blender5.3/$release.zip" $digest $archive
Expand-Archive -LiteralPath $archive -DestinationPath $stage
$blender = Get-Item -LiteralPath (Join-Path $stage "$release/blender.exe")
if ($blender.LinkType -or (Get-ImageMachine -Path $blender.FullName -Os windows) -ne $platform.pe_machine) {
    throw 'Expected the pinned Windows ARM64 Blender executable.'
}

foreach ($variable in (Get-ChildItem Env: | Where-Object {
    $_.Name -match '^BLENDER_(USER|SYSTEM)_' -or $_.Name -in @('PYTHONHOME', 'PYTHONPATH')
})) {
    Remove-Item -LiteralPath ("Env:" + $variable.Name)
}
$env:BLENDER_USER_RESOURCES = [IO.Directory]::CreateDirectory((Join-Path $stage 'profile')).FullName
$env:TEMP = [IO.Directory]::CreateDirectory((Join-Path $stage 'temp')).FullName
$env:TMP = $env:TEMP
$env:PYTHONDONTWRITEBYTECODE = '1'

$tests = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../blender_addon/tests'))
$headless = @('--background', '--factory-startup', '--offline-mode', '--disable-autoexec',
    '--threads', '2', '--python-exit-code', '1')
& $blender.FullName @headless --python (Join-Path $tests 'blender_smoke.py') | Out-Host
if ($LASTEXITCODE -ne 0) { throw "Native Blender $version synthetic smoke test failed." }
& $blender.FullName @headless --python (Join-Path $tests 'verify_portable_packages.py') -- $ArtifactDirectory --target $Target --bpy-version $version | Out-Host
if ($LASTEXITCODE -ne 0) { throw "Native Blender $version package exchange failed." }
