$ErrorActionPreference='Stop'

function New-P4kxMsiSnapshotConsole {
    param([ValidateRange(1,9)][int]$Sequence)
    return @{sequence=$Sequence;revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        target=$env:P4KX_TARGET;pending='';bytes=0;seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);
        validate=(Get-Item 'Function:Read-P4kxMsiSnapshotConsoleRecord').ScriptBlock;
        write=(Get-Item 'Function:Write-P4kxMsiSnapshotConsoleMarker').ScriptBlock}
}

function Write-P4kxMsiSnapshotConsoleMarker {
    param([hashtable]$State, [string]$Stage, [ValidateSet('begin','end')][string]$Status)
    $stages=@('caller','callerReturn','portalStart','portalInputClose','portalCapture','portalCleanup',
        'worker.readRequest','worker.binding','worker.readHandle','worker.parentState','worker.wait',
        'worker.stopExpiredParent','worker.saveCheckpoint','worker.workflowOutput')
    if ($Stage -cnotin $stages) { throw 'Snapshot console stage is not predefined.' }
    if (-not $State.seen.Add($Stage+'.'+$Status)) { return }
    $marker=@{schema=1;role='native-msi-snapshot-console';sequence=$State.sequence;stage=$Stage;status=$Status;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    Write-Host ('[P4KX-MSI-SNAPSHOT]'+($marker | ConvertTo-Json -Compress))
}

function Read-P4kxMsiSnapshotConsoleRecord {
    param([hashtable]$State, [string]$Line)
    $prefix='[P4KX-MSI-OBSERVER]'
    if (-not $Line.StartsWith($prefix,[StringComparison]::Ordinal)) { return $null }
    try { $row=$Line.Substring($prefix.Length) | ConvertFrom-Json -AsHashtable -Depth 4 }
    catch { return $null }
    $fields=@('schema','role','stage','status','revision','run','attempt','target','observed_utc','request_sha256',
        'binding_unproven','completed_success','native_completion_unproven','mutation_authority')
    $stages=@('readRequest','binding','readHandle','parentState','wait','stopExpiredParent','saveCheckpoint','workflowOutput')
    if ($row -isnot [Collections.IDictionary] -or $row.Count -ne $fields.Count -or
        @($row.Keys | Where-Object { $_ -cnotin $fields }).Count -ne 0 -or
        $row.schema -isnot [long] -and $row.schema -isnot [int]) { return $null }
    if ($row.schema -ne 1 -or $row.role -isnot [string] -or $row.role -cne 'native-msi-observer-progress' -or
        $row.stage -isnot [string] -or $row.stage -cnotin $stages -or
        $row.status -isnot [string] -or $row.status -cnotin @('begin','end')) { return $null }
    foreach ($field in @('revision','run','attempt','target')) {
        if ($row[$field] -isnot [string] -or [string]::IsNullOrEmpty($State[$field]) -or $row[$field] -cne $State[$field]) { return $null }
    }
    foreach ($field in @('binding_unproven','native_completion_unproven')) {
        if ($row[$field] -isnot [bool] -or $row[$field] -ne $true) { return $null }
    }
    foreach ($field in @('completed_success','mutation_authority')) {
        if ($row[$field] -isnot [bool] -or $row[$field] -ne $false) { return $null }
    }
    $observed=[DateTimeOffset]::MinValue
    if ($row.observed_utc -isnot [DateTime] -and $row.observed_utc -isnot [DateTimeOffset] -and
        ($row.observed_utc -isnot [string] -or -not [DateTimeOffset]::TryParseExact($row.observed_utc,'o',
        [Globalization.CultureInfo]::InvariantCulture,[Globalization.DateTimeStyles]::None,[ref]$observed))) { return $null }
    if ($null -ne $row.request_sha256 -and ($row.request_sha256 -isnot [string] -or
        $row.request_sha256 -cnotmatch '\A[0-9a-fA-F]{64}\z')) { return $null }
    return $row
}

function Receive-P4kxMsiSnapshotConsole {
    param([hashtable]$State, [string]$Stream, [string]$Chunk)
    if ($Stream -ceq 'stage') {
        $stages=@('portalStart.begin','portalStart.end','portalInputClose.begin','portalInputClose.end',
            'portalCapture.begin','portalCapture.end','portalCleanup.begin','portalCleanup.end')
        if ($Chunk -cnotin $stages) { throw 'Snapshot portal stage is not predefined.' }
        $stage,$status=$Chunk.Split('.')
        & $State.write $State $stage $status
        return
    }
    if ($Stream -cne 'stdout') { return }
    $State.bytes += [Text.Encoding]::UTF8.GetByteCount($Chunk)
    if ($State.bytes -gt 65536) { throw 'Snapshot console capture exceeds its existing output bound.' }
    $State.pending += $Chunk
    while (($newline=$State.pending.IndexOf([char]10)) -ge 0) {
        $line=$State.pending.Substring(0,$newline).TrimEnd([char]13)
        $State.pending=$State.pending.Substring($newline+1)
        $row=& $State.validate $State $line
        if ($null -ne $row) { & $State.write $State ('worker.'+$row.stage) $row.status }
    }
}
