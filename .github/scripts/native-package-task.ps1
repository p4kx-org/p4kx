$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')

function New-P4kxNativePackageTask {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][ValidatePattern('^[1-9][0-9]*$')][string]$RunId,
        [Parameter(Mandatory)][ValidatePattern('^[1-9][0-9]*$')][string]$Attempt,
        [Parameter(Mandatory)][string]$Target,
        [Parameter(Mandatory)][ValidateSet('producer', 'consumer')][string]$Role
    )
    if (-not [IO.Path]::IsPathFullyQualified($Directory)) { throw 'Native package temporary root must be absolute.' }
    $platform = Get-P4kxPlatform -Target $Target
    $temporary = [IO.Path]::GetFullPath($Directory)
    $path = Join-Path $temporary "p4kx-native-package/$RunId/$Attempt/$($platform.target)/$Role"
    if ([IO.Path]::Exists($path) -or [IO.FileInfo]::new($path).LinkTarget -or [IO.DirectoryInfo]::new($path).LinkTarget) {
        throw 'Native package task directory must be new.'
    }
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    return $path
}
