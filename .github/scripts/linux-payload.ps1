$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-elf.ps1')
. (Join-Path $PSScriptRoot 'linux-desktop.ps1')

function New-P4kxLinuxPayload {
    param([string]$Target, [string]$Version, [string]$Package, [string]$BinaryDirectory, [string]$OutputDirectory)
    $platform = Get-P4kxLinuxPlatform -Target $Target
    Assert-P4kxNativeHost -Platform $platform
    if ($Package -cnotin $platform.packages) { throw 'Package is not a Linux format in the shared contract.' }
    if (Test-Path -LiteralPath $OutputDirectory) { throw 'Linux payload output must be new.' }
    $binaryOutput = Join-Path $OutputDirectory 'bin'
    New-Item -ItemType Directory -Force -Path $binaryOutput | Out-Null
    $images = Get-P4kxLinuxApplicationEvidence -Directory $BinaryDirectory -Platform $platform
    foreach ($image in $images) {
        $destination = Join-Path $binaryOutput $image.name
        Copy-Item -LiteralPath (Join-Path $BinaryDirectory $image.name) -Destination $destination
        Invoke-P4kxLinuxTool chmod @('0755', $destination) | Out-Null
    }
    @{target=$Target; package=$Package} | ConvertTo-Json | Set-Content (Join-Path $binaryOutput 'p4kx-distribution.json') -Encoding utf8NoBOM
    $root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
    $licences = Join-Path $OutputDirectory 'share/licenses/p4kx'
    New-Item -ItemType Directory -Force -Path $licences | Out-Null
    foreach ($name in @('LICENSE-MIT', 'LICENSE-MIT.md', 'LICENSE-APACHE', 'LICENSE-APACHE.md', 'THIRD-PARTY-NOTICES.md', 'CHANGELOG.md')) {
        Copy-Item -LiteralPath (Join-Path $root $name) -Destination $licences
    }
    if ($env:SOURCE_DATE_EPOCH -cnotmatch '^[0-9]+$') { throw 'Linux payload requires a source-bound SOURCE_DATE_EPOCH.' }
    $date = [DateTimeOffset]::FromUnixTimeSeconds([long]$env:SOURCE_DATE_EPOCH).UtcDateTime.ToString('yyyy-MM-dd')
    Install-P4kxLinuxDesktop -Prefix $OutputDirectory -Version $Version -ReleaseDate $date
    return $images
}

function Write-P4kxLinuxPackageProducer {
    param(
        [string]$Target, [string]$Version, [string]$Package, [string]$Asset,
        [string]$BinaryDirectory, [object[]]$OriginalImages, [string[]]$Transformations, [string]$Output
    )
    $platform = Get-P4kxLinuxPlatform -Target $Target
    $images = @(Get-P4kxLinuxApplicationEvidence -Directory $BinaryDirectory -Platform $platform)
    foreach ($image in $images) {
        $original = @($OriginalImages | Where-Object name -CEQ $image.name)
        if ($original.Count -ne 1 -or $image.text_sha256 -cne $original[0].text_sha256) {
            throw 'Packaged executable code differs from the source release build.'
        }
    }
    if ($env:GITHUB_SHA -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') { throw 'Package producer requires a full source commit digest.' }
    $pins = & (Join-Path $PSScriptRoot 'check-build-tools.ps1') -Verify -Nextest
    $compiler = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
    $nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
    [ordered]@{
        schema=1; target=$Target; version=$Version; package=$Package; revision=$env:GITHUB_SHA
        compiler=$compiler; nextest=$nextest; binaries=$images; source_binaries=$OriginalImages
        archive_sha256=(Get-FileHash -LiteralPath $Asset -Algorithm SHA256).Hash.ToLowerInvariant()
        transformations=@($Transformations); minimum_distribution=(Get-P4kxLinuxPackaging).minimum_distribution
    } | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Output -Encoding utf8NoBOM
}
