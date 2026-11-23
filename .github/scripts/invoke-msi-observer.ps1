param(
    [Parameter(Mandatory)][ValidateSet('Start','Delay','Snapshot','Finish','StartFinish','WaitFinish','CollectFinish')][string]$Stage,
    [Parameter(Mandatory)][string]$Directory,
    [string]$Invocation,
    [ValidateSet('Lifecycle','Control')][string]$Mode='Lifecycle',
    [ValidateRange(1,9)][int]$Sequence=1,
    [ValidateRange(0,75000)][int]$WaitMilliseconds=75000,
    [ValidateRange(1,100000)][int]$ControlMilliseconds=10000,
    [switch]$ScheduledDelay, [switch]$OriginalRawBoundary, [switch]$OriginalInstallBoundary,
    [ValidateRange(1,2147483647)][int]$FinishMaximumMilliseconds=[int]::MaxValue
)
$ErrorActionPreference = 'Stop'
if (($OriginalRawBoundary -or $OriginalInstallBoundary) -and $Stage -cne 'Snapshot') { throw 'An original phase boundary requires a passive snapshot.' }
if ($OriginalRawBoundary -and $OriginalInstallBoundary) { throw 'A passive snapshot observes one original phase.' }
$boundary = if ($OriginalRawBoundary) { 'preamble/original-raw' } elseif ($OriginalInstallBoundary) { 'lifecycle/install' }
if ($ScheduledDelay -and ($Stage -cne 'Snapshot' -or $Sequence -ne 2 -or $WaitMilliseconds -ne 0)) {
    throw 'The split wait applies only to the zero-wait checkpoint 2 snapshot.'
}
if ($Stage -ceq 'Delay') {
    if ($Mode -cne 'Lifecycle' -or $Sequence -ne 2 -or $WaitMilliseconds -ne 75000) {
        throw 'The split wait applies only to the original 75-second lifecycle checkpoint 2.'
    }
    . (Join-Path $PSScriptRoot 'msi-observer-progress.ps1')
    Write-P4kxMsiObserverProgress delayCaller begin
    try {
        . (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
        . (Join-Path $PSScriptRoot 'msi-observer-snapshot-delay.ps1')
        Wait-P4kxMsiCheckpoint2Delay $Directory
    } finally { Write-P4kxMsiObserverProgress delayCaller end }
    return
}
if ($Stage -cin @('StartFinish','WaitFinish','CollectFinish')) {
    . (Join-Path $PSScriptRoot 'msi-observer-finish-control.ps1')
    switch ($Stage) {
        'StartFinish' { Start-P4kxMsiFinishSupervision $Directory }
        'WaitFinish' { Wait-P4kxMsiSupervisedFinish $Directory }
        'CollectFinish' { Read-P4kxMsiSupervisedFinish $Directory }
    }
    return
}
if ($Stage -ceq 'Finish') {
    . (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')
    Invoke-P4kxMsiObserverSnapshot $Directory 9 0 -Finish -MaximumMilliseconds $FinishMaximumMilliseconds
    return
}
if ($Stage -ceq 'Snapshot') {
    . (Join-Path $PSScriptRoot 'msi-observer-snapshot-caller.ps1')
    $parameters = @{Directory=$Directory;Sequence=$Sequence;WaitMilliseconds=$WaitMilliseconds;ScheduledDelay=$ScheduledDelay}
    if ($boundary) { $parameters.OriginalBoundary = $boundary }
    Invoke-P4kxMsiObserverSnapshot @parameters
    return
}
. (Join-Path $PSScriptRoot 'msi-observer-evidence.ps1')
. (Join-Path $PSScriptRoot 'msi-observer-progress.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if ($Stage -ceq 'Start') {
    if ([string]::IsNullOrEmpty($Invocation)) { throw 'Observer start requires its original invocation file.' }
    $request = New-P4kxMsiObserverRequest $root ([IO.Path]::GetFullPath($Invocation)) $Mode $ControlMilliseconds
    Start-P4kxMsiObservedParent $request
    return
}
