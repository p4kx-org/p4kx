$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-database.ps1')
. (Join-Path $PSScriptRoot 'msi-invocation.ps1')
. (Join-Path $PSScriptRoot 'msi-path.ps1')

<#
.SYNOPSIS
Starts the configured MSI process.
#>
function Start-P4kxMsiProcess {
    param($StartInfo)
    return [Diagnostics.Process]::Start($StartInfo)
}

function Invoke-P4kxMsiexec {
    param([string[]]$Arguments, [string]$Log, [int]$ExpectedCode = 0, $RawFixture = $null)
    if ($null -ne $RawFixture) { Assert-P4kxMsiMutationReady $RawFixture }
    $start = New-P4kxMsiexecStartInfo -Arguments $Arguments -Log $Log
    $pathBefore = Get-P4kxUserPathState
    $rawBefore = if ($null -ne $RawFixture) { Get-P4kxRawMsiImage $RawFixture } else { $null }
    $process = $null; $completed = $false; $failure = $null
    try {
        $process = Start-P4kxMsiProcess $start
        if (-not $process.WaitForExit(300000)) { if ($null -ne $RawFixture) { $RawFixture.interrupted=$true }; $process.Kill($false); if (-not $process.WaitForExit(5000)) { throw 'MSI process did not stop after its deadline.' }; throw "MSI operation timed out: $Log" }
        $completed = $true
        Write-P4kxPathTransition $pathBefore (Get-P4kxUserPathState) "$Log.path.json"
        $rawAfter = if ($null -ne $RawFixture) { Get-P4kxRawMsiImage $RawFixture } else { $null }
        if ($process.ExitCode -ne $ExpectedCode) {
            if (Test-Path -LiteralPath $Log) { Get-Content -LiteralPath $Log -Tail 50 | Write-Host }
            throw "MSI exited $($process.ExitCode), expected $ExpectedCode ($Log)"
        }
    } catch {
        if (-not $completed -and $null -ne $RawFixture) { $RawFixture.interrupted=$true }
        $failure = $_.Exception
    }
    if ($null -ne $process) {
        try { $process.Dispose() } catch {
            if ($null -ne $failure) { throw [AggregateException]::new('MSI invocation and disposal failed.', [Exception[]]@($failure,$_.Exception)) }
            throw
        }
    }
    if ($null -ne $failure) { throw $failure }
    if ($null -ne $RawFixture) { return @{raw_before=$rawBefore;raw_after=$rawAfter;exit_code=$ExpectedCode} }
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

function Assert-P4kxLegacyOwnershipAbsent {
    param($Context)
    if ($Context.legacy_keys.Count -eq 0) { return }
    $folder = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4K-Extractor'
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'P4K-Extractor.lnk'
    if ((Test-Path -LiteralPath $folder) -or (Test-Path -LiteralPath $shortcut)) { throw 'Historical installation files remain.' }
    foreach ($key in $Context.legacy_keys) { if (Test-Path -LiteralPath "HKCU:\$key") { throw 'Historical installer identity remains.' } }
}
