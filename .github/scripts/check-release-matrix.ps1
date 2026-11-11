param(
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Revision,
    [switch]$RequireMacosDeviceEvidence,
    [string]$MacosCurrentVersion,
    [string]$MacosDeviceEvidencePath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')
$contract = Get-P4kxPlatformContract
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
    if ($row.os -ceq 'macos' -and $RequireMacosDeviceEvidence) {
        if (-not $MacosDeviceEvidencePath) { throw 'Native macOS device evidence is required for publication.' }
        $deviceRuns = Get-Content -LiteralPath $MacosDeviceEvidencePath -Raw | ConvertFrom-Json -NoEnumerate
        if ($deviceRuns -isnot [array]) { throw 'Native macOS device evidence must be a JSON array.' }
        Assert-P4kxMacosReleaseDevices $runtime $row $MacosCurrentVersion $deviceRuns
    }
}
$targetCount = @($contract.targets).Count
$packageCount = @($contract.targets | ForEach-Object packages).Count
$applicationCount = $targetCount * 3
Write-Output "Complete release matrix verified: $packageCount packages, $applicationCount target/binary SBOMs and $targetCount native runtime proofs."
