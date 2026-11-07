param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Package,
    [hashtable]$Launchers,
    [string]$ExpectedVersion
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'package-mcp-smoke.ps1')
. (Join-Path $PSScriptRoot 'package-launchers.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row
if ($row.os -eq 'windows') {
    . (Join-Path $PSScriptRoot 'windows-runtime.ps1')
    Assert-P4kxWindowsRuntimeInventory -Directory $BinaryDirectory -Platform $row
}
if ($Package -cnotin $row.packages) { throw 'Package format does not belong to this target.' }
$directory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$archive = (Resolve-Path (Join-Path $FixtureDirectory 'native-smoke.p4k')).Path
$expected = Get-Content (Join-Path $FixtureDirectory 'hello.txt') -Raw -Encoding utf8
$identity = Get-Content (Join-Path $BinaryDirectory 'p4kx-distribution.json') -Raw | ConvertFrom-Json
if ($identity.target -cne $Target -or $identity.package -cne $Package) { throw 'Packaged distribution identity differs.' }
$binaries = @(foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $filename = if ($row.os -eq 'windows') { "$name.exe" } else { $name }
    $file = (Resolve-Path (Join-Path $BinaryDirectory $filename)).Path
    $machine = Get-ImageMachine -Path $file -Os $row.os
    $required = if ($row.os -eq 'windows') { $row.pe_machine } else { $row.elf_machine }
    if ($machine -ne $required) { throw "Packaged $name is not native $Target." }
    [ordered]@{name=$name; machine=$machine; sha256=(Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$commands = Get-P4kxPackageLaunchers -BinaryDirectory $BinaryDirectory -Os $row.os -Overrides $Launchers
$cli = $commands['p4kx']
$version = Invoke-P4kxPackageProcess $cli.executable (Get-P4kxPackageArguments $cli @('--version')) $directory (Join-Path $directory 'cli-version.log')
if ($ExpectedVersion -and ($version -split '\r?\n')[0] -cne "p4kx $ExpectedVersion") { throw 'Packaged CLI version differs from the release.' }
$null = Invoke-P4kxPackageProcess $cli.executable (Get-P4kxPackageArguments $cli @('--help')) $directory (Join-Path $directory 'cli-help.log')
$list = Invoke-P4kxPackageProcess $cli.executable (Get-P4kxPackageArguments $cli @('list', $archive)) $directory (Join-Path $directory 'cli-list.log')
if ($list -cnotmatch 'Data/hello\.txt' -or $list -cnotmatch 'Data/smoke\.xml') { throw 'CLI omitted synthetic Entries.' }
$extract = Join-Path $directory 'extract'
$null = Invoke-P4kxPackageProcess $cli.executable (Get-P4kxPackageArguments $cli @('extract', $archive, '--filter', '*', '--out', $extract)) $directory (Join-Path $directory 'cli-extract.log')
if ((Get-FileHash (Join-Path $extract 'Data/hello.txt')).Hash -cne
    (Get-FileHash (Join-Path $FixtureDirectory 'hello.txt')).Hash) { throw 'CLI extracted different bytes.' }
$convert = Join-Path $directory 'convert'
$null = Invoke-P4kxPackageProcess $cli.executable (Get-P4kxPackageArguments $cli @('extract', $archive, '--filter', '*', '--out', $convert, '--convert')) $directory (Join-Path $directory 'cli-convert.log')
$xml = [xml](Get-Content (Join-Path $convert 'Data/smoke.xml') -Raw)
if ($xml.NativeSmoke.value -cne 'synthetic') { throw 'CLI CryXml conversion differs.' }
$mcp = $commands['p4kx-mcp']
$null = Invoke-P4kxPackageProcess $mcp.executable (Get-P4kxPackageArguments $mcp @('--help')) $directory (Join-Path $directory 'mcp-help.log')
Invoke-P4kxPackageMcpSmoke $mcp.executable $archive $directory $expected $mcp.prefix
$window = $commands['p4kx-gui']
$gui = New-P4kxPackageProcess $window.executable $window.prefix $directory
try {
    $gui.StartInfo.Environment['P4KX_SMOKE_FRAMES'] = '3'
    [void]$gui.Start()
    $stdout = $gui.StandardOutput.ReadToEndAsync()
    $stderr = $gui.StandardError.ReadToEndAsync()
    if (-not $gui.WaitForExit(120000)) { $gui.Kill($true); $gui.WaitForExit(); throw 'Packaged GUI frame smoke timed out.' }
    $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult() | Set-Content (Join-Path $directory 'gui.log') -Encoding utf8NoBOM
    if ($gui.ExitCode -ne 0) { throw "Packaged GUI exited $($gui.ExitCode)." }
}
finally { $gui.Dispose() }
[ordered]@{
    target=$Target; package=$Package; binaries=$binaries
    runtime=@{cli_list='success'; cli_extract='success'; cli_convert='success'; mcp_tools='success'; mcp_read='success'; gui_frames='success'}
} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $directory 'runtime.json') -Encoding utf8NoBOM
