$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'package-process.ps1')

function New-P4kxPortalChannels {
    param([Diagnostics.Process]$Process)
    $channels = @()
    foreach ($reader in @($Process.StandardOutput, $Process.StandardError)) {
        $buffer = [char[]]::new(1024)
        $channels += @{reader=$reader; buffer=$buffer; pending=$reader.ReadAsync($buffer, 0, $buffer.Length); text=[Text.StringBuilder]::new(); bytes=0}
    }
    return ,$channels
}

function Update-P4kxPortalChannels {
    param([object[]]$Channels, [int]$MaximumBytes)
    $bytes = [long]0
    foreach ($channel in $Channels) { $bytes += $channel.bytes }
    foreach ($channel in $Channels) {
        if ($null -eq $channel.pending -or -not $channel.pending.IsCompleted) { continue }
        $count = $channel.pending.GetAwaiter().GetResult()
        if ($count -eq 0) { $channel.pending = $null; continue }
        $chunk = [string]::new($channel.buffer, 0, $count)
        $size = [Text.Encoding]::UTF8.GetByteCount($chunk)
        $bytes += $size
        if ($bytes -gt $MaximumBytes) { throw 'Portal measurement output limit exceeded.' }
        [void]$channel.text.Append($chunk)
        $channel.bytes += $size
        $channel.pending = $channel.reader.ReadAsync($channel.buffer, 0, $channel.buffer.Length)
    }
}

function Wait-P4kxPortalChannels {
    param([object[]]$Channels)
    $pending = [Threading.Tasks.Task[]]@($Channels | Where-Object { $null -ne $_.pending } | ForEach-Object { $_.pending })
    if ($pending.Count -ne 0) { [void][Threading.Tasks.Task]::WaitAny($pending, 10) }
    else { Start-Sleep -Milliseconds 10 }
}

function Receive-P4kxPortalChannels {
    param([object[]]$Channels, [Diagnostics.Stopwatch]$Timer, [int]$Milliseconds, [int]$MaximumBytes)
    while (@($Channels | Where-Object { $null -ne $_.pending }).Count -ne 0) {
        if ($Timer.ElapsedMilliseconds -ge $Milliseconds) { throw 'Portal measurement timed out.' }
        Update-P4kxPortalChannels $Channels $MaximumBytes
        Wait-P4kxPortalChannels $Channels
    }
}
function Close-P4kxPortalProcess {
    param([Diagnostics.Process]$Process, [bool]$Started, [object[]]$Channels, [string]$Directory, [string]$Name,
        [Collections.Generic.List[Exception]]$Failures, [hashtable]$Observation = @{})
    try {
        $Observation['started'] = $Started
        $Observation['signal_contract'] = 'direct-process-only'
        if ($Started) {
            $Observation['pid'] = $Process.Id
            if (-not $Process.HasExited) {
                $Process.Kill($false)
                if (-not $Process.WaitForExit(5000)) { throw 'Portal child did not terminate after its watchdog.' }
            }
            $Observation['exited'] = $Process.HasExited
            if ($Process.HasExited) { $Observation['exit_code'] = $Process.ExitCode }
        }
    }
    catch { $Failures.Add($_.Exception) }
    for ($index = 0; $index -lt $Channels.Count; $index++) {
        try {
            $stream = if ($index -eq 0) { 'stdout' } else { 'stderr' }
            $Channels[$index].text.ToString() | Set-Content -LiteralPath (Join-Path $Directory "$Name-$stream.log") -Encoding utf8NoBOM
        }
        catch { $Failures.Add($_.Exception) }
    }
    try { $Process.Dispose() } catch { $Failures.Add($_.Exception) }
}

function Invoke-P4kxPortalProcess {
    <#
    .SYNOPSIS
    Captures bounded stdout, stderr and actual exit status without classifying it as success.
    #>
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [int]$Milliseconds = 5000, [int]$MaximumBytes = 131072, [hashtable]$Environment = @{})
    if ($Milliseconds -le 0 -or $MaximumBytes -le 0 -or $Name -cnotmatch '^[a-z0-9-]+$') { throw 'Invalid portal measurement bounds or name.' }
    $process = New-P4kxPackageProcess $Executable $Arguments $Directory
    foreach ($entry in $Environment.GetEnumerator()) { $process.StartInfo.Environment[$entry.Key] = [string]$entry.Value }
    $channels = @()
    $started = $false
    $result = $null
    $failures = [Collections.Generic.List[Exception]]::new()
    try {
        [void]$process.Start()
        $started = $true
        $process.StandardInput.Close()
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $channels = New-P4kxPortalChannels $process
        Receive-P4kxPortalChannels $channels $timer $Milliseconds $MaximumBytes
        $remaining = [Math]::Max(1, $Milliseconds - [int]$timer.ElapsedMilliseconds)
        if (-not $process.WaitForExit($remaining)) { throw 'Portal measurement timed out.' }
        $result = @{exit_code=$process.ExitCode; stdout=$channels[0].text.ToString(); stderr=$channels[1].text.ToString()}
    }
    catch { $failures.Add($_.Exception) }
    finally { Close-P4kxPortalProcess $process $started $channels $Directory $Name $failures }
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Portal measurement or cleanup failed.', $failures) }
    return $result
}
