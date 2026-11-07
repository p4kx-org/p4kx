<#
.SYNOPSIS
Verifies Debian staging paths and scanner failures with synthetic tools.
#>
param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-debian.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root)) { throw 'Debian path test directory must be new.' }
$caller = Join-Path $root 'caller directory'
$stage = Join-Path $caller 'package stage'
$tools = Join-Path $root 'tools'
New-Item -ItemType Directory -Path $tools, (Join-Path $stage 'debian'), (Join-Path $stage 'package/usr/lib/p4kx') -Force | Out-Null
$control = Join-Path $stage 'debian/control'
[IO.File]::WriteAllText($control, 'synthetic control')
$binaries = @(foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
    $path = Join-Path $stage "package/usr/lib/p4kx/$name"
    [IO.File]::WriteAllText($path, 'synthetic payload')
    $path
})
$expected = Join-Path $root 'expected.json'
@{stage=$stage; binaries=$binaries} | ConvertTo-Json | Set-Content -LiteralPath $expected -Encoding utf8NoBOM
$scannerName = if ($IsLinux) { 'scanner-fixture.ps1' } else { 'dpkg-shlibdeps.ps1' }
$scanner = Join-Path $tools $scannerName
@'
$ErrorActionPreference = 'Stop'
$expected = Get-Content -LiteralPath $env:P4KX_TEST_DEBIAN_EXPECTED -Raw | ConvertFrom-Json
if ($PWD.ProviderPath -cne $expected.stage -or -not (Test-Path -LiteralPath 'debian/control' -PathType Leaf)) { throw 'Dependency scanner received the wrong process working directory.' }
if ($IsLinux -and [Environment]::CurrentDirectory -cne $expected.stage) { throw 'Native scanner process did not inherit the staged working directory.' }
if ($args.Count -ne 4 -or $args[0] -cne '-O') { throw 'Dependency scanner arguments differ.' }
foreach ($index in 0..2) {
    if ($args[$index + 1] -cne ('-e' + $expected.binaries[$index]) -or -not (Test-Path -LiteralPath $expected.binaries[$index] -PathType Leaf)) { throw 'Dependency scanner used a workspace-relative or missing executable.' }
}
'invoked' | Add-Content -LiteralPath $env:P4KX_TEST_DEBIAN_RECEIPT
if ($env:P4KX_TEST_DEBIAN_FAILURE -eq '1') { exit 37 }
'shlibs:Depends=libasound2t64 (>= 1.2), libc6 (>= 2.38)'
exit 0
'@ | Set-Content -LiteralPath $scanner -Encoding utf8NoBOM
$launcher = $scanner
if ($IsLinux) {
    $launcher = Join-Path $tools 'dpkg-shlibdeps'
    $source = @'
#!/bin/sh
exec "$P4KX_TEST_DEBIAN_PWSH" -NoLogo -NoProfile -File "$P4KX_TEST_DEBIAN_SCRIPT" "$@"
'@
    [IO.File]::WriteAllText($launcher, $source.Replace("`r`n", "`n") + "`n", [Text.UTF8Encoding]::new($false))
}
$variables = @('PATH', 'P4KX_TEST_DEBIAN_EXPECTED', 'P4KX_TEST_DEBIAN_RECEIPT', 'P4KX_TEST_DEBIAN_FAILURE', 'P4KX_TEST_DEBIAN_SCRIPT', 'P4KX_TEST_DEBIAN_PWSH')
$original = @{}
foreach ($name in $variables) { $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$nativeDirectory = [Environment]::CurrentDirectory
$previousExitCode = $global:LASTEXITCODE
Push-Location -LiteralPath $caller
try {
    [Environment]::CurrentDirectory = $root
    $env:PATH = $tools + [IO.Path]::PathSeparator + $env:PATH
    $env:P4KX_TEST_DEBIAN_EXPECTED = $expected
    $env:P4KX_TEST_DEBIAN_RECEIPT = Join-Path $root 'receipt.txt'
    $env:P4KX_TEST_DEBIAN_FAILURE = '0'
    $env:P4KX_TEST_DEBIAN_SCRIPT = $scanner
    $env:P4KX_TEST_DEBIAN_PWSH = [Environment]::ProcessPath
    if (-not $env:P4KX_TEST_DEBIAN_PWSH -or -not (Test-Path -LiteralPath $env:P4KX_TEST_DEBIAN_PWSH -PathType Leaf)) { throw 'The current PowerShell executable is unavailable.' }
    if ($IsLinux) { Invoke-P4kxLinuxTool '/usr/bin/chmod' @('0755', $launcher) | Out-Null }
    if ((Get-Command dpkg-shlibdeps).Source -cne $launcher) { throw 'Unqualified dependency scanner did not resolve to the concrete synthetic launcher.' }
    $dependencies = Get-P4kxDebianPackageDependencies -StagingDirectory 'package stage'
    if ($dependencies -notmatch 'libc6 \(>= 2\.39\)' -or $dependencies -notmatch 'libasound2t64 \(>= 1\.2\)' -or $PWD.ProviderPath -cne $caller) { throw 'Staged dependency result or caller location differs.' }
    $env:P4KX_TEST_DEBIAN_FAILURE = '1'
    $failure = $null
    try { Get-P4kxDebianPackageDependencies -StagingDirectory $stage | Out-Null } catch { $failure = $_.Exception.Message }
    if ($failure -cne 'Linux package tool failed (37): dpkg-shlibdeps' -or $PWD.ProviderPath -cne $caller) { throw 'Original scanner exit 37 was lost or changed the caller location.' }
    Remove-Item -LiteralPath $binaries[0]
    $rejected = $false
    try { Get-P4kxDebianPackageDependencies -StagingDirectory $stage | Out-Null } catch { $rejected = $true }
    if (-not $rejected -or @(Get-Content -LiteralPath $env:P4KX_TEST_DEBIAN_RECEIPT).Count -ne 2) { throw 'Missing staged payload was scanned or accepted.' }
}
finally {
    Pop-Location
    [Environment]::CurrentDirectory = $nativeDirectory
    $global:LASTEXITCODE = $previousExitCode
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
}
function Invoke-P4kxFailingToolFixture {
    $global:LASTEXITCODE = 37
    'synthetic package failure detail'
}
$messages = [Collections.Generic.List[object]]::new()
$failure = $null
$previousExitCode = $global:LASTEXITCODE
try {
    Invoke-P4kxLinuxTool Invoke-P4kxFailingToolFixture @() 6>&1 | ForEach-Object { $messages.Add($_) }
}
catch { $failure = $_.Exception.Message }
finally { $global:LASTEXITCODE = $previousExitCode }
if ($failure -cne 'Linux package tool failed (37): Invoke-P4kxFailingToolFixture' -or
    @($messages | Where-Object { $_.ToString() -ceq 'synthetic package failure detail' }).Count -ne 1) {
    throw 'Failed package tool stdout was discarded or its original exit code was lost.'
}
'PASS: actual scanner invocation uses absolute staged executables despite distinct PowerShell/.NET locations, preserves dependencies, propagates failure and refuses missing payloads.'
