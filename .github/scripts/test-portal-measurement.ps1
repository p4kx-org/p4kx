<#
.SYNOPSIS
Checks bounded portal measurement and API parsing without connecting to any service.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-measurement.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Portal measurement test directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$xml4 = '<node><interface name="org.freedesktop.portal.Documents"><property name="version" type="u" access="read"/></interface></node>'
$xml5 = '<node><interface name="org.freedesktop.portal.Documents"><method name="GetHostPaths"><arg direction="in" type="as"/><arg direction="out" type="a{say}"/></method></interface></node>'
$v4 = ConvertFrom-P4kxDocumentsInterface "u 4`n" $xml4
$v5 = ConvertFrom-P4kxDocumentsInterface 'u 5' $xml5
if ($v4.interface_version -ne 4 -or $v4.get_host_paths -or $v5.interface_version -ne 5 -or -not $v5.get_host_paths) { throw 'Observed interface capabilities were misreported.' }
$withDtd = '<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "https://invalid.example/not-fetched">' + $xml5
if (-not (ConvertFrom-P4kxDocumentsInterface 'u 5' $withDtd).get_host_paths) { throw 'A standard introspection DOCTYPE was not safely ignored.' }
$interfaceBody = $xml4 -replace '<node>|</node>', ''
$duplicateInterfaces = '<node>' + $interfaceBody + $interfaceBody + '</node>'
foreach ($case in @(
    @('s "5"', $xml5), @('u 4294967296', $xml5), @('u 5', '<node/>'),
    @('u 5', ($xml5 -replace 'a\{say\}', 'a{ss}')), @('u 5', ($xml5 -replace '</interface>', '<method name="GetHostPaths"/></interface>')),
    @('u 5', ($xml5 -replace '</method>', '<arg type="s"/></method>')),
    @('u 5', ($xml5 -replace '</method>', '<arg direction="inout" type="s"/></method>')),
    @('u 5', $duplicateInterfaces),
    @('u 5', ('x' * 131073)), @('u 5', '<!DOCTYPE node [<!ENTITY leak SYSTEM "file:///not-a-real-private-path">]><node>&leak;</node>')
)) {
    $rejected = $false
    try { ConvertFrom-P4kxDocumentsInterface $case[0] $case[1] | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Malformed, ambiguous or oversized Documents evidence was accepted.' }
}
$mock = Join-Path $Directory 'probe.ps1'
@'
param([string]$Mode)
[IO.File]::WriteAllText((Join-Path $PSScriptRoot "$Mode.pid"), [string]$PID)
switch ($Mode) {
    success { [Console]::Out.Write('u 5'); [Console]::Error.Write('synthetic diagnostic') }
    failure { [Console]::Error.Write('synthetic private path /not-a-real-user/file'); exit 37 }
    excessive { [Console]::Out.Write('x' * 20000) }
    silent { Start-Sleep -Seconds 20 }
}
'@ | Set-Content -LiteralPath $mock -Encoding utf8NoBOM
$executable = (Get-Process -Id $PID).Path
$result = Invoke-P4kxPortalMeasurement $executable @('-NoProfile', '-File', $mock, 'success') $Directory 'probe-success'
if ($result -cne 'u 5') { throw 'A real probe did not preserve stdout separately from stderr.' }
foreach ($case in @(@('failure', 'exited 37'), @('excessive', 'output limit'), @('silent', 'timed out'))) {
    $failure = $null
    try { Invoke-P4kxPortalMeasurement $executable @('-NoProfile', '-File', $mock, $case[0]) $Directory "probe-$($case[0])" -Milliseconds 2000 -MaximumBytes 4096 | Out-Null }
    catch { $failure = $_.Exception.Message }
    if (-not $failure -or $failure -notlike "*$($case[1])*" -or $failure -like '*not-a-real-user*') { throw 'Probe failure lost its bounded classification or disclosed private output.' }
    $child = [int](Get-Content -LiteralPath (Join-Path $Directory "$($case[0]).pid") -Raw)
    if (Get-Process -Id $child -ErrorAction SilentlyContinue) { throw 'A failed or timed-out probe left its child running.' }
}
'PASS: interface 4/5, exact two-argument signature, safe XML, ten malformed inputs, real stdout/stderr, exit 37, output cap and joined timeout; no portal or service started.'
