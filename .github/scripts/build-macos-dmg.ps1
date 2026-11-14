param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$TaskDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$SigningIdentity,
    [string]$NotaryProfile,
    [string]$NotaryKeychain,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'macho-image.ps1')
. (Join-Path $PSScriptRoot 'macos-bundle.ps1')
. (Join-Path $PSScriptRoot 'macos-resources.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'macos') { throw 'The DMG builder requires a native macOS target.' }
$plan = Get-P4kxMacSigningPlan -DryRun:$DryRun -SigningIdentity $SigningIdentity -NotaryProfile $NotaryProfile
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$work = [IO.Path]::GetFullPath($TaskDirectory)
$out = [IO.Path]::GetFullPath($OutputDirectory)
$null = New-Item -ItemType Directory -Path $work
$null = New-Item -ItemType Directory -Path $out -Force
$imageRoot = Join-Path $work 'image'
$bundle = Join-Path $imageRoot 'P4KX.app'
$dmg = Join-Path $out (Get-P4kxAssetName -Target $Target -Version $Version -Package dmg)
if (Test-Path -LiteralPath $dmg) { throw 'Refusing to overwrite an existing DMG.' }
$icon = New-P4kxMacIcon -Repository $repo -Directory $work
$paths = New-P4kxMacBundle -Destination $bundle -BinaryDirectory $BinaryDirectory -Icon $icon -Repository $repo -Version $Version -Platform $platform
Invoke-P4kxMacTool /usr/bin/plutil @('-lint', $paths.plist) | Out-Host

foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $binary = Join-Path $paths.binaries $name
    $null = Assert-P4kxMachOImage -Path $binary -Platform $platform -Kind Executable
    Assert-P4kxMacSystemLibraries -Binary $binary
    Invoke-P4kxMacTool /bin/chmod @('755', $binary)
    if ($name -cne 'p4kx-gui') {
        Set-P4kxMacSignature -Path $binary -Plan $plan -Identifier "io.github.p4kx-org.p4kx.$name" -ExecutableCode
    }
}
Set-P4kxMacSignature -Path $bundle -Plan $plan -ExecutableCode
Assert-P4kxMacSignature -Path $bundle -Plan $plan -Bundle -ExecutableCode
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    Assert-P4kxMacSignature -Path (Join-Path $paths.binaries $name) -Plan $plan -ExecutableCode
}
$null = New-Item -ItemType SymbolicLink -Path (Join-Path $imageRoot 'Applications') -Target '/Applications'

Invoke-P4kxMacTool /usr/bin/hdiutil @('create', '-srcfolder', $imageRoot, '-volname', 'P4KX', '-fs', 'APFS', '-format', 'UDZO', $dmg) | Out-Host
Set-P4kxMacSignature -Path $dmg -Plan $plan -Identifier 'io.github.p4kx-org.p4kx.dmg'
$notarization = Complete-P4kxMacNotarization -Dmg $dmg -Plan $plan -Keychain $NotaryKeychain
Assert-P4kxMacSignature -Path $dmg -Plan $plan
Invoke-P4kxMacTool /usr/bin/hdiutil @('verify', $dmg) | Out-Host
[ordered]@{
    package = 'dmg'
    mode = $plan.mode
    reason = $plan.reason
    signature = 'verified'
    notarization = $notarization
    archive_sha256 = (Get-FileHash -LiteralPath $dmg -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $work 'signing.json') -Encoding utf8NoBOM
$dmg
