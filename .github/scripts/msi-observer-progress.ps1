$ErrorActionPreference = 'Stop'

function Write-P4kxMsiObserverProgress {
    param(
        [Parameter(Mandatory)][ValidateSet(
            'readRequest','binding','readHandle','parentState','wait','stopExpiredParent','saveCheckpoint','workflowOutput','validateResult','writeResult',
            'delayCaller','delayBinding','delayReadRequest','delayStartReceipt','delayWait','delayResultReceipt',
            'finishLaunchBinding','finishLaunchProcess','finishLaunchReceipt',
            'finishWaitBinding','finishWaitPoll','finishWaitDigest','finishWaitReceipt',
            'finishWaitPortalStart','finishWaitPortalInputClose','finishWaitPortalCapture','finishWaitPortalCleanup',
            'finishCollectBinding','finishCollectWaitReceipt','finishCollectSupervisorReceipt',
            'finishCollectCallerReceipt','finishCollectOriginalResult',
            'finishSupervisorBinding','finishSupervisorCaller','finishSupervisorReceipt',
            'finishProbeCaller','finishProbeBinding','finishProbeWait','finishProbeParent','finishProbeCheckpoint','finishProbeReceipt'
        )][string]$Stage,
        [Parameter(Mandatory)][ValidateSet('begin','end')][string]$Status,
        [string]$Directory,
        [string]$RequestDigest,
        [ValidateRange(4,5)][int]$Sequence,
        [switch]$FileOnly
    )
    $receipt = @{schema=1;role='native-msi-observer-progress';stage=$Stage;status=$Status;
        revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');request_sha256=$RequestDigest;binding_unproven=$true;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    if ($PSBoundParameters.ContainsKey('Sequence')) { $receipt.sequence=$Sequence }
    if (-not [string]::IsNullOrEmpty($Directory)) {
        Write-P4kxDetachedReceipt (Join-Path $Directory ($Stage+'-'+$Status+'.json')) $receipt
    }
    if (-not $FileOnly) {
        Write-Host ('[P4KX-MSI-OBSERVER]'+($receipt | ConvertTo-Json -Depth 3 -Compress))
    }
}

function Write-P4kxMsiFinishWaitNotice {
    param(
        [ValidateSet('predicate-setup','predicate-start','predicate-wait','predicate-cleanup','receipt-digest','wait-receipt')][string]$Phase,
        [ValidateSet('begin','end')][string]$Status
    )
    if ($env:GITHUB_ACTIONS -ceq 'true' -and $env:RUNNER_ENVIRONMENT -ceq 'github-hosted') {
        Write-Host "::notice title=Native MSI Finish observation::phase=$Phase status=$Status completion=unproven"
    }
}
