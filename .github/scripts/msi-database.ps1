$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-package-code.ps1')
. (Join-Path $PSScriptRoot 'msi-transport-contract.ps1')

function Close-P4kxComObject {
    param($Object)
    if ($null -ne $Object) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($Object) }
}

function Get-P4kxComProperty {
    param($Object, [string]$Name, [object[]]$Arguments)
    return ,$Object.GetType().InvokeMember($Name, [Reflection.BindingFlags]::GetProperty, $null, $Object, $Arguments)
}

function Invoke-P4kxComMethod {
    param($Object, [string]$Name, [object[]]$Arguments)
    return ,$Object.GetType().InvokeMember($Name, [Reflection.BindingFlags]::InvokeMethod, $null, $Object, $Arguments)
}

function Read-P4kxMsiRows {
    param($Database, [string]$Query, [int]$Columns)
    $view = Invoke-P4kxComMethod $Database 'OpenView' @($Query)
    try {
        [void](Invoke-P4kxComMethod $view 'Execute' @())
        while ($record = Invoke-P4kxComMethod $view 'Fetch' @()) {
            try { $row = @(for ($index = 1; $index -le $Columns; $index++) { Get-P4kxComProperty $record 'StringData' @($index) }) }
            finally { Close-P4kxComObject $record }
            Write-Output -NoEnumerate $row
        }
    }
    finally { [void](Invoke-P4kxComMethod $view 'Close' @()); Close-P4kxComObject $view }
}

