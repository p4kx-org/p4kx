$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-path-fixture.ps1')

function Assert-P4kxPrependProbe {
    param([string]$Path, $Platform, [string]$Version, [string]$UpgradeCode)
    $binding = Get-Content -LiteralPath "$Path.probe.json" -Raw | ConvertFrom-Json
    if ($binding.schema_version -ne 1 -or $binding.role -cne 'prepend_hypothesis' -or
        $binding.path_part -cne 'first' -or $binding.target -cne $Platform.target -or
        $binding.version -cne $Version -or $binding.revision -cne $env:GITHUB_SHA -or
        $binding.msi_sha256 -cne (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()) {
        throw 'Prepend probe source/target/version/digest binding differs.'
    }
    $metadata = Get-P4kxMsiMetadata $Path
    Assert-P4kxMsiMetadata $metadata $Platform $Version $UpgradeCode
    $rows = @($metadata.environment | Where-Object { $_[0] -match '^[=-]+PATH$' })
    if ($rows.Count -ne 1 -or $rows[0][0] -cnotin @('=-PATH','-=PATH') -or
        $rows[0][1] -cne '[APPLICATIONFOLDER];[~]') { throw 'Probe MSI does not author the exact per-user prepend operation.' }
    return $metadata
}

function New-P4kxPathProbeContext {
    param([string]$Target, [string]$Msi, [string]$UpgradeCode, [string]$OutputDirectory)
    Assert-P4kxHostedPathFixture
    $platform = Get-P4kxPlatform -Target $Target
    Assert-P4kxNativeHost -Platform $platform
    $msi = Resolve-P4kxMsiInput $Msi
    $metadata = Get-P4kxMsiMetadata $msi
    $metadata = Assert-P4kxPrependProbe $msi $platform $metadata.version $UpgradeCode
    $logs = [IO.Path]::GetFullPath($OutputDirectory)
    New-Item -ItemType Directory -Force -Path $logs | Out-Null
    return [pscustomobject]@{
        msi=$msi; metadata=$metadata; platform=$platform; upgrade=$UpgradeCode; logs=$logs
        legacy_keys=@()
        folder=(Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4KX')
        shortcut=(Join-Path ([Environment]::GetFolderPath('Programs')) 'P4KX.lnk')
    }
}

function Assert-P4kxLegacyOwnershipAbsent {
    param($Context)
    if ($Context.legacy_keys.Count -eq 0) { return }
    $folder = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4K-Extractor'
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'P4K-Extractor.lnk'
    if ((Test-Path -LiteralPath $folder) -or (Test-Path -LiteralPath $shortcut)) { throw 'Historical installation files remain.' }
    foreach ($key in $Context.legacy_keys) { if (Test-Path -LiteralPath "HKCU:\$key") { throw 'Historical installer identity remains.' } }
}
