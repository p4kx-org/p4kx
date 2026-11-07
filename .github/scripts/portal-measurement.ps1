$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'package-process.ps1')

function Invoke-P4kxPortalMeasurement {
    <#
    .SYNOPSIS
    Captures one portal probe with bounded output and an independent process deadline.
    #>
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [int]$Milliseconds = 5000, [int]$MaximumBytes = 131072)
    if ($Milliseconds -le 0 -or $MaximumBytes -le 0 -or $Name -cnotmatch '^[a-z0-9-]+$') { throw 'Invalid portal measurement bounds or name.' }
    $process = New-P4kxPackageProcess $Executable $Arguments $Directory
    $channels = @()
    $started = $false
    try {
        [void]$process.Start()
        $started = $true
        $process.StandardInput.Close()
        $timer = [Diagnostics.Stopwatch]::StartNew()
        foreach ($reader in @($process.StandardOutput, $process.StandardError)) {
            $buffer = [char[]]::new(1024)
            $channels += @{reader=$reader; buffer=$buffer; pending=$reader.ReadAsync($buffer, 0, $buffer.Length); text=[Text.StringBuilder]::new()}
        }
        $bytes = 0
        while (@($channels | Where-Object { $null -ne $_.pending }).Count -ne 0) {
            if ($timer.ElapsedMilliseconds -ge $Milliseconds) { throw 'Portal measurement timed out.' }
            foreach ($channel in $channels) {
                if ($null -eq $channel.pending -or -not $channel.pending.IsCompleted) { continue }
                $count = $channel.pending.GetAwaiter().GetResult()
                if ($count -eq 0) { $channel.pending = $null; continue }
                $chunk = [string]::new($channel.buffer, 0, $count)
                $bytes += [Text.Encoding]::UTF8.GetByteCount($chunk)
                if ($bytes -gt $MaximumBytes) { throw 'Portal measurement output limit exceeded.' }
                [void]$channel.text.Append($chunk)
                $channel.pending = $channel.reader.ReadAsync($channel.buffer, 0, $channel.buffer.Length)
            }
            $pending = [Threading.Tasks.Task[]]@($channels | Where-Object { $null -ne $_.pending } | ForEach-Object { $_.pending })
            if ($pending.Count -ne 0) { [void][Threading.Tasks.Task]::WaitAny($pending, 10) }
        }
        $remaining = [Math]::Max(1, $Milliseconds - [int]$timer.ElapsedMilliseconds)
        if (-not $process.WaitForExit($remaining)) { throw 'Portal measurement timed out.' }
        if ($process.ExitCode -ne 0) { throw "Portal measurement exited $($process.ExitCode); see its bounded artifact logs." }
        return $channels[0].text.ToString()
    }
    finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        for ($index = 0; $index -lt $channels.Count; $index++) {
            $stream = if ($index -eq 0) { 'stdout' } else { 'stderr' }
            $channels[$index].text.ToString() | Set-Content -LiteralPath (Join-Path $Directory "$Name-$stream.log") -Encoding utf8NoBOM
        }
        $process.Dispose()
    }
}

function ConvertFrom-P4kxDocumentsInterface {
    <#
    .SYNOPSIS
    Parses the observed Documents version and exact GetHostPaths signature.
    #>
    param([string]$Version, [string]$Introspection)
    if ($Version.Length -gt 64 -or [Text.Encoding]::UTF8.GetByteCount($Introspection) -gt 131072) { throw 'Documents interface evidence exceeds its limit.' }
    if ($Version -cnotmatch '^u ([0-9]+)\s*$') { throw 'Documents version reply is not one uint32 property.' }
    $number = [uint32]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    $settings = [Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [Xml.DtdProcessing]::Ignore
    $settings.XmlResolver = $null
    $settings.MaxCharactersInDocument = 131072
    $text = [IO.StringReader]::new($Introspection)
    $reader = [Xml.XmlReader]::Create($text, $settings)
    try {
        $document = [Xml.XmlDocument]::new()
        $document.XmlResolver = $null
        $document.Load($reader)
        $interfaces = @($document.SelectNodes('/node/interface[@name="org.freedesktop.portal.Documents"]'))
        if ($interfaces.Count -ne 1) { throw 'Documents introspection has no unambiguous interface.' }
        $methods = @($interfaces[0].SelectNodes('method[@name="GetHostPaths"]'))
        if ($methods.Count -gt 1) { throw 'Documents introspection has duplicate GetHostPaths methods.' }
        $available = $methods.Count -eq 1
        if ($available) {
            $arguments = @($methods[0].SelectNodes('arg'))
            $inputs = @($methods[0].SelectNodes('arg[@direction="in"]'))
            $outputs = @($methods[0].SelectNodes('arg[@direction="out"]'))
            if ($arguments.Count -ne 2 -or $inputs.Count -ne 1 -or $outputs.Count -ne 1 -or $inputs[0].type -cne 'as' -or $outputs[0].type -cne 'a{say}') {
                throw 'Documents GetHostPaths introspection has an unexpected signature.'
            }
        }
        return @{interface_version=$number; get_host_paths=$available}
    }
    finally { $reader.Dispose(); $text.Dispose() }
}
