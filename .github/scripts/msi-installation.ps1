$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-database.ps1')

function Invoke-P4kxMsiexec {
    param([string[]]$Arguments, [string]$Log, [int]$ExpectedCode = 0)
    $start = [Diagnostics.ProcessStartInfo]::new('msiexec.exe')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    foreach ($argument in $Arguments + @('/qn', '/norestart', '/l*v', $Log)) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($start)
    try {
        if (-not $process.WaitForExit(300000)) { $process.Kill($true); $process.WaitForExit(); throw "MSI operation timed out: $Log" }
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
    return ,@(for ($index = 0; $index -lt $count; $index++) { Get-P4kxComProperty $related 'Item' @($index) })
}

function Assert-P4kxInstalledProduct {
    param($Metadata, [string]$Folder, [string]$Shortcut)
    $products = Get-P4kxInstalledProducts $Metadata.upgrade
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
    param([string]$UpgradeCode, [string]$Folder, [string]$Shortcut, [string]$OriginalPath)
    if ((Get-P4kxInstalledProducts $UpgradeCode).Count -ne 0 -or
        (Test-Path -LiteralPath $Folder) -or (Test-Path -LiteralPath $Shortcut) -or
        (Test-Path 'HKCU:\Software\P4KX')) { throw 'MSI left behind installation ownership.' }
    if ([Environment]::GetEnvironmentVariable('Path', 'User') -cne $OriginalPath) { throw 'MSI did not restore the original user PATH.' }
}
