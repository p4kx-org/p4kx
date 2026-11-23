# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-blender-addon-evidence.ps1')

function Assert-BlenderReceiptRefused {
    param($Runtime, [string]$Role, [string]$ArtifactId, [string]$Digest, [string]$Case)

    try {
        Assert-P4kxMacosBlenderAddonEvidence $Runtime $Role $ArtifactId $Digest
    }
    catch {
        if ($_.Exception.Message -cnotmatch 'lacks matching native Blender 5\.2 LTS acceptance') { throw }
        return
    }
    throw "Incompatible Blender evidence was accepted for $Case."
}

$artifactId = '52'
$addonHash = 'a' * 64
$pin = Get-P4kxMacosBlenderPin
$stable = Get-P4kxCurrentMacosReleaseVersion
$platform = Get-P4kxPlatform -Target 'aarch64-apple-darwin'
foreach ($case in @(
    @{ name = 'tag minimum'; role = 'minimum'; version = '26.7.2'; build = '25G123'; run = '12'; artifact = $artifactId },
    @{ name = 'tag current'; role = 'current'; version = $stable; build = '26B123'; run = '12'; artifact = $artifactId },
    @{ name = 'dry-run minimum'; role = 'minimum'; version = '26.7.2'; build = '25G123'; run = '13'; artifact = '53' },
    @{ name = 'dry-run current'; role = 'current'; version = "$(([version]$stable).Major).0.1"; build = '26A434'; run = '13'; artifact = '53' }
)) {
    $role = $case.role
    $runtime = @{
        revision = 'a' * 40
        workflow_run = "https://github.com/p4kx-org/p4kx/actions/runs/$($case.run)"
        workflow_attempt = '2'
        runtime_host = @{ architecture = 'arm64'; os_version = $case.version; os_build = $case.build }
    }
    $runtime.blender = @{
        scope = 'native-blender-synthetic'
        role = $role
        revision = $runtime.revision
        workflow_run = $runtime.workflow_run
        workflow_attempt = $runtime.workflow_attempt
        extension_artifact_id = $case.artifact
        extension_sha256 = $addonHash
        cpu = 'arm64'
        os_version = $runtime.runtime_host.os_version
        os_build = $runtime.runtime_host.os_build
        blender_version = $pin.version
        blender_distribution_sha256 = $pin.sha256
        blender_sha256 = 'b' * 64
        extension_install = 'success'
        installed_import = 'success'
    }
    Assert-P4kxMacosBlenderAddonEvidence $runtime $role $case.artifact $addonHash
    $missing = $runtime.Clone()
    $missing.Remove('blender')
    Assert-BlenderReceiptRefused $missing $role $case.artifact $addonHash "$($case.name) missing receipt"
    Assert-BlenderReceiptRefused $runtime $role $case.artifact ('c' * 64) "$($case.name) changed ZIP"
    $otherRole = if ($role -ceq 'minimum') { 'current' } else { 'minimum' }
    foreach ($mutation in @(
        @{ name = 'role'; value = $otherRole },
        @{ name = 'scope'; value = 'generic-smoke' },
        @{ name = 'revision'; value = ('b' * 40) },
        @{ name = 'workflow_run'; value = 'https://github.com/p4kx-org/p4kx/actions/runs/14' },
        @{ name = 'workflow_attempt'; value = '3' },
        @{ name = 'extension_artifact_id'; value = '54' },
        @{ name = 'extension_sha256'; value = ('b' * 64) },
        @{ name = 'cpu'; value = 'x64' },
        @{ name = 'os_version'; value = '27.2' },
        @{ name = 'os_build'; value = '26B124' },
        @{ name = 'blender_version'; value = '5.3.0' },
        @{ name = 'blender_distribution_sha256'; value = ('c' * 64) },
        @{ name = 'blender_sha256'; value = 'missing' },
        @{ name = 'extension_install'; value = 'skipped' },
        @{ name = 'installed_import'; value = 'failed' }
    )) {
        $changed = $runtime.Clone()
        $changed.blender = $runtime.blender.Clone()
        $changed.blender[$mutation.name] = $mutation.value
        Assert-BlenderReceiptRefused $changed $role $case.artifact $addonHash "$($case.name) $($mutation.name)"
    }
    if ($case.name -ceq 'tag current') {
        Assert-P4kxMacosHostedRole $runtime.runtime_host $platform current -RequireStableRelease
    }
    if ($case.name -ceq 'dry-run current') {
        $stableRefused = $false
        try {
            Assert-P4kxMacosHostedRole $runtime.runtime_host $platform current -RequireStableRelease
        }
        catch {
            if ($_.Exception.Message -cnotmatch 'Tagged release requires the current stable macOS') { throw }
            $stableRefused = $true
        }
        if (!$stableRefused) { throw 'A non-stable dry-run host satisfied the exact tagged release gate.' }
    }
}
Write-Output 'Native macOS Blender receipt controls passed for both roles on dry runs and tags.'
