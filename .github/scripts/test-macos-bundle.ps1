param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-bundle.ps1')
$repository = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$inputs = Join-Path $Directory 'synthetic input'
$null = New-Item -ItemType Directory -Path $inputs
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp', 'fixture.icns')) {
    "Synthetic test input for $name" | Set-Content -LiteralPath (Join-Path $inputs $name) -Encoding utf8NoBOM
}
$icon = Join-Path $inputs 'fixture.icns'
$bundle = Join-Path $Directory 'P4KX.app'
$platform = [pscustomobject]@{
    os = 'macos'; target = 'aarch64-apple-darwin'; deployment_target = '26.0'; packages = @('dmg')
}
$arguments = @{
    Destination = $bundle; BinaryDirectory = $inputs; Icon = $icon; Repository = $repository
    Version = '0.5.0-rc.1+synthetic'; Platform = $platform
}
$paths = New-P4kxMacBundle @arguments

$identity = Get-Content -LiteralPath (Join-Path $bundle 'Contents/Resources/p4kx-distribution.json') -Raw | ConvertFrom-Json
if ($identity.target -cne $platform.target -or $identity.package -cne 'dmg' -or
    @($identity.PSObject.Properties).Count -ne 2) {
    throw 'Bundle distribution identity differs.'
}
$settings = [Xml.XmlReaderSettings]::new()
$settings.DtdProcessing = [Xml.DtdProcessing]::Ignore
$reader = [Xml.XmlReader]::Create($paths.plist, $settings)
try {
    $plist = [Xml.XmlDocument]::new()
    $plist.Load($reader)
} finally {
    $reader.Dispose()
}
$expected = @{
    CFBundlePackageType = 'APPL'; CFBundleExecutable = 'p4kx-gui'; CFBundleIconFile = 'P4KX.icns'
    CFBundleIdentifier = 'io.github.p4kx-org.p4kx'; CFBundleShortVersionString = '0.5.0'
    CFBundleVersion = '0.5.0'; LSMinimumSystemVersion = '26.0'
}
foreach ($entry in $expected.GetEnumerator()) {
    $value = $plist.SelectSingleNode("/plist/dict/key[text()='$($entry.Key)']/following-sibling::*[1][self::string]")
    if ($value.InnerText -cne $entry.Value) { throw "Unexpected bundle metadata: $($entry.Key)" }
}
if ($plist.SelectSingleNode("/plist/dict/key[text()='LSRequiresNativeExecution']/following-sibling::*[1][self::true]") -eq $null -or
    $plist.SelectSingleNode("/plist/dict/key[text()='LSArchitecturePriority']/following-sibling::*[1][self::array]/string").InnerText -cne 'arm64') {
    throw 'Bundle does not declare native Apple Silicon execution.'
}
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $original = Get-FileHash -LiteralPath (Join-Path $inputs $name)
    $packaged = Get-FileHash -LiteralPath (Join-Path $bundle "Contents/MacOS/$name")
    if ($original.Hash -cne $packaged.Hash) { throw "Bundle changed executable bytes: $name" }
}
foreach ($name in @('LICENSE-MIT', 'LICENSE-MIT.md', 'LICENSE-APACHE', 'LICENSE-APACHE.md', 'THIRD-PARTY-NOTICES.md', 'CHANGELOG.md')) {
    if ((Get-FileHash -LiteralPath (Join-Path $repository $name)).Hash -cne
        (Get-FileHash -LiteralPath (Join-Path $bundle "Contents/Resources/Notices/$name")).Hash) {
        throw "Bundle changed notice bytes: $name"
    }
}
if ((Get-FileHash -LiteralPath $icon).Hash -cne (Get-FileHash -LiteralPath $paths.icon).Hash) {
    throw 'Bundle changed the icon bytes.'
}

function Assert-RejectedBundle {
    param([hashtable]$InputArguments, [string]$ExpectedError)
    try {
        $null = New-P4kxMacBundle @InputArguments
    } catch {
        if ($_.Exception.Message -cnotmatch $ExpectedError) { throw }
        return
    }
    throw "Bundle should have been rejected: $ExpectedError"
}

Assert-RejectedBundle $arguments 'fresh P4KX\.app'
$arguments.Destination = Join-Path $Directory 'invalid/P4KX.app'
$arguments.Version = '0.5.0</string>'
Assert-RejectedBundle $arguments 'semantic version'
if (Test-Path -LiteralPath $arguments.Destination) { throw 'Invalid bundle was materialized.' }
'macOS bundle layout contract passed.'
