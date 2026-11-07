param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$BuildMetadata,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-consumer.ps1')
. (Join-Path $PSScriptRoot 'flatpak-prerequisites.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-interface.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-session.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
Assert-P4kxOwnedDocumentsSession 'stock'
$base = Get-Content -LiteralPath $BuildMetadata -Raw | ConvertFrom-Json
$results = @()
$failures = [Collections.Generic.List[Exception]]::new()
try {
    Invoke-P4kxLinuxTool dbus-update-activation-environment @('DISPLAY', 'XAUTHORITY', 'XDG_CURRENT_DESKTOP') | Out-Null
    $interface = Get-P4kxDocumentsInterfaceEvidence $OutputDirectory 4
    Save-P4kxDocumentsSessionIdentity $OutputDirectory $null 'initial'
    foreach ($package in @('tar-gz', 'app-image', 'deb')) {
        try { $results += Invoke-P4kxLinuxHostPackage $package $AssetDirectory $FixtureDirectory (Join-Path $OutputDirectory $package) $Version $platform $base }
        catch { $failures.Add($_.Exception) }
    }
    try {
        $directory = Join-Path $OutputDirectory 'flatpak'
        New-Item -ItemType Directory -Path $directory | Out-Null
        $producer = Get-P4kxLinuxConsumerProducer 'flatpak' $AssetDirectory $Version $platform $base
        $state = Install-P4kxFlatpak $producer.asset $producer.proof $platform $FixtureDirectory $directory
        $images = @(Get-P4kxLinuxApplicationEvidence $state.binaries $platform)
        Assert-P4kxLinuxPackageExecution @{target=$Target; package='flatpak'; binaries=$images} $producer.proof $platform
        @{state=$state; producer=$producer.proof; binaries=$images; target=$Target; revision=$env:GITHUB_SHA} | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'flatpak-state.json') -Encoding utf8NoBOM
        $negative = Test-P4kxFlatpakPrerequisites $state $platform $FixtureDirectory (Join-Path $directory 'prerequisites')
        @{target=$Target; revision=$env:GITHUB_SHA; interface=$interface; prerequisite=$negative} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'negative.json') -Encoding utf8NoBOM
    }
    catch { $failures.Add($_.Exception) }
}
catch { $failures.Add($_.Exception) }
finally {
    try { Save-P4kxDocumentsSessionIdentity $OutputDirectory } catch { $failures.Add($_.Exception) }
    try { $results | ConvertTo-Json -Depth 20 -AsArray | Set-Content -LiteralPath (Join-Path $OutputDirectory 'packages.json') -Encoding utf8NoBOM }
    catch { $failures.Add($_.Exception) }
}
if ($failures.Count -ne 0) { throw [AggregateException]::new('Stock-session native checks failed.', $failures) }
