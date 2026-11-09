$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-phase.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-utf8.ps1')

function Read-P4kxMsiProbeJson([string]$Path) {
    $text=Read-P4kxMsiDiagnosticUtf8 $Path 65536 'Probe JSON exceeds its fixed diagnostic bound.'
    return $text | ConvertFrom-Json -AsHashtable -Depth 8
}
function Assert-P4kxMsiProbeBinding {
    param([hashtable]$Value,[string]$Target,[string]$Mode,[string]$Revision,[string]$Head,[string]$Run,[string]$Attempt)
    $arch=if($Target -ceq 'x86_64-pc-windows-msvc'){'X64'}elseif($Target -ceq 'aarch64-pc-windows-msvc'){'Arm64'}else{throw 'Unknown native probe target.'}
    if($Value.schema -ne 1 -or $Value.role -cne 'native-msi-runner-fuse-probe' -or $Value.revision -cne $Revision -or $Value.head_revision -cne $Head -or
        $Value.run -cne $Run -or $Value.attempt -cne $Attempt -or $Value.target -cne $Target -or $Value.mode -cne $Mode -or $Value.process_arch -cne $arch){throw 'Probe artifact source/run/attempt/CPU/mode binding differs.'}
    $expected=@('invoke-msi-feedback-probe.ps1','msi-diagnostic-phase.ps1','msi-diagnostic-utf8.ps1')
    if($Value.sources.Count -ne $expected.Count){throw 'Probe source inventory differs.'}
    foreach($name in $expected){
        $actual=@($Value.sources | Where-Object {$_.name -ceq $name})
        $hash=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        if($actual.Count -ne 1 -or $actual[0].sha256 -cne $hash){throw 'Probe original writer/fixture source hash differs.'}
    }
}
function Assert-P4kxMsiProbeArtifact {
    param([string]$Directory,[string]$Target,[string]$Mode,[string]$Revision,[string]$Head,[string]$Run,[string]$Attempt)
    $feedback=Read-P4kxMsiProbeJson (Join-Path $Directory 'feedback/step-feedback.json')
    $result=Read-P4kxMsiProbeJson (Join-Path $Directory 'diagnostics/diagnostic-result.json')
    Assert-P4kxMsiProbeBinding $feedback $Target $Mode $Revision $Head $Run $Attempt
    Assert-P4kxMsiProbeBinding $result $Target $Mode $Revision $Head $Run $Attempt
    $complete=$Mode -ceq 'complete'
    if($result.completed_success -isnot [bool] -or $result.native_completion_unproven -isnot [bool] -or $result.mutation_authority -isnot [bool] -or
        $result.completed_success -ne $complete -or $result.native_completion_unproven -eq $complete -or $result.mutation_authority){throw 'Probe result invents completion or mutation authority.'}
    $path=Join-Path $Directory 'diagnostics/phase-journal.jsonl'
    $journal=Read-P4kxMsiPhaseJournal $path
    if(-not $journal.present -or -not $journal.complete){throw 'Probe original phase journal is absent or truncated.'}
    $events=@(Get-Content -LiteralPath $path | ForEach-Object {$_ | ConvertFrom-Json -AsHashtable -Depth 4})
    foreach($event in $events){
        if($event.target -cne $Target -or $event.process_arch -cne $feedback.process_arch){throw 'Original probe phase CPU binding differs.'}
    }
    if($complete){
        if($journal.events -ne 4 -or $journal.active.Count -ne 0 -or $journal.terminal -cne 'end'){throw 'Probe complete control lacks original terminal closure.'}
    }elseif($journal.events -ne 2 -or $journal.active.Count -ne 2 -or $journal.active -cnotcontains 'worker' -or $journal.active -cnotcontains 'fixture/runner-stop' -or $null -ne $journal.terminal){
        throw 'Runner-timeout probe lost its original active phase or invented terminal completion.'
    }
    if(-not (Test-Path -LiteralPath (Join-Path $Directory 'diagnostics/probe.log'))){throw 'Probe diagnostic stream artifact is missing.'}
    return @{target=$Target;mode=$Mode;events=$journal.events;completed_success=$complete;mutation_authority=$false}
}


function Assert-P4kxMsiProbeMissingPhaseRejected {
    param([string]$Directory,[string]$ReplayDirectory,[hashtable]$Binding)
    New-Item -ItemType Directory -Path (Join-Path $ReplayDirectory 'feedback'),(Join-Path $ReplayDirectory 'diagnostics') -Force | Out-Null
    foreach($relative in @('feedback/step-feedback.json','diagnostics/diagnostic-result.json','diagnostics/probe.log')){
        Copy-Item -LiteralPath (Join-Path $Directory $relative) -Destination (Join-Path $ReplayDirectory $relative)
    }
    Get-Content -LiteralPath (Join-Path $Directory 'diagnostics/phase-journal.jsonl') | Select-Object -First 1 |
        Set-Content -LiteralPath (Join-Path $ReplayDirectory 'diagnostics/phase-journal.jsonl') -Encoding utf8NoBOM
    $message=$null
    try{Assert-P4kxMsiProbeArtifact -Directory $ReplayDirectory @Binding | Out-Null}catch{$message=$_.Exception.Message}
    if($message -cne 'Runner-timeout probe lost its original active phase or invented terminal completion.'){throw 'Captured missing-active-phase mutation did not fail at the exact evidence oracle.'}
}
