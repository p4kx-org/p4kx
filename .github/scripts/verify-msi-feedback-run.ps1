param(
    [Parameter(Mandatory)][string]$ArtifactDirectory,
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [ValidateSet('literal','chain')][string]$Experiment='literal'
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-feedback-probe-artifact.ps1')
if($Experiment -ceq 'chain'){. (Join-Path $PSScriptRoot 'msi-feedback-chain-mutations.ps1')}
if($env:GITHUB_REPOSITORY -cnotmatch '\A[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\z' -or $env:GITHUB_RUN_ID -cnotmatch '\A[1-9][0-9]*\z' -or $env:GITHUB_RUN_ATTEMPT -cnotmatch '\A[1-9][0-9]*\z'){throw 'Invalid service feedback coordinates.'}
New-Item -ItemType Directory -Path $EvidenceDirectory -ErrorAction Stop | Out-Null

function Get-P4kxMsiProbeServiceJson([string]$Endpoint,[string]$Name) {
    $text=(& gh api $Endpoint) -join [char]10
    if($LASTEXITCODE -ne 0){throw 'Actual GitHub feedback request failed.'}
    if([Text.Encoding]::UTF8.GetByteCount($text) -gt 1MB){throw 'Actual GitHub feedback exceeds its bound.'}
    [IO.File]::WriteAllText((Join-Path $EvidenceDirectory ($Name+'.json')),$text,[Text.UTF8Encoding]::new($false))
    return $text | ConvertFrom-Json -AsHashtable -Depth 20
}
$base="repos/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
$run=Get-P4kxMsiProbeServiceJson $base 'workflow-run'
if([string]$run.id -cne $env:GITHUB_RUN_ID -or [string]$run.run_attempt -cne $env:GITHUB_RUN_ATTEMPT -or $run.head_sha -cne $env:P4KX_MSI_PROBE_HEAD){throw 'Actual service run/attempt/head binding differs.'}
$jobs=Get-P4kxMsiProbeServiceJson "$base/attempts/$env:GITHUB_RUN_ATTEMPT/jobs?per_page=100" 'workflow-jobs'
if($jobs.total_count -gt 100){throw 'Probe service job inventory exceeds its bound.'}
$results=@()
$modes=if($Experiment -ceq 'chain'){@('complete','inner-watchdog','outer-runner')}else{@('complete','timeout')}
$jobPrefix=if($Experiment -ceq 'chain'){'MSI feedback chain'}else{'MSI feedback fuse'}
$stepName=if($Experiment -ceq 'chain'){'Dynamic parent and Portal fixture'}else{'Runner step fuse fixture'}
$uploadName=if($Experiment -ceq 'chain'){'Upload original parent-chain capture'}else{'Upload actual diagnostic-only capture'}
foreach($row in @((Get-P4kxPlatformContract).targets | Where-Object {$_.os -ceq 'windows'})){
    foreach($mode in $modes){
        $name="$jobPrefix ($($row.target), $mode)"
        $matching=@($jobs.jobs | Where-Object {$_.name -ceq $name})
        if($matching.Count -ne 1){throw 'Missing or duplicate actual native probe job.'}
        $job=$matching[0]
        if($job.status -cne 'completed' -or [string]$job.run_id -cne $env:GITHUB_RUN_ID){throw 'Actual probe job is unfinished or from another run.'}
        $steps=@($job.steps | Where-Object {$_.name -ceq $stepName})
        $uploads=@($job.steps | Where-Object {$_.name -ceq $uploadName})
        if($steps.Count -ne 1 -or $uploads.Count -ne 1 -or $steps[0].status -cne 'completed' -or $uploads[0].conclusion -cne 'success'){throw 'Actual probe step or artifact upload did not complete.'}
        $check=[Uri]$job.check_run_url
        $prefix="/repos/$env:GITHUB_REPOSITORY/check-runs/"
        if($check.Scheme -cne 'https' -or $check.Host -cne 'api.github.com' -or -not $check.AbsolutePath.StartsWith($prefix,[StringComparison]::Ordinal)){throw 'Actual check-run annotation endpoint differs.'}
        $label=$row.arch+'-'+$mode
        $annotations=@(Get-P4kxMsiProbeServiceJson ($check.AbsolutePath.TrimStart('/')+'/annotations?per_page=100') ($label+'-annotations'))
        if($annotations.Count -ge 100){throw 'Probe check-run annotation inventory may be truncated.'}
        $outer=$mode -cin @('timeout','outer-runner')
        if($outer){
            $timeout=@($annotations | Where-Object {$_.message -match '(?i)(timed out|exceeded.*execution time)'})
            if($steps[0].conclusion -cnotin @('failure','timed_out','cancelled') -or $timeout.Count -eq 0){throw 'Expected native runner timeout lacks actual failed-step and official timeout annotation.'}
        }elseif($steps[0].conclusion -cne 'success' -or $job.conclusion -cne 'success'){throw 'Native successful control did not actually succeed.'}
        if($Experiment -ceq 'chain'){
            if($outer){
                $expected="The action '$stepName' has timed out after 1 minutes."
                if(@($annotations | Where-Object {$_.annotation_level -ceq 'failure' -and $_.message -ceq $expected}).Count -ne 1 -or
                    @($annotations | Where-Object {$_.message -match '(?i)job.*maximum execution time'}).Count -ne 0){throw 'Chain needs its exact dynamic one-minute step annotation, not a job timeout.'}
            }
            $preparedUploads=@($job.steps | Where-Object {$_.name -ceq 'Preserve original prepared chain receipt'})
            $preparations=@($job.steps | Where-Object {$_.name -ceq 'Prepare original dynamic chain allowance'})
            if($preparedUploads.Count -ne 1 -or $preparedUploads[0].conclusion -cne 'success' -or
                $preparations.Count -ne 1 -or $preparations[0].conclusion -cne 'success'){throw 'Chain original prepare or pre-risk upload did not succeed.'}
            $artifact="msi-chain-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT-$($row.target)-$mode"
            $directory=Join-Path $ArtifactDirectory $artifact
            $proof=Assert-P4kxMsiChainArtifact $directory (Join-Path $ArtifactDirectory ($artifact+'-prepared')) $row.target $mode $env:GITHUB_SHA $env:P4KX_MSI_PROBE_HEAD $env:GITHUB_RUN_ID $env:GITHUB_RUN_ATTEMPT
            $binding=@{Target=$row.target;Mode=$mode;Revision=$env:GITHUB_SHA;Head=$env:P4KX_MSI_PROBE_HEAD;Run=$env:GITHUB_RUN_ID;Attempt=$env:GITHUB_RUN_ATTEMPT}
            $proof.rejected_mutations=Assert-P4kxMsiChainMutations $directory (Join-Path $ArtifactDirectory ($artifact+'-prepared')) (Join-Path $EvidenceDirectory ($label+'-mutations')) $binding
        }else{
            $artifact="msi-feedback-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT-$($row.target)-$mode"
            $directory=Join-Path $ArtifactDirectory $artifact
            $proof=Assert-P4kxMsiProbeArtifact $directory $row.target $mode $env:GITHUB_SHA $env:P4KX_MSI_PROBE_HEAD $env:GITHUB_RUN_ID $env:GITHUB_RUN_ATTEMPT
            if($mode -ceq 'timeout'){
                $binding=@{Target=$row.target;Mode=$mode;Revision=$env:GITHUB_SHA;Head=$env:P4KX_MSI_PROBE_HEAD;Run=$env:GITHUB_RUN_ID;Attempt=$env:GITHUB_RUN_ATTEMPT}
                Assert-P4kxMsiProbeMissingPhaseRejected $directory (Join-Path $EvidenceDirectory ($label+'-missing-phase')) $binding
                $proof.missing_active_phase_rejected=$true
            }
        }

        $proof.job_id=$job.id;$proof.step_conclusion=$steps[0].conclusion;$proof.annotation_count=$annotations.Count
        $results+=$proof
    }
}
@{schema=1;role='native-msi-feedback-service-verdict';experiment=$Experiment;revision=$env:GITHUB_SHA;head_revision=$env:P4KX_MSI_PROBE_HEAD;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;cases=$results;release_runtime_evidence=$false} |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'verdict.json') -Encoding utf8NoBOM
'PASS: actual native runner annotations and downloaded original feedback for the selected experiment; no installer fix or release receipt.'
