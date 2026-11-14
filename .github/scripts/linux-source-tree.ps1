$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Copy-P4kxLinuxSourceTree {
    param([Parameter(Mandatory)][string]$Source, [Parameter(Mandatory)][string]$Destination)
    if (-not $IsLinux) { throw 'Source-tree preservation requires native Linux coreutils.' }
    $sourceItem = Get-Item -LiteralPath $Source -Force
    if (-not $sourceItem.PSIsContainer -or $sourceItem.LinkType) { throw 'Source-tree input must be a real directory.' }
    if ([IO.Path]::Exists($Destination) -or [IO.FileInfo]::new($Destination).LinkTarget) { throw 'Source-tree destination must be new.' }
    Invoke-P4kxLinuxTool '/usr/bin/cp' @('--archive', '--no-target-directory', '--', $sourceItem.FullName, [IO.Path]::GetFullPath($Destination)) | Out-Null
}
