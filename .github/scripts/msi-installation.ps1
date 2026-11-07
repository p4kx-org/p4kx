$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-database.ps1')
. (Join-Path $PSScriptRoot 'msi-invocation.ps1')
. (Join-Path $PSScriptRoot 'msi-path.ps1')

function Invoke-P4kxMsiexec {
    param([string[]]$Arguments, [string]$Log, [int]$ExpectedCode = 0)
    $start = New-P4kxMsiexecStartInfo -Arguments $Arguments -Log $Log
    $pathBefore = Get-P4kxUserPathState
    $process = [Diagnostics.Process]::Start($start)
    try {
        if (-not $process.WaitForExit(300000)) { $process.Kill($true); $process.WaitForExit(); throw "MSI operation timed out: $Log" }
        Write-P4kxPathTransition $pathBefore (Get-P4kxUserPathState) "$Log.path.json"
        if ($process.ExitCode -ne $ExpectedCode) {
            if (Test-Path -LiteralPath $Log) { Get-Content -LiteralPath $Log -Tail 50 | Write-Host }
            throw "MSI exited $($process.ExitCode), expected $ExpectedCode ($Log)"
        }
    }
    finally { $process.Dispose() }
}

function Get-P4kxInstalledProducts {
    param([string]$UpgradeCode)
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $related = Get-P4kxComProperty $installer 'RelatedProducts' @("{$($UpgradeCode.Trim('{}'))}")
    $count = Get-P4kxComProperty $related 'Count' @()
    return @(for ($index = 0; $index -lt $count; $index++) { Get-P4kxComProperty $related 'Item' @($index) })
}

function Assert-P4kxInstalledProduct {
    param($Metadata, [string]$Folder, [string]$Shortcut)
    $products = @(Get-P4kxInstalledProducts $Metadata.upgrade)
    if ($products.Count -ne 1 -or $products[0] -ine $Metadata.product) { throw 'MSI registration is not the expected single installation.' }
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $version = Get-P4kxComProperty $installer 'ProductInfo' @($products[0], 'VersionString')
    if ($version -cne $Metadata.version) { throw 'Installed MSI version differs.' }
    if (-not (Test-Path -LiteralPath $Shortcut)) { throw 'Missing Start menu shortcut.' }
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ })
    if (@($entries | Where-Object { $_.TrimEnd('\') -ieq $Folder }).Count -ne 1) { throw 'Installed user PATH entry is not unique.' }
    if (-not (Test-Path 'HKCU:\Software\P4KX')) { throw 'Missing installer HKCU identity.' }
}

function Assert-P4kxUninstalled {
    param([string]$UpgradeCode, [string]$Folder, [string]$Shortcut, $OriginalPath)
    Assert-P4kxInstallationOwnershipAbsent $UpgradeCode $Folder $Shortcut
    Assert-P4kxPathRestored $OriginalPath (Get-P4kxUserPathState)
}

function Assert-P4kxInstallationOwnershipAbsent {
    param([string]$UpgradeCode, [string]$Folder, [string]$Shortcut)
    if (@(Get-P4kxInstalledProducts $UpgradeCode).Count -ne 0 -or
        (Test-Path -LiteralPath $Folder) -or (Test-Path -LiteralPath $Shortcut) -or
        (Test-Path 'HKCU:\Software\P4KX')) { throw 'MSI left behind installation ownership.' }
}
