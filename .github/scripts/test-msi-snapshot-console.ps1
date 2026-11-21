$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-snapshot-console.ps1')
$saved=@{revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET}
function Assert-SnapshotConsole([bool]$Condition,[string]$Message) { if (-not $Condition) { throw $Message } }
function New-SnapshotConsoleLine([hashtable]$Changes=@{}) {
    $row=@{schema=1;role='native-msi-observer-progress';stage='parentState';status='begin';
        revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');request_sha256=$null;binding_unproven=$true;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    foreach ($entry in $Changes.GetEnumerator()) { $row[$entry.Key]=$entry.Value }
    return '[P4KX-MSI-OBSERVER]'+($row | ConvertTo-Json -Compress)+[char]10
}
try {
    $env:GITHUB_SHA='synthetic-revision';$env:GITHUB_RUN_ID='synthetic-run';$env:GITHUB_RUN_ATTEMPT='1';$env:P4KX_TARGET='aarch64-pc-windows-msvc'
    $state=New-P4kxMsiSnapshotConsole 1
    $line=New-SnapshotConsoleLine
    $split=[Math]::Floor($line.Length/2)
    $first=@(Receive-P4kxMsiSnapshotConsole $state 'stdout' $line.Substring(0,$split) 6>&1)
    Assert-SnapshotConsole ($first.Count -eq 0) 'An incomplete frame was forwarded.'
    $second=@(Receive-P4kxMsiSnapshotConsole $state 'stdout' $line.Substring($split) 6>&1)
    Assert-SnapshotConsole ($second.Count -eq 1) 'One complete context-matching marker was not forwarded live.'
    $text=[string]$second[0]
    Assert-SnapshotConsole ($text.StartsWith('[P4KX-MSI-SNAPSHOT]',[StringComparison]::Ordinal)) 'Raw observer output was republished.'
    $marker=$text.Substring('[P4KX-MSI-SNAPSHOT]'.Length) | ConvertFrom-Json -AsHashtable
    Assert-SnapshotConsole ($marker.sequence -eq 1 -and $marker.stage -ceq 'worker.parentState' -and $marker.status -ceq 'begin' -and
        $marker.completed_success -eq $false -and $marker.native_completion_unproven -eq $true -and $marker.mutation_authority -eq $false -and
        -not $marker.ContainsKey('revision') -and -not $marker.ContainsKey('request_sha256')) 'Console output gained source identities or completion authority.'
    $negative=@(Receive-P4kxMsiSnapshotConsole $state 'stdout' $line 6>&1)
    Assert-SnapshotConsole ($negative.Count -eq 0) 'A repeated stage was forwarded twice.'
    foreach ($changes in @(@{revision='different'},@{target='other'},@{role='native-msi-result'},@{stage='installer-secret'},@{status='complete'},
        @{completed_success=$true},@{mutation_authority=$true},@{native_completion_unproven=$false},@{binding_unproven=$false},
        @{observed_utc='invalid'},@{request_sha256='raw-secret'},@{extra='raw-secret'},@{schema='1'},@{completed_success='false'},@{role=@('native-msi-observer-progress')})) {
        $negative=@(Receive-P4kxMsiSnapshotConsole (New-P4kxMsiSnapshotConsole 2) 'stdout' (New-SnapshotConsoleLine $changes) 6>&1)
        Assert-SnapshotConsole ($negative.Count -eq 0) 'An invalid or unbound observer marker reached the console.'
    }
    foreach ($stream in @('stdout','stderr')) {
        $negative=@(Receive-P4kxMsiSnapshotConsole (New-P4kxMsiSnapshotConsole 2) $stream ('raw-secret'+[char]10) 6>&1)
        Assert-SnapshotConsole ($negative.Count -eq 0) 'Unmarked output reached the console.'
    }
    $stage=@(Receive-P4kxMsiSnapshotConsole $state 'stage' 'portalCleanup.begin' 6>&1)
    Assert-SnapshotConsole ($stage.Count -eq 1) 'A predefined cleanup boundary was not emitted.'
    $refused=$false
    try { Receive-P4kxMsiSnapshotConsole $state 'stage' 'installer.complete' } catch { $refused=$true }
    Assert-SnapshotConsole $refused 'A made-up installer stage was admitted.'
    $refused=$false
    try { Receive-P4kxMsiSnapshotConsole (New-P4kxMsiSnapshotConsole 3) 'stdout' ('x'*65537) } catch { $refused=$true }
    Assert-SnapshotConsole $refused 'The existing bounded-output allowance was exceeded.'
    'PASS: bounded Snapshot console framing, context, predefined stages and unproven verdicts; no native execution.'
} finally {
    $env:GITHUB_SHA=$saved.revision;$env:GITHUB_RUN_ID=$saved.run;$env:GITHUB_RUN_ATTEMPT=$saved.attempt;$env:P4KX_TARGET=$saved.target
}
