$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'package-process.ps1')

function Send-P4kxMcpMessage {
    param($Process, $Message)
    $Process.StandardInput.WriteLine(($Message | ConvertTo-Json -Depth 12 -Compress))
    $Process.StandardInput.Flush()
}

function Receive-P4kxMcpResponse {
    param($Process, [int]$Id)
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        $line = $Process.StandardOutput.ReadLineAsync()
        $remaining = [Math]::Max(1, [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds)
        if (-not $line.Wait($remaining)) { throw "MCP response $Id timed out." }
        $json = $line.GetAwaiter().GetResult()
        if ($null -eq $json) { throw 'MCP closed stdout before responding.' }
        $message = $json | ConvertFrom-Json
        if ($message.id -eq $Id) {
            if ($message.error -or $message.result.isError) { throw "MCP request failed: $json" }
            return $message.result
        }
    }
    throw "MCP response $Id timed out."
}

function Invoke-P4kxPackageMcpSmoke {
    param([string]$Executable, [string]$Archive, [string]$Directory, [string]$ExpectedText, [string[]]$ArgumentPrefix = @())
    $process = New-P4kxPackageProcess -Executable $Executable -Arguments ($ArgumentPrefix + @('--archive', $Archive)) -Directory $Directory
    try {
        [void]$process.Start()
        $stderr = $process.StandardError.ReadToEndAsync()
        Send-P4kxMcpMessage $process @{jsonrpc='2.0'; id=1; method='initialize'; params=@{
            protocolVersion='2025-11-25'; capabilities=@{}; clientInfo=@{name='p4kx-package-smoke'; version='1.0.0'}
        }}
        $init = Receive-P4kxMcpResponse $process 1
        if ($init.protocolVersion -cne '2025-11-25') { throw 'MCP protocol negotiation failed.' }
        Send-P4kxMcpMessage $process @{jsonrpc='2.0'; method='notifications/initialized'}
        Send-P4kxMcpMessage $process @{jsonrpc='2.0'; id=2; method='tools/list'; params=@{}}
        $tools = Receive-P4kxMcpResponse $process 2
        if ('read_entry_text' -cnotin $tools.tools.name) { throw 'MCP did not list read_entry_text.' }
        Send-P4kxMcpMessage $process @{jsonrpc='2.0'; id=3; method='tools/call'; params=@{
            name='read_entry_text'; arguments=@{path='Data/hello.txt'}
        }}
        $read = Receive-P4kxMcpResponse $process 3
        if (-not [String]::Equals($read.structuredContent.text, $ExpectedText, [StringComparison]::Ordinal)) { throw 'MCP synthetic Entry text differs.' }
        $read | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $Directory 'mcp-read.json') -Encoding utf8NoBOM
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(10000)) { $process.Kill($true); $process.WaitForExit(); throw 'MCP did not exit after stdin closed.' }
        $stderr.GetAwaiter().GetResult() | Set-Content (Join-Path $Directory 'mcp-stderr.log') -Encoding utf8NoBOM
        if ($process.ExitCode -ne 0) { throw "MCP exited $($process.ExitCode)." }
    }
    finally {
        if ($process.Id -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
