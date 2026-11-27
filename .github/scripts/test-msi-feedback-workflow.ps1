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
$refresh=Get-FeedbackWorkflowStep $verify 'Refresh installer allowance against the original deadline'
Assert-FeedbackWorkflow ($verify.IndexOf($admission,[StringComparison]::Ordinal) -lt $verify.IndexOf($refresh,[StringComparison]::Ordinal)) 'Read-only admission must precede the refreshed allowance for the native lifecycle.'
Assert-FeedbackWorkflow ($refresh -cmatch '(?m)^        id: msi-budget\r?$' -and $refresh -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $refresh.Contains('prepare-msi-feedback.ps1') -and $refresh.Contains("'installer-check/lifecycle-feedback'") -and
    -not ($refresh -match '(?m)^        (if|continue-on-error|env):')) 'Lifecycle must consume a newly prepared allowance under the original deadline.'
Assert-FeedbackWorkflow ($verify -cmatch '(?ms)^      - name: Refresh installer allowance against the original deadline\r?\n(?:(?!^      - ).)*^      - name: Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation\r?$') 'No step may intervene between the refreshed allowance and the installer.'
Assert-FeedbackWorkflow ($admission.Contains('check-msi.ps1 @parameters -AdmissionOnly') -and $admission.Contains("'msi-admission'") -and
    $admission -cmatch '(?m)^        timeout-minutes: 2\r?$' -and $admission -cmatch '(?m)^        continue-on-error: true\r?$') 'Read-only native admission requires a separately bounded step and diagnostic directory.'
Assert-FeedbackWorkflow ($installer -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.minutes\) \}\}\r?$') 'Installer workflow step lacks an independent deadline-derived runner timeout.'
Assert-FeedbackWorkflow (-not ($installer -match '(?m)^        (continue-on-error|if):')) 'Installer failure must retain ordinary failure gating.'
$finishStart=Get-FeedbackWorkflowStep $verify 'Start the isolated original MSI Finish supervisor'
$finishWait=Get-FeedbackWorkflowStep $verify 'Wait for the original MSI Finish receipt'
$observerFinish=Get-FeedbackWorkflowStep $verify 'Finish the original native MSI lifecycle'
$finishLaunchEvidence=Get-FeedbackWorkflowStep $verify 'Preserve the original MSI Finish launch binding'
Assert-FeedbackWorkflow (-not $verify.Contains('-Stage ProbeFinish') -and
    $finishLaunchEvidence.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-finish-launch') -and
    $finishLaunchEvidence.Contains('path: ${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/finish-supervision/launch.json') -and
    $finishLaunchEvidence.Contains("always() && !cancelled() && steps.msi-finish-start.outcome == 'success'") -and
    $finishLaunchEvidence.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
    $finishLaunchEvidence.Contains('overwrite: false') -and
    $finishLaunchEvidence -cmatch '(?m)^        timeout-minutes: 1\r?$') 'The original launch binding must survive the mandatory terminal wait as an immutable checkpoint.'

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
    $finishStart.Contains('ConvertFrom-P4kxMsiBudgetTimestamp $launch.collection_cutoff_utc') -and
    $finishStart.Contains('"launch_digest=$(Get-P4kxDetachedReceiptDigest $launchPath)" >> $env:GITHUB_OUTPUT') -and
    $finishWait.Contains('. .github/scripts/msi-observer-finish-wait-host.ps1') -and
    $finishWait.Contains('Invoke-P4kxMsiHostedFinishWait -Directory') -and
    $finishWait.Contains('-LaunchDigest $env:P4KX_MSI_FINISH_LAUNCH_DIGEST') -and
    $finishWait.Contains('P4KX_MSI_FINISH_LAUNCH_DIGEST: ${{ steps.msi-finish-start.outputs.launch_digest }}') -and
    -not $finishWait.Contains('-MonitorPid') -and
    -not $finishWait.Contains('Invoke-P4kxMsiHostedFinishWatchdog') -and
    $finishWait.Contains('P4KX_MSI_FINISH_COLLECTION_CUTOFF_UTC: ${{ steps.msi-finish-start.outputs.collection_cutoff_utc }}') -and
    $finishWait -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.observer_finish_minutes\) \}\}\r?$' -and
    $finishWait.Contains("always() && !cancelled() && steps.msi-finish-start.outcome == 'success'") -and
    -not $finishWait.Contains('continue-on-error') -and
    $observerFinish.Contains('invoke-msi-observer.ps1 -Stage CollectFinish') -and
    $observerFinish.Contains('-LaunchDigest $env:P4KX_MSI_FINISH_LAUNCH_DIGEST') -and
    $observerFinish.Contains('P4KX_MSI_FINISH_LAUNCH_DIGEST: ${{ steps.msi-finish-start.outputs.launch_digest }}') -and
    $observerFinish -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $observerFinish.Contains("always() && !cancelled() && steps.msi-finish-wait.outcome == 'success'") -and
    -not $observerFinish.Contains('continue-on-error')) 'Finish must bound the terminal wait independently and require local validation of the bound receipt.'
