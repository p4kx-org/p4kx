$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')

function Get-P4kxFileRangeHash {
    param([string]$Path, [uint64]$Offset, [uint64]$Length)
    $stream = [IO.File]::OpenRead($Path)
    $hasher = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    try {
        if ($Offset -gt [uint64]$stream.Length -or $Length -gt [uint64]$stream.Length - $Offset) {
            throw 'ELF executable section exceeds the file.'
        }
        $stream.Position = [long]$Offset
        $buffer = [byte[]]::new(64KB)
        $remaining = $Length
        while ($remaining -gt 0) {
            $count = [int][Math]::Min([uint64]$buffer.Length, $remaining)
            $read = $stream.Read($buffer, 0, $count)
            if ($read -eq 0) { throw 'ELF executable section is truncated.' }
            $hasher.AppendData($buffer, 0, $read)
            $remaining -= [uint64]$read
        }
        return [Convert]::ToHexString($hasher.GetHashAndReset()).ToLowerInvariant()
    }
    finally { $hasher.Dispose(); $stream.Dispose() }
}

function Get-P4kxElfTextRange {
    param([Parameter(Mandatory)][string[]]$SectionHeaders)
    $matchesFound = @($SectionHeaders | Where-Object { $_ -match '^\s*\[\s*\d+\]\s+\.text\s+PROGBITS\s+[0-9a-f]+\s+[0-9a-f]+\s+[0-9a-f]+\s' })
    if ($matchesFound.Count -ne 1 -or $matchesFound[0] -notmatch '\.text\s+PROGBITS\s+[0-9a-f]+\s+([0-9a-f]+)\s+([0-9a-f]+)\s') {
        throw 'ELF must contain exactly one executable .text section.'
    }
    $range = @{offset=[Convert]::ToUInt64($Matches[1], 16); length=[Convert]::ToUInt64($Matches[2], 16)}
    if ($range.length -eq 0) { throw 'ELF executable .text section is empty.' }
    return $range
}

function Get-P4kxGlibcRequirements {
    param([string[]]$VersionInfo)
    return @($VersionInfo | ForEach-Object {
        foreach ($match in [regex]::Matches($_, 'Name:\s+GLIBC_([0-9]+(?:\.[0-9]+)+)')) { $match.Groups[1].Value }
    } | Sort-Object { [version]$_ } -Descending -Unique)
}

function Assert-P4kxGlibcBaseline {
    param([string[]]$Requirements, [string]$Baseline)
    foreach ($requirement in $Requirements) {
        if ([version]$requirement -gt [version]$Baseline) { throw "ELF requires glibc $requirement, above $Baseline." }
    }
}

function Get-P4kxElfEvidence {
    param([string]$Path, $Platform)
    Assert-P4kxNativeHost -Platform $Platform
    if ((Get-ImageMachine -Path $Path -Os linux) -ne $Platform.elf_machine) { throw 'ELF CPU differs from native package target.' }
    $sections = @(Invoke-P4kxLinuxTool readelf @('--section-headers', '--wide', $Path))
    $range = Get-P4kxElfTextRange -SectionHeaders $sections
    $dynamic = @(Invoke-P4kxLinuxTool readelf @('--dynamic', '--wide', $Path))
    $versions = @(Invoke-P4kxLinuxTool readelf @('--version-info', '--wide', $Path))
    $glibc = @(Get-P4kxGlibcRequirements -VersionInfo $versions)
    $baseline = (Get-P4kxLinuxPackaging).maximum_glibc_requirement
    Assert-P4kxGlibcBaseline -Requirements $glibc -Baseline $baseline
    return [ordered]@{
        machine=$Platform.elf_machine
        sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        text_sha256=(Get-P4kxFileRangeHash -Path $Path -Offset $range.offset -Length $range.length)
        glibc_versions=$glibc
        needed=@($dynamic | ForEach-Object { if ($_ -match '\(NEEDED\).*\[([^\]]+)\]') { $Matches[1] } })
        runpaths=@($dynamic | ForEach-Object { if ($_ -match '\((?:RUNPATH|RPATH)\).*\[([^\]]*)\]') { $Matches[1] } })
    }
}

function Get-P4kxLinuxApplicationEvidence {
    param([string]$Directory, $Platform)
    return @(foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $image = Get-P4kxElfEvidence -Path (Join-Path $Directory $name) -Platform $Platform
        $image['name'] = $name
        $image
    })
}
