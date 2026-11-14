param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Dmg,
    [Parameter(Mandatory)][string]$BuildMetadata,
    [Parameter(Mandatory)][string]$SigningReceipt,
    [Parameter(Mandatory)][string]$MetalEvidence,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$TaskDirectory,
    [Parameter(Mandatory)][string]$OutputPath,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'macos-dmg-mount.ps1')
. (Join-Path $PSScriptRoot 'macos-package-evidence.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'macos' -or
    (Split-Path -Leaf $Dmg) -cne (Get-P4kxAssetName -Target $Target -Version $Version -Package dmg)) {
    throw 'The macOS consumer requires the selected canonical native DMG.'
}
$Dmg = (Resolve-Path -LiteralPath $Dmg).Path
& (Join-Path $PSScriptRoot 'check-native-platform.ps1') -Target $Target -ArchivePath $Dmg -MetadataPath $BuildMetadata -ValidateArchive
$build = Get-Content -LiteralPath $BuildMetadata -Raw | ConvertFrom-Json
if ($build.version -cne $Version) { throw 'The macOS package version differs from its build.' }
$signing = Read-P4kxMacSigningReceipt -Path $SigningReceipt -Build $build -DryRun:$DryRun
$metal = Get-Content -LiteralPath $MetalEvidence -Raw | ConvertFrom-Json
Assert-P4kxMacMetalEvidence -Evidence $metal -Build $build

$plan = @{ mode = $signing.mode }
Invoke-P4kxMacTool /usr/bin/hdiutil @('verify', $Dmg) | Out-Host
Assert-P4kxMacSignature -Path $Dmg -Plan $plan
if ($plan.mode -ceq 'developer-id') {
    Invoke-P4kxMacTool /usr/bin/xcrun @('stapler', 'validate', $Dmg) | Out-Host
}

$task = [IO.Path]::GetFullPath($TaskDirectory)
$null = New-Item -ItemType Directory -Path $task
$bundle = Copy-P4kxMacMountedBundle -Dmg $Dmg -TaskDirectory $task
$paths = Assert-P4kxMacRelocatedBundle -Bundle $bundle -Platform $platform -Version $Version -Plan $plan
$launchers = @{ }
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $launchers[$name] = @{ executable = (Join-Path $paths.binaries $name); prefix = @() }
}
$alias = Join-Path $task 'user p4kx'
$null = New-Item -ItemType SymbolicLink -Path $alias -Target $launchers['p4kx'].executable
$launchers['p4kx'].executable = $alias
$checks = Join-Path $task 'smoke'
$arguments = @{
    Target = $Target
    Package = 'dmg'
    ExpectedVersion = $Version
    BinaryDirectory = $paths.binaries
    MacBundle = $bundle
    ExpectedBinaries = $build.binaries
    Launchers = $launchers
    FixtureDirectory = $FixtureDirectory
    OutputDirectory = $checks
}
& (Join-Path $PSScriptRoot 'check-package-runtime.ps1') @arguments

$smoke = Get-Content -LiteralPath (Join-Path $checks 'runtime.json') -Raw | ConvertFrom-Json -AsHashtable
$smoke = Complete-P4kxMacHostedPackageSmoke -Smoke $smoke -Build $build -Signing $signing -Metal $metal
$native = Join-Path $task 'native-runtime.json'
& (Join-Path $PSScriptRoot 'record-native-runtime.ps1') -Target $Target -MetadataPath $BuildMetadata -OutputPath $native -SmokeOutcomes $smoke.runtime
$runtime = Get-Content -LiteralPath $native -Raw | ConvertFrom-Json -AsHashtable
$runtime.package_smokes = @($smoke)
$runtime | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
