$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')

function Get-P4kxLinuxTools {
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $tools = Get-Content -LiteralPath (Join-Path $root '.config/linux-package-tools.json') -Raw | ConvertFrom-Json
    $libraries = Get-Content -LiteralPath (Join-Path $root '.config/linux-runtime-libraries.json') -Raw | ConvertFrom-Json
    $architectures = @((Get-P4kxPlatformContract).targets | Where-Object os -EQ linux | Select-Object -ExpandProperty unix_arch | Sort-Object)
    if ($tools.schema -ne 1 -or (($libraries.PSObject.Properties.Name | Sort-Object) -join ',') -cne ($architectures -join ',')) {
        throw 'Linux host tools must cover exactly the supported Linux architectures.'
    }
    foreach ($arch in $architectures) {
        foreach ($name in @('linuxdeploy', 'appimagetool', 'cyclonedx')) {
            if ($tools.$name.sha256.$arch -cnotmatch '^[0-9a-f]{64}$' -or $tools.$name.url -cnotmatch '^https://github\.com/') {
                throw 'Linux packaging tool URL/digest is not pinned.'
            }
        }
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($package in $libraries.$arch) {
            if (-not $names.Add($package.name) -or $package.name -cnotmatch '^[a-z0-9][a-z0-9+_.-]+$' -or
                $package.version -cnotmatch '^[0-9][A-Za-z0-9+_.~-]+$' -or
                $package.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $package.recipe_commit -cnotmatch '^[0-9a-f]{40}$' -or
                $package.url -cnotmatch '^https://dl-cdn\.alpinelinux\.org/alpine/v3\.23/') {
                throw 'Static runtime library input identity is invalid.'
            }
        }
    }
    $tools.runtime | Add-Member -NotePropertyName libraries -NotePropertyValue $libraries
    return $tools
}

function Save-P4kxPinnedDownload {
    param([string]$Url, [string]$Digest, [string]$Destination)
    Invoke-WebRequest -Uri $Url -OutFile $Destination
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Digest) {
        throw "Pinned source/tool digest mismatch: $Url"
    }
}

function Install-P4kxLinuxPackageTools {
    param([string]$Target, [string]$Directory)
    $platform = Get-P4kxLinuxPlatform -Target $Target
    Assert-P4kxNativeHost -Platform $platform
    $tools = Get-P4kxLinuxTools
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $inventory = @(foreach ($name in @('linuxdeploy', 'appimagetool', 'cyclonedx')) {
        $tool = $tools.$name
        $url = $tool.url.Replace('{arch}', $platform.unix_arch)
        $download = Join-Path $Directory ([Uri]$url).Segments[-1]
        Save-P4kxPinnedDownload $url $tool.sha256.($platform.unix_arch) $download
        if ($name -eq 'cyclonedx') {
            $extract = Join-Path $Directory 'cyclonedx'
            New-Item -ItemType Directory -Force -Path $extract | Out-Null
            Invoke-P4kxLinuxTool tar @('--extract', '--xz', '--file', $download, '--directory', $extract) | Out-Null
            $files = @(Get-ChildItem -LiteralPath $extract -Recurse -File -Filter 'cargo-cyclonedx')
            if ($files.Count -ne 1) { throw 'Expected one native CycloneDX executable.' }
            $path = $files[0].FullName
            if ($env:GITHUB_PATH) { $files[0].DirectoryName >> $env:GITHUB_PATH }
        }
        else { $path = $download }
        if ((Get-ImageMachine -Path $path -Os linux) -ne $platform.elf_machine) { throw 'Packaging tool does not match the native CPU.' }
        Invoke-P4kxLinuxTool chmod @('0755', $path) | Out-Null
        [ordered]@{name=$name; version=$tool.version; role='host packaging/dependency analysis'; url=$url; archive_sha256=$tool.sha256.($platform.unix_arch)}
    })
    @{target=$Target; tools=$inventory} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory 'package-tools.json') -Encoding utf8NoBOM
}
