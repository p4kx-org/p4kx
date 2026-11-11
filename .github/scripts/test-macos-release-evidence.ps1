param(
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Revision
)

$ErrorActionPreference = 'Stop'
$current = '27.2.1'
$target = 'aarch64-apple-darwin'
$proofPath = Join-Path $AssetDirectory "P4KX-$Version-$target-runtime.json"
$validator = Join-Path $PSScriptRoot 'check-release-matrix.ps1'
$devicePath = Join-Path $AssetDirectory 'native-device-fixture.json'
$nativeValidation = @{
    AssetDirectory=$AssetDirectory; Version=$Version; Revision=$Revision
    RequireMacosDeviceEvidence=$true; MacosCurrentVersion=$current; MacosDeviceEvidencePath=$devicePath
}
$original = Get-Content -LiteralPath $proofPath -Raw

function Assert-RejectedMacosEvidence {
    param([hashtable]$Arguments, [string]$ExpectedError)
    try { & $validator @Arguments | Out-Null } catch {
        if ($_.Exception.Message -cnotmatch $ExpectedError) { throw }
        return
    }
    throw "Release preflight accepted invalid macOS evidence: $ExpectedError"
}

function Write-MacosFixture {
    param([string]$Json)
    $candidate = $Json | ConvertFrom-Json
    ConvertTo-Json -InputObject @($candidate.macos_devices) -Depth 24 |
        Set-Content -LiteralPath $devicePath -Encoding utf8NoBOM
    $candidate.PSObject.Properties.Remove('macos_devices')
    $candidate | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
}

try {
    '[]' | Set-Content -LiteralPath $devicePath -Encoding utf8NoBOM
    Assert-RejectedMacosEvidence $nativeValidation 'distinct minimum and current'
    $proof = $original | ConvertFrom-Json
    $smoke = $proof.package_smokes[0]
    $lifecycle = @{}
    foreach ($name in @(
        'mounted','finder_install','finder_launch','relocated','symlink_launch','upgrade',
        'uninstalled','profile_preserved','quarantine_gatekeeper'
    )) { $lifecycle[$name] = 'success' }
    $desktop = @{}
    foreach ($name in @(
        'core_audio','no_audio_device','clipboard','open_save_dialogs','command_shortcuts',
        'cancelled_dialog','permission_denial','unicode_paths','case_sensitive_apfs',
        'case_insensitive_apfs','external_volume','wine_discovery','crossover_discovery',
        'guard_running','guard_stopped','guard_unavailable','protected_aliases','blender_52_import'
    )) { $desktop[$name] = 'success' }
    $proof | Add-Member -NotePropertyName macos_devices -NotePropertyValue @(
        @{
            role='minimum'; scope='native-device'; target=$target; revision=$Revision
            archive_sha256=$smoke.archive_sha256; signing_mode=$smoke.signing.mode
            host=$proof.runtime_host; binaries=$smoke.binaries; runtime=$smoke.runtime
            lifecycle=$lifecycle; device_checks=$desktop
        },
        @{
            role='current'; scope='native-device'; target=$target; revision=$Revision
            archive_sha256=$smoke.archive_sha256; signing_mode=$smoke.signing.mode
            host=@{ os_version=$current; os_build='26C102'; architecture='arm64' }
            binaries=$smoke.binaries; runtime=$smoke.runtime; lifecycle=$lifecycle; device_checks=@{}
        }
    )
    $complete = $proof | ConvertTo-Json -Depth 24
    Write-MacosFixture $complete
    & $validator @nativeValidation | Out-Null
    foreach ($case in @(
        @{ edit={ param($p) $p.package_smokes[0].signing.signature='skipped' }; error='signature evidence' },
        @{ edit={ param($p) $p.package_smokes[0].signing.mode='developer-id' }; error='stapled notarization' },
        @{ edit={ param($p) $p.package_smokes[0].signing.reason='package-dry-run' }; error='dry-run DMG' },
        @{ edit={ param($p) $p.package_smokes[0].binaries[0].sha256='f' * 64 }; error='differs from release build' },
        @{ edit={ param($p) $p.package_smokes[0].runtime.metal_adapter='skipped' }; error='metal_adapter' },
        @{ edit={ param($p) $p.macos_devices[0].archive_sha256='f' * 64 }; error='different release bytes' },
        @{ edit={ param($p) $p.macos_devices[0].binaries[0].sha256='f' * 64 }; error='different packaged binary' },
        @{ edit={ param($p) $p.macos_devices[1].host.os_version='26.0' }; error='Incorrect macOS version' },
        @{ edit={ param($p) $p.macos_devices[0].host.architecture='x64' }; error='incompatible native macOS host' },
        @{ edit={ param($p) $p.macos_devices[1].role='minimum' }; error='distinct minimum and current' },
        @{ edit={ param($p) $p.macos_devices[0].lifecycle.finder_install='skipped' }; error='finder_install' },
        @{ edit={ param($p) $p.macos_devices[0].device_checks.clipboard='skipped' }; error='clipboard' }
    )) {
        $changed = $complete | ConvertFrom-Json
        & $case.edit $changed
        Write-MacosFixture ($changed | ConvertTo-Json -Depth 24)
        Assert-RejectedMacosEvidence $nativeValidation $case.error
    }
    'PASS: macOS release preflight rejects absent and mismatched package, signing and native-device evidence.'
} finally {
    $original | Set-Content -LiteralPath $proofPath -Encoding utf8NoBOM
    Remove-Item -LiteralPath $devicePath -ErrorAction SilentlyContinue
}
