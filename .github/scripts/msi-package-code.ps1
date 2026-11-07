$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Binds the private helper property to the actual MSI SummaryInformation package code.
#>
function Set-P4kxMsiPrivatePackageCode {
    param($Database, [Parameter(Mandatory)][string]$PackageCode)
    if ($PackageCode -cnotmatch '^\{[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\}$' -or
        [guid]$PackageCode -eq [guid]::Empty) { throw 'Invalid actual MSI PackageCode.' }
    $rows = @(Read-P4kxMsiRows $Database 'SELECT `Value` FROM `Property` WHERE `Property` = ''P4kxPathPackageCode''' 1)
    if ($rows.Count -ne 1) { throw 'The authored private PackageCode property is missing or ambiguous.' }
    $query = 'UPDATE `Property` SET `Value` = ''{0}'' WHERE `Property` = ''P4kxPathPackageCode''' -f $PackageCode
    $view = Invoke-P4kxComMethod $Database 'OpenView' @($query)
    try { [void](Invoke-P4kxComMethod $view 'Execute' @()) }
    finally { [void](Invoke-P4kxComMethod $view 'Close' @()); Close-P4kxComObject $view }
}

<#
.SYNOPSIS
Stamps a newly authored MSI before validation, signing and package hashing.
#>
function Complete-P4kxMsiPackageCode {
    param([Parameter(Mandatory)][string]$Path)
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $null
    $summary = $null
    try {
        $database = Invoke-P4kxComMethod $installer 'OpenDatabase' @([IO.Path]::GetFullPath($Path), 1)
        $summary = Get-P4kxComProperty $database 'SummaryInformation' @(0)
        $package = Get-P4kxComProperty $summary 'Property' @(9)
        Set-P4kxMsiPrivatePackageCode $database $package
        [void](Invoke-P4kxComMethod $database 'Commit' @())
    }
    finally { Close-P4kxComObject $summary; Close-P4kxComObject $database; Close-P4kxComObject $installer }
}
