param([string]$Repository=(Split-Path (Split-Path $PSScriptRoot -Parent) -Parent))
$ErrorActionPreference='Stop'

function Assert-FeedbackWorkflow([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function Get-FeedbackWorkflowStep([string]$Text,[string]$Name) {
    $pattern='(?ms)^      - name: '+[regex]::Escape($Name)+'\r?\n.*?(?=^      - |\z)'
    $matches=[regex]::Matches($Text,$pattern)
    if($matches.Count -ne 1){throw "Expected one actual workflow step: $Name"}
    return $matches[0].Value
}

$workflow=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows.yml')
$verify=[regex]::Match($workflow,'(?ms)^  verify:\r?\n.*\z').Value
Assert-FeedbackWorkflow ($verify.Length -gt 0) 'Native Windows consumer job is missing.'
Assert-FeedbackWorkflow ($verify -cmatch '(?m)^    timeout-minutes: 45\r?$') 'The native consumer job must remain bounded to 45 minutes.'
Assert-FeedbackWorkflow ($verify.Contains('UtcNow.AddMinutes(35)')) 'The original 35-minute absolute diagnostic deadline changed.'
$runner=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/msi-diagnostic-runner.ps1')
$budgetOwner=Join-Path $Repository '.github/scripts/msi-diagnostic-budget.ps1'
if(Test-Path -LiteralPath $budgetOwner){$runner+=Get-Content -Raw -LiteralPath $budgetOwner}
Assert-FeedbackWorkflow ($runner.Contains('600000')) 'The existing ten-minute worker maximum changed.'
$installer=Get-FeedbackWorkflowStep $verify 'Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation'
Assert-FeedbackWorkflow ($installer -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.minutes\) \}\}\r?$') 'Installer workflow step lacks an independent deadline-derived runner timeout.'
Assert-FeedbackWorkflow (-not ($installer -match '(?m)^        (continue-on-error|if):')) 'Installer failure must retain ordinary failure gating.'
$uploads=@([regex]::Matches($verify,'(?ms)^      - uses: actions/upload-artifact@[^\r\n]+\r?\n.*?(?=^      - |\z)') |
    Where-Object {$_.Value.Contains('name: windows-package-logs-')})
Assert-FeedbackWorkflow ($uploads.Count -eq 1) 'Expected the existing diagnostic upload step.'
$upload=$uploads[0].Value
Assert-FeedbackWorkflow ($upload -cmatch '(?m)^        if: always\(\)\r?$') 'Diagnostic upload lost cancellation-aware always gating.'
Assert-FeedbackWorkflow ($upload -cmatch '(?m)^        timeout-minutes: 5\r?$') 'Diagnostic upload lacks its finite five-minute runner bound.'
$evidence=Get-FeedbackWorkflowStep $verify 'Record executed release payload evidence'
Assert-FeedbackWorkflow (-not ($evidence -match '(?m)^        (if|continue-on-error):')) 'Release runtime evidence must retain ordinary success gating.'
'PASS: actual workflow keeps job/deadline/worker policies and independently bounds installer plus always-upload; no native execution.'
