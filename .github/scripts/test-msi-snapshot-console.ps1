param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference='Stop'
$null=New-Item -ItemType Directory -Path $Directory
. (Join-Path $PSScriptRoot 'msi-snapshot-console.ps1')
$saved=@{revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;actions=$env:GITHUB_ACTIONS;
    summary=$env:GITHUB_STEP_SUMMARY}
function Assert-SnapshotConsole([bool]$Condition,[string]$Message) { if (-not $Condition) { throw $Message } }
function New-SnapshotConsoleLine([hashtable]$Changes=@{}) {
    $row=@{schema=1;role='native-msi-observer-progress';stage='parentState';status='begin';
        revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');request_sha256=$null;binding_unproven=$true;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    foreach ($entry in $Changes.GetEnumerator()) { $row[$entry.Key]=$entry.Value }
    return '[P4KX-MSI-OBSERVER]'+($row | ConvertTo-Json -Compress)+[char]10
}
function New-InstallCheckpointFrame([hashtable]$Changes=@{}) {
    $row=@{
        schema=1;role='native-msi-install-checkpoint';sequence=3;revision=$env:GITHUB_SHA
        run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET
        request_sha256=('a'*64);install_phase='lifecycle/install';install_outcome='failed'
        journal_events=7;journal_terminal=$null;completed_success=$false;native_completion_unproven=$true
        mutation_authority=$false
    }
    foreach ($entry in $Changes.GetEnumerator()) { $row[$entry.Key]=$entry.Value }
    return '[P4KX-MSI-INSTALL-CHECKPOINT]'+($row | ConvertTo-Json -Compress)+[char]10
}
try {
    $env:GITHUB_SHA='synthetic-revision';$env:GITHUB_RUN_ID='synthetic-run';$env:GITHUB_RUN_ATTEMPT='1';$env:P4KX_TARGET='aarch64-pc-windows-msvc'
    $env:GITHUB_ACTIONS='false'
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
    $installBoundary=New-P4kxMsiSnapshotConsole 3 -OriginalInstallBoundary
    $checkpointLine=New-InstallCheckpointFrame
    $beforeBinding=@(Receive-P4kxMsiSnapshotConsole $installBoundary 'stdout' $checkpointLine 6>&1)
    Assert-SnapshotConsole ($beforeBinding.Count -eq 0) 'The install observation was forwarded before the original request binding.'
    $bindingLine=New-SnapshotConsoleLine @{request_sha256=('a'*64)}
    Receive-P4kxMsiSnapshotConsole $installBoundary 'stdout' $bindingLine 6>&1 | Out-Null
    foreach ($changes in @(@{request_sha256=('b'*64)},@{run='other'},@{sequence=2},@{native_completion_unproven=$false},
        @{mutation_authority=$true},@{completed_success=$true},@{install_phase='lifecycle/previous-install'},
        @{install_outcome='success'},@{journal_events='7'},@{extra='raw-secret'})) {
        $negative=@(Receive-P4kxMsiSnapshotConsole $installBoundary 'stdout' (New-InstallCheckpointFrame $changes) 6>&1)
        Assert-SnapshotConsole ($negative.Count -eq 0) 'An invalid install checkpoint was forwarded.'
    }
    $exact=@(Receive-P4kxMsiSnapshotConsole $installBoundary 'stdout' $checkpointLine 6>&1)
    Assert-SnapshotConsole ($exact.Count -eq 1) 'The bound native install observation was not forwarded.'
    $seen=([string]$exact[0]).Substring('[P4KX-MSI-INSTALL-CHECKPOINT]'.Length) | ConvertFrom-Json -AsHashtable
    Assert-SnapshotConsole ($seen.role -ceq 'native-msi-install-checkpoint-console' -and $seen.install_outcome -ceq 'failed' -and
        $seen.install_phase -ceq 'lifecycle/install' -and $seen.journal_events -eq 7 -and -not $seen.completed_success -and
        $seen.native_completion_unproven -and -not $seen.mutation_authority -and -not $seen.ContainsKey('request_sha256') -and
        -not $seen.ContainsKey('revision')) 'Install boundary lost its detail or published identity material.'
    $duplicate=@(Receive-P4kxMsiSnapshotConsole $installBoundary 'stdout' $checkpointLine 6>&1)
    $ordinary=@(Receive-P4kxMsiSnapshotConsole (New-P4kxMsiSnapshotConsole 3) 'stdout' ($bindingLine+$checkpointLine) 6>&1)
    Assert-SnapshotConsole ($duplicate.Count -eq 0 -and $ordinary.Count -eq 1) 'Only the original install boundary may report the checkpoint once.'
    $env:GITHUB_ACTIONS='true'
    $hostedBoundary=New-P4kxMsiSnapshotConsole 3 -OriginalInstallBoundary
    Receive-P4kxMsiSnapshotConsole $hostedBoundary 'stdout' $bindingLine 6>&1 | Out-Null
    $published=@(Receive-P4kxMsiSnapshotConsole $hostedBoundary 'stdout' ($checkpointLine+$checkpointLine) 6>&1)
    $noticePrefix='::notice title=Native MSI install checkpoint::'
    Assert-SnapshotConsole ($published.Count -eq 2 -and ([string]$published[0]).StartsWith('[P4KX-MSI-INSTALL-CHECKPOINT]',[StringComparison]::Ordinal) -and
        ([string]$published[1]).StartsWith($noticePrefix,[StringComparison]::Ordinal)) 'GitHub Actions must get one checkpoint notice alongside its console observation.'
    $summary=([string]$published[0]).Substring('[P4KX-MSI-INSTALL-CHECKPOINT]'.Length)
    $notice=([string]$published[1]).Substring($noticePrefix.Length)
    Assert-SnapshotConsole ($notice -ceq $summary -and $notice -cnotmatch '[\r\n%]' -and
        $notice -cnotmatch '"(?:revision|run|attempt|request_sha256)"') 'The checkpoint notice must be one safe line of existing sanitized fields.'
    $header='### Native MSI install observation'+[Environment]::NewLine+[Environment]::NewLine
    $env:GITHUB_STEP_SUMMARY=Join-Path $Directory 'observed.md'
    Write-P4kxMsiInstallStepSummary $hostedBoundary $true
    Write-P4kxMsiInstallStepSummary $hostedBoundary $true
    $expected=$header+'    '+$summary+[Environment]::NewLine
    Assert-SnapshotConsole ([IO.File]::ReadAllText($env:GITHUB_STEP_SUMMARY) -ceq $expected) 'The summary must write the typed projection exactly once.'

    $env:GITHUB_STEP_SUMMARY=Join-Path $Directory 'returned.md'
    $unframed=New-P4kxMsiSnapshotConsole 3 -OriginalInstallBoundary
    $forged=New-InstallCheckpointFrame @{extra='raw-secret'}
    Receive-P4kxMsiSnapshotConsole $unframed 'stdout' ($bindingLine+$forged) 6>&1 | Out-Null
    Write-P4kxMsiInstallStepSummary $unframed $true
    $fallback=[IO.File]::ReadAllText($env:GITHUB_STEP_SUMMARY).Substring($header.Length).Trim() | ConvertFrom-Json -AsHashtable
    Assert-SnapshotConsole ($fallback.Count -eq 5 -and $fallback.snapshot_returned -eq $true -and $fallback.valid_frame -eq $false -and
        $fallback.completed_success -eq $false -and $fallback.native_completion_unproven -eq $true -and
        $fallback.mutation_authority -eq $false) 'A returned snapshot without an accepted frame must report observation only.'

    $env:GITHUB_STEP_SUMMARY=$null
    $withoutSummary=New-P4kxMsiSnapshotConsole 3 -OriginalInstallBoundary
    Assert-SnapshotConsole (@(Write-P4kxMsiInstallStepSummary $withoutSummary $true *>&1).Count -eq 0) 'A missing summary destination must be ignored.'
    $env:GITHUB_ACTIONS='false'
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
    $env:GITHUB_ACTIONS=$saved.actions;$env:GITHUB_STEP_SUMMARY=$saved.summary
}
