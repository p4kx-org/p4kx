param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-test-fixtures.ps1')
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$contract = Get-P4kxPlatformContract
$revision = 'a' * 40
$version = '0.5.0'
$pins = & (Join-Path $PSScriptRoot 'check-build-tools.ps1')
$compiler = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
$nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
$validator = Join-Path $PSScriptRoot 'check-release-matrix.ps1'

foreach ($row in $contract.targets) {
    foreach ($package in $row.packages) {
        $name = Get-P4kxAssetName -Target $row.target -Version $version -Package $package
        Set-Content -LiteralPath (Join-Path $Directory $name) -Value 'synthetic package fixture' -Encoding utf8NoBOM
    }
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $sbom = [ordered]@{
            bomFormat = 'CycloneDX'
            specVersion = '1.5'
            components = @(@{ type = 'library'; name = 'synthetic'; version = '1' })
            metadata = @{
                component = @{ type = 'application'; name = $binary; version = $version }
                properties = @(@{ name = 'p4kx:target'; value = $row.target }, @{ name = 'p4kx:binary'; value = $binary })
            }
        }
        $name = "P4KX-$version-$($row.target)-$binary.cdx.json"
        $sbom | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Directory $name) -Encoding utf8NoBOM
    }
    $proof = New-P4kxTestEvidence -Platform $row -Version $version -Revision $revision -Compiler $compiler -Nextest $nextest
    if ($row.os -in @('windows', 'linux')) {
        $smokes = @(foreach ($package in $row.packages) {
            $asset = Get-P4kxAssetName -Target $row.target -Version $version -Package $package
            $smoke = [ordered]@{
                target=$row.target; package=$package; binaries=$proof.binaries; runtime=$proof.runtime
                archive_sha256=(Get-FileHash (Join-Path $Directory $asset) -Algorithm SHA256).Hash.ToLowerInvariant()
            }
            if ($row.os -eq 'linux') {
                $elf = @($proof.binaries | ConvertTo-Json -Depth 6 | ConvertFrom-Json)
                foreach ($image in $elf) { $image | Add-Member -NotePropertyName text_sha256 -NotePropertyValue ('e' * 64) }
                $smoke.producer = [ordered]@{
                    schema=1; target=$row.target; package=$package; version=$version; revision=$revision
                    compiler=$compiler; nextest=$nextest; binaries=$elf; source_binaries=$elf; transformations=@()
                    minimum_distribution='Ubuntu 24.04'; archive_sha256=$smoke.archive_sha256
                }
                if ($package -eq 'app-image') {
                    $sourceName = "P4KX-$version-$($row.target)-appimage-corresponding-source.tar.gz"
                    Set-Content -LiteralPath (Join-Path $Directory $sourceName) -Value 'synthetic corresponding source' -Encoding utf8NoBOM
                    $smoke.producer.corresponding_source = @{
                        name=$sourceName; sha256=(Get-FileHash -LiteralPath (Join-Path $Directory $sourceName) -Algorithm SHA256).Hash.ToLowerInvariant()
                    }
                }
                if ($package -eq 'flatpak') {
                    $smoke.documents = @{target=$row.target; revision=$revision; read_only_archive='success'; writable_export='success'; game_alias_refusal='success'; source_unchanged='success'}
                }
            }
            $smoke
        })
        $proof | Add-Member -NotePropertyName package_smokes -NotePropertyValue $smokes
    }
    $name = "P4KX-$version-$($row.target)-runtime.json"
    $proof | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Directory $name) -Encoding utf8NoBOM
}

& $validator -AssetDirectory $Directory -Version $version -Revision $revision
$windows = @($contract.targets | Where-Object os -EQ windows)[0]
$windowsProofPath = Join-Path $Directory "P4KX-$version-$($windows.target)-runtime.json"
$originalProof = Get-Content -LiteralPath $windowsProofPath -Raw
foreach ($mutation in @(
    { param($proof) $proof.package_smokes = @($proof.package_smokes | Select-Object -First 1) },
    { param($proof) $proof.package_smokes[0].archive_sha256 = 'f' * 64 },
    { param($proof) $proof.package_smokes[0].binaries[0].sha256 = 'f' * 64 },
    { param($proof) $proof.package_smokes[0].runtime.gui_frames = 'skipped' }
)) {
    $changed = $originalProof | ConvertFrom-Json
    & $mutation $changed
    $changed | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $windowsProofPath -Encoding utf8NoBOM
    $rejected = $false
    try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Incomplete or mismatched executed package evidence was accepted.' }
}
$originalProof | Set-Content -LiteralPath $windowsProofPath -Encoding utf8NoBOM
$linux = @($contract.targets | Where-Object os -EQ linux)[0]
$linuxProofPath = Join-Path $Directory "P4KX-$version-$($linux.target)-runtime.json"
$linuxOriginal = Get-Content -LiteralPath $linuxProofPath -Raw
$transformed = $linuxOriginal | ConvertFrom-Json
$appImage = @($transformed.package_smokes | Where-Object package -CEQ 'app-image')[0]
$appImage.binaries[0].sha256 = 'f' * 64
$appImage.producer.binaries[0].sha256 = 'f' * 64
$transformed | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $linuxProofPath -Encoding utf8NoBOM
& $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null
foreach ($mutation in @(
    { param($proof) $proof.package_smokes[0].producer.binaries[0].text_sha256 = 'f' * 64 },
    { param($proof) $proof.package_smokes[0].producer.source_binaries[0].sha256 = 'f' * 64 },
    { param($proof) $proof.package_smokes[0].producer.package = 'app-image' },
    { param($proof) ($proof.package_smokes | Where-Object package -CEQ 'app-image').producer.corresponding_source.sha256 = 'f' * 64 },
    { param($proof) ($proof.package_smokes | Where-Object package -CEQ 'flatpak').documents.game_alias_refusal = 'skipped' },
    { param($proof) ($proof.package_smokes | Where-Object package -CEQ 'flatpak').documents.revision = 'f' * 40 }
)) {
    $changed = $linuxOriginal | ConvertFrom-Json
    & $mutation $changed
    $changed | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $linuxProofPath -Encoding utf8NoBOM
    $rejected = $false
    try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Changed Linux code/source identity or missing portal execution evidence was accepted.' }
}
$linuxOriginal | Set-Content -LiteralPath $linuxProofPath -Encoding utf8NoBOM
$row = $contract.targets[1]
$packageName = Get-P4kxAssetName -Target $row.target -Version $version -Package $row.packages[0]
$packagePath = Join-Path $Directory $packageName
[IO.File]::WriteAllBytes($packagePath, [byte[]]::new(0))
$rejected = $false
try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'An empty ARM64 package was accepted.' }
Set-Content -LiteralPath $packagePath -Value 'synthetic package fixture' -Encoding utf8NoBOM

$proofPath = Join-Path $Directory "P4KX-$version-$($row.target)-runtime.json"
$proof = Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json
$proof.runtime.gui_frames = 'skipped'
$proof | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
$rejected = $false
try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'A skipped ARM64 GUI runtime check was accepted.' }
Write-Output 'PASS: complete release matrix; missing/changed/skipped package evidence, empty ARM64 package and skipped GUI proof rejected.'
