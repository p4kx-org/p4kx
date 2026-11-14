$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-source.ps1')
. (Join-Path $PSScriptRoot 'documents-session-proof.ps1')
. (Join-Path $PSScriptRoot 'flatpak-prerequisite-proof.ps1')

function Assert-P4kxDocumentsServiceBuild {
    param($Build, [string]$Target, [string]$Revision)
    $expected = Get-P4kxDocumentsPortalRecipe
    if ($Build.schema -ne 1 -or $Build.target -cne $Target -or $Build.revision -cne $Revision -or $Build.role -cne 'disposable CI services only' -or
        $Build.source.sha256 -cne $expected.sha256 -or $Build.source.source_commit -cne $expected.source_commit -or $Build.source.version -cne $expected.version -or $Build.source.bytes -ne $expected.bytes) {
        throw 'Native Documents-service build has different source/target provenance.'
    }
    $null = ConvertFrom-P4kxDocumentsPortalRecipe ($Build.source | ConvertTo-Json -Depth 8)
    foreach ($wrap in $expected.wraps.PSObject.Properties) { if ($Build.source.wraps.($wrap.Name) -cne $wrap.Value) { throw 'Documents vendored source revision differs.' } }
    foreach ($field in @('apt_packages', 'pkg_config')) {
        if ((@($Build.source.$field | Sort-Object) -join ',') -cne (@($expected.$field | Sort-Object) -join ',')) { throw 'Documents build dependency recipe differs.' }
    }
    if ((@($Build.binaries.name | Sort-Object) -join ',') -cne 'xdg-document-portal,xdg-permission-store') { throw 'Native Documents build does not contain both exact services.' }
    $platform = Get-P4kxLinuxPlatform $Target
    foreach ($binary in $Build.binaries) {
        if ($binary.machine -ne $platform.elf_machine -or $binary.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Native Documents executable CPU/hash differs.' }
    }
    Assert-P4kxDocumentsBuildInventory $Build $expected
}

function Assert-P4kxDocumentsBuildInventory {
    param($Build, $Recipe)
    if ((@($Build.tools.name | Sort-Object) -join ',') -cne 'cc,meson,ninja,pkg-config' -or
        (@($Build.dependencies.name | Sort-Object) -join ',') -cne (@($Recipe.pkg_config | Sort-Object) -join ',') -or
        -not $Build.apt_packages -or @($Build.licenses).Count -lt 3) { throw 'Observed Documents tools, dependency or license inventory is incomplete.' }
    foreach ($item in @($Build.tools) + @($Build.dependencies)) {
        if ($item.version -isnot [string] -or -not $item.version.Trim()) { throw 'Documents tool/dependency version was not observed.' }
    }
    foreach ($license in $Build.licenses) {
        if (-not $license.path -or $license.path.StartsWith('/') -or '..' -cin ($license.path -split '/') -or
            $license.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Documents license source identity is invalid.' }
    }
}

function Assert-P4kxFlatpakEntryRefusal {
    <#
    .SYNOPSIS
    Requires the selected hardlink Entry to fail without writing any Entry.
    #>
    param($Command)
    $report = $Command.report
    if (($Command.exit_code -isnot [int] -and $Command.exit_code -isnot [long]) -or $Command.exit_code -ne 1 -or
        ($report.extracted -isnot [int] -and $report.extracted -isnot [long]) -or $report.extracted -ne 0 -or
        $report.failed -isnot [Array] -or $report.failed.Count -ne 1 -or
        $report.skipped -isnot [Array] -or $report.skipped.Count -ne 0 -or
        $report.cancelled -isnot [Array] -or $report.cancelled.Count -ne 0 -or
        $report.archive_changed -isnot [bool] -or $report.archive_changed -or $null -ne $report.records) {
        throw 'Hardlink Entry refusal lacks its actual partial-failure status and exact unwritten report.'
    }
    $failure = $report.failed[0]
    if ($failure.path -cne 'Data/hello.txt' -or $failure.error -isnot [string] -or
        $failure.error -cnotmatch '\A\[p4kx\.write\.(source-overwrite|target-identity)\]') {
        throw 'Hardlink Entry did not fail for its exact typed write-protection reason.'
    }
    $classification = $Matches[1]
    $codes = @([regex]::Matches($failure.error, '\[p4kx\.write\.([^\]]+)\]') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ($codes.Count -ne 1 -or $codes[0] -cne $classification) { throw 'Hardlink Entry has conflicting write-protection categories.' }
    return $classification
}

function Assert-P4kxFlatpakDocumentsEvidence {
    param($Documents, $Platform, [string]$Revision, [string]$PackageHash)
    if ($Documents.schema -ne 2 -or $Documents.target -cne $Platform.target -or $Documents.revision -cne $Revision -or $Documents.package_sha256 -cne $PackageHash) {
        throw 'Documents verification is not bound to this source/CPU/package.'
    }
    foreach ($name in @('read_only_archive', 'writable_export', 'game_alias_refusal', 'source_unchanged', 'source_membership_refusal', 'empty_protection_refusal', 'absent_suffix_refusal', 'ordinary_copy')) {
        if ($Documents.$name -cne 'success') { throw "Documents required case '$name' did not execute successfully." }
    }
    Assert-P4kxDocumentsServiceBuild $Documents.service_build $Platform.target $Revision
    Assert-P4kxDocumentsSessionsEvidence $Documents
    $hardlink = $Documents.hardlink
    if ($hardlink.outcome -cne 'success' -or $hardlink.host_hardlink -cne 'proven' -or $hardlink.separate_documents -ne $true -or
        $hardlink.classification -cnotin @('source-overwrite', 'target-identity') -or $hardlink.source_unchanged -cne 'success' -or
        $hardlink.target_unchanged -cne 'success' -or $hardlink.foreign_output_unchanged -cne 'success') {
        throw 'Documents hardlink refusal lacks actual host/link/error evidence.'
    }
    if ((Assert-P4kxFlatpakEntryRefusal $hardlink.command) -cne $hardlink.classification) { throw 'Hardlink report and evidence classifications differ.' }
    $forgery = $Documents.forged_underlying_xattr
    if ($forgery.outcome -cne 'success' -or $forgery.underlying -cne 'forged' -or $forgery.fuse_view -cnotin @('forwarded', 'portal-synthesized') -or
        $forgery.product_visible_forgery_proven -ne $false) { throw 'Underlying forgery and observed host-FUSE view were conflated.' }
}

function Assert-P4kxDocumentsSessionsEvidence {
    param($Documents)
    $negative = $Documents.api4_prerequisite
    if ($negative.target -cne $Documents.target -or $negative.revision -cne $Documents.revision -or $negative.interface.interface_version -ne 4 -or $negative.prerequisite.outcome -cne 'expected-refusal' -or $negative.prerequisite.runtime_pass -ne $false) { throw 'API4 negative was omitted, rebound or promoted to runtime success.' }
    Assert-P4kxFlatpakApi4Evidence $negative $Documents.stock_session
    if ($Documents.api5_interface.interface_version -ne 5 -or $Documents.api5_interface.get_host_paths -ne $true -or $Documents.api5_interface.empty_host_paths -cne 'success') { throw 'Real API5 interface/method evidence is missing.' }
    foreach ($session in @($Documents.stock_session, $Documents.api5_session)) {
        if ($session.controller.worker.exit_code -ne 0) { throw 'Documents main-session worker failed; teardown is not runtime success.' }
        if (-not $session.session -or $session.teardown.source_session -cne $session.session -or
            $session.teardown.processes_exited -cne 'success' -or $session.teardown.mount_gone -cne 'success' -or $session.teardown.controller_processes_exited -cne 'success') { throw 'Documents session teardown does not prove its owned process and mount removal.' }
    }
    Assert-P4kxDocumentsControllerEvidence $Documents.stock_session.controller $Documents.stock_session.session 'stock'
    Assert-P4kxDocumentsControllerEvidence $Documents.api5_session.controller $Documents.api5_session.session 'api5'
    if ($Documents.stock_session.session -ceq $Documents.api5_session.session -or
        $Documents.api5_session.session -cnotmatch '^unix:abstract=p4kx-docs-[0-9a-f]{32}(?:,guid=[0-9a-f]{32})?$' -or
        $Documents.api5_session.auth -cne 'EXTERNAL' -or $Documents.api5_session.configuration_sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'API5 did not use an independently authenticated private bus.' }
    $observed = @($Documents.api5_session.initial_services)
    $uid = $Documents.api5_session.user_uid
    if (($uid -isnot [int] -and $uid -isnot [long] -and $uid -isnot [uint32]) -or $uid -lt 0 -or $uid -gt [uint32]::MaxValue) { throw 'API5 native user credential was not observed.' }
    if ((@($observed.service | Sort-Object) -join ',') -cne 'org.freedesktop.impl.portal.PermissionStore,org.freedesktop.portal.Documents') { throw 'API5 owner-source evidence is incomplete.' }
    foreach ($service in $observed) {
        $built = @($Documents.service_build.binaries | Where-Object path -CEQ $service.executable)
        if ($built.Count -ne 1 -or $built[0].sha256 -cne $service.sha256 -or $service.owner -cnotmatch '^:[0-9]+\.[0-9]+$' -or
            $service.uid -ne $uid) { throw 'Actual API5 owner executable/user is not the built official source.' }
        if ($service.service -ceq 'org.freedesktop.portal.Documents' -and $service.owner -cne $Documents.api5_interface.owner) { throw 'API5 capability query and executable ownership differ.' }
    }
}

function Get-P4kxDocumentsCombinedEvidence {
    param($Phases, $Services, [string]$PackageHash)
    $package = Get-Content -LiteralPath (Join-Path $Phases.positive 'package.json') -Raw | ConvertFrom-Json
    $negative = Get-Content -LiteralPath (Join-Path $Phases.stock 'negative.json') -Raw | ConvertFrom-Json
    $stock = Get-Content -LiteralPath (Join-Path $Phases.stock 'session-services-final.json') -Raw | ConvertFrom-Json
    $stockInitial = Get-Content -LiteralPath (Join-Path $Phases.stock 'session-services-initial.json') -Raw | ConvertFrom-Json
    $positive = Get-Content -LiteralPath (Join-Path $Phases.positive 'session-services-initial.json') -Raw | ConvertFrom-Json
    $stockEnd = Get-Content -LiteralPath (Join-Path $Phases.stock 'teardown.json') -Raw | ConvertFrom-Json
    $positiveEnd = Get-Content -LiteralPath (Join-Path $Phases.positive 'teardown.json') -Raw | ConvertFrom-Json
    $stockController = Get-Content -LiteralPath (Join-Path $Phases.stock 'session-controller.json') -Raw | ConvertFrom-Json
    $positiveController = Get-Content -LiteralPath (Join-Path $Phases.positive 'session-controller.json') -Raw | ConvertFrom-Json
    $documents = $package.documents
    $configPath = Join-Path $Phases.positive 'activation/session.conf'
    $config = [Xml.XmlDocument]::new(); $config.XmlResolver = $null; $config.LoadXml((Get-Content -LiteralPath $configPath -Raw))
    $documents | Add-Member -NotePropertyName schema -NotePropertyValue 2
    $documents | Add-Member -NotePropertyName package_sha256 -NotePropertyValue $PackageHash
    $documents | Add-Member -NotePropertyName service_build -NotePropertyValue $Services
    $documents | Add-Member -NotePropertyName api4_prerequisite -NotePropertyValue $negative
    $documents | Add-Member -NotePropertyName stock_session -NotePropertyValue @{session=$stock.session; initial_session=$stockInitial.session; initial_services=$stockInitial.services; final_services=$stock.observed_services; teardown=$stockEnd; controller=$stockController}
    $documents | Add-Member -NotePropertyName api5_session -NotePropertyValue @{session=$positive.session; controller=$positiveController; user_uid=$positive.user_uid; initial_services=$positive.services; teardown=$positiveEnd; auth=$config.SelectSingleNode('/busconfig/auth').InnerText; configuration_sha256=(Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash.ToLowerInvariant()}
    return $package
}
