[CmdletBinding()]
param(
    [Parameter(Mandatory)][string[]]$Packages,
    [switch]$IncludeRecommended
)
$ErrorActionPreference = 'Stop'
$sources = @(
    '-o', 'Dir::Etc::SourceList=/etc/apt/sources.list.d/ubuntu.sources'
    '-o', 'Dir::Etc::SourceParts=-'
)
sudo apt-get @sources update --no-list-cleanup
if ($LASTEXITCODE -ne 0) { throw "Ubuntu APT index refresh failed with exit $LASTEXITCODE." }

$installOptions = @('--yes')
if (!$IncludeRecommended) { $installOptions += '--no-install-recommends' }
sudo apt-get @sources install @installOptions @Packages
if ($LASTEXITCODE -ne 0) { throw "Ubuntu package installation failed with exit $LASTEXITCODE." }
