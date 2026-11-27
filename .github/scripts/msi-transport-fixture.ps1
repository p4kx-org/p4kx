$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Projects authored transport XML into a synthetic table fixture without WiX or COM execution.
#>
function Get-P4kxMsiTransportFixture {
    param([string]$Include, [string]$Main)
    $xml = [xml](Get-Content -LiteralPath $Include -Raw)
    $mainXml = [xml](Get-Content -LiteralPath $Main -Raw)
    $package = '{11111111-2222-4333-8444-555555555555}'
    $properties = @{}
    $hidden = @()
    foreach ($property in $xml.Include.Property) {
        $properties[$property.Id] = [string]$property.Value
        if ($property.Hidden -ceq 'yes') { $hidden += [string]$property.Id }
    }
    $properties.P4kxPathPackageCode = $package
    $properties.MsiHiddenProperties = $hidden -join ';'
    $actions = @(foreach ($action in $xml.Include.CustomAction) {
        $type = if ($action.HasAttribute('ExeCommand')) {
            $base = switch ($action.Execute) { 'immediate' {2};'deferred' {1026};'rollback' {1282};'commit' {1538};default {throw 'Unknown synthetic execution mode.'} }
            if ($action.HideTarget -ceq 'yes') { $base = $base -bor 8192 }
            if ($action.Impersonate -ceq 'no') { $base = $base -bor 2048 }
            if ($action.Return -ceq 'ignore') { $base = $base -bor 64 }
            $base
        } elseif ($action.HasAttribute('Error')) {19} else {51}
        $source = if ($type -eq 51) { [string]$action.Property } else { [string]$action.BinaryRef }
        $target = if ($type -eq 51) { [string]$action.Value } elseif ($type -eq 19) { [string]$action.Error } else { [string]$action.ExeCommand }
        Write-Output -NoEnumerate @([string]$action.Id,[string]$type,$source,$target)
    })
    $execute = @(foreach ($action in $xml.Include.InstallExecuteSequence.ChildNodes) {
        if ($action -isnot [Xml.XmlElement]) { continue }
        $name = if ($action.LocalName -ceq 'Custom') { [string]$action.Action } else { $action.LocalName }
        $sequence = if ($action.HasAttribute('OnExit')) { if ($action.OnExit -ceq 'error') {-3} else {-2} } else { [int]$action.Sequence }
        Write-Output -NoEnumerate @($name,[string]$action.Condition,[string]$sequence)
    })
    foreach ($pair in @(@('CostInitialize',800),@('FileCost',900),@('CostFinalize',1000),@('InstallValidate',1400),@('InstallInitialize',1500),@('RemoveExistingProducts',1501),@('ProcessComponents',1600),@('MigrateFeatureStates',1200),@('InstallFinalize',6600))) {
        $execute += ,@($pair[0],'',[string]$pair[1])
    }
    $ui = @(foreach ($action in $xml.Include.InstallUISequence.ChildNodes) {
        if ($action -is [Xml.XmlElement] -and $action.Suppress -cne 'yes') { Write-Output -NoEnumerate @($action.LocalName,'','50') }
    })
    $manager = [Xml.XmlNamespaceManager]::new($mainXml.NameTable)
    $manager.AddNamespace('w','http://wixtoolset.org/schemas/v4/wxs')
    $files = @($mainXml.SelectNodes('//w:File',$manager) | ForEach-Object { [string]$_.Name })
    return [pscustomobject]@{
        package=$package;properties=$properties;execute=$execute;ui=$ui;custom_actions=$actions
        environment=@($mainXml.SelectNodes('//w:Environment',$manager));files=$files
        binary_names=@($xml.Include.Binary | ForEach-Object { [string]$_.Id })
        app_search=@(,@('P4KX_PATH_BINDING','P4kxInvocationBinding'))
        reg_locator=@(,@('P4kxInvocationBinding','1','Software\P4KX-Installer\PathOwnership\Invocation','Binding','18'))
        upgrade_properties=@('WIX_DOWNGRADE_DETECTED','WIX_UPGRADE_DETECTED')
    }
}
