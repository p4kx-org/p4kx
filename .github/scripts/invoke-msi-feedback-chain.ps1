param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][ValidateSet('complete','inner-watchdog','outer-runner')][string]$Mode,
    [Parameter(Mandatory)][string]$Directory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')
Assert-P4kxMsiStepAdmission $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $env:P4KX_MSI_STEP_MINUTES $env:P4KX_MSI_STEP_COMPUTED_AT_UTC
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-feedback-chain-binding.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-runner.ps1')
Assert-P4kxNativeHost (Get-P4kxPlatform $Target)
$feedback=Join-Path $Directory 'feedback'
$binding=Read-P4kxMsiProbeJson (Join-Path $feedback 'chain-binding.json')
$prepared=Read-P4kxMsiProbeJson (Join-Path $feedback 'step-feedback.json')
Assert-P4kxMsiChainBinding $binding $Target $Mode $env:GITHUB_SHA $env:P4KX_MSI_PROBE_HEAD $env:GITHUB_RUN_ID $env:GITHUB_RUN_ATTEMPT
Assert-P4kxMsiChainPreparation $prepared $binding
if($prepared.deadline_utc -cne $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC -or [string]$prepared.minutes -cne $env:P4KX_MSI_STEP_MINUTES -or
    $prepared.computed_at_utc -cne $env:P4KX_MSI_STEP_COMPUTED_AT_UTC){throw 'Chain admission no longer uses its original prepared values.'}
$admitted=[DateTimeOffset]::UtcNow
Assert-P4kxMsiStepAdmission $prepared.deadline_utc ([string]$prepared.minutes) $prepared.computed_at_utc $admitted
$originalDeadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
$workerDeadline=if($Mode -ceq 'inner-watchdog'){$admitted.AddSeconds(3).ToString('o')}else{$originalDeadline}
@{schema=1;role='native-msi-feedback-chain-admission';revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
    target=$Target;mode=$Mode;admitted_at_utc=$admitted.ToString('o');prepared_deadline_utc=$originalDeadline;
    worker_deadline_utc=$workerDeadline;fixture_inner_subcase=($Mode -ceq 'inner-watchdog');mutation_authority=$false} |
    ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $feedback 'chain-admission.json') -Encoding utf8NoBOM
$fixtureMode=if($Mode -ceq 'inner-watchdog'){'timeout'}else{$Mode}
$parameters=[ordered]@{Target=$Target;Msi=$fixtureMode;FixtureDirectory=$PSScriptRoot;OutputDirectory=$Directory}
$failure=$null
try{
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$workerDeadline
    Invoke-P4kxMsiDiagnosticWorker (Join-Path $PSScriptRoot 'msi-feedback-worker-fixture.ps1') $parameters
}catch{$failure=$_.Exception}
finally{$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$originalDeadline}
if($Mode -ceq 'outer-runner'){throw 'Outer runner fixture returned without the required service cancellation.'}
if($Mode -ceq 'complete'){
    if($null -ne $failure){throw $failure}
}else{
    if(-not (Test-P4kxMsiChainWatchdogFailure $failure)){throw 'Inner fixture lacks the actual Portal watchdog exception.'}
}
@{schema=1;role='native-msi-feedback-chain-return';mode=$Mode;expected_inner_watchdog=($Mode -ceq 'inner-watchdog');
    completed_success=($null -eq $failure);native_completion_unproven=($null -ne $failure);mutation_authority=$false} |
    ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $feedback 'chain-return.json') -Encoding utf8NoBOM
