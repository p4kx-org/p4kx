param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-application-binaries.ps1')
. (Join-Path $PSScriptRoot 'macho-test-image.ps1')
. (Join-Path $PSScriptRoot 'macos-package-evidence.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')

function Assert-P4kxMacFixtureRefusal {
    param([scriptblock]$Action)
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Expected a mismatched macOS package proof to be rejected.' }
}

$null = New-Item -ItemType Directory -Path $Directory
$platform = Get-P4kxPlatform -Target 'aarch64-apple-darwin'
$record = [ordered]@{
    target = $platform.target
    revision = 'a' * 40
    archive_sha256 = 'b' * 64
}
$receipt = [ordered]@{
    package = 'dmg'
    mode = 'ad-hoc'
    reason = 'package-dry-run'
    signature = 'verified'
    notarization = 'not-requested'
    archive_sha256 = $record.archive_sha256
}
$path = Join-Path $Directory 'signing.json'
$receipt | ConvertTo-Json | Set-Content -LiteralPath $path -Encoding utf8NoBOM
$record.signing_sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
if ((Read-P4kxMacSigningReceipt -Path $path -Build $record -DryRun).reason -cne 'package-dry-run') {
    throw 'The dry-run signing reason was not retained.'
}
Assert-P4kxMacFixtureRefusal { Read-P4kxMacSigningReceipt -Path $path -Build $record }
$receipt.reason = 'notarization-unavailable'
$receipt | ConvertTo-Json | Set-Content -LiteralPath $path -Encoding utf8NoBOM
Assert-P4kxMacFixtureRefusal { Read-P4kxMacSigningReceipt -Path $path -Build $record }
$record.signing_sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
$null = Read-P4kxMacSigningReceipt -Path $path -Build $record

$receipt.mode = 'developer-id'
$receipt.reason = $null
$receipt.notarization = 'stapled-and-validated'
$receipt | ConvertTo-Json | Set-Content -LiteralPath $path -Encoding utf8NoBOM
$record.signing_sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
$null = Read-P4kxMacSigningReceipt -Path $path -Build $record
Assert-P4kxMacFixtureRefusal { Read-P4kxMacSigningReceipt -Path $path -Build $record -DryRun }

$metal = @{ target = $record.target; revision = $record.revision; archive_sha256 = $record.archive_sha256; outcome = 'success' }
Assert-P4kxMacMetalEvidence -Evidence $metal -Build $record
$metal.outcome = 'skipped'
Assert-P4kxMacFixtureRefusal { Assert-P4kxMacMetalEvidence -Evidence $metal -Build $record }
$metal.outcome = 'success'
$metal.archive_sha256 = 'c' * 64
Assert-P4kxMacFixtureRefusal { Assert-P4kxMacMetalEvidence -Evidence $metal -Build $record }

$minimum = [version]$platform.deployment_target
$minimumWord = [uint32](($minimum.Major -shl 16) -bor ($minimum.Minor -shl 8))
$bytes = New-P4kxMachOTestImage -Machine $platform.mach_cpu_type -Minimum $minimumWord
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    [IO.File]::WriteAllBytes((Join-Path $Directory $name), $bytes)
}
$actual = @(Get-P4kxApplicationBinaryEvidence -Directory $Directory -Platform $platform)
$expected = $actual | ConvertTo-Json -Depth 8 | ConvertFrom-Json -NoEnumerate
Assert-P4kxMatchingApplicationBinaries -Actual $actual -Expected $expected -Platform $platform
$expected[0].sdk = '99.0.0'
Assert-P4kxMacFixtureRefusal { Assert-P4kxMatchingApplicationBinaries -Actual $actual -Expected $expected -Platform $platform }
$expected[0].sdk = $actual[0].sdk
$bytes[$bytes.Length - 1] = 1
[IO.File]::WriteAllBytes((Join-Path $Directory 'p4kx-gui'), $bytes)
$changed = @(Get-P4kxApplicationBinaryEvidence -Directory $Directory -Platform $platform)
Assert-P4kxMacFixtureRefusal { Assert-P4kxMatchingApplicationBinaries -Actual $changed -Expected $expected -Platform $platform }
$version = '0.5.0'
$archive = Join-Path $Directory (Get-P4kxAssetName -Target $platform.target -Version $version -Package dmg)
'synthetic signed-image evidence fixture' | Set-Content -LiteralPath $archive -Encoding utf8NoBOM
$record.archive_sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
$receipt.archive_sha256 = $record.archive_sha256
$receipt | ConvertTo-Json | Set-Content -LiteralPath $path -Encoding utf8NoBOM
$record.signing_sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
$signing = Read-P4kxMacSigningReceipt -Path $path -Build $record
$metal.archive_sha256 = $record.archive_sha256
Assert-P4kxMacMetalEvidence -Evidence $metal -Build $record

# The packaged app smoke records app execution and the producer separately proves native Metal.
$smoke = [ordered]@{
    target = $platform.target
    package = 'dmg'
    binaries = $actual
    runtime = [ordered]@{
        cli_list = 'success'
        cli_extract = 'success'
        cli_convert = 'success'
        mcp_tools = 'success'
        mcp_read = 'success'
        gui_frames = 'success'
    }
}
$smoke = Complete-P4kxMacHostedPackageSmoke -Smoke $smoke -Build $record -Signing $signing -Metal $metal
$runtime = @{ archive_sha256 = $record.archive_sha256; binaries = $actual; package_smokes = @($smoke) }
Assert-P4kxPackageRuntime -Runtime $runtime -Platform $platform -AssetDirectory $Directory -Version $version
$smoke.runtime.Remove('metal_adapter')
Assert-P4kxMacFixtureRefusal {
    Assert-P4kxPackageRuntime -Runtime $runtime -Platform $platform -AssetDirectory $Directory -Version $version
}
'PASS: final DMG/signing/Metal bindings and per-application Mach-O/signed-byte evidence.'
