param(
    [Parameter(Mandatory)][string]$ArtifactDirectory,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-feedback-probe-artifact.ps1')
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
foreach($row in @((Get-P4kxPlatformContract).targets | Where-Object {$_.os -ceq 'windows'})){
    foreach($mode in @('complete','timeout')){
        $name="MSI feedback fuse ($($row.target), $mode)"
        $matching=@($jobs.jobs | Where-Object {$_.name -ceq $name})
        if($matching.Count -ne 1){throw 'Missing or duplicate actual native probe job.'}
        $job=$matching[0]
        if($job.status -cne 'completed' -or [string]$job.run_id -cne $env:GITHUB_RUN_ID){throw 'Actual probe job is unfinished or from another run.'}
        $steps=@($job.steps | Where-Object {$_.name -ceq 'Runner step fuse fixture'})
        $uploads=@($job.steps | Where-Object {$_.name -ceq 'Upload actual diagnostic-only capture'})
        if($steps.Count -ne 1 -or $uploads.Count -ne 1 -or $steps[0].status -cne 'completed' -or $uploads[0].conclusion -cne 'success'){throw 'Actual probe step or artifact upload did not complete.'}
        $check=[Uri]$job.check_run_url
        $prefix="/repos/$env:GITHUB_REPOSITORY/check-runs/"
        if($check.Scheme -cne 'https' -or $check.Host -cne 'api.github.com' -or -not $check.AbsolutePath.StartsWith($prefix,[StringComparison]::Ordinal)){throw 'Actual check-run annotation endpoint differs.'}
        $label=$row.arch+'-'+$mode
        $annotations=@(Get-P4kxMsiProbeServiceJson ($check.AbsolutePath.TrimStart('/')+'/annotations?per_page=100') ($label+'-annotations'))
        if($annotations.Count -ge 100){throw 'Probe check-run annotation inventory may be truncated.'}
        if($mode -ceq 'timeout'){
            $timeout=@($annotations | Where-Object {$_.message -match '(?i)(timed out|exceeded.*execution time)'})
            if($steps[0].conclusion -cnotin @('failure','timed_out','cancelled') -or $timeout.Count -eq 0){throw 'Expected native runner timeout lacks actual failed-step and official timeout annotation.'}
        }elseif($steps[0].conclusion -cne 'success' -or $job.conclusion -cne 'success'){throw 'Native successful control did not actually succeed.'}
        $artifact="msi-feedback-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT-$($row.target)-$mode"
        $directory=Join-Path $ArtifactDirectory $artifact
        $proof=Assert-P4kxMsiProbeArtifact $directory $row.target $mode $env:GITHUB_SHA $env:P4KX_MSI_PROBE_HEAD $env:GITHUB_RUN_ID $env:GITHUB_RUN_ATTEMPT

        if($mode -ceq 'timeout'){
            $binding=@{Target=$row.target;Mode=$mode;Revision=$env:GITHUB_SHA;Head=$env:P4KX_MSI_PROBE_HEAD;Run=$env:GITHUB_RUN_ID;Attempt=$env:GITHUB_RUN_ATTEMPT}
            Assert-P4kxMsiProbeMissingPhaseRejected $directory (Join-Path $EvidenceDirectory ($label+'-missing-phase')) $binding
            $proof.missing_active_phase_rejected=$true
        }

        $proof.job_id=$job.id;$proof.step_conclusion=$steps[0].conclusion;$proof.annotation_count=$annotations.Count
        $results+=$proof
    }
}
@{schema=1;role='native-msi-feedback-service-verdict';revision=$env:GITHUB_SHA;head_revision=$env:P4KX_MSI_PROBE_HEAD;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;cases=$results;release_runtime_evidence=$false} |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'verdict.json') -Encoding utf8NoBOM
'PASS: actual native runner timeout annotations plus downloaded original AMD64/ARM64 artifacts and complete controls; no installer fix or release receipt.'
