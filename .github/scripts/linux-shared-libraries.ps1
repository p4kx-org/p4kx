$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-elf.ps1')

function Find-P4kxNativeLibrary {
    param([string]$Name, $Platform)
    $cache = @(Invoke-P4kxLinuxTool ldconfig @('-p'))
    $pattern = '^\s*' + [regex]::Escape($Name) + '\s+\([^)]*\)\s+=>\s+(\S+)\s*$'
    foreach ($line in $cache) {
        if ($line -notmatch $pattern) { continue }
        $path = $Matches[1]
        $header = (Invoke-P4kxLinuxTool readelf @('--file-header', '--wide', $path)) -join "`n"
        if ($header -notmatch 'Class:\s+ELF64') { continue }
        if ((Get-ImageMachine -Path $path -Os linux) -eq $Platform.elf_machine) { return $path }
    }
    throw "Required native runtime library is unavailable: $Name"
}

function Get-P4kxLibraryPackage {
    param([string]$Path, $Platform)
    $Path = (Invoke-P4kxLinuxTool realpath @('--', $Path) | Out-String).Trim()
    $owners = @(Invoke-P4kxLinuxTool dpkg-query @('--search', $Path))
    $matchesFound = @($owners | Where-Object { $_ -match '^([^ ]+):\s+/' })
    if ($matchesFound.Count -ne 1 -or $matchesFound[0] -notmatch '^([^ ]+):\s+/') { throw "Library source package ownership is ambiguous: $Path" }
    $package = $Matches[1]
    $format = '${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\t${Architecture}\n'
    $fields = ((Invoke-P4kxLinuxTool dpkg-query @('--show', "--showformat=$format", $package)) -join '').Trim() -split "`t"
    if ($fields.Count -ne 5 -or $fields[4] -cne $Platform.deb_arch) { throw 'Bundled library package architecture differs.' }
    return [pscustomobject]@{
        package=$fields[0]; version=$fields[1]; source_package=$fields[2]; source_version=$fields[3]
        source_path=$Path; copyright=(Join-Path '/usr/share/doc' (($package -replace ':[^:]+$', '') + '/copyright'))
    }
}

function Export-P4kxLibrarySources {
    param([object[]]$Packages, [string]$OutputDirectory)
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    Push-Location $OutputDirectory
    try {
        foreach ($group in $Packages | Group-Object source_package, source_version) {
            $package = $group.Group[0]
            if (-not $package.source_package -or -not $package.source_version) { throw 'Bundled library lacks corresponding-source identity.' }
            Invoke-P4kxLinuxTool apt-get @('source', '--download-only', "$($package.source_package)=$($package.source_version)") | Out-Null
        }
    }
    finally { Pop-Location }
    return @(Get-ChildItem -LiteralPath $OutputDirectory -File | Sort-Object Name | ForEach-Object {
        @{name=$_.Name; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}

function Write-P4kxAppImageLibraryInventory {
    param([string]$AppDir, [string]$SourceDirectory, $Platform)
    $libraryDirectory = Join-Path $AppDir 'usr/lib'
    $licences = Join-Path $AppDir 'usr/share/licenses/p4kx-system'
    New-Item -ItemType Directory -Force -Path $licences | Out-Null
    $packages = [Collections.Generic.List[object]]::new()
    $images = @(foreach ($file in Get-ChildItem -LiteralPath $libraryDirectory -Recurse -File | Sort-Object FullName) {
        if ($file.LinkType -or $file.Name -notmatch '\.so(?:\.|$)') { continue }
        $dynamic = @(Invoke-P4kxLinuxTool readelf @('--dynamic', '--wide', $file.FullName))
        $soname = @($dynamic | ForEach-Object { if ($_ -match '\(SONAME\).*\[([^\]]+)\]') { $Matches[1] } })
        if ($soname.Count -ne 1) { throw "Bundled runtime library has no unique SONAME: $($file.Name)" }
        $original = Find-P4kxNativeLibrary -Name $soname[0] -Platform $Platform
        $package = Get-P4kxLibraryPackage -Path $original -Platform $Platform
        $packages.Add($package)
        $noticeDirectory = Join-Path $licences ($package.package -replace ':', '_')
        New-Item -ItemType Directory -Force -Path $noticeDirectory | Out-Null
        Copy-Item -LiteralPath $package.copyright -Destination (Join-Path $noticeDirectory 'copyright')
        if ((Get-ImageMachine -Path $file.FullName -Os linux) -ne $Platform.elf_machine) { throw 'Bundled library has a different CPU.' }
        [ordered]@{
            path=[IO.Path]::GetRelativePath($AppDir, $file.FullName); soname=$soname[0]; machine=$Platform.elf_machine
            sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            package=$package.package; version=$package.version; source_package=$package.source_package; source_version=$package.source_version
        }
    })
    $common = Join-Path $licences 'common'
    Copy-Item -LiteralPath '/usr/share/common-licenses' -Destination $common -Recurse
    $sources = @(Export-P4kxLibrarySources -Packages $packages.ToArray() -OutputDirectory $SourceDirectory)
    [ordered]@{target=$Platform.target; libraries=$images; sources=$sources} | ConvertTo-Json -Depth 12 |
        Set-Content (Join-Path $AppDir 'usr/share/licenses/p4kx-system/inventory.json') -Encoding utf8NoBOM
}
