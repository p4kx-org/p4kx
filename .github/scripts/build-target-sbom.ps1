param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RuntimeInventory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'target-sbom-document.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
& cargo cyclonedx --format json --spec-version 1.5 --describe binaries --target $Target
if ($LASTEXITCODE -ne 0) { throw "Target dependency SBOM generation failed: $Target" }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$runtime = if ($RuntimeInventory) { Get-Content -LiteralPath $RuntimeInventory -Raw | ConvertFrom-Json }
foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $sbom = Get-P4kxTargetSbomDocument -ManifestDirectory "crates/$binary" -Binary $binary -Target $Target -Version $Version
    Add-P4kxRuntimeSbomComponents $sbom $runtime $platform
    $path = Join-Path $OutputDirectory "P4KX-$Version-$Target-$binary.cdx.json"
    $sbom | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
}
