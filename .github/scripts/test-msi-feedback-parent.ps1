param(
    [Parameter(Mandatory)][string]$Directory,
    [string]$SourceDirectory=$PSScriptRoot
)
$ErrorActionPreference='Stop'
if(-not $IsWindows){throw 'The actual parent/Portal fixture requires Windows; no installer executes.'}
. (Join-Path $SourceDirectory 'msi-diagnostic-runner.ps1')
if(Test-Path -LiteralPath $Directory){throw 'MSI feedback fixture directory must be new.'}
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$root=[IO.Path]::GetFullPath($Directory)
$target=if([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq 'Arm64'){'aarch64-pc-windows-msvc'}else{'x86_64-pc-windows-msvc'}

function Assert-FeedbackParent([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function Test-FeedbackTimeoutException([Exception]$Failure) {
    if($null -eq $Failure){return $false}
    if($Failure.Message -ceq 'Portal measurement timed out.'){return $true}
    if($Failure -is [AggregateException]){
        foreach($inner in $Failure.InnerExceptions){if(Test-FeedbackTimeoutException $inner){return $true}}
    }elseif($null -ne $Failure.InnerException){return Test-FeedbackTimeoutException $Failure.InnerException}
    return $false
}
function Test-FeedbackParentCase([string]$Mode) {
    $output=Join-Path $root $Mode
    New-Item -ItemType Directory -Path $output | Out-Null
    $parameters=[ordered]@{Target=$target;Msi=$Mode;FixtureDirectory=$SourceDirectory;OutputDirectory=$output}
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddSeconds(3).ToString('o')
    $failure=$null
    try{Invoke-P4kxMsiDiagnosticWorker (Join-Path $SourceDirectory 'msi-feedback-worker-fixture.ps1') $parameters}
    catch{$failure=$_.Exception}
    $diagnostics=Join-Path $output 'diagnostics'
    $result=Get-Content -Raw -LiteralPath (Join-Path $diagnostics 'diagnostic-result.json') | ConvertFrom-Json -AsHashtable
    $journal=Read-P4kxMsiPhaseJournal (Join-Path $diagnostics 'phase-journal.jsonl')
    Assert-FeedbackParent ($result.mutation_authority -eq $false -and $result.journal_invalid -eq $false) 'Actual diagnostic result claimed mutation authority or lost valid journal inspection.'
    Assert-FeedbackParent ($result.worker.started -eq $true -and $result.worker.exited -eq $true -and $result.worker.signal_contract -ceq 'direct-process-only') 'Actual parent did not retain directly owned child completion.'
    Assert-FeedbackParent (-not (Get-Process -Id $result.worker.pid -ErrorAction SilentlyContinue)) 'Actual feedback fixture left its directly owned child alive.'
    $stdout=Get-Content -Raw -LiteralPath (Join-Path $diagnostics 'lifecycle-worker-stdout.log')
    $stderr=Get-Content -Raw -LiteralPath (Join-Path $diagnostics 'lifecycle-worker-stderr.log')
    Assert-FeedbackParent ($stdout.Contains('[P4KX-MSI-PHASE]') -and $stdout.Contains('feedback-fixture-stdout') -and $stderr.Contains('feedback-fixture-stderr')) 'Actual timeout chain lost phase or stream capture.'
    Assert-FeedbackParent ($result.target -ceq $target -and $result.deadline_utc -ceq $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC) 'Actual result lost original target or deadline binding.'
    if($Mode -ceq 'timeout'){
        Assert-FeedbackParent (Test-FeedbackTimeoutException $failure) 'Actual parent did not retain the Portal watchdog failure.'
        Assert-FeedbackParent ($result.completed_success -eq $false -and $result.native_completion_unproven -eq $true -and $result.worker_result_returned -eq $false) 'Timed-out worker was falsely classified as successful or returned.'
        Assert-FeedbackParent ($journal.complete -and $journal.events -eq 2 -and $journal.active.Count -eq 2 -and $journal.active -ccontains 'worker' -and $journal.active -ccontains 'fixture/wait' -and $null -eq $journal.terminal) 'Timeout failed to retain both original open scopes without terminal completion.'
    }else{
        Assert-FeedbackParent ($null -eq $failure -and $result.completed_success -eq $true -and $result.native_completion_unproven -eq $false -and $result.worker_result_returned -eq $true) 'Completed actual worker was not proven successful.'
        Assert-FeedbackParent ($journal.complete -and $journal.events -eq 4 -and $journal.active.Count -eq 0 -and $journal.terminal -ceq 'end') 'Completed actual worker lacks the inverse closed phase proof.'
    }
    Write-Host ('PASS: actual parent/Portal/phase chain '+$Mode)
}
$saved=@{actions=$env:GITHUB_ACTIONS;runner=$env:RUNNER_ENVIRONMENT;deadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC}
try{
    $env:GITHUB_ACTIONS='true';$env:RUNNER_ENVIRONMENT='github-hosted'
    Test-FeedbackParentCase 'complete'
    Test-FeedbackParentCase 'timeout'
}finally{
    $env:GITHUB_ACTIONS=$saved.actions;$env:RUNNER_ENVIRONMENT=$saved.runner;$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$saved.deadline
}
'PASS: real parent/Portal/phase persistence controls only; no MSI/Registry/product adapter and no original intermittent-hang reproduction.'
