param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Directory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'pinned-download.ps1')
. (Join-Path $PSScriptRoot 'macos-signing.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'macos') { throw 'macOS tools require the Apple Silicon host.' }
$null = New-Item -ItemType Directory -Path $Directory
$version = '0.5.9'
$url = "https://github.com/CycloneDX/cyclonedx-rust-cargo/releases/download/cargo-cyclonedx-$version/cargo-cyclonedx-$Target.tar.xz"
$digest = '4c53dfa21e70b65bf7f8d2592aadde3bcb02c1a40b6ec63b877e5ca65a29e180'
$archive = Join-Path $Directory 'cargo-cyclonedx.tar.xz'
Save-P4kxPinnedDownload $url $digest $archive
Invoke-P4kxMacTool /usr/bin/tar @('-xJf', $archive, '-C', $Directory) | Out-Null
$files = @(Get-ChildItem -LiteralPath $Directory -Recurse -File -Filter 'cargo-cyclonedx')
if ($files.Count -ne 1 -or $files[0].LinkType -or
    (Get-ImageMachine -Path $files[0].FullName -Os macos) -ne $platform.mach_cpu_type) {
    throw 'Expected one pinned native CycloneDX host executable.'
}
Invoke-P4kxMacTool /bin/chmod @('755', $files[0].FullName) | Out-Null
$actual = @(Invoke-P4kxMacTool $files[0].FullName @('cyclonedx', '--version'))
if ($actual.Count -ne 1 -or $actual[0] -cne "cargo-cyclonedx-cyclonedx $version") {
    throw "Unexpected CycloneDX host tool version: $($actual -join ' | ')"
}
if (-not $env:GITHUB_PATH) { throw 'The macOS packaging tool requires a GitHub path file.' }
$files[0].DirectoryName | Add-Content -LiteralPath $env:GITHUB_PATH -Encoding utf8NoBOM
[ordered]@{
    target = $Target
    cyclonedx = @{
        version = $version
        role = 'host target-filtered dependency analysis'
        archive_sha256 = $digest
        url = $url
    }
    xcode = @(Invoke-P4kxMacTool /usr/bin/xcodebuild @('-version'))
    sdk = @(Invoke-P4kxMacTool /usr/bin/xcrun @('--sdk', 'macosx', '--show-sdk-version'))
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Directory 'package-tools.json') -Encoding utf8NoBOM
