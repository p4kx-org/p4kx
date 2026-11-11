param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$ArchivePath,
    [Parameter(Mandatory)][string]$MetadataPath,
    [string]$BinaryDirectory,
    [switch]$ValidateArchive
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row

$toolCheck = Join-Path $PSScriptRoot 'check-build-tools.ps1'
$pins = @(& $toolCheck -Target $Target -Verify -Nextest)
$compilerVersion = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
$runnerVersion = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
$archiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
$head = @(& git rev-parse HEAD)
if ($LASTEXITCODE -ne 0 -or $head.Count -ne 1 -or $head[0] -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$' -or
    ($env:GITHUB_SHA -and $env:GITHUB_SHA -cne $head[0])) {
    throw 'Native archive requires the actual checked-out source revision.'
}
$revision = $head[0]

. (Join-Path $PSScriptRoot 'native-application-binaries.ps1')

if ($ValidateArchive) {
    $metadata = Get-Content -LiteralPath $MetadataPath -Raw | ConvertFrom-Json
    Assert-P4kxArchiveEvidence -Metadata $metadata -Platform $row -Revision $revision -Compiler $compilerVersion -Nextest $runnerVersion -ArchiveHash $archiveHash
    Write-Output "Validated native test archive for $Target at $revision."
    return
}
if (-not $BinaryDirectory) { throw 'A native producer requires BinaryDirectory.' }
$binaries = @(Get-P4kxApplicationBinaryEvidence -Directory $BinaryDirectory -Platform $row)
$workspace = & cargo metadata --no-deps --format-version 1 --locked
if ($LASTEXITCODE -ne 0) { throw 'Application version query failed.' }
$versions = @(($workspace | ConvertFrom-Json).packages | Where-Object { $_.name -cin @('p4kx', 'p4kx-gui', 'p4kx-mcp') } | Select-Object -ExpandProperty version -Unique)
if ($versions.Count -ne 1) { throw 'Application versions differ.' }
$metadata = [ordered]@{
    schema = 1
    version = $versions[0]
    compiler_host = $Target
    nextest_host = $Target
    target = $Target
    os = $row.os
    arch = $row.arch
    revision = $revision
    compiler = $compilerVersion
    nextest = $runnerVersion
    archive_sha256 = $archiveHash
    binaries = $binaries
}
if ($row.os -ceq 'macos') { $metadata.build_host = Get-P4kxMacosHostEvidence -Platform $row }
$metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $MetadataPath -Encoding utf8NoBOM
$metadata | ConvertTo-Json -Depth 6
