param([Parameter(Mandatory)][string]$Repository)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-transport-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-transport-fixture.ps1')
$include = Join-Path $Repository 'crates/p4kx-gui/wix/path-transport.wxi'
$main = Join-Path $Repository 'crates/p4kx-gui/wix/main.wxs'
$baseline = Get-P4kxMsiTransportFixture $include $main
Assert-P4kxMsiTransport $baseline
$json = $baseline | ConvertTo-Json -Depth 12
$mutations = @(
    {param($m) $m.package='{99999999-2222-4333-8444-555555555555}'},
    {param($m) $m.properties.P4kxPathPackageCode='UNSTAMPED'},
    {param($m) $value=$m.properties.P4kxPathPackageCode; $m.properties.PSObject.Properties.Remove('P4kxPathPackageCode'); $m.properties | Add-Member -NotePropertyName P4KXPATHPACKAGECODE -NotePropertyValue $value},
    {param($m) $m.properties.P4KX_PATH_BINDING='preseeded'},
    {param($m) $m.properties.P4kxPathCapturedBinding='preseeded'},
    {param($m) $m.properties.MsiHiddenProperties='P4KX_PATH_BINDING'},
    {param($m) $m.app_search+=,@('P4KX_PATH_BINDING','Other')},
    {param($m) $m.reg_locator[0][4]='2'},
    {param($m) $m.reg_locator[0][1]='2'},
    {param($m) $m.reg_locator[0][2]='Software\Unrelated'},
    {param($m) $m.reg_locator[0][3]='StateV1'},
    {param($m) $m.ui+=,@('AppSearch','','50')},
    {param($m) $m.ui+=,@('FindRelatedProducts','','25')},
    {param($m) $m.ui+=,@('P4kxPathReserveEnsure','','1420')},
    {param($m) $m.binary_names=@()},
    {param($m) $m.binary_names+= 'P4kxPathHelper'},
    {param($m) $m.files+='p4kx-msi-path.exe'},
    {param($m) $m.files=@($m.files | Where-Object { $_ -cne 'p4kx-msi-helper.cdx.json' })},
    {param($m) $m.files=@($m.files | Where-Object { $_ -cne 'p4kx-msi-helper.json' })},
    {param($m) $m.environment=@(,@('=-P4KX_DUMMY','1','Application'))},
    {param($m) $m.upgrade_properties+='UNRESET_PREDECESSOR'},
    {param($m) $m.custom_actions+=,@('RewriteCaptured','51','P4kxPathCapturedBinding','foreign')},
    {param($m) $m.custom_actions+=,@('ReplacePackage','51','P4kxPathPackageCode','foreign')},
    {param($m) $m.custom_actions+=,@('ForeignDll','1','OtherBinary','Entry')},
    {param($m) $m.custom_actions+=,@('AutoRecover','8194','P4kxPathHelper','recover-reserved')},
    {param($m) ($m.execute | Where-Object { $_[0] -ceq 'RemoveExistingProducts' })[2]='1540'},
    {param($m) ($m.execute | Where-Object { $_[0] -ceq 'InstallInitialize' })[2]='1465'},
    {param($m) ($m.execute | Where-Object { $_[0] -ceq 'FindRelatedProducts' })[2]='1300'},
    {param($m) ($m.execute | Where-Object { $_[0] -ceq 'AppSearch' })[2]='1415'},
    {param($m) $m.execute+=,@('AppSearch','','1440')},
    {param($m) $m.execute+=,@('InstallExecute','','6400')}
)
$count = 0
foreach ($mutation in $mutations) {
    $changed = $json | ConvertFrom-Json
    & $mutation $changed
    $rejected = $false
    try { Assert-P4kxMsiTransport $changed } catch { $rejected = $true }
    if (-not $rejected) { throw "Transport table mutation $count was accepted." }
    $count++
}
foreach ($action in Get-P4kxMsiTransportActions) {
    foreach ($field in @('definition','condition','sequence')) {
        $changed = $json | ConvertFrom-Json
        if ($field -ceq 'definition') {
            $row = @($changed.custom_actions | Where-Object { $_[0] -ceq $action.name })[0]
            $row[3] = $row[3] + ' altered'
        } else {
            $row = @($changed.execute | Where-Object { $_[0] -ceq $action.name })[0]
            if ($field -ceq 'condition') { $row[1] = '1' } else { $row[2] = '100' }
        }
        $rejected = $false
        try { Assert-P4kxMsiTransport $changed } catch { $rejected = $true }
        if (-not $rejected) { throw "Transport $field mutation was accepted: $($action.name)" }
        $count++
    }
}
foreach ($name in @('P4kxPathPrepare','P4kxPathApply','P4kxPathCommit','P4kxPathRollback')) {
    foreach ($bits in @(64,128,2048)) {
        $changed = $json | ConvertFrom-Json
        $row = @($changed.custom_actions | Where-Object { $_[0] -ceq $name })[0]
        $row[1] = [string]([int]$row[1] -bor $bits)
        $rejected = $false
        try { Assert-P4kxMsiTransport $changed } catch { $rejected = $true }
        if (-not $rejected) { throw 'Unchecked, asynchronous or elevated helper action was accepted.' }
        $count++
    }
}
"PASS: $count source/table mutations; no WiX, COM, MSI, Registry or helper execution."
