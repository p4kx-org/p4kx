<#
.SYNOPSIS
Mutates source-bound Documents verification without connecting to any service.
#>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'test-documents-evidence-fixture.ps1')
. (Join-Path $PSScriptRoot 'flatpak-documents.ps1')
. (Join-Path $PSScriptRoot 'flatpak-prerequisites.ps1')
$platform = Get-P4kxLinuxPlatform 'x86_64-unknown-linux-gnu'
$revision = 'a' * 40
$hash = 'b' * 64
$original = (New-P4kxDocumentsEvidenceFixture $platform $revision $hash) | ConvertTo-Json -Depth 24
Assert-P4kxFlatpakDocumentsEvidence ($original | ConvertFrom-Json) $platform $revision $hash
foreach ($mutation in @(
    { param($value) $value.schema = 1 }, { param($value) $value.target = 'aarch64-unknown-linux-gnu' },
    { param($value) $value.package_sha256 = 'f' * 64 }, { param($value) $value.service_build.source.sha256 = 'f' * 64 },
    { param($value) $value.service_build.binaries[0].machine = 0 }, { param($value) $value.api4_prerequisite.prerequisite.runtime_pass = $true },
    { param($value) $value.api4_prerequisite.prerequisite.applications = @() }, { param($value) $value.api4_prerequisite.revision = 'f' * 40 },
    { param($value) $value.api5_interface.get_host_paths = $false }, { param($value) $value.api5_interface.owner = ':1.99' },
    { param($value) $value.stock_session.teardown.mount_gone = 'skipped' }, { param($value) $value.api5_session.teardown.processes_exited = 'skipped' },
    { param($value) $value.stock_session.session = $value.api5_session.session }, { param($value) $value.api5_session.auth = 'ANONYMOUS' },
    { param($value) $value.api5_session.initial_services[0].sha256 = 'f' * 64 }, { param($value) $value.game_alias_refusal = 'skipped' },
    { param($value) $value.hardlink.classification = 'proxy-inodes-differ' }, { param($value) $value.hardlink.host_hardlink = 'not-tested' },
    { param($value) $value.hardlink.separate_documents = $false }, { param($value) $value.forged_underlying_xattr.product_visible_forgery_proven = $true },
    { param($value) $value.service_build.tools = @() }, { param($value) $value.service_build.dependencies[0].version = '' },
    { param($value) $value.service_build.licenses = @() },
    { param($value) $value.api5_session.user_uid = $null }, { param($value) $value.api5_session.initial_services[0].uid = 2000 },
    { param($value) $value.stock_session.controller = $null },
    { param($value) $value.api5_session.controller.session = 'unix:abstract=foreign' },
    { param($value) $value.api5_session.controller.bus.exited = $false },
    { param($value) $value.api5_session.controller.worker.signal_contract = 'entire-process-tree' },
    { param($value) $value.stock_session.controller.worker.pid = $value.stock_session.controller.bus.pid },
    { param($value) $value.stock_session.controller.worker.exit_code = 37 },
    { param($value) $value.api5_session.controller.worker.exit_code = 37 },
    { param($value) $value.api5_session.teardown.controller_processes_exited = 'skipped' }
)) {
    $changed = $original | ConvertFrom-Json
    & $mutation $changed
    $rejected = $false
    try { Assert-P4kxFlatpakDocumentsEvidence $changed $platform $revision $hash } catch { $rejected = $true }
    if (-not $rejected) { throw 'Fabricated, skipped or rebound Documents evidence was accepted.' }
}
$membership = @{exit_code=2; stdout=''; stderr='[p4kx.write.source-membership] synthetic refusal'}
if ((Assert-P4kxFlatpakDocumentOutcome $membership 'SourceMembership') -cne 'source-membership') { throw 'Typed neutral membership was lost.' }
foreach ($reason in @('source-overwrite', 'target-identity')) {
    $result = @{exit_code=2; stdout=''; stderr="[p4kx.write.$reason] synthetic refusal"}
    if ((Assert-P4kxFlatpakDocumentOutcome $result 'ExistingSource') -cne $reason) { throw 'Hardlink identity uncertainty was mislabeled.' }
}
foreach ($text in @('other failure', 'the tool never writes there', '[p4kx.write.source-identity]', '[p4kx.write.source-membership] [p4kx.write.target-identity]')) {
    $rejected = $false
    try { Assert-P4kxFlatpakDocumentOutcome @{exit_code=2; stderr=$text} 'SourceMembership' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Another refusal category was promoted to neutral membership.' }
}
Assert-P4kxFlatpakPrerequisiteOutcome @{exit_code=1; stderr='[p4kx.runtime.documents-api5] synthetic'}
'PASS: structural contract, thirty-three provenance/capability/controller mutations and exact typed refusals; no portal or runtime executed.'
