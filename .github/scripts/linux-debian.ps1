$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Get-P4kxDebianPackageDependencies {
    <#
    .SYNOPSIS
    Scans the three staged executables with their matching Debian control directory.
    #>
    param([Parameter(Mandatory)][string]$StagingDirectory)
    $stage = [IO.Path]::GetFullPath($StagingDirectory, $PWD.ProviderPath)
    if (-not (Test-Path -LiteralPath (Join-Path $stage 'debian/control') -PathType Leaf)) { throw 'Debian dependency scan requires its staged source control file.' }
    $scan = @(foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $path = Join-Path $stage "package/usr/lib/p4kx/$binary"
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Debian dependency scan payload is missing: $binary" }
        '-e' + $path
    })
    Push-Location -LiteralPath $stage
    try {
        return Get-P4kxDebianDependencies @(Invoke-P4kxLinuxTool dpkg-shlibdeps (@('-O') + $scan))
    }
    finally { Pop-Location }
}

function Get-P4kxDebianVersion {
    param([Parameter(Mandatory)][string]$Version)
    if ($Version -cnotmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$') { throw 'Invalid upstream package version.' }
    return [regex]::new('-').Replace($Version, '~', 1) + '-1'
}

function Get-P4kxDebianDependencies {
    param([string[]]$SharedLibraryDependencies)
    $dependencies = [Collections.Generic.List[string]]::new()
    foreach ($item in $SharedLibraryDependencies) {
        foreach ($dependency in ($item -replace '^shlibs:Depends=', '') -split ',\s*') {
            if ($dependency -and $dependency -notmatch '^libc6(?:\s|:|$)') { $dependencies.Add($dependency) }
        }
    }
    $glibc = (Get-P4kxLinuxPackaging).maximum_glibc_requirement
    $dependencies.Add("libc6 (>= $glibc)")
    foreach ($dependency in @(
        'libx11-6', 'libxcursor1', 'libxi6', 'libxrandr2', 'libxkbcommon0', 'libxkbcommon-x11-0',
        'libwayland-client0', 'libwayland-cursor0', 'libwayland-egl1', 'libegl1', 'libgl1',
        'libvulkan1', 'libdbus-1-3', 'xdg-desktop-portal'
    )) {
        $pattern = '^' + [regex]::Escape($dependency) + '(?:\s|:|$)'
        if (@($dependencies | Where-Object { $_ -match $pattern }).Count -eq 0) { $dependencies.Add($dependency) }
    }
    return ($dependencies | Sort-Object -Unique) -join ', '
}

function Write-P4kxDebianCopyright {
    param([string]$OutputDirectory)
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $mit = Get-Content -LiteralPath (Join-Path $root 'LICENSE-MIT') -Raw
    $licence = @($mit.TrimEnd() -split '\r?\n' | ForEach-Object { if ($_ -eq '') { ' .' } else { " $_" } }) -join "`n"
    @"
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: P4KX
Upstream-Contact: P4KX contributors <lucas.greuloch@gmail.com>
Source: https://github.com/p4kx-org/p4kx

Files: *
Copyright: 2026 the P4KX contributors and the authors listed in THIRD-PARTY-NOTICES.md
License: P4KX-and-dependencies
 P4KX source is available under MIT or Apache-2.0, reproduced below.
 The linked Rust dependencies retain their own licences and copyright notices.
 Their complete original notices are in THIRD-PARTY-NOTICES.md in this directory.

Files: usr/share/applications/* usr/share/icons/* usr/share/metainfo/* usr/share/man/* usr/share/doc/p4kx/changelog*
Copyright: 2026 the P4KX contributors
License: MIT or Apache-2.0

License: P4KX-and-dependencies
 The executable combines P4KX source under MIT or Apache-2.0 with dependencies
 under their respective terms. All dependency copyright notices and full licence
 texts are shipped in /usr/share/doc/p4kx/THIRD-PARTY-NOTICES.md.

License: MIT
$licence

License: Apache-2.0
 On Debian and Ubuntu systems the complete Apache License, Version 2.0, is
 available in /usr/share/common-licenses/Apache-2.0.
"@ | Set-Content -LiteralPath (Join-Path $OutputDirectory 'copyright') -Encoding utf8NoBOM
}

function Write-P4kxDebianMd5Sums {
    param([string]$PackageRoot, [string]$Output)
    $lines = @(foreach ($file in Get-ChildItem -LiteralPath (Join-Path $PackageRoot 'usr') -File -Recurse | Sort-Object FullName) {
        if ($file.LinkType) { continue }
        $relative = [IO.Path]::GetRelativePath([IO.Path]::GetFullPath($PackageRoot), $file.FullName)
        '{0}  {1}' -f (Get-FileHash -LiteralPath $file.FullName -Algorithm MD5).Hash.ToLowerInvariant(), $relative
    })
    $lines | Set-Content -LiteralPath $Output -Encoding utf8NoBOM
}
