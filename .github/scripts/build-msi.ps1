<#
.SYNOPSIS
Builds and validates the native MSI with its independent embedded PATH helper.
#>
param(
    [Parameter(Mandatory)][string]$Wix,
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$ApplicationVersion,
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory)][string]$BinaryFolder,
    [Parameter(Mandatory)][string]$HelperDirectory,
    [Parameter(Mandatory)][string]$Revision
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
. (Join-Path $PSScriptRoot 'msi-helper-payload.ps1')
. (Join-Path $PSScriptRoot 'msi-build.ps1')
. (Join-Path $PSScriptRoot 'msi-embedded-helper.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'windows') { throw 'An MSI requires a native Windows target.' }
$identity = Get-Content (Join-Path $BinaryFolder 'p4kx-distribution.json') -Raw | ConvertFrom-Json
if ($identity.target -cne $Target -or $identity.package -cne 'msi') { throw 'MSI payload identity mismatch.' }
Assert-P4kxWindowsRuntimeInventory -Directory $BinaryFolder -Platform $platform
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    if ((Get-ImageMachine -Path (Join-Path $BinaryFolder "$name.exe") -Os windows) -ne $platform.pe_machine) { throw "Wrong CPU in MSI payload: $name" }
}
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$cargoLockHash = (Get-FileHash -LiteralPath (Join-Path $repository 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
Assert-P4kxMsiHelperPayload $HelperDirectory $platform $ApplicationVersion $Revision `
    (Join-Path $repository 'THIRD-PARTY-NOTICES.md') $cargoLockHash
Invoke-P4kxWixMsiBuild $Wix $platform $Version $Output $BinaryFolder $HelperDirectory $repository
$embedded = Get-P4kxMsiEmbeddedHelperDigest $Output
$original = (Get-FileHash -LiteralPath (Join-Path $HelperDirectory 'p4kx-msi-path.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($embedded.sha256 -cne $original) { throw 'The actual MSI Binary differs from the analyzed helper executable.' }
