param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Package
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'package-mcp-smoke.ps1')
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
$suffix = if ($row.os -eq 'windows') { '.exe' } else { '' }
$cli = Join-Path $BinaryDirectory "p4kx$suffix"
$list = Invoke-P4kxPackageProcess $cli @('list', $archive) $directory (Join-Path $directory 'cli-list.log')
if ($list -cnotmatch 'Data/hello\.txt' -or $list -cnotmatch 'Data/smoke\.xml') { throw 'CLI omitted synthetic Entries.' }
$extract = Join-Path $directory 'extract'
$null = Invoke-P4kxPackageProcess $cli @('extract', $archive, '--out', $extract) $directory (Join-Path $directory 'cli-extract.log')
if ((Get-FileHash (Join-Path $extract 'Data/hello.txt')).Hash -cne
    (Get-FileHash (Join-Path $FixtureDirectory 'hello.txt')).Hash) { throw 'CLI extracted different bytes.' }
$convert = Join-Path $directory 'convert'
$null = Invoke-P4kxPackageProcess $cli @('extract', $archive, '--out', $convert, '--convert') $directory (Join-Path $directory 'cli-convert.log')
$xml = [xml](Get-Content (Join-Path $convert 'Data/smoke.xml') -Raw)
if ($xml.NativeSmoke.value -cne 'synthetic') { throw 'CLI CryXml conversion differs.' }
Invoke-P4kxPackageMcpSmoke (Join-Path $BinaryDirectory "p4kx-mcp$suffix") $archive $directory $expected
$gui = New-P4kxPackageProcess (Join-Path $BinaryDirectory "p4kx-gui$suffix") @() $directory
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
