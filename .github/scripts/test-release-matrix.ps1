param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-test-fixtures.ps1')
. (Join-Path $PSScriptRoot 'test-documents-evidence-fixture.ps1')
. (Join-Path $PSScriptRoot 'linux-package-lifecycle-evidence.ps1')
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
        Set-Content -LiteralPath (Join-Path $Directory $name) -Value "synthetic package fixture for $($row.target) $package" -Encoding utf8NoBOM
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
    if ($row.os -cin @('windows', 'linux', 'macos')) {
        $smokes = @(foreach ($package in $row.packages) {
            $asset = Get-P4kxAssetName -Target $row.target -Version $version -Package $package
            $smoke = [ordered]@{
                target=$row.target; package=$package; binaries=$proof.binaries; runtime=$proof.runtime
                archive_sha256=(Get-FileHash (Join-Path $Directory $asset) -Algorithm SHA256).Hash.ToLowerInvariant()
            }
            if ($row.os -ceq 'macos') {
                $smoke.signing = @{
                    package='dmg'; mode='ad-hoc'; reason='developer-id-unavailable'
                    signature='verified'; notarization='not-requested'; archive_sha256=$smoke.archive_sha256
                }
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
                    $smoke.documents = New-P4kxDocumentsEvidenceFixture $row $revision $smoke.archive_sha256
                }
            }
            $smoke
        })
        $proof | Add-Member -NotePropertyName package_smokes -NotePropertyValue $smokes
        $sourcePackage = switch -CaseSensitive ($row.os) {
            windows { 'portable-zip' }
            linux { 'tar-gz' }
            macos { 'dmg' }
        }
        $proof.archive_sha256 = ($smokes | Where-Object package -CEQ $sourcePackage).archive_sha256
    }
    if ($row.os -ceq 'linux') {
        $fixtureVersion = Get-P4kxLinuxLifecycleFixtureVersion
        $settings = Get-P4kxLinuxPackaging
        $lifecycle = @(foreach ($format in @('deb', 'flatpak')) {
            $executed = @($smokes | Where-Object package -CEQ $format)[0]
            if ($format -ceq 'deb') {
                $identity = "p4kx:$($row.deb_arch)"
                $baseline = Get-P4kxDebianVersion $fixtureVersion
                $candidate = Get-P4kxDebianVersion $version
            } else {
                $identity = "app/$($settings.application_id)/$($row.unix_arch)/$($settings.flatpak.application_branch)"
                $baseline = 'e' * 64
                $candidate = 'd' * 64
                $executed.producer.flatpak = @{app_ref=$identity; app_commit=$candidate}
            }
            @{
                schema=1; target=$row.target; revision=$revision; package=$format
                fixture_kind='same-source-lower-package-version'
                baseline=@{identity=$identity; installed=$baseline; version=$fixtureVersion; archive_sha256='c' * 64}
                candidate=@{identity=$identity; installed=$candidate; version=$version; archive_sha256=$executed.archive_sha256}
                candidate_launch='success'; uninstalled='success'; profile_preserved='success'
            }
        })
        $proof | Add-Member -NotePropertyName linux_lifecycle -NotePropertyValue $lifecycle
    }
    if ($row.os -ceq 'windows') {
        $msi = Get-P4kxAssetName -Target $row.target -Version $version -Package msi
        $msiHash = (Get-FileHash -LiteralPath (Join-Path $Directory $msi) -Algorithm SHA256).Hash.ToLowerInvariant()
        $reports = @(& (Join-Path $PSScriptRoot 'test-msi-path-verification.ps1') -FixtureTarget $row.target -FixtureRevision $revision -FixtureMsiSha256 $msiHash)
        if ($reports.Count -ne 1 -or $reports[0] -isnot [Collections.IDictionary] -or $reports[0].scope -cne 'native-synthetic') {
            throw 'The pure MSI fixture must return exactly one synthetic report in its own script scope.'
        }
        $proof | Add-Member -NotePropertyName installer -NotePropertyValue @{
            lifecycle='success'; repair='success'; rollback='success'; path_contract=$reports[0]
        }
    }
    $name = "P4KX-$version-$($row.target)-runtime.json"
    $proof | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath (Join-Path $Directory $name) -Encoding utf8NoBOM
}

