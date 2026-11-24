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

$release=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release.yml')
$control=[regex]::Match($release,'(?ms)^  windows-observation-control:\r?\n.*?(?=^  \w)').Value
Assert-FeedbackWorkflow ($control.Contains("github.event_name == 'pull_request'") -and
    $control.Contains("github.event_name == 'workflow_dispatch' && inputs.windows-observation-control-only") -and
    $control.Contains('uses: ./.github/workflows/windows-msi-detached-control.yml')) 'Native observation control must exercise release pull requests and remain selectable on its own.'

$workflow=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows-execute.yml')
$verify=[regex]::Match($workflow,'(?ms)^  verify:\r?\n.*\z').Value
Assert-FeedbackWorkflow ($verify.Length -gt 0) 'Native Windows consumer job is missing.'
Assert-FeedbackWorkflow ($verify -cmatch '(?m)^    timeout-minutes: 45\r?$') 'The native consumer job must remain bounded to 45 minutes.'
Assert-FeedbackWorkflow ($verify.Contains('UtcNow.AddMinutes(35)')) 'The original 35-minute absolute diagnostic deadline changed.'
$download = Get-FeedbackWorkflowStep $verify 'Download native Windows package inputs'
$retry = Get-FeedbackWorkflowStep $verify 'Retry native Windows package input download'
$select = Get-FeedbackWorkflowStep $verify 'Select the successful native Windows package inputs'
$validate = Get-FeedbackWorkflowStep $verify 'Validate release build source, native CPU and toolchain before execution'
$pinnedDownload = 'uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333'
foreach ($step in $download,$retry) {
    Assert-FeedbackWorkflow ($step.Contains($pinnedDownload) -and $step.Contains('pattern: windows-build-*') -and
        $step.Contains('digest-mismatch: error') -and -not ($step -match '(?m)^          (run-id|repository|github-token):')) 'Both attempts must require the pinned current-run native artifacts and their digests.'
}
Assert-FeedbackWorkflow ([regex]::Matches($verify,[regex]::Escape($pinnedDownload)).Count -eq 2 -and
    $download.Contains('id: windows-input-download') -and $download.Contains('continue-on-error: true') -and
    $download -cmatch '(?m)^        timeout-minutes: 3\r?$' -and
    $download.Contains('path: ${{ env.P4KX_PACKAGE_TASK }}/windows-inputs/first') -and
    $retry.Contains('id: windows-input-retry') -and $retry.Contains("steps.windows-input-download.outcome == 'failure'") -and
    $retry -cmatch '(?m)^        timeout-minutes: 3\r?$' -and -not $retry.Contains('continue-on-error:') -and
    $retry.Contains('path: ${{ env.P4KX_PACKAGE_TASK }}/windows-inputs/retry')) 'Native inputs may retry once into a fresh directory, with ordinary failure on a failed retry.'
Assert-FeedbackWorkflow ($verify.IndexOf($download,[StringComparison]::Ordinal) -lt $verify.IndexOf($retry,[StringComparison]::Ordinal) -and
    $verify.IndexOf($retry,[StringComparison]::Ordinal) -lt $verify.IndexOf($select,[StringComparison]::Ordinal) -and
    $verify.IndexOf($select,[StringComparison]::Ordinal) -lt $verify.IndexOf($validate,[StringComparison]::Ordinal) -and
    $select.Contains("steps.windows-input-download.outcome == 'success'") -and
    $select.Contains('steps.windows-input-download.outputs.download-path || steps.windows-input-retry.outputs.download-path') -and
    -not ($select -match '(?m)^        (if|continue-on-error):') -and
    -not ($validate -match '(?m)^        (if|continue-on-error):') -and
    $validate.Contains('Join-Path $env:P4KX_WINDOWS_INPUTS') -and $validate.Contains('-ValidateArchive') -and
    -not $verify.Contains('artifacts/windows-build-')) 'Select completed download output before the original mandatory package admission.'
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
$finishStart=Get-FeedbackWorkflowStep $verify 'Start the isolated original MSI Finish supervisor'
$finishStartUpload=Get-FeedbackWorkflowStep $verify 'Preserve original MSI Finish supervisor launch'
$finishWait=Get-FeedbackWorkflowStep $verify 'Wait for the original MSI Finish receipt'
$observerFinish=Get-FeedbackWorkflowStep $verify 'Finish the original native MSI lifecycle'
Assert-FeedbackWorkflow (-not $verify.Contains('-Stage ProbeFinish') -and
    $verify -cmatch '(?ms)^      - name: Preserve original MSI Finish supervisor launch\r?\n(?:(?!^      - ).)*^      - name: Wait for the original MSI Finish receipt\r?$') 'The launch receipt must proceed directly to the mandatory terminal wait.'

