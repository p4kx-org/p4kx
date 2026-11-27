param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Archive,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row
$expected = Get-P4kxAssetName -Target $Target -Version $Version -Package portable-zip
if ([IO.Path]::GetFileName($Archive) -cne $expected) { throw 'Portable ZIP filename does not match its target/version.' }
$rootName = $expected -replace '-portable\.zip$', ''
$zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath $Archive).Path)
try {
    foreach ($entry in $zip.Entries) {
        $path = $entry.FullName.Replace('\', '/')
        if (-not $path.StartsWith("$rootName/", [StringComparison]::Ordinal) -or
            $path -match '(^|/)\.\.?(/|$)|[:\x00]') { throw "Unsafe or foreign portable entry: $path" }
    }
}
finally { $zip.Dispose() }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Portable verification output must be new.' }
$originalPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$registry = Test-Path 'HKCU:\Software\P4KX'
Expand-Archive -LiteralPath $Archive -DestinationPath $OutputDirectory
$payload = Join-Path $OutputDirectory $rootName
& (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Target -BinaryDirectory $payload -FixtureDirectory $FixtureDirectory -OutputDirectory (Join-Path $OutputDirectory 'smoke') -Package portable-zip
if ([Environment]::GetEnvironmentVariable('Path', 'User') -cne $originalPath -or
    (Test-Path 'HKCU:\Software\P4KX') -ne $registry) { throw 'Portable execution changed installer state.' }