$noticeBracket = '(?ms)^        run: \|\r?\n\s*\$waitOutcome = ''error''\r?\n\s*Write-Host ''::notice title=Native MSI Finish outer Wait::state=enter completion=unproven''\r?\n\s*try \{\r?\n(?<call>.*?)^\s*\} finally \{\r?\n(?<unwind>.*?)^\s*\}\s*$'
$waitNotice = [regex]::Match($finishWait,$noticeBracket)
$waitCall = $waitNotice.Groups['call'].Value
$waitInvocation = $waitCall.IndexOf('Invoke-P4kxMsiHostedFinishWait -Directory',[StringComparison]::Ordinal)
Assert-FeedbackWorkflow ($waitNotice.Success -and $waitInvocation -ge 0 -and
    $waitCall.IndexOf('$waitOutcome = ''returned''',[StringComparison]::Ordinal) -gt $waitInvocation -and
    $waitNotice.Groups['unwind'].Value.Contains('Write-Host "::notice title=Native MSI Finish outer Wait::state=unwind outcome=$waitOutcome completion=unproven"')) 'The hosted Wait shell must signal entry before its canonical call and signal unwind in finally.'
Assert-FeedbackWorkflow (-not ($verify -match 'msi-finish-(observer-start|ready)|waiter-ready\.json|invoke-msi-finish-observation\.ps1')) 'The native consumer must observe the bound supervisor directly without the detached readiness handshake.'
$liveCheckpoint=Get-FeedbackWorkflowStep $verify 'Observe the original MSI after Finish launch'
Assert-FeedbackWorkflow (-not $finishWait.Contains('msi-finish-live')) 'Wait must not depend on the optional live checkpoint.'
Assert-FeedbackWorkflow ($verify.Contains($liveCheckpoint + $finishWait + $observerFinish)) 'Live capture, Wait and CollectFinish must run consecutively.'
Assert-FeedbackWorkflow (-not $verify.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-finish-live')) 'The final lifecycle artifact must carry the live checkpoint.'
Assert-FeedbackWorkflow ($liveCheckpoint.Contains("always() && !cancelled() && steps.msi-finish-start.outcome == 'success'") -and
    $liveCheckpoint.Contains('observe-msi-finish-launch.ps1') -and
    $liveCheckpoint.Contains('P4KX_MSI_FINISH_COLLECTION_CUTOFF_UTC: ${{ steps.msi-finish-start.outputs.collection_cutoff_utc }}') -and
    $liveCheckpoint.Contains('P4KX_MSI_FINISH_LAUNCH_DIGEST: ${{ steps.msi-finish-start.outputs.launch_digest }}') -and
    $liveCheckpoint -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $liveCheckpoint -cmatch '(?m)^        continue-on-error: true\r?$') 'Live capture must stay bounded, optional and bound to the exact original launch.'
$liveScript=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/observe-msi-finish-launch.ps1')
Assert-FeedbackWorkflow ($liveScript -cnotmatch '\b(Start-Sleep|Start-Process|WaitForExit|Stop-Process|ProbeFinish)\b') 'Live sampling must use bounded observation of the original processes and journal.'
$firstCheckpoint=Get-FeedbackWorkflowStep $verify 'Preserve original MSI phase checkpoint 1'
$passive=Get-FeedbackWorkflowStep $verify 'Observe the original ARM64 MSI Registry capture'
$install=Get-FeedbackWorkflowStep $verify 'Observe the first original MSI install outcome'
$preFinishUpload=Get-FeedbackWorkflowStep $verify 'Upload original MSI phase diagnostics before Finish'
$expectedPreFinishPaths=@(
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/01/checkpoint.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/02/checkpoint.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/03/checkpoint.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/snapshots/01/caller-result.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/snapshots/02/caller-result.json',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/snapshots/03/caller-result.json'
)
$preFinishPaths=@([regex]::Matches($preFinishUpload,'(?m)^            ([^\r\n]+)\r?$') | ForEach-Object { $_.Groups[1].Value })
Assert-FeedbackWorkflow ($preFinishUpload.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    -not $preFinishUpload.Contains('matrix.') -and
    $preFinishUpload.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
    $preFinishUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-pre-finish') -and
    $preFinishUpload.Contains('if-no-files-found: warn') -and $preFinishUpload.Contains('overwrite: false') -and
    $preFinishUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $preFinishUpload -cmatch '(?m)^        continue-on-error: true\r?$' -and
    [string]::Join([char]10,$preFinishPaths) -ceq [string]::Join([char]10,$expectedPreFinishPaths)) 'Early diagnostics must cover both Windows targets and remain optional, immutable and limited to original phase metadata.'
$finalUpload=Get-FeedbackWorkflowStep $verify 'Upload final original MSI lifecycle evidence'
Assert-FeedbackWorkflow ($passive.Contains('id: msi-passive-checkpoint') -and
    $passive.Contains("matrix.arch == 'arm64' && steps.msi-launch.outcome == 'success'") -and
    $passive.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and
    $passive.Contains('-Sequence 2 -WaitMilliseconds 0 -OriginalRawBoundary') -and
    $passive -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $passive -cmatch '(?m)^        continue-on-error: true\r?$') 'The original Registry observation must stay optional, ARM64-only and bound to the already admitted lifecycle.'
Assert-FeedbackWorkflow ($install.Contains('id: msi-install-checkpoint') -and
    $install.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    -not $install.Contains('matrix.arch') -and
    $install.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and
    $install.Contains('-Sequence 3 -WaitMilliseconds 0 -OriginalInstallBoundary') -and
    $install -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    $install -cmatch '(?m)^        continue-on-error: true\r?$') 'The first install outcome must remain a bounded optional observation on both native targets.'
$snapshotWorker=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/msi-observer-snapshot-worker.ps1')
Assert-FeedbackWorkflow ($snapshotWorker -cmatch 'if \(-not \$OriginalBoundary -and -not \$state\.lifetime_ended[^\r\n]+\) \{\r?\n[^\r\n]+\r?\n\s+\$state=Stop-P4kxMsiObserverExpiredParent' -and
    $snapshotWorker -cmatch 'if \(-not \$OriginalBoundary -and \$state\.lifetime_ended -and \$env:GITHUB_ENV\)') 'The passive observer must never signal the original parent or change its workflow state.'
Assert-FeedbackWorkflow ($firstCheckpoint.Contains('id: msi-checkpoint-1') -and
    $firstCheckpoint.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success'") -and
    $firstCheckpoint.Contains("env.P4KX_MSI_OBSERVER_FINISHED != 'true'") -and
    $firstCheckpoint.Contains('invoke-msi-observer.ps1 -Stage Snapshot') -and $firstCheckpoint.Contains('-Sequence 1 -WaitMilliseconds 0') -and
    $firstCheckpoint -cmatch '(?m)^        timeout-minutes: 2\r?$' -and
    -not ($firstCheckpoint -match '(?m)^        continue-on-error:')) 'The original lifecycle requires its bounded immediate checkpoint.'
Assert-FeedbackWorkflow ($observerFinish.Contains('id: msi-finish') -and
    $finalUpload.Contains('id: msi-finish-evidence') -and
    $finalUpload.Contains("always() && !cancelled() && steps.msi-launch.outcome == 'success' && steps.msi-finish-start.outcome == 'success'") -and
    $finalUpload.Contains('name: windows-msi-observer-${{ github.run_id }}-${{ github.run_attempt }}-${{ env.P4KX_TARGET }}-final') -and
    $finalUpload.Contains('if-no-files-found: error') -and $finalUpload -cmatch '(?m)^        timeout-minutes: 1\r?$' -and
    -not $finalUpload.Contains('continue-on-error')) 'Original lifecycle evidence requires a bounded upload even when the terminal wait fails.'
Assert-FeedbackWorkflow ($finalUpload.Contains('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9') -and
    -not ($finalUpload -match '(?m)^        (run|env):')) 'Terminal original evidence must use the pinned upload action.'
$finalPaths=@([regex]::Matches($finalUpload,'(?m)^            ([^\r\n]+)\r?$') | ForEach-Object { $_.Groups[1].Value })
$expectedLivePaths=@(
    'post-launch.json','checkpoint.json','phase-journal.jsonl','original-diagnostic-inputs.json',
    'finish-result.json','finish-wait-result.json'
) | ForEach-Object { '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/04/' + $_ }
$expectedFinalPaths=$expectedLivePaths + @(
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/checkpoints/09/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/snapshots/09/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/finish-supervision/*',
    '${{ env.P4KX_PACKAGE_TASK }}/installer-check/observer/observer-result.json'
)
Assert-FeedbackWorkflow ([string]::Join([char]10,$finalPaths) -ceq [string]::Join([char]10,$expectedFinalPaths)) 'Final compact proof must contain exactly the allowlisted live checkpoint, original terminal checkpoint, bounded Finish progress and observer verdict.'
$localStages=@($installer,$firstCheckpoint,$passive,$install,$preFinishUpload,$finishStart,$finishLaunchEvidence,$liveCheckpoint,$finishWait,$observerFinish,$finalUpload)
$positions=@($localStages | ForEach-Object { $verify.IndexOf($_,[StringComparison]::Ordinal) })
for ($index=1; $index -lt $positions.Count; $index++) {
    Assert-FeedbackWorkflow ($positions[$index-1] -lt $positions[$index]) 'Capture phase diagnostics before starting Finish and validate the lifecycle before the final evidence transfer.'
}
$beforeTransfer=$verify.Substring(0,$positions[-1]).Replace($preFinishUpload,'').Replace($finishLaunchEvidence,'')
Assert-FeedbackWorkflow (-not $beforeTransfer.Contains('uses: actions/upload-artifact@')) 'Only original phase and launch allowlists can transfer before native lifecycle validation.'
Assert-FeedbackWorkflow (([regex]::Matches($verify,'invoke-msi-observer\.ps1 -Stage Snapshot').Count -eq 3) -and
    -not ($verify -match 'invoke-msi-observer\.ps1 -Stage Delay|\s-WaitMilliseconds [1-9]|-ScheduledDelay')) 'The installer must not depend on a fixed observation wait.'
$observerEntry=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/invoke-msi-observer.ps1')
Assert-FeedbackWorkflow ($observerEntry.Contains('Invoke-P4kxMsiObserverSnapshot $Directory 9 0 -Finish') -and
    $snapshotWorker.Contains('Assert-P4kxMsiObserverOriginalResult $request $handle $snapshot') -and
    $snapshotWorker.Contains('Write-P4kxMsiObserverProgress writeResult end')) 'Finish must use fixed sequence 09, bounded capture and original-result validation.'
$harmless=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/windows-msi-detached-control.yml')
$harmlessStart=Get-FeedbackWorkflowStep $harmless 'Start harmless tracked phase worker and leave its launcher'
$harmlessFinish=Get-FeedbackWorkflowStep $harmless 'Release and verify only the owned harmless worker'
Assert-FeedbackWorkflow ($harmlessStart.Contains('id: detached-start') -and
    $harmlessFinish.Contains("always() && steps.detached-start.outcome != '' && steps.detached-start.outcome != 'skipped'")) 'The harmless cleanup must run only after its worker launch step was attempted.'
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
Assert-FeedbackWorkflow ($evidence.Contains('id: msi-release-record') -and
    $evidence.Contains("steps.msi-finish.outcome == 'success' && steps.msi-finish-evidence.outcome == 'success'") -and
    -not ($evidence -match '(?m)^        continue-on-error:') -and
    $verify.IndexOf($finalUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($evidence,[StringComparison]::Ordinal)) 'Release runtime evidence requires the validated lifecycle and its successful terminal proof upload.'
$releaseUploads=@([regex]::Matches($verify,'(?ms)^      - uses: actions/upload-artifact@[^\r\n]+\r?\n.*?(?=^      - |\z)') |
    Where-Object {$_.Value.Contains('name: release-${{ matrix.target }}')})
Assert-FeedbackWorkflow ($releaseUploads.Count -eq 1) 'Expected one native release payload transfer.'
$releaseUpload=$releaseUploads[0].Value
Assert-FeedbackWorkflow ($releaseUpload.Contains("steps.msi-release-record.outcome == 'success'") -and
    -not ($releaseUpload -match '(?m)^        continue-on-error:') -and
    $verify.IndexOf($evidence,[StringComparison]::Ordinal) -lt $verify.IndexOf($releaseUpload,[StringComparison]::Ordinal) -and
    $verify.IndexOf($releaseUpload,[StringComparison]::Ordinal) -lt $verify.IndexOf($upload,[StringComparison]::Ordinal)) 'Transfer the admitted payload before the aggregate diagnostic upload.'
$diagnosticPaths=@(
    '/installer-check/**/*.log','/installer-check/**/*.json','/installer-check/**/*.jsonl',
    '/msi-admission/diagnostics/*.log','/msi-admission/diagnostics/*.json','/msi-admission/diagnostics/*.jsonl',
    '/contracts/msi-feedback-parent/**/*.log','/contracts/msi-feedback-parent/**/*.json','/contracts/msi-feedback-parent/**/*.jsonl'
)
foreach ($path in $diagnosticPaths) {
    Assert-FeedbackWorkflow ($upload.Contains(('${{ env.P4KX_PACKAGE_TASK }}' + $path))) 'The diagnostic artifact must retain original launch, checkpoints and admission evidence.'
}
'PASS: actual workflow keeps job/deadline/worker policies and independently bounds installer plus always-upload; no native execution.'
