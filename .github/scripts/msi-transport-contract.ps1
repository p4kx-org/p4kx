$ErrorActionPreference = 'Stop'

function Get-P4kxMsiTransportActions {
    $active = 'NOT UPGRADINGPRODUCTCODE AND ($Application = 2 OR $Application = 3)'
    $captured = 'NOT UPGRADINGPRODUCTCODE AND P4kxPathCapturedBinding AND P4kxPathCapturedBinding <> "unbound"'
    $arguments = '"[ProductCode]" "[P4kxPathPackageCode]" "[P4kxPathCapturedBinding]"'
    return @(
        @{name='P4kxClearCapturedBinding';type=51;source='P4kxPathCapturedBinding';target='unbound';sequence=1;condition='NOT UPGRADINGPRODUCTCODE'},
        @{name='P4kxClearUpgradeDetected';type=51;source='WIX_UPGRADE_DETECTED';target='';sequence=20;condition='NOT UPGRADINGPRODUCTCODE'},
        @{name='P4kxClearDowngradeDetected';type=51;source='WIX_DOWNGRADE_DETECTED';target='';sequence=21;condition='NOT UPGRADINGPRODUCTCODE'},
        @{name='P4kxRequireRollback';type=19;source='';target='P4KX PATH ownership requires Windows Installer rollback.';sequence=1410;condition="$active AND RollbackDisabled"},
        @{name='P4kxPathReserveEnsure';type=8194;source='P4kxPathHelper';target='reserve ensure "[ProductCode]" "[P4kxPathPackageCode]" "[WIX_UPGRADE_DETECTED]" "[APPLICATIONFOLDER]."';sequence=1420;condition='NOT UPGRADINGPRODUCTCODE AND $Application = 3'},
        @{name='P4kxPathReserveRemove';type=8194;source='P4kxPathHelper';target='reserve remove "[ProductCode]" "[P4kxPathPackageCode]"';sequence=1421;condition='NOT UPGRADINGPRODUCTCODE AND $Application = 2'},
        @{name='P4kxResetBindingSearch';type=51;source='P4KX_PATH_BINDING';target='unbound';sequence=1430;condition=$active},
        @{name='P4kxCaptureBinding';type=51;source='P4kxPathCapturedBinding';target='[P4KX_PATH_BINDING]';sequence=1450;condition=$active},
        @{name='P4kxClearBindingSearch';type=51;source='P4KX_PATH_BINDING';target='unbound';sequence=1460;condition=$active},
        @{name='P4kxPathPreflight';type=8194;source='P4kxPathHelper';target="preflight $arguments";sequence=1470;condition=$active},
        @{name='P4kxPathRollback';type=9474;source='P4kxPathHelper';target="rollback $arguments";sequence=1530;condition=$active},
        @{name='P4kxPathPrepare';type=9218;source='P4kxPathHelper';target="prepare $arguments";sequence=6410;condition=$active},
        @{name='P4kxPathApply';type=9218;source='P4kxPathHelper';target="apply $arguments";sequence=6420;condition=$active},
        @{name='P4kxPathCommit';type=9730;source='P4kxPathHelper';target="commit $arguments";sequence=6430;condition=$active},
        @{name='P4kxPathErrorCleanup';type=8194;source='P4kxPathHelper';target="rollback $arguments";sequence=-3;condition=$captured},
        @{name='P4kxPathCancelCleanup';type=8194;source='P4kxPathHelper';target="rollback $arguments";sequence=-2;condition=$captured}
    )
}

function Assert-P4kxMsiTransportActions {
    param($Metadata)
    $expected = @(Get-P4kxMsiTransportActions)
    foreach ($action in $expected) {
        $rows = @($Metadata.custom_actions | Where-Object { $_[0] -ceq $action.name })
        if ($rows.Count -ne 1 -or [int]$rows[0][1] -ne $action.type -or
            [string]$rows[0][2] -cne $action.source -or [string]$rows[0][3] -cne $action.target) {
            throw "MSI helper action definition differs: $($action.name)"
        }
        $scheduled = @($Metadata.execute | Where-Object { $_[0] -ceq $action.name })
        if ($scheduled.Count -ne 1 -or [int]$scheduled[0][2] -ne $action.sequence -or
            [string]$scheduled[0][1] -cne $action.condition) {
            throw "MSI helper sequence or upgrade exclusion differs: $($action.name)"
        }
    }
    foreach ($row in $Metadata.custom_actions) {
        if ($row[0] -cin $expected.name) { continue }
        if ([int]$row[1] -ne 19 -or [string]$row[2] -cne '') {
            throw 'MSI contains an unaudited custom action outside the helper transport.'
        }
    }
    $actual = @($Metadata.custom_actions | Where-Object { $_[2] -ceq 'P4kxPathHelper' } | ForEach-Object { $_[0] } | Sort-Object)
    $names = @($expected | Where-Object source -CEQ 'P4kxPathHelper' | ForEach-Object name | Sort-Object)
    if (($actual -join ',') -cne ($names -join ',')) { throw 'MSI contains an unexpected embedded-helper command.' }
    if (@($Metadata.ui | Where-Object { $_[0] -in @('AppSearch','FindRelatedProducts') -or $_[0] -in $expected.name }).Count -gt 0) {
        throw 'MSI UI sequence must not search, reserve or mutate helper state.'
    }
}

