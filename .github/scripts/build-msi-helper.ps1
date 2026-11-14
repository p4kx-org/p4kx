param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$Revision
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
. (Join-Path $PSScriptRoot 'msi-helper-payload.ps1')
. (Join-Path $PSScriptRoot 'target-sbom-document.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'windows') { throw 'Embedded MSI helpers require a native Windows target.' }
if (Test-Path -LiteralPath $Directory) { throw 'The private helper build needs a new owned task directory.' }
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$notices = Join-Path $repository 'THIRD-PARTY-NOTICES.md'
$cargoLock = Join-Path $repository 'Cargo.lock'
$cargoLockHash = (Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = Join-Path $repository 'crates/p4kx-msi-path/Cargo.toml'
$build = Join-Path $Directory 'build'
$payload = Join-Path $Directory 'payload'
New-Item -ItemType Directory -Path $payload | Out-Null
$names = @('CARGO_TARGET_DIR','CARGO_ENCODED_RUSTFLAGS','RUSTFLAGS')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:CARGO_TARGET_DIR = $build
    $env:CARGO_ENCODED_RUSTFLAGS = '-Ctarget-feature=+crt-static'
    [Environment]::SetEnvironmentVariable('RUSTFLAGS', $null)
    & cargo build --release --locked --target $Target -p p4kx-msi-path --bin p4kx-msi-path
    if ($LASTEXITCODE -ne 0) { throw 'Embedded helper static-CRT build failed.' }
    & cargo cyclonedx --manifest-path $manifest --format json --spec-version 1.5 --describe binaries --all --target $Target
    if ($LASTEXITCODE -ne 0) { throw 'Embedded helper target dependency SBOM failed.' }
}
finally { foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) } }
$binary = Join-Path $payload 'p4kx-msi-path.exe'
Copy-Item -LiteralPath (Join-Path $build "$Target/release/p4kx-msi-path.exe") -Destination $binary
$studio = Get-P4kxVisualStudio
$dumpbin = Get-P4kxDumpbin $platform $studio
$imports = @(Get-P4kxPeDependencies $dumpbin $binary | Sort-Object -Unique)
Assert-P4kxMsiHelperImports $imports $platform (Join-Path $env:SystemRoot 'System32')
$hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$sbom = Get-P4kxTargetSbomDocument -ManifestDirectory (Split-Path $manifest) -Binary 'p4kx-msi-path' -Target $Target -Version $Version -Role 'embedded-msi-helper'
$sbom.metadata.component | Add-Member -NotePropertyName hashes -NotePropertyValue @(@{alg='SHA-256';content=$hash}) -Force
$sbomPath = Join-Path $payload 'p4kx-msi-helper.cdx.json'
$sbom | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $sbomPath -Encoding utf8NoBOM
$compiler = & rustc -Vv
if ($LASTEXITCODE -ne 0) { throw 'Embedded helper compiler identity could not be read.' }
Copy-Item -LiteralPath (Join-Path $repository 'crates/p4kx-gui/wix/MSI-HELPER-NOTICE.txt') -Destination (Join-Path $payload 'p4kx-msi-helper-NOTICE.txt')
[ordered]@{
    schema_version=1;role='embedded-msi-helper';target=$Target;version=$Version;revision=$Revision
    binary='p4kx-msi-path.exe';binary_table='P4kxPathHelper';machine=(Get-ImageMachine $binary windows);sha256=$hash
    crt='static-msvc';rustflags='-Ctarget-feature=+crt-static';imports=$imports;compiler=($compiler -join [Environment]::NewLine)
    sbom_sha256=(Get-FileHash -LiteralPath $sbomPath -Algorithm SHA256).Hash.ToLowerInvariant()
    helper_notice_sha256=(Get-FileHash -LiteralPath (Join-Path $payload 'p4kx-msi-helper-NOTICE.txt') -Algorithm SHA256).Hash.ToLowerInvariant()
    notices='THIRD-PARTY-NOTICES.md';notices_sha256=(Get-FileHash -LiteralPath $notices -Algorithm SHA256).Hash.ToLowerInvariant()
    cargo_lock_sha256=$cargoLockHash
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $payload 'p4kx-msi-helper.json') -Encoding utf8NoBOM
Assert-P4kxMsiHelperPayload $payload $platform $Version $Revision $notices `
    ((Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash.ToLowerInvariant())