& $validator -AssetDirectory $Directory -Version $version -Revision $revision
$notesFixture = Join-Path $Directory 'release-notes-fixture'
$notesAssets = Join-Path $notesFixture 'assets'
New-Item -ItemType Directory -Path $notesAssets | Out-Null
foreach ($fixture in Get-ChildItem -LiteralPath $Directory -File) {
    Copy-Item -LiteralPath $fixture.FullName -Destination $notesAssets
}
$notesInput = Join-Path $notesFixture 'changes.md'
$notesOutput = Join-Path $notesFixture 'notes.md'
Set-Content -LiteralPath $notesInput -Value 'Synthetic release notes' -Encoding utf8NoBOM
& (Join-Path $PSScriptRoot 'assemble-release.ps1') -AssetDirectory $notesAssets -NotesPath $notesInput -OutputNotesPath $notesOutput -Repository 'p4kx-org/p4kx' -Version $version -Revision $revision
$notes = Get-Content -LiteralPath $notesOutput -Raw
if ($notes -cnotmatch 'ad-hoc signed because Developer ID was unavailable' -or $notes -cnotmatch 'Open Anyway') {
    throw 'Release notes omitted the macOS ad-hoc signing disclosure.'
}
if ($notes -cmatch 'gh attestation verify' -or
    $notes -cnotmatch '(?m)^This validation run did not generate Sigstore attestations\.\r?$') {
    throw 'Validation-only release notes claimed attestations that this run did not generate.'
}
foreach ($row in $contract.targets) {
    $proofPath = Join-Path $Directory "P4KX-$version-$($row.target)-runtime.json"
    $original = Get-Content -LiteralPath $proofPath -Raw
    $changed = $original | ConvertFrom-Json
    $otherPackage = if ($row.os -ceq 'windows') { 'msi' } else { 'app-image' }
    $otherDigest = if ($row.os -ceq 'macos') { 'f' * 64 } else {
        ($changed.package_smokes | Where-Object package -CEQ $otherPackage).archive_sha256
    }
    if ($otherDigest -ceq $changed.archive_sha256) { throw 'Fixture package digests must differ.' }
    $changed.archive_sha256 = $otherDigest
    $changed | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
    $rejected = $false
    try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null }
    catch { $rejected = $true }
    finally { Set-Content -LiteralPath $proofPath -Value $original -Encoding utf8NoBOM }
    if (-not $rejected) { throw "Release accepted the $otherPackage digest as the source archive for $($row.target)." }
}
$windows = @($contract.targets | Where-Object os -EQ windows)[0]
$windowsProofPath = Join-Path $Directory "P4KX-$version-$($windows.target)-runtime.json"
$originalProof = Get-Content -LiteralPath $windowsProofPath -Raw
foreach ($mutation in @(
    { param($proof) $proof.PSObject.Properties.Remove('installer') },
    { param($proof) $proof.installer.repair = 'skipped' },
    { param($proof) $proof.installer.path_contract.raw_cases = @($proof.installer.path_contract.raw_cases | Select-Object -Skip 1) },
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
    { param($proof) ($proof.package_smokes | Where-Object package -CEQ 'flatpak').documents.revision = 'f' * 40 },
    { param($proof) $proof.PSObject.Properties.Remove('linux_lifecycle') },
    { param($proof) $proof.linux_lifecycle = @($proof.linux_lifecycle[0], $proof.linux_lifecycle[0]) },
    { param($proof) $proof.linux_lifecycle[0].baseline.installed = $proof.linux_lifecycle[0].candidate.installed },
    { param($proof) $proof.linux_lifecycle[1].baseline.installed = $proof.linux_lifecycle[1].candidate.installed },
    { param($proof) $proof.linux_lifecycle[1].candidate.identity = 'app/io.github.foreign/x86_64/stable' },
    { param($proof) $proof.linux_lifecycle[0].candidate.archive_sha256 = 'f' * 64 },
    { param($proof) $proof.linux_lifecycle[1].revision = 'f' * 40 },
    { param($proof) $proof.linux_lifecycle[1].profile_preserved = 'skipped' },
    { param($proof) $proof.linux_lifecycle[0].candidate_launch = 'skipped' },
    { param($proof) $proof.linux_lifecycle[1].uninstalled = 'failed' }
)) {
    $changed = $linuxOriginal | ConvertFrom-Json
    & $mutation $changed
    $changed | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $linuxProofPath -Encoding utf8NoBOM
    $rejected = $false
    try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Changed Linux identity or incomplete portal/lifecycle evidence was accepted.' }
}
$linuxOriginal | Set-Content -LiteralPath $linuxProofPath -Encoding utf8NoBOM
$row = $contract.targets[1]
$packageName = Get-P4kxAssetName -Target $row.target -Version $version -Package $row.packages[0]
$packagePath = Join-Path $Directory $packageName
$originalBytes = [IO.File]::ReadAllBytes($packagePath)
[IO.File]::WriteAllBytes($packagePath, [byte[]]::new(0))
$rejected = $false
try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'An empty ARM64 package was accepted.' }
[IO.File]::WriteAllBytes($packagePath, $originalBytes)

$proofPath = Join-Path $Directory "P4KX-$version-$($row.target)-runtime.json"
$proof = Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json
$proof.runtime.gui_frames = 'skipped'
$windowsFinal = Get-Content -LiteralPath $proofPath -Raw
$proof | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
$rejected = $false
try { & $validator -AssetDirectory $Directory -Version $version -Revision $revision | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'A skipped ARM64 GUI runtime check was accepted.' }
$windowsFinal | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
& (Join-Path $PSScriptRoot 'test-macos-release-evidence.ps1') -AssetDirectory $Directory -Version $version -Revision $revision
Write-Output 'PASS: complete release matrix; missing/changed/skipped package evidence, empty ARM64 package and skipped GUI proof rejected.'
