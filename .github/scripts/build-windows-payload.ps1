param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][ValidateSet('msi', 'portable-zip')][string]$Package
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row
if ($row.os -cne 'windows') { throw 'Windows payloads require a Windows target.' }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Payload output must be a new directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $source = Join-Path $BinaryDirectory "$name.exe"
    if ((Get-ImageMachine -Path $source -Os windows) -ne $row.pe_machine) { throw "Wrong CPU in $source" }
    Copy-Item -LiteralPath $source -Destination $OutputDirectory
}
foreach ($name in @('LICENSE-MIT', 'LICENSE-MIT.md', 'LICENSE-APACHE', 'LICENSE-APACHE.md', 'THIRD-PARTY-NOTICES.md', 'CHANGELOG.md')) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination $OutputDirectory
}
@{target=$Target; package=$Package} | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'p4kx-distribution.json') -Encoding utf8NoBOM
Copy-P4kxWindowsRuntime -Directory $OutputDirectory -Platform $row
