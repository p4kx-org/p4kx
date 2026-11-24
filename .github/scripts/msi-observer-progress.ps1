$ErrorActionPreference = 'Stop'

function Write-P4kxMsiObserverProgress {
    param(
        [Parameter(Mandatory)][ValidateSet(
            'readRequest','binding','readHandle','parentState','wait','stopExpiredParent','saveCheckpoint','workflowOutput','validateResult','writeResult',
            'delayCaller','delayBinding','delayReadRequest','delayStartReceipt','delayWait','delayResultReceipt',
            'finishLaunchBinding','finishLaunchProcess','finishLaunchReceipt'
        )][string]$Stage,
        [Parameter(Mandatory)][ValidateSet('begin','end')][string]$Status,
        [string]$Directory,
        [string]$RequestDigest
    )
    $receipt = @{schema=1;role='native-msi-observer-progress';stage=$Stage;status=$Status;
        revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$env:P4KX_TARGET;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');request_sha256=$RequestDigest;binding_unproven=$true;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    if (-not [string]::IsNullOrEmpty($Directory)) {
        Write-P4kxDetachedReceipt (Join-Path $Directory ($Stage+'-'+$Status+'.json')) $receipt
    }
    Write-Host ('[P4KX-MSI-OBSERVER]'+($receipt | ConvertTo-Json -Depth 3 -Compress))
}
