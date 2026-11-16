$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'arm64-device-evidence.ps1')

$contract = Get-P4kxPlatformContract
$release = @{ Version='0.5.0'; Revision=('1' * 40); ReleaseRunId='1234' }

function New-DesktopFixture($Row, $Release) {
    $kind = if ($Row.os -ceq 'windows') { 'portable-zip' } else { 'flatpak' }
    $runtime = @{
        scope='native-synthetic'; target=$Row.target; version=$Release.Version; revision=$Release.Revision
        workflow_run="https://github.com/p4kx-org/p4kx/actions/runs/$($Release.ReleaseRunId)"
        workflow_attempt='2'
        package_smokes=@(
            @{
                package=$kind; target=$Row.target; archive_sha256=('a' * 64)
                binaries=@(
                    @{ name='p4kx-gui'; machine=(Get-P4kxNativeMachine $Row); sha256=('b' * 64) }
                )
            }
        )
    }
    $checks = @{}
    foreach ($name in (Get-P4kxArm64RequiredChecks)) {
        $checks[$name] = @{ outcome='success'; observation='synthetic observation' }
    }
    $checks.file_dialogs.expected_sha256 = 'c' * 64
    $checks.file_dialogs.observed_sha256 = 'c' * 64
    $checks.no_audio_device.test = 'a_missing_device_is_reported_to_the_player_and_does_not_panic'
    $checks.hardware_gpu = @{ outcome='unavailable'; observation='synthetic unavailable device' }
    $record = @{
        schema=1; scope='native-device'; target=$Row.target; version=$runtime.version; revision=$runtime.revision
        workflow_run=$runtime.workflow_run; workflow_attempt=$runtime.workflow_attempt
        package=$kind; package_filename=(Get-P4kxAssetName -Target $Row.target -Version $runtime.version -Package $kind)
        archive_sha256=('a' * 64); gui_sha256=('b' * 64); fixture_revision=$runtime.revision
        tested_on='2026-11-16'
        host=@{
            os=$Row.os; architecture='arm64'; os_version='synthetic OS'; os_build_or_kernel='synthetic build'
            desktop_session='synthetic desktop'; gpu='synthetic adapter'; gpu_driver='synthetic driver'
            audio_backend='synthetic audio'
        }
        checks=$checks
    }
    return @{ runtime=$runtime; records=@($record) } | ConvertTo-Json -Depth 12
}

function Assert-DeviceFixture($Fixture, $Platform, $Release) {
    Assert-P4kxArm64DeviceEvidence @Release -Runtime $Fixture.runtime -Platform $Platform -Records $Fixture.records
}

function Assert-RejectedDeviceFixture($Fixture, $Platform, $Release, [string]$ExpectedError) {
    try { Assert-DeviceFixture $Fixture $Platform $Release }
    catch {
        if ($_.Exception.Message -cnotmatch $ExpectedError) { throw }
        return
    }
    throw "Accepted an invalid ARM64 desktop receipt that should have failed: $ExpectedError"
}

$targets = @($contract.targets | Where-Object { Test-P4kxArm64DesktopTarget $_ })
$all = @()
foreach ($row in $targets) {
    $fixture = New-DesktopFixture $row $release
    $valid = $fixture | ConvertFrom-Json
    $all += $valid.records
    Assert-DeviceFixture $valid $row $release
    foreach ($case in @(
        @{ edit={ param($p) $p.records=@() }; error='Missing native ARM64' },
        @{ edit={ param($p) $p.records[0].schema='1' }; error='schema or scope' },
        @{ edit={ param($p) $p.records[0].scope='native-synthetic' }; error='schema or scope' },
        @{ edit={ param($p) $p.records[0].target='x86_64-unknown-linux-gnu' }; error='target differs' },
        @{ edit={ param($p) $p.records[0].version='0.4.2' }; error='version differs' },
        @{ edit={ param($p) $p.records[0].revision='2' * 40 }; error='revision differs' },
        @{ edit={ param($p) $p.records[0].workflow_run='https://github.com/p4kx-org/p4kx/actions/runs/4321' }; error='workflow_run differs' },
        @{ edit={ param($p) $p.records[0].workflow_attempt='3' }; error='workflow_attempt differs' },
        @{ edit={ param($p) $p.runtime.PSObject.Properties.Remove('workflow_attempt') }; error='required release revision' },
        @{ edit={ param($p) $p.runtime.workflow_run='https://github.com/p4kx-org/p4kx/actions/runs/4321' }; error='required release revision' },
        @{ edit={ param($p) $p.records[0].package='dmg' }; error='package is outside' },
        @{ edit={ param($p) $p.records[0].package_filename='an-unrelated-package.zip' }; error='filename, target or SHA-256' },
        @{ edit={ param($p) $p.records[0].archive_sha256='f' * 64 }; error='filename, target or SHA-256' },
        @{ edit={ param($p) $p.records[0].gui_sha256='f' * 64 }; error='GUI machine or SHA-256' },
        @{ edit={ param($p) $p.runtime.package_smokes[0].binaries[0].machine=0 }; error='GUI machine or SHA-256' },
        @{ edit={ param($p) $p.records[0].fixture_revision='3' * 40 }; error='fixture revision differs' },
        @{ edit={ param($p) $p.records[0].host.architecture='amd64' }; error='host OS or native CPU' },
        @{ edit={ param($p) $p.records[0].host.os='macos' }; error='host OS or native CPU' },
        @{ edit={ param($p) $p.records[0].host=@($p.records[0].host) }; error='host OS or native CPU' },
        @{ edit={ param($p) $p.records[0].host.gpu_driver='' }; error='host field.*gpu_driver' },
        @{ edit={ param($p) $p.records[0].tested_on='2026-02-31' }; error='observed test date' },
        @{ edit={ param($p) $p.records[0].checks=@($p.records[0].checks) }; error='checks must be a JSON object' },
        @{ edit={ param($p) $p.records[0].checks.PSObject.Properties.Remove('clipboard') }; error='clipboard.*exact check name' },
        @{ edit={ param($p) $p.records[0].checks.hardware_gpu.observation='' }; error='hardware_gpu.*observed evidence' },
        @{ edit={ param($p) $p.records[0].checks.file_dialogs.observed_sha256='d' * 64 }; error='matching expected and extracted' },
        @{ edit={ param($p) $p.records[0].checks.viewer.outcome='skipped' }; error='viewer.*outcome' },
        @{ edit={ param($p) $p.records[0].checks.audio_pause.outcome='not_checked' }; error='audio_pause.*no successful record' },
        @{ edit={ param($p) $p.records[0].checks.no_audio_device.test='different test' }; error='required native test' }
    )) {
        $changed = $fixture | ConvertFrom-Json
        & $case.edit $changed
        Assert-RejectedDeviceFixture $changed $row $release $case.error
    }
    $first = $fixture | ConvertFrom-Json
    $second = $fixture | ConvertFrom-Json
    $first.records[0].checks.clipboard.outcome = 'not_checked'
    $second.records[0].checks.viewer.outcome = 'unavailable'
    $first.records += $second.records
    Assert-DeviceFixture $first $row $release
}
Assert-P4kxArm64ReceiptRows -Rows $all -Contract $contract
$all[0].target = 'aarch64-apple-darwin'
try {
    Assert-P4kxArm64ReceiptRows -Rows $all -Contract $contract
    throw 'Accepted an unrelated target in the ARM64 receipt array.'
} catch {
    if ($_.Exception.Message -cnotmatch 'unsupported device target') { throw }
}
'PASS: ARM64 desktop receipt validation binds native Windows and Linux observations to the exact release packages.'
