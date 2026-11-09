param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')
$budget=New-P4kxMsiStepBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
if($env:GITHUB_SHA -cnotmatch '\A[0-9a-f]{40}\z' -or $env:GITHUB_RUN_ID -cnotmatch '\A[1-9][0-9]*\z' -or $env:GITHUB_RUN_ATTEMPT -cnotmatch '\A[1-9][0-9]*\z' -or
    $env:P4KX_TARGET -cnotin @('x86_64-pc-windows-msvc','aarch64-pc-windows-msvc')){
    throw 'Invalid source/run/CPU binding for MSI step feedback.'
}
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
@{schema=1;role='native-msi-step-feedback';deadline_utc=$budget.deadline_utc;computed_at_utc=$budget.computed_at_utc;minutes=$budget.minutes;
    revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET} |
    ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $Directory 'step-feedback.json') -Encoding utf8NoBOM
"minutes=$($budget.minutes)" >> $env:GITHUB_OUTPUT
"P4KX_MSI_STEP_MINUTES=$($budget.minutes)" >> $env:GITHUB_ENV
"P4KX_MSI_STEP_COMPUTED_AT_UTC=$($budget.computed_at_utc)" >> $env:GITHUB_ENV
