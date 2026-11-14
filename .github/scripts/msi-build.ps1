$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-database.ps1')
. (Join-Path $PSScriptRoot 'msi-transport-contract.ps1')

<#
.SYNOPSIS
Authors the shared WiX package and stamps the real PackageCode before validation.
#>
function Invoke-P4kxWixMsiBuild {
    param([string]$Wix, $Platform, [string]$Version, [string]$Output,
        [string]$BinaryFolder, [string]$HelperDirectory, [string]$RepositoryFolder)
    $guids = if ($platform.arch -eq 'amd64') {
        @('5DA0A84A-B1EA-4B11-A4C1-7DB8F1449E7D', '5EB01C4B-DB2E-44BF-BFCA-71F808DB8600', 'D2CDD1BC-E0D6-4DA5-BBC7-C1CE0DC830AA')
    } else {
        @('B7C3DE88-0912-45E2-9DA2-F88CE52CB4CE', '409FB402-C7B5-4C73-A50D-9BC610D05622', '137D0BCF-FCC5-4A23-8093-A38A0BF6CD4D')
    }
    $repository = $RepositoryFolder
    $source = Join-Path $repository 'crates\p4kx-gui\wix\main.wxs'
    $outputPath = [IO.Path]::GetFullPath($Output)
    $outputFolder = [IO.Path]::GetDirectoryName($outputPath)
    $helperTask = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($HelperDirectory))
    $work = Join-Path (Join-Path $helperTask 'wix') ([IO.Path]::GetFileNameWithoutExtension($outputPath))
    $work = [IO.Path]::GetFullPath($work)
    $outputPrefix = $outputFolder.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if ([string]::Equals($work, $outputFolder, [StringComparison]::OrdinalIgnoreCase) -or
        $work.StartsWith($outputPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Private WiX work must be outside the MSI output directory.'
    }
    $buildWork = Join-Path $work 'build'
    $validationWork = Join-Path $work 'validate'
    New-Item -ItemType Directory -Path $buildWork, $validationWork -Force | Out-Null
    $runtimeSource = Join-Path $work 'runtime.wxs'
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
        -pdbtype none -intermediatefolder $buildWork `
        -d "Version=$Version" `
        -d "ApplicationGuid=$($guids[0])" `
        -d "ShortcutGuid=$($guids[1])" `
        -d "HelperFolder=$([IO.Path]::GetFullPath($HelperDirectory))" `
        -d "BinaryFolder=$(Resolve-Path $BinaryFolder)" `
        -d "RepositoryFolder=$repository" `
        -o $outputPath
    if ($LASTEXITCODE -ne 0) { throw "wix build failed with exit code $LASTEXITCODE" }

    Complete-P4kxMsiPackageCode $outputPath
    & $Wix msi validate -acceptEula wix7 -intermediatefolder $validationWork $outputPath
    if ($LASTEXITCODE -ne 0) { throw "wix msi validate failed with exit code $LASTEXITCODE" }

    Assert-P4kxMsiTransport (Get-P4kxMsiMetadata $outputPath)
}
