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

$workflow=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows-execute.yml')
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
$observerFinish=Get-FeedbackWorkflowStep $verify 'Finish the original native MSI lifecycle'
Assert-FeedbackWorkflow ($installer.Contains('id: msi-launch') -and $installer.Contains('invoke-msi-observer.ps1 -Stage Start') -and
    $observerFinish.Contains('invoke-msi-observer.ps1 -Stage Finish') -and $observerFinish.Contains('-WaitMilliseconds 0') -and
    $observerFinish -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.minutes\) \}\}\r?$' -and
    $observerFinish.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    -not $observerFinish.Contains('continue-on-error')) 'Original lifecycle completion lost its source-bound deadline or failure gate.'
$observerStart=Get-FeedbackWorkflowStep $verify 'Preserve original MSI observer start'
$firstCheckpoint=Get-FeedbackWorkflowStep $verify 'Preserve original MSI phase checkpoint 1'
Assert-FeedbackWorkflow ($observerStart.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    $observerStart.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
    $observerStart.Contains('name: windows-msi-start-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.target }}') -and
    $observerStart -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $observerStart.Contains('if-no-files-found: error') -and
    $observerStart -cmatch '(?m)^        continue-on-error: true\r?$' -and
    -not ($observerStart -match '(?m)^        (run|env):')) 'Original observer start requires a bounded, source-bound upload after successful launch.'
$startPaths=@([regex]::Matches($observerStart,'(?m)^            ([^\r\n]+)\r?$') | ForEach-Object { $_.Groups[1].Value })
$expectedStartPaths=@(
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/request.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/handle.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/parent-start.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/diagnostic-sources.json'
)
Assert-FeedbackWorkflow ([string]::Join([char]10,$startPaths) -ceq [string]::Join([char]10,$expectedStartPaths)) 'Original start upload must contain only the four explicit synthetic launch-evidence paths.'
Assert-FeedbackWorkflow ($verify.IndexOf($installer,[StringComparison]::Ordinal) -lt $verify.IndexOf($observerStart,[StringComparison]::Ordinal) -and
    $verify.IndexOf($observerStart,[StringComparison]::Ordinal) -lt $verify.IndexOf($firstCheckpoint,[StringComparison]::Ordinal)) 'Original launch receipts must upload before the first snapshot.'
$delay=Get-FeedbackWorkflowStep $verify 'Wait separately for original MSI phase checkpoint 2'
$delayUpload=Get-FeedbackWorkflowStep $verify 'Upload original MSI phase checkpoint 2 delay'
Assert-FeedbackWorkflow ($delay.Contains('id: msi-checkpoint-2-delay') -and
    $delay.Contains('invoke-msi-observer.ps1 -Stage Delay') -and $delay.Contains('-Sequence 2 -WaitMilliseconds 75000') -and
    $delay -cmatch '(?m)^        timeout-minutes: 2\r?$' -and
    $delayUpload.Contains("always() && steps.msi-launch.outcome == 'success' && steps.msi-checkpoint-2-delay.outcome != 'skipped'") -and
    $delayUpload.Contains('/installer-check/observer/delays/02/*.json') -and
    $delayUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and $delayUpload.Contains('if-no-files-found: error') -and
    $verify.IndexOf($delay,[StringComparison]::Ordinal) -lt $verify.IndexOf($delayUpload,[StringComparison]::Ordinal)) 'Checkpoint 2 delay and its evidence must be bounded and independently observable.'
$checkpoints=@([regex]::Matches($verify,'(?ms)^      - name: Preserve original MSI phase checkpoint [1-8]\r?\n.*?(?=^      - |\z)'))
Assert-FeedbackWorkflow ($checkpoints.Count -eq 8) 'Expected eight bounded pre-terminal file checkpoints.'
for($index=0;$index -lt $checkpoints.Count;$index++){
    $checkpoint=$checkpoints[$index].Value
    $ordinal=$index+1
    $waitMilliseconds=if($ordinal -le 2){0}else{75000}
    $runnerMinutes=if($ordinal -eq 2){1}else{2}
    $uploaded=Get-FeedbackWorkflowStep $verify "Upload original MSI phase checkpoint $ordinal"
    Assert-FeedbackWorkflow ($checkpoint.Contains('always() && !cancelled()') -and $checkpoint.Contains("steps.msi-launch.outcome == 'success'") -and
        $checkpoint.Contains("env.P4KX_MSI_OBSERVER_FINISHED != 'true'") -and $checkpoint.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and
        $checkpoint.Contains("-Sequence $ordinal -WaitMilliseconds $waitMilliseconds") -and
        $checkpoint -cmatch ('(?m)^        timeout-minutes: {0}\r?$' -f $runnerMinutes) -and
        -not ($checkpoint -match '(?m)^        (continue-on-error|uses):')) 'Direct checkpoint can restart work, mask failure or bypass its bounded original observation.'
    if ($ordinal -eq 2) {
        Assert-FeedbackWorkflow ($checkpoint.Contains('-ScheduledDelay') -and
            $checkpoint.Contains("steps.msi-checkpoint-2-delay.outcome == 'success'") -and
            $verify.IndexOf($delayUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($checkpoint,[StringComparison]::Ordinal)) 'Checkpoint 2 must consume a recorded delay without resetting its budget.'
    }
    Assert-FeedbackWorkflow ($uploaded.Contains('always() && !cancelled()') -and $uploaded.Contains("steps.msi-checkpoint-$ordinal.outcome != 'skipped'") -and
        $uploaded.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
        $uploaded.Contains(('/observer/*/{0:d2}/*' -f $ordinal)) -and $uploaded -cmatch '(?m)^        timeout-minutes: 1\r?$' -and -not $uploaded.Contains('continue-on-error')) 'Checkpoint upload lost its separate existing transport budget or failure gate.'
    Assert-FeedbackWorkflow ($verify.IndexOf($checkpoint,[StringComparison]::Ordinal) -lt $verify.IndexOf($uploaded,[StringComparison]::Ordinal)) 'Checkpoint must be frozen before its independent upload.'
}
$entryWorkflow=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows.yml')
$delegation=[regex]::Match($entryWorkflow,'(?ms)^  verify:\r?\n.*\z').Value
Assert-FeedbackWorkflow ($delegation.Contains('needs: build') -and $delegation.Contains("needs.build.result == 'success'") -and
    $delegation.Contains('uses: ./.github/workflows/release-windows-execute.yml') -and $delegation.Contains('matrix: ${{ inputs.matrix }}')) 'Native verification lost its build-success admission or shared platform matrix.'

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
