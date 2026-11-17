$ErrorActionPreference = 'Stop'

function Write-P4kxMsiObserverProgress {
    param(
        [Parameter(Mandatory)][ValidateSet('readRequest','binding','readHandle','parentState','wait','saveCheckpoint','workflowOutput')][string]$Stage,
        [Parameter(Mandatory)][ValidateSet('begin','end')][string]$Status
    )
    $receipt = @{schema=1;role='native-msi-observer-progress';stage=$Stage;status=$Status;
        observed_utc=[DateTimeOffset]::UtcNow.ToString('o');completed_success=$false;
        native_completion_unproven=$true;mutation_authority=$false}
    Write-Host ('[P4KX-MSI-OBSERVER]'+($receipt | ConvertTo-Json -Depth 3 -Compress))
}
