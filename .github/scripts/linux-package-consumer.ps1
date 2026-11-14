$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'linux-package-evidence.ps1')
. (Join-Path $PSScriptRoot 'linux-profile-preservation.ps1')
. (Join-Path $PSScriptRoot 'linux-elf.ps1')

function Get-P4kxLinuxConsumerProducer {
    param([string]$Package, [string]$Assets, [string]$Version, $Platform, $Base)
    $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package $Package
    $producer = Get-Content -LiteralPath (Join-Path $Assets "$name.producer.json") -Raw | ConvertFrom-Json
    Assert-P4kxLinuxPackageProducer $producer $Platform $Base $Assets $Version
    return @{name=$name; asset=(Join-Path $Assets $name); proof=$producer}
}

function Invoke-P4kxLinuxHostPackage {
    param([ValidateSet('tar-gz', 'app-image', 'deb')][string]$Package, [string]$Assets,
        [string]$Fixture, [string]$Directory, [string]$Version, $Platform, $Base)
    $producer = Get-P4kxLinuxConsumerProducer $Package $Assets $Version $Platform $Base
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $preserved = @(New-P4kxProfileSentinels (Join-Path $Directory 'preserved-profile'))
    $state = $null
    $result = $null
    $failures = [Collections.Generic.List[Exception]]::new()
    try {
        $state = switch ($Package) {
            'tar-gz' { Expand-P4kxLinuxTar $producer.asset (Join-Path $Directory 'payload') }
            'app-image' { Expand-P4kxAppImage $producer.asset (Join-Path $Directory 'payload') $Platform }
            'deb' { Install-P4kxDeb $producer.asset $Version $Platform }
        }
        $images = @(Get-P4kxLinuxApplicationEvidence -Directory $state.binaries -Platform $Platform)
        Assert-P4kxLinuxPackageExecution @{target=$Platform.target; package=$Package; binaries=$images} $producer.proof $Platform
        & (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Platform.target -BinaryDirectory $state.binaries -FixtureDirectory $Fixture -OutputDirectory (Join-Path $Directory 'smoke') -Package $Package -Launchers $state.launchers -ExpectedVersion $Version
        $result = Get-Content -LiteralPath (Join-Path $Directory 'smoke/runtime.json') -Raw | ConvertFrom-Json
        Assert-P4kxLinuxPackageExecution $result $producer.proof $Platform
        $result | Add-Member -NotePropertyName producer -NotePropertyValue $producer.proof
        $result | Add-Member -NotePropertyName archive_sha256 -NotePropertyValue $producer.proof.archive_sha256
    }
    catch { $failures.Add($_.Exception) }
    finally {
        try { if ($state) { Remove-P4kxLinuxPackage $Package $Platform } } catch { $failures.Add($_.Exception) }
        try { Assert-P4kxProfileSentinels $preserved } catch { $failures.Add($_.Exception) }
    }
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Native host package or cleanup failed.', $failures) }
    return $result
}
