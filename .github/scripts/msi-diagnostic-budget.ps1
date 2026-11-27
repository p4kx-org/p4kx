$ErrorActionPreference='Stop'

function ConvertFrom-P4kxMsiBudgetTimestamp([string]$Value) {
    $timestamp=[DateTimeOffset]::MinValue
    if(-not [DateTimeOffset]::TryParseExact($Value,'o',[Globalization.CultureInfo]::InvariantCulture,[Globalization.DateTimeStyles]::None,[ref]$timestamp)){
        throw 'Missing or invalid native MSI diagnostic deadline.'
    }
    return $timestamp
}
function Get-P4kxMsiRemainingBudget {
    param([string]$Deadline,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $timestamp=ConvertFrom-P4kxMsiBudgetTimestamp $Deadline
    $remaining=[Math]::Floor(($timestamp-$Now).TotalMilliseconds)
    if($remaining -le 0 -or $remaining -gt 35*60*1000){throw 'Native MSI diagnostic deadline expired or exceeds its job budget.'}
    return [long]$remaining
}
function Get-P4kxMsiWorkerBudget {
    param([string]$Deadline,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    return [int][Math]::Min(600000,(Get-P4kxMsiRemainingBudget $Deadline $Now))
}
function New-P4kxMsiWorkerWindow {
    param([string]$Deadline,[bool]$AdmissionOnly,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $milliseconds=Get-P4kxMsiWorkerBudget $Deadline $Now
    if ($AdmissionOnly) { $milliseconds=[Math]::Min($milliseconds,90000) }
    return @{budget_ms=$milliseconds;deadline_utc=$Now.AddMilliseconds($milliseconds).ToString('o')}
}

function Get-P4kxMsiRawFixtureBudget {
    param([string]$WorkerDeadline,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $remaining=Get-P4kxMsiRemainingBudget $WorkerDeadline $Now
    $cleanupMilliseconds=5000
    $evidenceMilliseconds=5000
    $captureMilliseconds=$remaining-$cleanupMilliseconds-$evidenceMilliseconds
    if ($captureMilliseconds -le 0) { throw 'Raw Registry fixture cannot fit execution, owned-process cleanup and evidence in the original worker deadline.' }
    return @{capture_ms=[int]$captureMilliseconds;cleanup_ms=$cleanupMilliseconds}
}

function New-P4kxMsiStepBudget {
    param([string]$Deadline,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    $remaining=Get-P4kxMsiRemainingBudget $Deadline $Now
    $minutes=[int][Math]::Min(12,[Math]::Floor($remaining/60000))
    if($minutes -le 0){throw 'Native MSI installer step has less than one whole minute remaining.'}
    return @{deadline_utc=$Deadline;computed_at_utc=$Now.ToString('o');minutes=$minutes;observer_finish_minutes=($minutes+1)}
}
function Assert-P4kxMsiStepAdmission {
    param([string]$Deadline,[string]$Minutes,[string]$ComputedAt,[DateTimeOffset]$Now=[DateTimeOffset]::UtcNow)
    if($Minutes -cnotmatch '\A(?:[1-9]|1[0-2])\z'){throw 'Invalid native MSI installer step allowance.'}
    $computed=ConvertFrom-P4kxMsiBudgetTimestamp $ComputedAt
    if($computed -gt $Now){throw 'Native MSI installer step preparation is from the future.'}
    $prepared=New-P4kxMsiStepBudget $Deadline $computed
    if([int]$Minutes -ne $prepared.minutes){throw 'Native MSI installer step allowance differs from its original preparation.'}
    $remaining=Get-P4kxMsiRemainingBudget $Deadline $Now
    if([long]$Minutes*60000 -gt $remaining){throw 'Native MSI installer step preparation is stale; allowance no longer fits its original deadline.'}
}
