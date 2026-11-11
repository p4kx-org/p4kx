$ErrorActionPreference = 'Stop'

function Assert-P4kxMacosPackageSigning {
    param($Signing, [string]$ArchiveHash)

    if ($Signing.package -cne 'dmg' -or $Signing.archive_sha256 -cne $ArchiveHash -or
        $Signing.signature -cne 'verified') {
        throw 'macOS signature evidence does not identify the executed DMG.'
    }
    switch -CaseSensitive ($Signing.mode) {
        'developer-id' {
            if ($Signing.reason -or $Signing.notarization -cne 'stapled-and-validated') {
                throw 'The Developer ID DMG has no validated stapled notarization ticket.'
            }
        }
        'ad-hoc' {
            if ($Signing.reason -cnotin @(
                'package-dry-run', 'developer-id-unavailable', 'notarization-unavailable',
                'developer-id-and-notarization-unavailable'
            ) -or $Signing.notarization -cne 'not-requested') {
                throw 'The ad-hoc DMG lacks its signing reason or reports inconsistent notarization.'
            }
        }
        default { throw 'Unrecognized macOS signing mode.' }
    }
}

function Assert-P4kxMacosReleaseDevices {
    param($Runtime, $Platform, [string]$CurrentVersion, [object[]]$DeviceRuns)

    if ($CurrentVersion -cnotmatch '^[0-9]+\.[0-9]+(?:\.[0-9]+)?$' -or
        [version]$CurrentVersion -lt [version]$Platform.deployment_target) {
        throw 'Specify the verified current stable macOS version for release preflight.'
    }
    $dmg = @($Runtime.package_smokes)[0]
    if ($dmg.signing.reason -ceq 'package-dry-run') { throw 'A dry-run DMG cannot be published.' }
    if ((($deviceRuns.role | Sort-Object) -join ',') -cne 'current,minimum') {
        throw 'Release requires distinct minimum and current native macOS device records.'
    }
    foreach ($run in $deviceRuns) {
        Assert-P4kxMacosReleaseDevice -Run $run -Runtime $Runtime -Platform $Platform -Dmg $dmg -CurrentVersion $CurrentVersion
    }
    foreach ($check in @(
        'core_audio', 'no_audio_device', 'clipboard', 'open_save_dialogs', 'command_shortcuts',
        'cancelled_dialog', 'permission_denial', 'unicode_paths', 'case_sensitive_apfs',
        'case_insensitive_apfs', 'external_volume', 'wine_discovery', 'crossover_discovery',
        'guard_running', 'guard_stopped', 'guard_unavailable', 'protected_aliases', 'blender_52_import'
    )) {
        if (-not ($deviceRuns | Where-Object { $_.device_checks.$check -ceq 'success' })) {
            throw "Required macOS device check '$check' has no successful record."
        }
    }
}

function Assert-P4kxMacosReleaseDevice {
    param($Run, $Runtime, $Platform, $Dmg, [string]$CurrentVersion)

    if ($Run.scope -cne 'native-device' -or $Run.target -cne $Platform.target -or
        $Run.revision -cne $Runtime.revision -or $Run.archive_sha256 -cne $Dmg.archive_sha256 -or
        $Run.signing_mode -cne $Dmg.signing.mode) {
        throw 'A native macOS device record belongs to different release bytes or source.'
    }
    Assert-P4kxMacosHostEvidence -HostEvidence $Run.host -Platform $Platform
    $observed = [version]$Run.host.os_version
    if (($Run.role -ceq 'minimum' -and $observed.Major -ne ([version]$Platform.deployment_target).Major) -or
        ($Run.role -ceq 'current' -and $observed -ne [version]$CurrentVersion)) {
        throw "Incorrect macOS version for $($Run.role) device verification."
    }
    Assert-P4kxBinaryEvidence -Metadata $Run -Platform $Platform
    foreach ($binary in $Run.binaries) {
        $expected = $Dmg.binaries | Where-Object name -CEQ $binary.name
        if ($binary.sha256 -cne $expected.sha256) { throw 'Device ran different packaged binary bytes.' }
    }
    foreach ($check in (Get-P4kxRuntimeChecks -Platform $Platform)) {
        if ($Run.runtime.$check -cne 'success') { throw "Required macOS device runtime check '$check' did not succeed." }
    }
    foreach ($check in @(
        'mounted', 'finder_install', 'finder_launch', 'relocated', 'symlink_launch',
        'upgrade', 'uninstalled', 'profile_preserved', 'quarantine_gatekeeper'
    )) {
        if ($Run.lifecycle.$check -cne 'success') { throw "Required macOS device lifecycle check '$check' did not succeed." }
    }
}
