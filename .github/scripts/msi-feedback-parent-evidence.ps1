$ErrorActionPreference='Stop'

function Copy-P4kxMsiParentControlFile {
    param([string]$Directory,[string]$EvidenceDirectory,[string]$Name)
    $maximumBytes=16KB
    $source=Join-Path (Join-Path $Directory 'diagnostics') $Name
    if(-not (Test-Path -LiteralPath $source -PathType Leaf)){
        return @{name=$Name;status='missing';bytes=$null;sha256=$null}
    }
    $stream=[IO.File]::Open($source,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
    try {
        $length=$stream.Length
        if($length -gt $maximumBytes){return @{name=$Name;status='oversize';bytes=$length;sha256=$null}}
        $buffer=[byte[]]::new($maximumBytes+1)
        $used=0
        while($used -lt $buffer.Length){
            $count=$stream.Read($buffer,$used,$buffer.Length-$used)
            if($count -eq 0){break}
            $used+=$count
        }
        if($used -gt $maximumBytes){return @{name=$Name;status='oversize';bytes=$used;sha256=$null}}
    }finally{$stream.Dispose()}
    $bytes=[byte[]]::new($used)
    [Array]::Copy($buffer,$bytes,$used)
    [IO.File]::WriteAllBytes((Join-Path $EvidenceDirectory $Name),$bytes)
    return @{name=$Name;status='copied';bytes=$used;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()}
}

function Read-P4kxMsiParentControlCopy {
    param([string]$EvidenceDirectory,[hashtable]$File)
    if($File.status -cne 'copied'){return $null}
    return [Text.UTF8Encoding]::new($false,$true).GetString([IO.File]::ReadAllBytes((Join-Path $EvidenceDirectory $File.name)))
}

function Save-P4kxMsiParentControlEvidence {
    param([string]$Directory,[string]$Mode,[string]$Target,[Exception]$WorkerFailure,
        [bool]$WorkerTimedOut,[Exception]$ControlFailure)
    if($Mode -cnotin @('complete','timeout','admission') -or
        $Target -cnotin @('x86_64-pc-windows-msvc','aarch64-pc-windows-msvc')){throw 'Invalid synthetic parent-control evidence binding.'}
    $evidence=Join-Path $Directory 'evidence'
    New-Item -ItemType Directory -Path $evidence -ErrorAction Stop | Out-Null
    $names=@('diagnostic-start.json','diagnostic-result.json','phase-journal.jsonl','lifecycle-worker-stdout.log','lifecycle-worker-stderr.log')
    $files=@();$byName=@{}
    foreach($name in $names){
        $file=Copy-P4kxMsiParentControlFile $Directory $evidence $name
        $files+=$file;$byName[$name]=$file
    }
    $stdout=Read-P4kxMsiParentControlCopy $evidence $byName['lifecycle-worker-stdout.log']
    $stderr=Read-P4kxMsiParentControlCopy $evidence $byName['lifecycle-worker-stderr.log']
    $proof=[ordered]@{
        schema=1;role='synthetic-msi-parent-control-observation';revision=$env:GITHUB_SHA
        run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$Target;mode=$Mode
        diagnostic_deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
        worker_failure_type=$(if($null -eq $WorkerFailure){$null}else{$WorkerFailure.GetType().FullName})
        worker_timed_out=$WorkerTimedOut
        control_failure_type=$(if($null -eq $ControlFailure){$null}else{$ControlFailure.GetType().FullName})
        markers=@{
            phase=$(if($null -eq $stdout){$null}else{$stdout.Contains('[P4KX-MSI-PHASE]')})
            stdout=$(if($null -eq $stdout){$null}else{$stdout.Contains('feedback-fixture-stdout')})
            stderr=$(if($null -eq $stderr){$null}else{$stderr.Contains('feedback-fixture-stderr')})
        }
        all_files_preserved=(@($files | Where-Object {$_.status -cne 'copied'}).Count -eq 0)
        maximum_file_bytes=16384;files=$files
        mutation_authority=$false;native_completion_unproven=$true;release_runtime_evidence=$false
    }
    $json=$proof | ConvertTo-Json -Depth 6
    $bytes=[Text.UTF8Encoding]::new($false,$true).GetBytes($json+[char]10)
    if($bytes.Length -gt 16KB){throw 'Synthetic parent-control observation exceeds its byte bound.'}
    [IO.File]::WriteAllBytes((Join-Path $evidence 'control-observation.json'),$bytes)
    Write-Host ('[P4KX-MSI-PARENT-CONTROL]'+($proof | ConvertTo-Json -Depth 6 -Compress))
}
