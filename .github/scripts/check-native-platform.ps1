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

$compiler = & rustc --version --verbose
if ($LASTEXITCODE -ne 0) { throw 'Compiler identity query failed.' }
$compilerHost = @($compiler | Where-Object { $_.StartsWith('host: ') })
if ($compilerHost.Count -ne 1 -or $compilerHost[0] -cne "host: $Target") { throw 'Compiler host does not match native target.' }
$runner = & cargo nextest --version
if ($LASTEXITCODE -ne 0) { throw 'Nextest identity query failed.' }
$runnerHost = @($runner | Where-Object { $_.StartsWith('host: ') })
if ($runnerHost.Count -ne 1 -or $runnerHost[0] -cne "host: $Target") { throw 'Nextest host does not match native target.' }
$compilerVersion = @($compiler | Where-Object { $_.StartsWith('release: ') })[0].Substring('release: '.Length)
$runnerVersion = @($runner | Where-Object { $_.StartsWith('release: ') })[0].Substring('release: '.Length)
$archiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
$revision = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git rev-parse HEAD).Trim() }
if ($LASTEXITCODE -ne 0) { throw 'Source revision query failed.' }

. (Join-Path $PSScriptRoot 'binary-machine.ps1')

if ($ValidateArchive) {
    $metadata = Get-Content -LiteralPath $MetadataPath -Raw | ConvertFrom-Json
    Assert-P4kxArchiveEvidence -Metadata $metadata -Platform $row -Revision $revision -Compiler $compilerVersion -Nextest $runnerVersion -ArchiveHash $archiveHash
    Write-Output "Validated native test archive for $Target at $revision."
    return
}
if (-not $BinaryDirectory) { throw 'A native producer requires BinaryDirectory.' }
$expectedMachine = if ($row.os -eq 'windows') { $row.pe_machine } else { $row.elf_machine }
$binaries = @(foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $filename = if ($row.os -eq 'windows') { "$name.exe" } else { $name }
    $path = Join-Path $BinaryDirectory $filename
    $machine = Get-ImageMachine -Path $path -Os $row.os
    if ($machine -ne $expectedMachine) { throw "$name has machine $machine, expected $expectedMachine for $Target." }
    [ordered]@{ name = $name; machine = $machine; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
})
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
$metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $MetadataPath -Encoding utf8NoBOM
$metadata | ConvertTo-Json -Depth 6
