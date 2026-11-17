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
Assert-FeedbackWorkflow (-not $runner.Contains('-OnOutput') -and -not $runner.Contains('[Console]::Out')) 'Native MSI supervision must persist its phase evidence independently of the console sink.'
$installer=Get-FeedbackWorkflowStep $verify 'Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation'
$admission=Get-FeedbackWorkflowStep $verify 'Observe native MSI worker admission without installing'
$admissionUpload=Get-FeedbackWorkflowStep $verify 'Preserve native MSI worker admission evidence'
$refresh=Get-FeedbackWorkflowStep $verify 'Refresh installer allowance against the original deadline'
Assert-FeedbackWorkflow ($verify.IndexOf($admission,[StringComparison]::Ordinal) -lt $verify.IndexOf($admissionUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($admissionUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($refresh,[StringComparison]::Ordinal) -and
    $verify.IndexOf($refresh,[StringComparison]::Ordinal) -lt $verify.IndexOf($installer,[StringComparison]::Ordinal)) 'Admission must upload before refreshing the allowance for the native lifecycle.'
Assert-FeedbackWorkflow ($refresh -cmatch '(?m)^        id: msi-budget\r?$' -and $refresh -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $refresh.Contains('prepare-msi-feedback.ps1') -and $refresh.Contains("'installer-check/lifecycle-feedback'") -and
    -not ($refresh -match '(?m)^        (if|continue-on-error|env):')) 'Lifecycle must consume a newly prepared allowance under the original deadline.'
Assert-FeedbackWorkflow ($verify -cmatch '(?ms)^      - name: Refresh installer allowance against the original deadline\r?\n(?:(?!^      - ).)*^      - name: Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation\r?$') 'No step may intervene between the refreshed allowance and the installer.'
Assert-FeedbackWorkflow ($admission.Contains('check-msi.ps1 @parameters -AdmissionOnly') -and $admission.Contains("'msi-admission'") -and
    $admission -cmatch '(?m)^        timeout-minutes: 2\r?$' -and $admission -cmatch '(?m)^        continue-on-error: true\r?$') 'Read-only native admission requires a separately bounded step and diagnostic directory.'
Assert-FeedbackWorkflow ($admissionUpload.Contains('always() && !cancelled()') -and $admissionUpload.Contains('msi-admission/diagnostics/*.jsonl') -and
    $admissionUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and $admissionUpload -cmatch '(?m)^        continue-on-error: true\r?$') 'Admission upload must be bounded independently from the lifecycle.'
Assert-FeedbackWorkflow ($installer -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.minutes\) \}\}\r?$') 'Installer workflow step lacks an independent deadline-derived runner timeout.'
Assert-FeedbackWorkflow (-not ($installer -match '(?m)^        (continue-on-error|if):')) 'Installer failure must retain ordinary failure gating.'
$uploads=@([regex]::Matches($verify,'(?ms)^      - uses: actions/upload-artifact@[^\r\n]+\r?\n.*?(?=^      - |\z)') |
    Where-Object {$_.Value.Contains('name: windows-package-logs-')})
Assert-FeedbackWorkflow ($uploads.Count -eq 1) 'Expected the existing diagnostic upload step.'
$upload=$uploads[0].Value
Assert-FeedbackWorkflow ($upload -cmatch '(?m)^        if: always\(\)\r?$') 'Diagnostic upload lost cancellation-aware always gating.'
Assert-FeedbackWorkflow ($upload -cmatch '(?m)^        timeout-minutes: 5\r?$') 'Diagnostic upload lacks its finite five-minute runner bound.'
$check=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/check-msi.ps1')
$gate=[regex]::Match($check,'(?m)^if \(\$AdmissionOnly\) \{\r?\n    Stop-P4kxMsiPhase \$diagnostics \$workerPhase\r?\n    return\r?\n\}')
Assert-FeedbackWorkflow ($gate.Success) 'Admission must close the actual worker and return at its read-only boundary.'
foreach($required in @('Assert-P4kxNativeHost $row','Get-P4kxMsiMetadata $Msi','Assert-P4kxMsiTransport $metadata','Get-P4kxMsiEmbeddedHelperDigest $Msi','Write-P4kxMsiDiagnosticInputs $PSBoundParameters')){
    $position=$check.IndexOf($required,[StringComparison]::Ordinal)
    Assert-FeedbackWorkflow ($position -ge 0 -and $position -lt $gate.Index) 'Admission returned before required native source or MSI input proof.'
}
foreach($excluded in @('New-P4kxRawMsiFixture $row','Get-P4kxRawMsiImage $fixture','New-Item -ItemType Directory -Path $profileFolder','Invoke-P4kxMsiexec $Arguments')){
    Assert-FeedbackWorkflow ($check.IndexOf($excluded,[StringComparison]::Ordinal) -gt ($gate.Index+$gate.Length)) 'Read-only admission reached the raw Registry fixture, profile or installer path.'
}
$evidence=Get-FeedbackWorkflowStep $verify 'Record executed release payload evidence'
Assert-FeedbackWorkflow (-not ($evidence -match '(?m)^        (if|continue-on-error):')) 'Release runtime evidence must retain ordinary success gating.'
'PASS: actual workflow keeps job/deadline/worker policies and independently bounds installer plus always-upload; no native execution.'