$prepare=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/prepare-msi-feedback.ps1')
$observerRequest=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/msi-observer-request.ps1')
Assert-FeedbackWorkflow ($prepare.Contains('"observer_finish_minutes=$($budget.observer_finish_minutes)" >> $env:GITHUB_OUTPUT') -and
    $prepare.Contains('"P4KX_MSI_STEP_MINUTES=$($budget.minutes)" >> $env:GITHUB_ENV') -and
    $observerRequest.Contains('$milliseconds = [int]$env:P4KX_MSI_STEP_MINUTES*60000') -and
    $observerRequest.Contains('observer_deadline_utc=$prepared.AddMilliseconds($milliseconds).ToString(''o'')') -and
    -not $observerRequest.Contains('observer_finish_minutes')) 'The cleanup reserve must affect only the hosted Finish timeout and preserve the original observer deadline.'
Assert-FeedbackWorkflow ($installer.Contains('id: msi-launch') -and $installer.Contains('invoke-msi-observer.ps1 -Stage Start') -and
    $finishStart.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    $finishStart.Contains('id: msi-finish-start') -and
    $finishStart.Contains('invoke-msi-observer.ps1 -Stage StartFinish') -and $finishStart -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $finishWait.Contains('id: msi-finish-wait') -and
    $finishWait.Contains('invoke-msi-observer.ps1 -Stage WaitFinish') -and
    $finishWait -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.observer_finish_minutes\) \}\}\r?$' -and
    $finishWait.Contains("always() && !cancelled() && steps.msi-finish-start.outcome == 'success'") -and
    -not $finishWait.Contains('continue-on-error') -and
    $observerFinish.Contains('invoke-msi-observer.ps1 -Stage CollectFinish') -and
    $observerFinish -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $observerFinish.Contains("always() && !cancelled() && steps.msi-finish-wait.outcome == 'success' && steps.msi-finish-evidence.outcome == 'success'") -and
    -not $observerFinish.Contains('continue-on-error')) 'Finish must bound the terminal wait independently and require the preserved proof for final validation.'
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
$firstUpload=Get-FeedbackWorkflowStep $verify 'Upload original MSI phase checkpoint 1'
$passive=Get-FeedbackWorkflowStep $verify 'Observe the original ARM64 MSI Registry capture'
$passiveUpload=Get-FeedbackWorkflowStep $verify 'Upload the original ARM64 MSI Registry capture checkpoint'
$install=Get-FeedbackWorkflowStep $verify 'Observe the first original MSI install outcome'
$installUpload=Get-FeedbackWorkflowStep $verify 'Upload the first original MSI install checkpoint'
$finalUpload=Get-FeedbackWorkflowStep $verify 'Upload final original MSI lifecycle evidence'
Assert-FeedbackWorkflow ($passive.Contains('id: msi-passive-checkpoint') -and
    $passive.Contains("matrix.arch == 'arm64' && steps.msi-launch.outcome == 'success'") -and
    $passive.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and
    $passive.Contains('-Sequence 2 -WaitMilliseconds 0 -OriginalRawBoundary') -and
    $passive -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $passive -cmatch '(?m)^        continue-on-error: true\r?$') 'The original Registry observation must stay optional, ARM64-only and bound to the already admitted lifecycle.'
