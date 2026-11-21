param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$StagingDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-payload.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
$name = Get-P4kxAssetName -Target $Target -Version $Version -Package tar-gz
$rootName = $name -replace '\.tar\.gz$', ''
$payload = Join-Path $StagingDirectory $rootName
$original = @(New-P4kxLinuxPayload -Target $Target -Version $Version -Package tar-gz -BinaryDirectory $BinaryDirectory -OutputDirectory $payload)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$asset = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $name))
Invoke-P4kxLinuxTool tar @('--sort=name', "--mtime=@$env:SOURCE_DATE_EPOCH", '--owner=0', '--group=0', '--numeric-owner', '--gzip', '--create', '--file', $asset, '--directory', $StagingDirectory, $rootName) | Out-Null
Write-P4kxLinuxPackageProducer -Target $Target -Version $Version -Package tar-gz -Asset $asset -BinaryDirectory (Join-Path $payload 'bin') -OriginalImages $original -Transformations @() -Output (Join-Path $OutputDirectory "$name.producer.json")
