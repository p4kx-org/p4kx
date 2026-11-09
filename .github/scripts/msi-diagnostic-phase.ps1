$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-utf8.ps1')

function New-P4kxMsiPhaseContext {
    param([string]$Directory, [string]$Target)
    if ($Target -cnotin @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')) { throw 'Invalid MSI diagnostic target.' }
    $journal = Join-Path $Directory 'phase-journal.jsonl'
    $stream = [IO.File]::Open($journal, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $stream.Dispose()
    return @{journal=$journal; target=$Target; timer=[Diagnostics.Stopwatch]::StartNew(); sequence=0; next_id=0; bytes=0; failures=[Collections.Generic.List[Exception]]::new(); active=[Collections.Generic.List[hashtable]]::new()}
}

function Write-P4kxMsiPhaseEvent {
    param([hashtable]$Context, [hashtable]$Token, [ValidateSet('begin','end','error')][string]$Status)
    $maximumEvents = 2048
    $maximumEventBytes = 1024
    $maximumJournalBytes = 1MB
    if ($Context.sequence -ge $maximumEvents) { throw 'MSI phase event limit exceeded.' }
    $phaseEvent = [ordered]@{
        schema=1; sequence=($Context.sequence+1); id=$Token.id; parent=$Token.parent
        phase=$Token.phase; status=$Status; utc_ms=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        elapsed_ms=$Context.timer.ElapsedMilliseconds; target=$Context.target
        process_arch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
    }
    $line = $phaseEvent | ConvertTo-Json -Compress
    $bytes = [Text.UTF8Encoding]::new($false,$true).GetBytes($line+[char]10)
    if ($bytes.Length -gt $maximumEventBytes -or $Context.bytes+$bytes.Length -gt $maximumJournalBytes) { throw 'MSI phase journal byte limit exceeded.' }
    $stream = [IO.File]::Open($Context.journal, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try { $stream.Write($bytes); $stream.Flush($true) } finally { $stream.Dispose() }
    $Context.bytes += $bytes.Length
    $Context.sequence++
    [Console]::Out.WriteLine('[P4KX-MSI-PHASE]'+$line)
    [Console]::Out.Flush()
}

function Start-P4kxMsiPhase {
    param([hashtable]$Context, [string]$Name, [switch]$RequiredCleanup)
    if ($Name -cnotmatch '\A[a-z0-9]+(?:[/-][a-z0-9]+)*\z') { throw 'Invalid fixed MSI phase name.' }
    $Context.next_id++
    $parent = if ($Context.active.Count) { $Context.active[$Context.active.Count-1].id } else { 0 }
    $token = @{id=$Context.next_id; parent=$parent; phase=$Name}
    try { Write-P4kxMsiPhaseEvent $Context $token begin } catch {
        if (-not $RequiredCleanup) { throw }
        $Context.failures.Add($_.Exception)
        return $null
    }
    $Context.active.Add($token)
    return $token
}

function Stop-P4kxMsiPhase {
    param([hashtable]$Context, [hashtable]$Token, [Exception]$Failure=$null, [switch]$RequiredCleanup)
    if ($null -eq $Token) { return }
    try {
        Write-P4kxMsiPhaseEvent $Context $Token $(if ($null -eq $Failure) { 'end' } else { 'error' })
        [void]$Context.active.Remove($Token)
    } catch {
        if ($RequiredCleanup) { $Context.failures.Add($_.Exception); return }
        if ($null -ne $Failure) { throw [AggregateException]::new('MSI operation and diagnostic persistence failed.', [Exception[]]@($Failure,$_.Exception)) }
        throw
    }
}

function Get-P4kxMsiDiagnosticFailure {
    param([hashtable]$Context, [Exception]$Primary=$null)
    if ($Context.failures.Count -eq 0) { return $Primary }
    $failures=[Collections.Generic.List[Exception]]::new()
    if ($null -ne $Primary) { $failures.Add($Primary) }
    foreach ($failure in $Context.failures) { $failures.Add($failure) }
    $Context.failures.Clear()
    return [AggregateException]::new('MSI verification includes diagnostic persistence failures.', $failures)
}

function Assert-P4kxMsiPhaseEvent {
    param([hashtable]$phaseEvent)
    $fields = @('schema','sequence','id','parent','phase','status','utc_ms','elapsed_ms','target','process_arch')
    if ($phaseEvent.Count -ne $fields.Count -or @($phaseEvent.Keys | Where-Object { $_ -cnotin $fields }).Count -ne 0 -or
        $phaseEvent.schema -ne 1 -or ($phaseEvent.sequence -isnot [long] -and $phaseEvent.sequence -isnot [int]) -or
        ($phaseEvent.id -isnot [long] -and $phaseEvent.id -isnot [int]) -or ($phaseEvent.parent -isnot [long] -and $phaseEvent.parent -isnot [int]) -or ($phaseEvent.elapsed_ms -isnot [long] -and $phaseEvent.elapsed_ms -isnot [int]) -or $phaseEvent.sequence -le 0 -or $phaseEvent.id -le 0 -or $phaseEvent.parent -lt 0 -or $phaseEvent.elapsed_ms -lt 0 -or
        $phaseEvent.phase -cnotmatch '\A[a-z0-9]+(?:[/-][a-z0-9]+)*\z' -or $phaseEvent.status -cnotin @('begin','end','error') -or
        $phaseEvent.target -cnotin @('x86_64-pc-windows-msvc','aarch64-pc-windows-msvc') -or
        $phaseEvent.process_arch -cnotin @('X64','Arm64')) { throw 'Invalid bounded MSI phase event.' }
    if (($phaseEvent.utc_ms -isnot [long] -and $phaseEvent.utc_ms -isnot [int]) -or $phaseEvent.utc_ms -le 0) { throw 'Invalid MSI phase UTC timestamp.' }
}

function Read-P4kxMsiPhaseJournal {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return @{present=$false; complete=$false; events=0; active=@(); terminal=$null} }
    $text = Read-P4kxMsiDiagnosticUtf8 $Path 1MB 'MSI phase journal exceeds its byte bound.'
    $lines = $text.Split([char]10)
    $complete = $text.EndsWith([string][char]10, [StringComparison]::Ordinal)
    $count = 0; $active = @{}; $terminal = $null
    for ($index=0; $index -lt $lines.Length-1; $index++) {
        if ($count -ge 2048 -or [Text.Encoding]::UTF8.GetByteCount($lines[$index])+1 -gt 1024) { throw 'MSI phase event bound exceeded.' }
        $phaseEvent = $lines[$index] | ConvertFrom-Json -AsHashtable -Depth 4
        Assert-P4kxMsiPhaseEvent $phaseEvent
        $count++
        if ($phaseEvent.sequence -ne $count) { throw 'MSI phase sequence is discontinuous.' }
        if ($phaseEvent.status -ceq 'begin') {
            if ($active.ContainsKey([string]$phaseEvent.id) -or ($phaseEvent.parent -ne 0 -and -not $active.ContainsKey([string]$phaseEvent.parent))) { throw 'MSI phase parent or identity differs.' }
            $active[[string]$phaseEvent.id] = $phaseEvent.phase
        } else {
            if (-not $active.ContainsKey([string]$phaseEvent.id) -or $active[[string]$phaseEvent.id] -cne $phaseEvent.phase) { throw 'MSI phase completion has no matching begin.' }
            $active.Remove([string]$phaseEvent.id)
            if ($phaseEvent.phase -ceq 'worker') { $terminal=$phaseEvent.status }
        }
    }
    return @{present=$true; complete=$complete; events=$count; active=@($active.Values); terminal=$terminal}
}

function New-P4kxMsiPhaseForwarder {
    $validate = (Get-Item 'Function:Assert-P4kxMsiPhaseEvent').ScriptBlock
    $state = @{pending=''; sequence=0; discard=$false}
    return {
        param([string]$Stream, [string]$Chunk)
        if ($Stream -cne 'stdout') { return }
        $state.pending += $Chunk
        while (($newline=$state.pending.IndexOf([char]10)) -ge 0) {
            $line=$state.pending.Substring(0,$newline).TrimEnd([char]13)
            $state.pending=$state.pending.Substring($newline+1)
            if ($state.discard) { $state.discard=$false; continue }
            if (-not $line.StartsWith('[P4KX-MSI-PHASE]', [StringComparison]::Ordinal)) { continue }
            if ([Text.Encoding]::UTF8.GetByteCount($line) -gt 1040) { throw 'Streamed MSI phase exceeds its bound.' }
            $phaseEvent=$line.Substring('[P4KX-MSI-PHASE]'.Length) | ConvertFrom-Json -AsHashtable -Depth 4
            & $validate $phaseEvent
            if ($phaseEvent.sequence -ne $state.sequence+1) { throw 'Streamed MSI phase sequence differs.' }
            $state.sequence++
            [Console]::Out.WriteLine($line)
            [Console]::Out.Flush()
        }
        if ($state.pending.Length -gt 4096) {
            if ($state.pending.StartsWith('[P4KX-MSI-PHASE]', [StringComparison]::Ordinal)) { throw 'MSI diagnostic phase line exceeds its bound.' }
            $state.pending=''; $state.discard=$true
        }
    }.GetNewClosure()
}
