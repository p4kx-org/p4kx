param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RuntimeInventory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
& cargo cyclonedx --format json --spec-version 1.5 --describe binaries --target $Target
if ($LASTEXITCODE -ne 0) { throw "Target dependency SBOM generation failed: $Target" }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$runtime = if ($RuntimeInventory) { Get-Content -LiteralPath $RuntimeInventory -Raw | ConvertFrom-Json }
if ($runtime -and $runtime.target -cne $Target) { throw 'Runtime SBOM inventory target mismatch.' }
foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $files = @(Get-ChildItem -LiteralPath "crates/$binary" -Recurse -File -Filter "$($binary)_bin.cdx.json")
    if ($files.Count -ne 1) { throw "Expected one generated SBOM for $binary, got $($files.Count)." }
    $sbom = Get-Content -LiteralPath $files[0].FullName -Raw | ConvertFrom-Json
    if ($sbom.metadata.component.version -cne $Version -or -not $sbom.components) { throw 'Generated SBOM application version/components differ.' }
    $properties = @($sbom.metadata.properties | Where-Object { $_ })
    $properties += @(@{name='p4kx:target'; value=$Target}, @{name='p4kx:binary'; value=$binary})
    $sbom.metadata | Add-Member -NotePropertyName properties -NotePropertyValue $properties -Force
    foreach ($dll in $runtime.redistributables) {
        if ($dll.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $dll.machine -ne $platform.pe_machine) { throw 'Runtime SBOM file has wrong CPU/digest.' }
        $sbom.components += [pscustomobject]@{
            type='file'; name=$dll.name; 'bom-ref'="microsoft-runtime:$Target/$($dll.name)"
            hashes=@(@{alg='SHA-256'; content=$dll.sha256})
            licenses=@(@{license=@{name='Microsoft Visual Studio 2022 redistribution terms'; url='https://visualstudio.microsoft.com/license-terms/'}})
            properties=@(@{name='p4kx:runtime-source'; value=$dll.source})
        }
    }
    $path = Join-Path $OutputDirectory "P4KX-$Version-$Target-$binary.cdx.json"
    $sbom | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
}