function Get-P4kxMsiTransportProperty {
    param($Metadata, [string]$Name)
    if ($Metadata.properties -is [Collections.IDictionary]) {
        $keys = @($Metadata.properties.Keys | Where-Object { $_ -ceq $Name })
        if ($keys.Count -ne 1) { throw 'Missing or ambiguous case-exact MSI transport property.' }
        return [string]$Metadata.properties[$keys[0]]
    }
    $properties = @($Metadata.properties.PSObject.Properties | Where-Object { $_.Name -ceq $Name })
    if ($properties.Count -ne 1) { throw 'Missing or ambiguous case-exact MSI transport property.' }
    return [string]$properties[0].Value
}
function Assert-P4kxMsiTransportSearch {
    param($Metadata)
    if ((Get-P4kxMsiTransportProperty $Metadata 'P4kxPathPackageCode') -cne $Metadata.package -or
        $Metadata.package -cnotmatch '^\{[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\}$' -or
        [guid]$Metadata.package -eq [guid]::Empty) { throw 'The helper does not bind the actual SummaryInformation PackageCode.' }
    foreach ($name in @('P4kxPathCapturedBinding','P4KX_PATH_BINDING')) {
        if ((Get-P4kxMsiTransportProperty $Metadata $name) -cne 'unbound' -or $name -cnotin ((Get-P4kxMsiTransportProperty $Metadata 'MsiHiddenProperties') -split ';')) {
            throw 'MSI binding sentinels or hidden-property declarations differ.'
        }
    }
    $searches = @($Metadata.app_search)
    if ($searches.Count -ne 1 -or $searches[0][0] -cne 'P4KX_PATH_BINDING' -or
        $searches[0][1] -cne 'P4kxInvocationBinding') { throw 'MSI must have exactly the explicit invocation AppSearch.' }
    $locators = @($Metadata.reg_locator)
    if ($locators.Count -ne 1 -or $locators[0][0] -cne 'P4kxInvocationBinding' -or
        [int]$locators[0][1] -ne 1 -or $locators[0][2] -cne 'Software\P4KX-Installer\PathOwnership\Invocation' -or
        $locators[0][3] -cne 'Binding' -or [int]$locators[0][4] -ne 18) {
        throw 'MSI binding lookup must read the fixed raw REG_SZ in the HKCU 64-bit view.'
    }
    $upgrades = @($Metadata.upgrade_properties | Sort-Object)
    if (($upgrades -join ',') -cne 'WIX_DOWNGRADE_DETECTED,WIX_UPGRADE_DETECTED') {
        throw 'Every generated Upgrade action property must have an explicit fresh execute reset.'
    }
}

<#
.SYNOPSIS
Validates compiled MSI helper transport without executing an installer or helper.
#>
function Assert-P4kxMsiTransport {
    param($Metadata)
    Assert-P4kxMsiTransportActions $Metadata
    Assert-P4kxMsiTransportSearch $Metadata
    if (@($Metadata.environment).Count -ne 0) { throw 'MSI must not author a PATH or dummy Environment row.' }
    if (@($Metadata.binary_names | Where-Object { $_ -ceq 'P4kxPathHelper' }).Count -ne 1 -or
        'p4kx-msi-path.exe' -cin $Metadata.files) { throw 'MSI helper must be embedded once and independent of installed files.' }
    foreach ($name in @('p4kx-msi-helper.json','p4kx-msi-helper.cdx.json','p4kx-msi-helper-NOTICE.txt','THIRD-PARTY-NOTICES.md')) {
        if (@($Metadata.files | Where-Object { $_ -ceq $name }).Count -ne 1) { throw 'MSI helper provenance or notices are missing or duplicated.' }
    }
    $sequence = @{}
    foreach ($row in $Metadata.execute) {
        if ($sequence.ContainsKey($row[0])) { throw 'MSI contains a duplicate execute action.' }
        $sequence[$row[0]] = [int]$row[2]
    }
    $active = 'NOT UPGRADINGPRODUCTCODE AND ($Application = 2 OR $Application = 3)'
    $search = @($Metadata.execute | Where-Object { $_[0] -ceq 'AppSearch' })
    if ($search.Count -ne 1 -or [int]$search[0][2] -ne 1440 -or $search[0][1] -cne $active) { throw 'MSI invocation AppSearch is not execute-only after Reserve.' }
    foreach ($name in @('FindRelatedProducts','InstallValidate','InstallInitialize','RemoveExistingProducts','ProcessComponents','InstallFinalize')) {
        if (-not $sequence.ContainsKey($name)) { throw "Missing MSI ordering boundary: $name" }
    }
    if ($sequence.FindRelatedProducts -ne 25 -or $sequence.InstallValidate -ge 1410 -or
        1470 -ge $sequence.InstallInitialize -or $sequence.InstallInitialize -ge $sequence.RemoveExistingProducts -or
        $sequence.RemoveExistingProducts -ge 1530 -or 1530 -ge $sequence.ProcessComponents -or
        6430 -ge $sequence.InstallFinalize -or ($sequence.ContainsKey('InstallExecute') -and 6430 -ge $sequence.InstallExecute)) {
        throw 'MSI helper reservation, transactional removal, rollback or deferred phase order differs.'
    }
    if ($sequence.ContainsKey('MigrateFeatureStates') -and $sequence.FindRelatedProducts -ge $sequence.MigrateFeatureStates) {
        throw 'Fresh related-product detection must precede feature-state migration.'
    }
}
