$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-feedback-chain-binding.ps1')

function Assert-P4kxMsiChainAdmission {
    param([hashtable]$Admission,[hashtable]$Prepared,[hashtable]$Binding)
    if($Admission.schema -ne 1 -or $Admission.role -cne 'native-msi-feedback-chain-admission' -or $Admission.revision -cne $Binding.revision -or
        $Admission.run -cne $Binding.run -or $Admission.attempt -cne $Binding.attempt -or $Admission.target -cne $Binding.target -or
        $Admission.mode -cne $Binding.mode -or $Admission.prepared_deadline_utc -cne $Prepared.deadline_utc -or
        $Admission.mutation_authority -isnot [bool] -or $Admission.mutation_authority -or $Admission.fixture_inner_subcase -isnot [bool]){throw 'Chain admission binding differs.'}
    $admitted=ConvertFrom-P4kxMsiBudgetTimestamp $Admission.admitted_at_utc
    Assert-P4kxMsiStepAdmission $Prepared.deadline_utc ([string]$Prepared.minutes) $Prepared.computed_at_utc $admitted
    $inner=$Binding.mode -ceq 'inner-watchdog'
    $expected=if($inner){$admitted.AddSeconds(3).ToString('o')}else{$Prepared.deadline_utc}
    if($Admission.fixture_inner_subcase -ne $inner -or $Admission.worker_deadline_utc -cne $expected){throw 'Chain confused the admitted deadline and explicit inner fixture subcase.'}
}
function Assert-P4kxMsiChainStart {
    param([hashtable]$Start,[hashtable]$Admission,[hashtable]$Binding)
    $hash=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'msi-feedback-worker-fixture.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()
    if($Start.schema -ne 1 -or $Start.role -cne 'native-msi-diagnostic-start' -or $Start.revision -cne $Binding.revision -or
        $Start.run -cne $Binding.run -or $Start.attempt -cne $Binding.attempt -or $Start.target -cne $Binding.target -or
        $Start.process_arch -cne $Binding.process_arch -or $Start.deadline_utc -cne $Admission.worker_deadline_utc -or
        ($Start.budget_ms -isnot [int] -and $Start.budget_ms -isnot [long]) -or
        ($Start.parent_pid -isnot [int] -and $Start.parent_pid -isnot [long]) -or $Start.worker_script -cne 'msi-feedback-worker-fixture.ps1' -or $Start.worker_sha256 -cne $hash -or $Start.parent_pid -le 0 -or
        $Start.completed_success -isnot [bool] -or $Start.completed_success -or $Start.native_completion_unproven -isnot [bool] -or
        -not $Start.native_completion_unproven -or $Start.mutation_authority -isnot [bool] -or $Start.mutation_authority){throw 'Chain parent start invents completion or differs from original input.'}
    if($Binding.mode -ceq 'inner-watchdog'){
        if($Start.budget_ms -le 0 -or $Start.budget_ms -gt 3000){throw 'Inner fixture did not use its explicitly shortened canonical worker budget.'}
    }elseif($Start.budget_ms -le 60000 -or $Start.budget_ms -gt 90000){throw 'Outer chain did not retain an inner worker budget longer than the dynamic step.'}
}
function Assert-P4kxMsiChainResult {
    param([hashtable]$Result,[hashtable]$Start,[hashtable]$Binding,[bool]$Complete)
    if($Result.schema -ne 1 -or $Result.role -cne 'native-msi-diagnostic-result' -or $Result.revision -cne $Binding.revision -or
        $Result.run -cne $Binding.run -or $Result.attempt -cne $Binding.attempt -or $Result.target -cne $Binding.target -or
        $Result.deadline_utc -cne $Start.deadline_utc -or $Result.budget_ms -ne $Start.budget_ms -or
        $Result.completed_success -isnot [bool] -or $Result.completed_success -ne $Complete -or $Result.native_completion_unproven -isnot [bool] -or
        $Result.native_completion_unproven -eq $Complete -or $Result.mutation_authority -isnot [bool] -or $Result.mutation_authority -or
        $Result.worker_result_returned -isnot [bool] -or $Result.worker_result_returned -ne $Complete -or
        $Result.journal_invalid -isnot [bool] -or $Result.journal_invalid){throw 'Chain final result differs or invents successful native completion.'}
    if($Binding.mode -cne 'outer-runner' -and ($Result.worker.started -ne $true -or $Result.worker.exited -ne $true -or
        $Result.worker.signal_contract -cne 'direct-process-only')){throw 'Returning chain lacks directly owned child completion.'}
}
function Assert-P4kxMsiChainArtifact {
    param([string]$Directory,[string]$PreparedDirectory,[string]$Target,[string]$Mode,[string]$Revision,[string]$Head,[string]$Run,[string]$Attempt)
    foreach($name in @('step-feedback.json','chain-binding.json')){
        $before=(Get-FileHash -LiteralPath (Join-Path $PreparedDirectory $name) -Algorithm SHA256).Hash
        $after=(Get-FileHash -LiteralPath (Join-Path $Directory ('feedback/'+$name)) -Algorithm SHA256).Hash
        if($before -cne $after){throw 'Chain changed its originally uploaded pre-risk receipt.'}
    }
    $binding=Read-P4kxMsiProbeJson (Join-Path $PreparedDirectory 'chain-binding.json')
    $prepared=Read-P4kxMsiProbeJson (Join-Path $PreparedDirectory 'step-feedback.json')
    Assert-P4kxMsiChainBinding $binding $Target $Mode $Revision $Head $Run $Attempt
    Assert-P4kxMsiChainPreparation $prepared $binding
    $admission=Read-P4kxMsiProbeJson (Join-Path $Directory 'feedback/chain-admission.json')
    Assert-P4kxMsiChainAdmission $admission $prepared $binding
    $start=Read-P4kxMsiProbeJson (Join-Path $Directory 'diagnostics/diagnostic-start.json')
    Assert-P4kxMsiChainStart $start $admission $binding
    $path=Join-Path $Directory 'diagnostics/phase-journal.jsonl'
    $journal=Read-P4kxMsiPhaseJournal $path
    if(-not $journal.present -or -not $journal.complete){throw 'Chain original journal is absent or truncated.'}
    $text=Read-P4kxMsiDiagnosticUtf8 $path 1MB 'Chain journal exceeds its bound.'
    foreach($line in @($text.Split([char]10) | Where-Object {$_ -ne ''})){
        $event=$line | ConvertFrom-Json -AsHashtable -Depth 4
        if($event.target -cne $Target -or $event.process_arch -cne $binding.process_arch){throw 'Chain original phase CPU binding differs.'}
    }
    $complete=$Mode -ceq 'complete'
    if($complete){
        if($journal.events -ne 4 -or $journal.active.Count -ne 0 -or $journal.terminal -cne 'end'){throw 'Completed chain lacks original closed scopes.'}
    }elseif($journal.events -ne 2 -or $journal.active.Count -ne 2 -or $journal.active -cnotcontains 'worker' -or
        $journal.active -cnotcontains 'fixture/wait' -or $null -ne $journal.terminal){throw 'Interrupted chain lost original active scopes or invented terminal completion.'}
    $resultPath=Join-Path $Directory 'diagnostics/diagnostic-result.json'
    if($Mode -cne 'outer-runner' -or (Test-Path -LiteralPath $resultPath)){
        Assert-P4kxMsiChainResult (Read-P4kxMsiProbeJson $resultPath) $start $binding $complete
    }
    $returnPath=Join-Path $Directory 'feedback/chain-return.json'
    if($Mode -ceq 'outer-runner'){
        if(Test-Path -LiteralPath $returnPath){throw 'Outer runner case unexpectedly returned its fixture.'}
    }else{
        $returned=Read-P4kxMsiProbeJson $returnPath
        if($returned.schema -ne 1 -or $returned.role -cne 'native-msi-feedback-chain-return' -or $returned.mode -cne $Mode -or
            $returned.expected_inner_watchdog -isnot [bool] -or $returned.expected_inner_watchdog -eq $complete -or
            $returned.completed_success -isnot [bool] -or $returned.completed_success -ne $complete -or
            $returned.native_completion_unproven -isnot [bool] -or $returned.native_completion_unproven -eq $complete -or
            $returned.mutation_authority -isnot [bool] -or $returned.mutation_authority){throw 'Returning chain confused the expected inner failure and success control.'}
        foreach($stream in @('stdout','stderr')){
            $captured=Read-P4kxMsiDiagnosticUtf8 (Join-Path $Directory ('diagnostics/lifecycle-worker-'+$stream+'.log')) 4MB 'Chain stream exceeds its bound.'
            if(-not $captured.Contains('feedback-fixture-'+$stream)){throw 'Returning chain lost original fixture streams.'}
        }
    }
    return @{target=$Target;mode=$Mode;prepared_minutes=$prepared.minutes;events=$journal.events;completed_success=$complete;
        native_completion_unproven=(-not $complete);mutation_authority=$false;release_runtime_evidence=$false}
}
