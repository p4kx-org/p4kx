$ErrorActionPreference = 'Stop'

function Get-P4kxMsiCheckpoint2Binding {
    param([string]$Directory, [switch]$ObserveRead)
    $requestPath = Join-Path $Directory 'request.json'
    if ($ObserveRead) { Write-P4kxMsiObserverProgress delayReadRequest begin }
    $request = Read-P4kxDetachedReceipt $requestPath
    if ($ObserveRead) { Write-P4kxMsiObserverProgress delayReadRequest end }
    if ($request.schema -ne 1 -or $request.role -cne 'native-msi-observer-request' -or $request.mode -cne 'Lifecycle' -or
        $request.directory -cne $Directory -or $request.revision -cne $env:GITHUB_SHA -or $request.run -cne $env:GITHUB_RUN_ID -or
        $request.attempt -cne $env:GITHUB_RUN_ATTEMPT -or $request.target -cne $env:P4KX_TARGET -or
        $request.deadline_utc -cne $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC) {
        throw 'Checkpoint 2 delay does not belong to the original lifecycle request.'
    }
    return @{
        revision=$request.revision;run=$request.run;attempt=$request.attempt;target=$request.target
        request_sha256=(Get-P4kxDetachedDigest $requestPath);deadline_utc=$request.deadline_utc
        observer_deadline_utc=$request.observer_deadline_utc
    }
}

function New-P4kxMsiCheckpoint2Window {
    param($Binding, [DateTimeOffset]$Started)
    $outer = $Started.AddSeconds(120)
    foreach ($timestamp in @($Binding.deadline_utc, $Binding.observer_deadline_utc)) {
        $deadline = ConvertFrom-P4kxMsiBudgetTimestamp $timestamp
        if ($deadline -lt $outer) { $outer = $deadline }
    }
    $inner = $Started.AddSeconds(100)
    if ($outer -lt $inner) { $inner = $outer }
    return @{sequence_deadline_utc=$outer.ToString('o');capture_deadline_utc=$inner.ToString('o')}
}

function Wait-P4kxMsiCheckpoint2Delay {
    param([string]$Directory)
    $started = [DateTimeOffset]::UtcNow
    $root = [IO.Path]::GetFullPath($Directory)
    Write-P4kxMsiObserverProgress delayBinding begin
    $binding = Get-P4kxMsiCheckpoint2Binding $root -ObserveRead
    Write-P4kxMsiObserverProgress delayBinding end
    $window = New-P4kxMsiCheckpoint2Window $binding $started
    $path = Join-Path $root 'delays/02'
    if ([IO.Path]::Exists($path)) { throw 'Checkpoint 2 needs a fresh delay receipt directory.' }
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    $record = $binding.Clone()
    $record += $window
    $record += @{
        schema=1;role='native-msi-snapshot-delay-start';sequence=2;started_utc=$started.ToString('o')
        requested_wait_ms=75000;delay_complete=$false
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    Write-P4kxMsiObserverProgress delayStartReceipt begin
    Write-P4kxDetachedReceipt (Join-Path $path 'delay-start.json') $record
    Write-P4kxMsiObserverProgress delayStartReceipt end
    $failure = $null
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        if ((Get-P4kxMsiRemainingBudget $window.capture_deadline_utc) -le $record.requested_wait_ms) {
            throw 'Checkpoint 2 delay leaves no capture budget inside the original allowance.'
        }
        Write-P4kxMsiObserverProgress delayWait begin
        Start-Sleep -Milliseconds $record.requested_wait_ms
        Write-P4kxMsiObserverProgress delayWait end
        [void](Get-P4kxMsiRemainingBudget $window.capture_deadline_utc)
        if ($timer.ElapsedMilliseconds -lt $record.requested_wait_ms) { throw 'Checkpoint 2 delay ended early.' }
        $record.delay_complete = $true
    } catch { $failure = $_.Exception }
    $record.role = 'native-msi-snapshot-delay-result'
    $record.finished_utc = [DateTimeOffset]::UtcNow.ToString('o')
    $record.elapsed_ms = $timer.ElapsedMilliseconds
    $record.failure_type = $(if ($null -ne $failure) { $failure.GetType().FullName } else { $null })
    Write-P4kxMsiObserverProgress delayResultReceipt begin
    Write-P4kxDetachedReceipt (Join-Path $path 'delay-result.json') $record
    Write-P4kxMsiObserverProgress delayResultReceipt end
    if ($null -ne $failure) { throw $failure }
}

function Read-P4kxMsiCheckpoint2Delay {
    param([string]$Directory, [DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $binding = Get-P4kxMsiCheckpoint2Binding $Directory
    $path = Join-Path $Directory 'delays/02'
    $start = Read-P4kxDetachedReceipt (Join-Path $path 'delay-start.json')
    $result = Read-P4kxDetachedReceipt (Join-Path $path 'delay-result.json')
    $started = ConvertFrom-P4kxMsiBudgetTimestamp $start.started_utc
    $finished = ConvertFrom-P4kxMsiBudgetTimestamp $result.finished_utc
    $window = New-P4kxMsiCheckpoint2Window $binding $started
    foreach ($record in @($start, $result)) {
        foreach ($field in (@($binding.Keys) + @($window.Keys))) {
            $expected = if ($binding.ContainsKey($field)) { $binding[$field] } else { $window[$field] }
            if ($record[$field] -cne $expected) { throw 'Checkpoint 2 delay receipt lost its original binding or deadline.' }
        }
        if ($record.schema -ne 1 -or $record.sequence -ne 2 -or $record.started_utc -cne $start.started_utc -or
            $record.requested_wait_ms -ne 75000 -or $record.completed_success -ne $false -or
            $record.native_completion_unproven -ne $true -or $record.mutation_authority -ne $false) {
            throw 'Checkpoint 2 delay receipt does not describe an observation-only wait.'
        }
    }
    if ($start.role -cne 'native-msi-snapshot-delay-start' -or $start.delay_complete -ne $false -or
        $result.role -cne 'native-msi-snapshot-delay-result' -or $result.delay_complete -ne $true -or
        $null -ne $result.failure_type -or $result.elapsed_ms -lt 75000 -or
        $finished -lt $started.AddMilliseconds(75000) -or $finished -gt $Now -or
        $finished -ge (ConvertFrom-P4kxMsiBudgetTimestamp $window.capture_deadline_utc)) {
        throw 'Checkpoint 2 requires a completed bounded delay before capture.'
    }
    return $window
}