function Get-P4kxMsiMetadata {
    param([string]$Path)
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = Invoke-P4kxComMethod $installer 'OpenDatabase' @((Resolve-Path -LiteralPath $Path).Path, 0)
    $summary = $null
    try {
        $properties = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
        foreach ($row in Read-P4kxMsiRows $database 'SELECT `Property`, `Value` FROM `Property`' 2) { $properties.Add($row[0],$row[1]) }
        $tables = @(Read-P4kxMsiRows $database 'SELECT `Name` FROM `_Tables`' 1 | ForEach-Object { $_[0] })
        $execute = @(Read-P4kxMsiRows $database 'SELECT `Action`, `Condition`, `Sequence` FROM `InstallExecuteSequence`' 3)
        $summary = Get-P4kxComProperty $database 'SummaryInformation' @(0)
        return [pscustomobject]@{
            product=$properties.ProductCode; upgrade=$properties.UpgradeCode; version=$properties.ProductVersion
            template=(Get-P4kxComProperty $summary 'Property' @(7)); package=(Get-P4kxComProperty $summary 'Property' @(9))
            components=@(Read-P4kxMsiRows $database 'SELECT `ComponentId`, `Attributes` FROM `Component`' 2)
            files=@(Read-P4kxMsiRows $database 'SELECT `FileName` FROM `File`' 1 | ForEach-Object { $_[0] -replace '^.*\|', '' })
            properties=$properties
            sequence=@(foreach ($row in $execute) { Write-Output -NoEnumerate @($row[0], $row[2]) })
            execute=$execute
            ui=@(if ('InstallUISequence' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Action`, `Condition`, `Sequence` FROM `InstallUISequence`' 3 })
            custom_actions=@(if ('CustomAction' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Action`, `Type`, `Source`, `Target` FROM `CustomAction`' 4 })
            app_search=@(if ('AppSearch' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Property`, `Signature_` FROM `AppSearch`' 2 })
            reg_locator=@(if ('RegLocator' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Signature_`, `Root`, `Key`, `Name`, `Type` FROM `RegLocator`' 5 })
            upgrade_properties=@(Read-P4kxMsiRows $database 'SELECT `ActionProperty` FROM `Upgrade`' 1 | ForEach-Object { $_[0] } | Sort-Object -Unique)
            binary_names=@(if ('Binary' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Name` FROM `Binary`' 1 | ForEach-Object { $_[0] } })
            directories=@(Read-P4kxMsiRows $database 'SELECT `Directory` FROM `Directory`' 1 | ForEach-Object { $_[0] })
            registry=@(Read-P4kxMsiRows $database 'SELECT `Root`, `Key`, `Name` FROM `Registry`' 3)
            environment=@(if ('Environment' -in $tables) { Read-P4kxMsiRows $database 'SELECT `Name`, `Value`, `Component_` FROM `Environment`' 3 })
        }
    }
    finally { Close-P4kxComObject $summary; Close-P4kxComObject $database; Close-P4kxComObject $installer }
}

function Assert-P4kxMsiMetadata {
    param($Metadata, $Platform, [string]$Version, [string]$UpgradeCode)
    $cpu = if ($Platform.msi_arch -eq 'arm64') { 'Arm64' } else { $Platform.msi_arch }
    if ($Metadata.template -cne "$cpu;1033" -or $Metadata.version -cne $Version -or
        $Metadata.upgrade.Trim('{}') -ine $UpgradeCode.Trim('{}') -or
        $Metadata.product -notmatch '^\{[0-9A-F-]{36}\}$') { throw 'MSI CPU/product/version/upgrade identity mismatch.' }
    if (@($Metadata.components | Where-Object { ([int]$_[1] -band 256) -eq 0 }).Count -gt 0) { throw 'MSI contains a non-64-bit component.' }
    foreach ($file in @('p4kx.exe', 'p4kx-gui.exe', 'p4kx-mcp.exe', 'p4kx-distribution.json',
        'THIRD-PARTY-NOTICES.md', 'windows-runtime.json', 'MICROSOFT-REDISTRIBUTABLE-NOTICE.txt')) {
        if (@($Metadata.files | Where-Object { $_ -ceq $file }).Count -ne 1) { throw "Missing or duplicate MSI file: $file" }
    }
    if (@($Metadata.files | Where-Object { $_ -match '^(settings\.toml|.*key.*)$' }).Count -ne 0 -or
        'AppDataFolder' -in $Metadata.directories) { throw 'MSI must not own profile/key files.' }
    $sequence = @{}
    foreach ($action in $Metadata.sequence) { $sequence[$action[0]] = [int]$action[1] }
    if ($sequence.RemoveExistingProducts -le $sequence.InstallInitialize -or
        $sequence.RemoveExistingProducts -ge $sequence.ProcessComponents) { throw 'MSI upgrade removal is not early and transactional.' }
    Assert-P4kxMsiTransport $Metadata
}

function New-P4kxRollbackMsi {
    param([string]$Source, [string]$Destination, [ValidateSet('after-execute', 'after-commit')][string]$FailureAt = 'after-execute')
    Copy-Item -LiteralPath $Source -Destination $Destination
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = Invoke-P4kxComMethod $installer 'OpenDatabase' @([IO.Path]::GetFullPath($Destination), 1)
    $summary = $null
    try {
        $queries = @()
        $actions = @(Read-P4kxMsiRows $database 'SELECT `Name` FROM `_Tables` WHERE `Name` = ''CustomAction''' 1)
        if ($actions.Count -eq 0) {
            $queries += 'CREATE TABLE `CustomAction` (`Action` CHAR(72) NOT NULL, `Type` SHORT NOT NULL, `Source` CHAR(72), `Target` CHAR(255) LOCALIZABLE, `ExtendedType` LONG PRIMARY KEY `Action`)'
        }
        $failureQuery = if ($FailureAt -ceq 'after-commit') {
            'INSERT INTO `CustomAction` (`Action`, `Type`, `Source`, `Target`) VALUES (''P4kxRollbackProbe'', 9730, ''P4kxPathHelper'', ''invalid-injected-commit-command'')'
        } else {
            'INSERT INTO `CustomAction` (`Action`, `Type`, `Target`) VALUES (''P4kxRollbackProbe'', 19, ''P4KX rollback verification failure'')'
        }
        $queries += @(
            ('UPDATE `Property` SET `Value` = ''{0}'' WHERE `Property` = ''ProductCode''' -f "{$([guid]::NewGuid())}"),
            $failureQuery,
            'INSERT INTO `InstallExecuteSequence` (`Action`, `Condition`, `Sequence`) VALUES (''P4kxRollbackProbe'', ''1'', 6550)'
        )
        $execute = @(Read-P4kxMsiRows $database 'SELECT `Action` FROM `InstallExecuteSequence` WHERE `Action` = ''InstallExecute''' 1)
        if ($execute.Count -eq 0) {
            $queries += 'INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES (''InstallExecute'', 6500)'
        }
        foreach ($query in $queries) {
            $view = Invoke-P4kxComMethod $database 'OpenView' @($query)
            try { [void](Invoke-P4kxComMethod $view 'Execute' @()) }
            finally { [void](Invoke-P4kxComMethod $view 'Close' @()); Close-P4kxComObject $view }
        }
        $summary = Get-P4kxComProperty $database 'SummaryInformation' @(1)
        $package = "{$([guid]::NewGuid())}"
        [void]$summary.GetType().InvokeMember('Property', [Reflection.BindingFlags]::SetProperty, $null, $summary, @(9, $package))
        Set-P4kxMsiPrivatePackageCode $database $package
        [void](Invoke-P4kxComMethod $summary 'Persist' @())
        [void](Invoke-P4kxComMethod $database 'Commit' @())
    }
    finally { Close-P4kxComObject $summary; Close-P4kxComObject $database; Close-P4kxComObject $installer }
}

<#
.SYNOPSIS
Returns the installer-owned per-user registry keys of the supported predecessor MSI.
#>
function Get-P4kxPredecessorRegistryKeys {
    param([Parameter(Mandatory)]$Metadata)
    $keys = @($Metadata.registry | Where-Object { $_[0] -eq '1' -and $_[1] -match '^Software\\KRT\\P4K-Extractor(?:\\|$)' } | ForEach-Object { $_[1] } | Select-Object -Unique)
    if ($keys.Count -eq 0) { throw 'Historical MSI has no expected per-user installer identity.' }
    return $keys
}
