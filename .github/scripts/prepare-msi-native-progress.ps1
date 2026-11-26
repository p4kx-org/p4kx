$ErrorActionPreference = 'Stop'
. .github/scripts/platform-contract.ps1
. .github/scripts/msi-diagnostic-runner.ps1
. .github/scripts/msi-detached-control.ps1

function Assert-P4kxProgressDigest([string]$Path, [string]$Expected) {
    if ((Get-P4kxDetachedDigest $Path) -cne $Expected) { throw 'An exact MSI diagnostic input digest differs.' }
}

function Get-P4kxProgressProducer {
    param([long]$Id, [string]$Target, [string]$Digest)
    $raw = gh api --method GET "repos/p4kx-org/p4kx/actions/artifacts/$Id"
    if ($LASTEXITCODE -ne 0) { throw 'Pinned MSI producer artifact metadata is unavailable.' }
    $artifact = $raw | ConvertFrom-Json
    if ($artifact.id -ne $Id -or $artifact.name -cne "windows-build-$Target" -or $artifact.digest -cne "sha256:$Digest" -or
        $artifact.expired -ne $false -or [string]$artifact.workflow_run.id -cne $env:P4KX_MSI_PRODUCER_RUN -or
        $artifact.workflow_run.head_sha -cne $env:P4KX_MSI_CANDIDATE) {
        throw 'Pinned MSI producer artifact source, target, or digest differs.'
    }
    return @{ id=$Id; target=$Target; sha256=$Digest }
}

$candidate = $env:P4KX_MSI_CANDIDATE
$harness = $env:GITHUB_SHA
$head = git rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $head -cne $candidate) { throw 'Check out the exact MSI candidate source.' }
$harnessHead = git -C .msi-progress rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $harnessHead -cne $harness) { throw 'Check out the actual diagnostic harness source.' }
$row = Get-P4kxPlatform -Target $env:P4KX_TARGET
Assert-P4kxNativeHost $row
if ($row.target -cne 'x86_64-pc-windows-msvc' -or [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -ne [Runtime.InteropServices.Architecture]::X64) {
    throw 'This MSI diagnostic requires a native Windows AMD64 runner.'
}
$amd = Get-P4kxProgressProducer 15146677135 $row.target '961ae37ad8ea6812ba56cf8135dd8338a9244c6670c64311c67eb5450f4f286c'
$arm = Get-P4kxProgressProducer 15144744704 'aarch64-pc-windows-msvc' 'db8e305bc697a2b96b6c3093e7d6e3b694f428fcecf109fd696cf9ac7054a857'
$artifacts = Join-Path $env:P4KX_PACKAGE_TASK 'windows-inputs'
$base = Join-Path $artifacts "windows-build-$($amd.target)"
$other = Join-Path $artifacts "windows-build-$($arm.target)"
$version = '0.4.2'
$next = '0.4.3'
$msi = Join-Path "$base/package" (Get-P4kxAssetName -Target $amd.target -Version $version -Package msi)
$opposite = Join-Path "$other/package" (Get-P4kxAssetName -Target $arm.target -Version $version -Package msi)
$nextMsi = Join-Path $base 'validation/next.msi'
$helper = Join-Path $base 'msi-helper/payload/p4kx-msi-path.exe'
Assert-P4kxProgressDigest $msi 'd57078919bdeb89b3ceb9f5032c3972d62cc1e212815546378b380dfedfe34c4'
Assert-P4kxProgressDigest $opposite '5cb060db3d54ef7c744fc142b4a4ef2c1aa3feb99eec8b83555fda250e126d10'
Assert-P4kxProgressDigest $nextMsi '66311be30c146de213dab15dc4f61a9e25ddf082b39a227082ce77c4f5be7ad9'
Assert-P4kxProgressDigest $helper '69b6337a3400b29e449b8f282d7e21a4c95f3b8b0c93591f4d05b1b4a6834719'
$zip = Join-Path "$base/package" (Get-P4kxAssetName -Target $amd.target -Version $version -Package portable-zip)
$env:GITHUB_SHA = $candidate
.github/scripts/check-native-platform.ps1 -Target $row.target -ArchivePath $zip -MetadataPath "$base/validation/build.json" -ValidateArchive

$previousName = 'P4K-Extractor-0.2.0-x86_64.msi'
$previous = Join-Path $env:P4KX_PACKAGE_TASK 'previous'
$null = New-Item -ItemType Directory -Path $previous
gh release download v0.2.0 --repo p4kx-org/p4kx --pattern $previousName --dir $previous
if ($LASTEXITCODE -ne 0) { throw 'The pinned historical AMD64 MSI could not be downloaded.' }
$previousMsi = Join-Path $previous $previousName
Assert-P4kxProgressDigest $previousMsi '0d9acc29eab4c95de5ae54f660dc2f7dca6e54e3fac3270f01ce57acafd65071'
$env:P4KX_FIXTURE_SOURCE_REVISION = $candidate
cargo build --release --locked --target $row.target -p p4kx-msi-path --example p4kx-msi-registry-fixture --features native-fixtures
if ($LASTEXITCODE -ne 0) { throw 'The source-bound raw Registry fixture did not compile.' }
$fixture = [IO.Path]::GetFullPath("target/$($row.target)/release/examples/p4kx-msi-registry-fixture.exe")
$fixtureSha = Get-P4kxDetachedDigest $fixture
$output = Join-Path $env:P4KX_PACKAGE_TASK 'installer-check'
$parameters = @{
    Target=$row.target; Msi=$msi; NextMsi=$nextMsi; NextVersion=$next; OtherArchitectureMsi=$opposite
    UpgradeCode=([xml](Get-Content -LiteralPath 'crates/p4kx-gui/wix/main.wxs')).Wix.Package.UpgradeCode
    FixtureDirectory=(Join-Path $base 'fixture'); OutputDirectory=$output; RegistryFixture=$fixture
    RegistryFixtureSha256=$fixtureSha; HelperExecutable=$helper; PreviousMsi=$previousMsi
}
$invocation = Join-Path $env:P4KX_PACKAGE_TASK 'msi-invocation.json'
$parameters | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $invocation -Encoding utf8NoBOM
$feedback = Join-Path $output 'feedback'
.github/scripts/prepare-msi-feedback.ps1 -Directory $feedback
Write-P4kxMsiDiagnosticSources $parameters .github/scripts $feedback
Write-P4kxMsiDiagnosticInputs $parameters $feedback
Write-P4kxDetachedReceipt (Join-Path $env:P4KX_PACKAGE_TASK 'candidate-binding.json') @{
    schema=1; role='native-msi-progress-candidate'; candidate_revision=$candidate; harness_revision=$harness
    producer_run=$env:P4KX_MSI_PRODUCER_RUN; producer_attempt=1; artifacts=@($amd,$arm)
    run=$env:GITHUB_RUN_ID; attempt=$env:GITHUB_RUN_ATTEMPT; target=$row.target
    registry_fixture_source=$candidate; rebuilt_registry_fixture_sha256=$fixtureSha
    invocation_sha256=(Get-P4kxDetachedReceiptDigest $invocation)
    diagnostic_inputs_sha256=(Get-P4kxDetachedReceiptDigest (Join-Path $feedback 'diagnostic-inputs.json'))
    release_acceptance_unproven=$true
}
