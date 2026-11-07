param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Directory | Out-Null
$wix = Join-Path $Directory 'wix'
dotnet tool install wix --version 7.0.0 --tool-path $wix
if ($LASTEXITCODE -ne 0) { throw 'Pinned WiX installation failed.' }
$archive = Join-Path $Directory 'cargo-cyclonedx.zip'
$digest = '8750e00775661dcb75bc482c1a298839fd94e8a0c033b49905ba0f246ffed202'
Invoke-WebRequest -Uri 'https://github.com/CycloneDX/cyclonedx-rust-cargo/releases/download/cargo-cyclonedx-0.5.9/cargo-cyclonedx-x86_64-pc-windows-msvc.zip' -OutFile $archive
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $digest) { throw 'Pinned cargo-cyclonedx archive hash mismatch.' }
$sbom = Join-Path $Directory 'cyclonedx'
Expand-Archive -LiteralPath $archive -DestinationPath $sbom
$tools = @(Get-ChildItem -LiteralPath $sbom -Recurse -File -Filter 'cargo-cyclonedx.exe')
if ($tools.Count -ne 1) { throw 'Expected exactly one CycloneDX host executable.' }
if ($env:GITHUB_ENV) { "WIX_EXE=$(Join-Path $wix 'wix.exe')" >> $env:GITHUB_ENV }
if ($env:GITHUB_PATH) { $tools[0].DirectoryName >> $env:GITHUB_PATH }
[ordered]@{
    wix=@{version='7.0.0'; role='host MSI authoring/validation'}
    cyclonedx=@{version='0.5.9'; role='host target-filtered dependency analysis'; executable_cpu='amd64'; archive_sha256=$digest}
} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $Directory 'package-tools.json') -Encoding utf8NoBOM
