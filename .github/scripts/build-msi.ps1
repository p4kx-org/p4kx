<#
.SYNOPSIS
Builds and validates the P4KX MSI with WiX Toolset 7 from release binaries.

.PARAMETER Wix
The wix.exe of WiX Toolset 7.

.PARAMETER Target
The Windows Rust target from the shared platform contract.

.PARAMETER Version
The installer version, three numbers such as 0.1.0.

.PARAMETER Output
The MSI to write.

.PARAMETER BinaryFolder
The prepared native MSI payload, including its distribution identity and runtime inventory.

.PARAMETER UserPathPart
The Environment operation; first builds an explicitly bound diagnostic probe.
#>
param(
    [Parameter(Mandatory)] [string] $Wix,
    [Parameter(Mandatory)] [string] $Target,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Output,
    [Parameter(Mandatory)] [string] $BinaryFolder,
    [ValidateSet('last', 'first')] [string] $UserPathPart = 'last'
)

$ErrorActionPreference = 'Stop'
$UserPathPart = $UserPathPart.ToLowerInvariant()
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
$platform = Get-P4kxPlatform -Target $Target
if ($platform.os -cne 'windows') { throw 'An MSI requires a Windows target.' }
$identity = Get-Content (Join-Path $BinaryFolder 'p4kx-distribution.json') -Raw | ConvertFrom-Json
if ($identity.target -cne $Target -or $identity.package -cne 'msi') { throw 'MSI payload identity mismatch.' }
Assert-P4kxWindowsRuntimeInventory -Directory $BinaryFolder -Platform $platform
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    if ((Get-ImageMachine -Path (Join-Path $BinaryFolder "$name.exe") -Os windows) -ne $platform.pe_machine) { throw "Wrong CPU in MSI payload: $name" }
}
$guids = if ($platform.arch -eq 'amd64') {
    @('5DA0A84A-B1EA-4B11-A4C1-7DB8F1449E7D', '5EB01C4B-DB2E-44BF-BFCA-71F808DB8600', 'D2CDD1BC-E0D6-4DA5-BBC7-C1CE0DC830AA')
} else {
    @('B7C3DE88-0912-45E2-9DA2-F88CE52CB4CE', '409FB402-C7B5-4C73-A50D-9BC610D05622', '137D0BCF-FCC5-4A23-8093-A38A0BF6CD4D')
}
$repository = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$source = Join-Path $repository 'crates\p4kx-gui\wix\main.wxs'
$runtimeSource = [IO.Path]::ChangeExtension([IO.Path]::GetFullPath($Output), '.runtime.wxs')
$runtimeFiles = @(Get-ChildItem -LiteralPath $BinaryFolder -Filter '*.dll' | Sort-Object Name)
$files = @(for ($index = 0; $index -lt $runtimeFiles.Count; $index++) {
    $path = [Security.SecurityElement]::Escape($runtimeFiles[$index].FullName)
    "<File Id=`"RuntimeFile$index`" Source=`"$path`" />"
}) -join "`n"
@"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Fragment><DirectoryRef Id="APPLICATIONFOLDER">
    <Component Id="RuntimeLibraries" Guid="$($guids[2])" Bitness="always64">
      $files
      <RegistryValue Root="HKCU" Key="Software\P4KX" Name="RuntimeLibraries" Type="integer" Value="1" KeyPath="yes" />
    </Component>
  </DirectoryRef></Fragment>
</Wix>
"@ | Set-Content -LiteralPath $runtimeSource -Encoding utf8NoBOM

& $Wix build $source $runtimeSource -acceptEula wix7 -arch $platform.msi_arch `
    -d "Version=$Version" `
    -d "ApplicationGuid=$($guids[0])" `
    -d "ShortcutGuid=$($guids[1])" `
    -d "UserPathPart=$UserPathPart" `
    -d "BinaryFolder=$(Resolve-Path $BinaryFolder)" `
    -d "RepositoryFolder=$repository" `
    -o $Output
if ($LASTEXITCODE -ne 0) { throw "wix build failed with exit code $LASTEXITCODE" }

& $Wix msi validate -acceptEula wix7 $Output
if ($LASTEXITCODE -ne 0) { throw "wix msi validate failed with exit code $LASTEXITCODE" }

if ($UserPathPart -ceq 'first') {
    [ordered]@{
        schema_version=1; role='prepend_hypothesis'; target=$Target; version=$Version
        path_part=$UserPathPart; revision=$env:GITHUB_SHA
        msi_sha256=(Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath "$Output.probe.json" -Encoding utf8NoBOM
}
