param([string]$Directory, [ValidateSet('success','nonzero','inherit','hold')][string]$Fixture)
$ErrorActionPreference = 'Stop'

if ($Fixture -ceq 'hold') {
    [Console]::Error.Write('synthetic inherited stderr')
    [IO.File]::WriteAllText((Join-Path $Directory 'holder.pid'),[string]$PID)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.ElapsedMilliseconds -lt 17000 -and -not [IO.File]::Exists((Join-Path $Directory 'release'))) {
        Start-Sleep -Milliseconds 25
    }
    exit 0
}

function Invoke-SyntheticMcp {
    $methods = [Collections.Generic.List[string]]::new()
    while ($null -ne ($line = [Console]::ReadLine())) {
        $request = $line | ConvertFrom-Json
        $methods.Add($request.method)
        $reply = switch ($request.method) {
            initialize {
                if ($request.params.protocolVersion -cne '2025-11-25') { exit 8 }
                @{protocolVersion='2025-11-25'}
            }
            'notifications/initialized' {}
            'tools/list' { @{tools=@(@{name='read_entry_text'})} }
            'tools/call' {
                if ($request.params.name -cne 'read_entry_text' -or $request.params.arguments.path -cne 'Data/hello.txt') { exit 8 }
                @{structuredContent=@{text='synthetic Entry'}}
            }
            default { exit 8 }
        }
        if ($null -ne $reply) {
            [Console]::Out.WriteLine((@{jsonrpc='2.0'; id=$request.id; result=$reply} | ConvertTo-Json -Depth 8 -Compress))
        }
    }
    if (($methods -join '|') -cne 'initialize|notifications/initialized|tools/list|tools/call') { exit 8 }
    [Console]::Error.Write('synthetic MCP diagnostic')
}

function Start-InheritedStderrWriter {
    $info = [Diagnostics.ProcessStartInfo]::new([Environment]::ProcessPath)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    foreach ($argument in @('-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,
        '-Directory',$Directory,'-Fixture','hold')) { $info.ArgumentList.Add($argument) }
    $holder = [Diagnostics.Process]::Start($info)
    try {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists((Join-Path $Directory 'holder.pid'))) {
            if ($timer.ElapsedMilliseconds -ge 3000 -or $holder.HasExited) { exit 9 }
            Start-Sleep -Milliseconds 25
        }
        [IO.File]::WriteAllText((Join-Path $Directory 'parent.pid'),[string]$PID)
    }
    finally { $holder.Dispose() }
}

if ($Fixture) {
    if ($args.Count -ne 2 -or $args[0] -cne '--archive' -or $args[1] -cne 'synthetic-unused.p4k') { exit 8 }
    Invoke-SyntheticMcp
    if ($Fixture -ceq 'nonzero') { exit 37 }
    if ($Fixture -ceq 'inherit') { Start-InheritedStderrWriter }
    exit 0
}

if (-not $IsWindows) { throw 'The inherited MCP writer fixture requires Windows.' }
if (-not $Directory -or [IO.Directory]::Exists($Directory)) { throw 'Specify a fresh package MCP fixture directory.' }
[void][IO.Directory]::CreateDirectory($Directory)
. (Join-Path $PSScriptRoot 'package-mcp-smoke.ps1')

function Invoke-McpFixture([string]$Mode, [string]$Output) {
    [void][IO.Directory]::CreateDirectory($Output)
    $prefix = @('-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,
        '-Directory',$Output,'-Fixture',$Mode)
    Invoke-P4kxPackageMcpSmoke ([Environment]::ProcessPath) 'synthetic-unused.p4k' $Output 'synthetic Entry' $prefix
}

function Assert-ReadFixture([string]$Path) {
    $result = [IO.File]::ReadAllText((Join-Path $Path 'mcp-read.json')) | ConvertFrom-Json
    if ($result.structuredContent.text -cne 'synthetic Entry') { throw 'The MCP text receipt has different content.' }
}

foreach ($mode in @('success','nonzero')) {
    $path = Join-Path $Directory $mode
    $failure = $null
    try { Invoke-McpFixture $mode $path }
    catch { $failure = $_.Exception.Message }
    if (($mode -ceq 'success' -and $null -ne $failure) -or
        ($mode -ceq 'nonzero' -and $failure -cne 'MCP exited 37.')) { throw "Unexpected $mode MCP verdict: $failure" }
    Assert-ReadFixture $path
    if ([IO.File]::ReadAllText((Join-Path $path 'mcp-stderr.log')).TrimEnd() -cne 'synthetic MCP diagnostic') {
        throw 'The MCP stderr log differs.'
    }
}

$path = Join-Path $Directory 'inherit'
$failure = $null
$holder = $null
$parent = $null
try {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    try { Invoke-McpFixture 'inherit' $path }
    catch { $failure = $_.Exception.Message }
    if ($failure -cne 'MCP stderr did not reach EOF after stdin closed.' -or $clock.ElapsedMilliseconds -gt 16000) {
        throw "The inherited MCP writer was not bounded: $failure"
    }
    Assert-ReadFixture $path
    $holder = Get-Process -Id ([int][IO.File]::ReadAllText((Join-Path $path 'holder.pid'))) -ErrorAction SilentlyContinue
    $parent = Get-Process -Id ([int][IO.File]::ReadAllText((Join-Path $path 'parent.pid'))) -ErrorAction SilentlyContinue
    if ($null -eq $holder -or $holder.HasExited -or ($null -ne $parent -and -not $parent.HasExited)) {
        throw 'The test must retain a live descendant writer after the MCP parent exited.'
    }
}
finally {
    [IO.File]::WriteAllText((Join-Path $path 'release'),'')
    if ($null -ne $parent) { $parent.Dispose() }
    if ($null -ne $holder) {
        try {
            if (-not $holder.WaitForExit(3000)) { throw 'The fixture writer did not stop after release.' }
        }
        finally { $holder.Dispose() }
    }
}
'PASS: MCP negotiation, reads, dedicated stderr log, exit 37 and inherited stderr at the shutdown deadline.'
