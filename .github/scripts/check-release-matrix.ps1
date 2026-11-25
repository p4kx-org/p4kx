param(
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Revision,
    [string]$WindowsDesktopEvidencePath,
    [string]$ReleaseRunId,
    [switch]$RequireStableMacosRelease
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
$contract = Get-P4kxPlatformContract
$desktopReceipts = $null
if ($WindowsDesktopEvidencePath -or $ReleaseRunId) {
    if (-not $WindowsDesktopEvidencePath -or $ReleaseRunId -cnotmatch '\A[1-9][0-9]*\z') {
        throw 'Windows AMD64 desktop validation requires its receipt file and the original release run ID.'
    }
    . (Join-Path $PSScriptRoot 'windows-desktop-device-evidence.ps1')
    $desktopReceipts = Get-Content -LiteralPath $WindowsDesktopEvidencePath -Raw | ConvertFrom-Json -NoEnumerate
    Assert-P4kxWindowsDesktopReceiptRows $desktopReceipts $contract
}
if ($Revision -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') { throw 'Release revision must be a full Git commit digest.' }
$pinLines = & (Join-Path $PSScriptRoot 'check-build-tools.ps1')
$compiler = ($pinLines | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
$nextest = ($pinLines | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)

foreach ($row in $contract.targets) {
    foreach ($package in $row.packages) {
        $name = Get-P4kxAssetName -Target $row.target -Version $Version -Package $package
        $file = Get-Item -LiteralPath (Join-Path $AssetDirectory $name) -ErrorAction Stop
        if (-not $file.PSIsContainer -and $file.Length -gt 0) { continue }
        throw "Missing or empty required package '$name'."
    }
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $name = "P4KX-$Version-$($row.target)-$binary.cdx.json"
        $sbom = Get-Content -LiteralPath (Join-Path $AssetDirectory $name) -Raw | ConvertFrom-Json
        $targetProperties = @($sbom.metadata.properties | Where-Object name -CEQ 'p4kx:target')
        $binaryProperties = @($sbom.metadata.properties | Where-Object name -CEQ 'p4kx:binary')
        if ($sbom.bomFormat -cne 'CycloneDX' -or -not $sbom.components -or
            $sbom.metadata.component.version -cne $Version -or
            $targetProperties.Count -ne 1 -or $targetProperties[0].value -cne $row.target -or
            $binaryProperties.Count -ne 1 -or $binaryProperties[0].value -cne $binary) {
            throw "SBOM '$name' does not identify the actual native target and binary."
        }
    }
    $name = "P4KX-$Version-$($row.target)-runtime.json"
    $runtime = Get-Content -LiteralPath (Join-Path $AssetDirectory $name) -Raw | ConvertFrom-Json
    Assert-P4kxArchiveEvidence -Metadata $runtime -Platform $row -Revision $Revision -Compiler $compiler -Nextest $nextest
    Assert-P4kxRuntimeEvidence -Metadata $runtime -Platform $row -Version $Version -Revision $Revision
    Assert-P4kxPackageRuntime $runtime $row $AssetDirectory $Version
    if ($RequireStableMacosRelease -and $row.os -ceq 'macos') {
        $current = @($runtime.package_smokes)[0].current_hosted_smoke
        Assert-P4kxMacosHostedRole $current.runtime_host $row current -RequireReleaseRunnerVersion
    }
    if ($null -ne $desktopReceipts -and (Test-P4kxWindowsDesktopTarget $row)) {
        Assert-P4kxWindowsDesktopEvidence -Runtime $runtime -Platform $row -Version $Version -Revision $Revision -ReleaseRunId $ReleaseRunId -Records $desktopReceipts
    }
}
$targetCount = @($contract.targets).Count
if ($null -ne $desktopReceipts) { Write-Output 'The Windows AMD64 desktop receipt matches the verified release candidate.' }
$packageCount = @($contract.targets | ForEach-Object packages).Count
$applicationCount = $targetCount * 3
Write-Output "Complete release matrix verified: $packageCount packages, $applicationCount target/binary SBOMs and $targetCount native runtime proofs."
