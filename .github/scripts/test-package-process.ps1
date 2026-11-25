param(
    [Parameter(Mandatory)][string]$Directory,
    [ValidateSet('success','failure','hang','inherit','hold','gui')][string]$Fixture
)
$ErrorActionPreference = 'Stop'

if ($Fixture -ceq 'success' -or $Fixture -ceq 'failure') {
    [Console]::Out.Write('synthetic stdout')
    [Console]::Error.Write('synthetic stderr')
    if ($Fixture -ceq 'failure') { exit 37 }
    exit 0
}
if ($Fixture -ceq 'gui') {
    if ($env:P4KX_SMOKE_FRAMES -cne '3' -or $env:WGPU_BACKEND -cne 'metal') { exit 9 }
    [Console]::Out.Write('synthetic frames completed')
    [Console]::Error.Write('synthetic GUI diagnostic')
    exit 0
}
if ($Fixture -ceq 'hang') {
    [IO.File]::WriteAllText((Join-Path $Directory 'hang.pid'),[string]$PID)
    Start-Sleep -Seconds 12
    exit 9
}
if ($Fixture -ceq 'hold') {
    [Console]::Out.Write('inherited stdout')
    [Console]::Error.Write('inherited stderr')
    [IO.File]::WriteAllText((Join-Path $Directory 'holder.pid'),[string]$PID)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.ElapsedMilliseconds -lt 12000 -and -not [IO.File]::Exists((Join-Path $Directory 'release'))) {
        Start-Sleep -Milliseconds 25
    }
    exit 0
}
if ($Fixture -ceq 'inherit') {
    $start = [Diagnostics.ProcessStartInfo]::new([Environment]::ProcessPath)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    foreach ($argument in @('-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,
        '-Directory',$Directory,'-Fixture','hold')) { $start.ArgumentList.Add($argument) }
    $holder = [Diagnostics.Process]::Start($start)
    try {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists((Join-Path $Directory 'holder.pid'))) {
            if ($clock.ElapsedMilliseconds -ge 3000 -or $holder.HasExited) { exit 9 }
            Start-Sleep -Milliseconds 25
        }
        [Console]::Out.Write('parent is returning')
        [IO.File]::WriteAllText((Join-Path $Directory 'parent.pid'),[string]$PID)
        exit 0
    }
    finally { $holder.Dispose() }
}

if (-not $IsWindows) { throw 'The inherited writer fixture requires Windows.' }
if ([IO.Directory]::Exists($Directory)) { throw 'Use a fresh package process fixture directory.' }
[void][IO.Directory]::CreateDirectory($Directory)
. (Join-Path $PSScriptRoot 'package-process.ps1')

function Invoke-Fixture([string]$Mode, [int]$Seconds) {
    $arguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,
        '-Directory',$Directory,'-Fixture',$Mode)
    Invoke-P4kxPackageProcess ([Environment]::ProcessPath) $arguments $Directory (Join-Path $Directory "$Mode.log") -Seconds $Seconds
}

function Get-FixtureProcess([string]$Name) {
    $path = Join-Path $Directory "$Name.pid"
    if (-not [IO.File]::Exists($path)) { throw "The fixture did not start its $Name process." }
    $processId = [int][IO.File]::ReadAllText($path)
    Get-Process -Id $processId -ErrorAction SilentlyContinue
}

$stdout = Invoke-Fixture success 4
if ($stdout -cne 'synthetic stdout' -or
    [IO.File]::ReadAllText((Join-Path $Directory 'success.log')).TrimEnd() -cne 'synthetic stdoutsynthetic stderr') {
    throw 'The helper did not preserve distinct stdout and the combined UTF-8 log.'
}

$failed = $null
try { Invoke-Fixture failure 4 | Out-Null }
catch { $failed = $_.Exception.Message }
if ($failed -cnotlike 'Packaged application exited 37:*' -or
    [IO.File]::ReadAllText((Join-Path $Directory 'failure.log')).TrimEnd() -cne 'synthetic stdoutsynthetic stderr') {
    throw 'Nonzero exit lost the actual exit code or combined log.'
}

$failed = $null
try { Invoke-Fixture hang 3 | Out-Null }
catch { $failed = $_.Exception.Message }
$owned = Get-FixtureProcess 'hang'
try {
    if ($failed -cnotlike 'Packaged application timed out:*' -or ($null -ne $owned -and -not $owned.HasExited)) {
        throw 'The directly owned process was not stopped after its runtime deadline.'
    }
}
finally { if ($null -ne $owned) { $owned.Dispose() } }

$failed = $null
$holder = $null
$parent = $null
try {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    try { Invoke-Fixture inherit 4 | Out-Null }
    catch { $failed = $_.Exception.Message }
    if ($failed -cnotlike 'Packaged application timed out:*stdout/stderr did not reach EOF' -or
        $clock.ElapsedMilliseconds -gt 7500) {
        throw "An inherited writer exceeded the output deadline or was classified incorrectly: $failed"
    }
    $holder = Get-FixtureProcess 'holder'
    $parent = Get-FixtureProcess 'parent'
    if ($null -eq $holder -or $holder.HasExited -or ($null -ne $parent -and -not $parent.HasExited)) {
        throw 'The fixture must retain a live writer after the packaged parent exited.'
    }
}
finally {
    [IO.File]::WriteAllText((Join-Path $Directory 'release'),'')
    if ($null -ne $parent) { $parent.Dispose() }
    if ($null -ne $holder) {
        try {
            if (-not $holder.WaitForExit(3000)) { throw 'The fixture writer did not stop after its release.' }
        }
        finally { $holder.Dispose() }
    }
}
$guiArguments = @('-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,
    '-Directory',$Directory,'-Fixture','gui')
Invoke-P4kxPackageGuiSmoke ([Environment]::ProcessPath) $guiArguments $Directory 'macos'
if ([IO.File]::ReadAllText((Join-Path $Directory 'gui.log')).TrimEnd() -cne 'synthetic frames completedsynthetic GUI diagnostic') {
    throw 'The GUI wrapper did not retain its configured environment and combined log.'
}
'PASS: stdout/logs, exit 37, owned-process and inherited-writer timeouts, and GUI environment/logs.'
