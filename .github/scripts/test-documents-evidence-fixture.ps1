$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-evidence.ps1')

function New-P4kxDocumentsEvidenceFixture {
    <#
    .SYNOPSIS
    Creates structural validator input without asserting any native execution.
    #>
    param($Platform, [string]$Revision, [string]$PackageHash)
    $binaries = @(
        @{name='xdg-document-portal'; path='/synthetic/services/xdg-document-portal'; machine=$Platform.elf_machine; sha256=('a' * 64)},
        @{name='xdg-permission-store'; path='/synthetic/services/xdg-permission-store'; machine=$Platform.elf_machine; sha256=('b' * 64)}
    )
    $observed = @(
        @{service='org.freedesktop.portal.Documents'; executable=$binaries[0].path; sha256=$binaries[0].sha256; uid=1000; owner=':1.10'},
        @{service='org.freedesktop.impl.portal.PermissionStore'; executable=$binaries[1].path; sha256=$binaries[1].sha256; uid=1000; owner=':1.11'}
    )
    $stock = 'unix:abstract=synthetic-stock'
    $positive = 'unix:abstract=p4kx-docs-' + ('c' * 32)
    $proof = @{schema=2; target=$Platform.target; revision=$Revision; package_sha256=$PackageHash;
        service_build=@{schema=1; target=$Platform.target; revision=$Revision; role='disposable CI services only'; source=(Get-P4kxDocumentsPortalRecipe); binaries=$binaries};
        api4_prerequisite=@{target=$Platform.target; revision=$Revision; interface=@{interface_version=4}; prerequisite=@{outcome='expected-refusal'; runtime_pass=$false; applications=@(
            @{application='p4kx'; prerequisite='expected-refusal'; help='success'; version='success'},
            @{application='p4kx-gui'; prerequisite='expected-refusal'; help='success'; version='success'},
            @{application='p4kx-mcp'; prerequisite='expected-refusal'; help='success'; version='success'})}};
        api5_interface=@{interface_version=5; get_host_paths=$true; empty_host_paths='success'; owner=':1.10'};
        stock_session=@{session=$stock; teardown=@{source_session=$stock; processes_exited='success'; mount_gone='success'; controller_processes_exited='success'}};
        api5_session=@{session=$positive; user_uid=1000; auth='EXTERNAL'; configuration_sha256=('d' * 64); initial_services=$observed; teardown=@{source_session=$positive; processes_exited='success'; mount_gone='success'; controller_processes_exited='success'}};
        hardlink=@{outcome='success'; host_hardlink='proven'; separate_documents=$true; classification='target-identity'; source_unchanged='success'};
        forged_underlying_xattr=@{outcome='success'; underlying='forged'; fuse_view='portal-synthesized'; product_visible_forgery_proven=$false}}
    foreach ($name in @('read_only_archive', 'writable_export', 'game_alias_refusal', 'source_unchanged', 'source_membership_refusal', 'empty_protection_refusal', 'absent_suffix_refusal', 'ordinary_copy')) { $proof[$name] = 'success' }
    $proof.service_build['tools'] = @(foreach ($name in @('cc', 'meson', 'ninja', 'pkg-config')) { @{name=$name; version='synthetic'} })
    $proof.service_build['dependencies'] = @(foreach ($name in $proof.service_build.source.pkg_config) { @{name=$name; version='synthetic'} })
    $proof.service_build['apt_packages'] = 'synthetic structural fixture'
    $proof.service_build['licenses'] = @(foreach ($path in @('COPYING', 'subprojects/libglnx/COPYING', 'subprojects/gvdb/COPYING')) { @{path=$path; sha256=('e' * 64)} })
    foreach ($session in @(@{name='stock_session'; role='stock'; bus=100; worker=101}, @{name='api5_session'; role='api5'; bus=102; worker=103})) {
        $proof[$session.name]['controller'] = @{schema=1; role=$session.role; session=$proof[$session.name].session;
            signal_contract='direct-process-only'; direct_processes_exited=$true;
            bus=@{started=$true; exited=$true; pid=$session.bus; exit_code=137; signal_contract='direct-process-only'};
            worker=@{started=$true; exited=$true; pid=$session.worker; exit_code=0; signal_contract='direct-process-only'}}
    }
    return $proof
}
