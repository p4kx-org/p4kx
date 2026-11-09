$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')

function Assert-FeedbackBudget([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function Assert-FeedbackBudgetFailure([scriptblock]$Action,[string]$Message) {
    $failed=$false
    try{& $Action | Out-Null}catch{$failed=$true}
    Assert-FeedbackBudget $failed $Message
}
$now=[DateTimeOffset]::ParseExact('2026-11-09T12:00:00.0000000+00:00','o',[Globalization.CultureInfo]::InvariantCulture)
foreach($case in @(@{seconds=2100;minutes=12},@{seconds=719;minutes=11},@{seconds=61;minutes=1})){
    $deadline=$now.AddSeconds($case.seconds).ToString('o')
    $budget=New-P4kxMsiStepBudget $deadline $now
    Assert-FeedbackBudget ($budget.minutes -eq $case.minutes -and $budget.computed_at_utc -ceq $now.ToString('o') -and $budget.deadline_utc -ceq $deadline) 'Pinned preparation changed its original deadline, integer cap or floor.'
    Assert-P4kxMsiStepAdmission $deadline ([string]$budget.minutes) $budget.computed_at_utc $now
}
foreach($deadline in @('','invalid',$now.ToString('o'),$now.AddMinutes(-1).ToString('o'),$now.AddMinutes(36).ToString('o'),$now.AddSeconds(59).ToString('o'))){
    Assert-FeedbackBudgetFailure {New-P4kxMsiStepBudget $deadline $now} 'Malformed, expired, inflated or sub-minute allowance was admitted.'
}
Assert-FeedbackBudget ((Get-P4kxMsiWorkerBudget ($now.AddMinutes(35).ToString('o')) $now) -eq 600000) 'Worker ten-minute maximum changed.'
Assert-FeedbackBudget ((Get-P4kxMsiWorkerBudget ($now.AddSeconds(3).ToString('o')) $now) -eq 3000) 'Worker short-deadline precision changed.'
$late=$now.AddSeconds(330).ToString('o')
$prepared=New-P4kxMsiStepBudget $late $now
Assert-FeedbackBudget ($prepared.minutes -eq 5) 'Late installer received a fresh twelve-minute allowance.'
Assert-P4kxMsiStepAdmission $late '5' $prepared.computed_at_utc ($now.AddSeconds(30))
Assert-FeedbackBudgetFailure {Assert-P4kxMsiStepAdmission $late '5' $prepared.computed_at_utc ($now.AddSeconds(31))} 'Stale preparation extended the original deadline.'
foreach($minutes in @('','0','01','1.5','13','360','6')){
    Assert-FeedbackBudgetFailure {Assert-P4kxMsiStepAdmission $late $minutes $prepared.computed_at_utc $now} 'Invalid or rebound installer allowance was admitted.'
}
Assert-FeedbackBudgetFailure {Assert-P4kxMsiStepAdmission $late '5' ($now.AddSeconds(1).ToString('o')) $now} 'Future preparation was admitted.'

$repository=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$workflow=Get-Content -Raw -LiteralPath (Join-Path $repository '.github/workflows/release-windows.yml')
$step=[regex]::Match($workflow,'(?ms)^      - name: Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation\r?\n.*?        run: \|\r?\n(?<body>.*?)(?=^      - |\z)')
if(-not $step.Success){throw 'Actual installer step body is missing.'}
$body=($step.Groups['body'].Value -split '\r?\n' | ForEach-Object {$_ -replace '^          ',''}) -join [char]10
$tokens=$null;$parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseInput($body,[ref]$tokens,[ref]$parseErrors)
if($parseErrors.Count -ne 0 -or $ast.EndBlock.Statements.Count -lt 2){throw 'Actual installer entry does not parse.'}
$entry=$ast.EndBlock.Statements | Select-Object -First 2
Assert-FeedbackBudget ($entry[0].Extent.Text -ceq '. .github/scripts/msi-diagnostic-budget.ps1' -and $entry[1].Extent.Text.StartsWith('Assert-P4kxMsiStepAdmission ')) 'Actual installer entry no longer validates admission before platform or native work.'
$savedLocation=Get-Location
$savedDeadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;$savedMinutes=$env:P4KX_MSI_STEP_MINUTES;$savedComputed=$env:P4KX_MSI_STEP_COMPUTED_AT_UTC
try{
    Set-Location -LiteralPath $repository
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddSeconds(1).ToString('o')
    $env:P4KX_MSI_STEP_MINUTES='1';$env:P4KX_MSI_STEP_COMPUTED_AT_UTC=[DateTimeOffset]::UtcNow.AddMinutes(-1).ToString('o')
    $evidence=@{invoked=0}
    $admission=[ScriptBlock]::Create(($entry.Extent.Text -join [char]10)+[char]10+'$evidence.invoked++')
    Assert-FeedbackBudgetFailure {& $admission} 'Actual installer entry admitted stale preparation.'
    Assert-FeedbackBudget ($evidence.invoked -eq 0) 'Stale actual admission continued to installer work.'
}finally{
    Set-Location -LiteralPath $savedLocation.Path
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$savedDeadline;$env:P4KX_MSI_STEP_MINUTES=$savedMinutes;$env:P4KX_MSI_STEP_COMPUTED_AT_UTC=$savedComputed
}

'PASS: pinned clock deadline/worker/step cap, floor and stale-admission controls; no native adapter or original-timeout reproduction.'