Assert-FeedbackWorkflow ($passiveUpload.Contains("always() && !cancelled() && steps.msi-passive-checkpoint.outcome != 'skipped'") -and
    $passiveUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-2') -and
    $passiveUpload -cmatch '(?m)^          path: \$\{\{ env.P4KX_PACKAGE_TASK \}\}/installer-check/observer/\*/02/\*\r?$' -and
    $passiveUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $passiveUpload -cmatch '(?m)^        continue-on-error: true\r?$') 'An optional unique upload must preserve the bounded passive evidence even if its observation fails.'
Assert-FeedbackWorkflow ($verify.IndexOf($firstUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($passive,[StringComparison]::Ordinal) -and
    $verify.IndexOf($passive,[StringComparison]::Ordinal) -lt $verify.IndexOf($passiveUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($passiveUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($install,[StringComparison]::Ordinal) -and
    $verify.IndexOf($install,[StringComparison]::Ordinal) -lt $verify.IndexOf($installUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($installUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($observerFinish,[StringComparison]::Ordinal)) 'The separately uploaded original phase checkpoints belong before the final lifecycle gate.'
Assert-FeedbackWorkflow ($install.Contains('id: msi-install-checkpoint') -and
    $install.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    -not $install.Contains('matrix.arch') -and
    $install.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and
    $install.Contains('-Sequence 3 -WaitMilliseconds 0 -OriginalInstallBoundary') -and
    $install -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $install -cmatch '(?m)^        continue-on-error: true\r?$') 'The first install outcome must remain a bounded optional observation on both native targets.'
Assert-FeedbackWorkflow ($installUpload.Contains("always() && !cancelled() && steps.msi-install-checkpoint.outcome != 'skipped'") -and
    $installUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-3') -and
    $installUpload -cmatch '(?m)^          path: \$\{\{ env.P4KX_PACKAGE_TASK \}\}/installer-check/observer/\*/03/\*\r?$' -and
    $installUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $installUpload -cmatch '(?m)^        continue-on-error: true\r?$') 'The original install observation requires its own bounded upload before Finish.'
$snapshotWorker=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/msi-observer-snapshot-worker.ps1')
Assert-FeedbackWorkflow ($snapshotWorker -cmatch 'if \(-not \$OriginalBoundary -and -not \$state\.lifetime_ended[^\r\n]+\) \{\r?\n[^\r\n]+\r?\n\s+\$state=Stop-P4kxMsiObserverExpiredParent' -and
    $snapshotWorker -cmatch 'if \(-not \$OriginalBoundary -and \$state\.lifetime_ended -and \$env:GITHUB_ENV\)') 'The passive observer must never signal the original parent or change its workflow state.'
Assert-FeedbackWorkflow ($firstCheckpoint.Contains('id: msi-checkpoint-1') -and
    $firstCheckpoint.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    $firstCheckpoint.Contains("env.P4KX_MSI_OBSERVER_FINISHED != 'true'") -and
    $firstCheckpoint.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and $firstCheckpoint.Contains('-Sequence 1 -WaitMilliseconds 0') -and
    $firstCheckpoint -cmatch '(?m)^        timeout-minutes: 2\r?$' -and
    -not ($firstCheckpoint -match '(?m)^        continue-on-error:')) 'The original lifecycle requires its bounded immediate checkpoint.'
Assert-FeedbackWorkflow ($firstUpload.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success' && steps.msi-checkpoint-1.outcome != 'skipped'") -and
    $firstUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-1') -and
    $firstUpload.Contains('/observer/*/01/*') -and $firstUpload.Contains('if-no-files-found: error') -and
    $firstUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and -not $firstUpload.Contains('continue-on-error')) 'The immediate snapshot requires its own bounded durable upload.'
Assert-FeedbackWorkflow ($observerFinish.Contains('id: msi-finish') -and
    $finalUpload.Contains('id: msi-finish-evidence') -and
    $finalUpload.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success' && steps.msi-finish-wait.outcome != 'skipped'") -and
    $finalUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-final') -and
    $finalUpload.Contains('if-no-files-found: error') -and $finalUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    -not $finalUpload.Contains('continue-on-error')) 'Original lifecycle evidence requires a bounded upload even when the terminal wait fails.'
Assert-FeedbackWorkflow ($finishStartUpload.Contains("steps.msi-finish-start.outcome == 'success'") -and
    $finishStartUpload.Contains('/observer/finish-supervision/launch.json') -and
    $finishStartUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $verify.IndexOf($finishStart,[StringComparison]::Ordinal) -lt $verify.IndexOf($finishStartUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($finishStartUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($finishWait,[StringComparison]::Ordinal)) 'Finish launch evidence must upload before waiting for the original lifecycle.'
foreach ($evidenceUpload in @($firstUpload,$passiveUpload,$installUpload,$finishStartUpload,$finalUpload)) {
    Assert-FeedbackWorkflow ($evidenceUpload.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
        -not ($evidenceUpload -match '(?m)^        (run|env):')) 'Observer evidence must use the pinned upload action.'
}
$finalPaths=@([regex]::Matches($finalUpload,'(?m)^            ([^\r\n]+)\r?$') | ForEach-Object { $_.Groups[1].Value })
$expectedFinalPaths=@(
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/09/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/snapshots/09/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/finish-supervision/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/observer-result.json'
)
Assert-FeedbackWorkflow ([string]::Join([char]10,$finalPaths) -ceq [string]::Join([char]10,$expectedFinalPaths)) 'Final compact proof must contain the original checkpoint, bounded Finish progress and observer verdict.'
Assert-FeedbackWorkflow ($verify.IndexOf($firstCheckpoint,[StringComparison]::Ordinal) -lt $verify.IndexOf($firstUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($firstUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($finishWait,[StringComparison]::Ordinal) -and
    $verify.IndexOf($finishWait,[StringComparison]::Ordinal) -lt $verify.IndexOf($finalUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($finalUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($observerFinish,[StringComparison]::Ordinal) -and
    $verify.IndexOf($observerFinish,[StringComparison]::Ordinal) -lt $verify.IndexOf('      - name: Record executed release payload evidence',[StringComparison]::Ordinal)) 'Preserve the raw original Finish before acceptance and final runtime publication.'
Assert-FeedbackWorkflow (([regex]::Matches($verify,'invoke-msi-observer\.ps1 -Stage Snapshot').Count -eq 3) -and
    -not ($verify -match 'invoke-msi-observer\.ps1 -Stage Delay|\s-WaitMilliseconds [1-9]|-ScheduledDelay')) 'The installer must not depend on a fixed observation wait.'
$observerEntry=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/invoke-msi-observer.ps1')
Assert-FeedbackWorkflow ($observerEntry.Contains('Invoke-P4kxMsiObserverSnapshot $Directory 9 0 -Finish') -and
    $snapshotWorker.Contains('Assert-P4kxMsiObserverOriginalResult $request $handle $snapshot') -and
    $snapshotWorker.Contains('Write-P4kxMsiObserverProgress writeResult end')) 'Finish must use fixed sequence 09, bounded capture and original-result validation.'
$harmless=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/windows-msi-detached-control.yml')
$harmlessDelay=Get-FeedbackWorkflowStep $harmless 'Wait separately for harmless MSI split-delay control'
$harmlessVerify=Get-FeedbackWorkflowStep $harmless 'Verify harmless MSI split-delay control'
Assert-FeedbackWorkflow ($harmlessDelay.Contains('invoke-msi-observer.ps1 -Stage Delay') -and $harmlessDelay.Contains('-Sequence 2 -WaitMilliseconds 75000') -and
    $harmlessVerify.Contains('invoke-msi-split-delay-control.ps1 -Stage Verify')) 'The independent harmless split-delay diagnostic must remain executable.'
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
