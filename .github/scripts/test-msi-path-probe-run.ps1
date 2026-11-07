param($Context, $Baseline, [string]$Directory)

$ErrorActionPreference = 'Stop'
$lifecycle = Join-Path $Directory 'synthetic-lifecycle.ps1'
@'
param([string]$OutputDirectory, [string]$PreviousMsi, $Tracker)
if ($PreviousMsi) { $Tracker.historical++ } else { $Tracker.modern++ }
'@ | Set-Content -LiteralPath $lifecycle -Encoding utf8NoBOM
$Context | Add-Member -NotePropertyName platform -NotePropertyValue ([pscustomobject]@{target='synthetic-target'}) -Force

function Resolve-P4kxMsiInput {
    param([string]$Path)
    if ($script:legacyFault -ceq 'missing') { throw [IO.FileNotFoundException]::new('synthetic missing predecessor') }
    return 'synthetic predecessor.msi'
}

function Get-P4kxMsiMetadata {
    param([string]$Path)
    if ($script:legacyFault -ceq 'metadata') { throw [IO.InvalidDataException]::new('synthetic predecessor metadata failure') }
    return [pscustomobject]@{
        template='x64;1033';version='0.2.0';upgrade=$Context.upgrade
        product='{11111111-2222-4333-8444-555555555555}'
        registry=@(,@('1','Software\KRT\P4K-Extractor','Application'))
    }
}

$Context.logs = Join-Path $Directory 'calibration-success'
$script:legacyFault = 'none'; $script:mutation = 'none'; $script:state = Copy-TestPathState $Baseline
$script:installed = $false; $script:cleanupFails = $false; $script:resets = 0; $script:installCount = 0
$tracker = [pscustomobject]@{historical=0;modern=0}
$results = @(Invoke-P4kxPathProbeScenarios $Context $Baseline @{Tracker=$tracker} $lifecycle 'synthetic previous.msi')
Write-P4kxPathProbeReport $Context $Baseline $results
$report = Get-Content (Join-Path $Context.logs 'probe.json') -Raw | ConvertFrom-Json
if ($tracker.modern -ne 1 -or $tracker.historical -ne 1 -or $script:installCount -ne 9 -or
    $script:resets -ne 20 -or $report.completed -ne 11 -or $report.failed -ne 0 -or
    $report.skipped -ne 0 -or $report.legacy_original_preserved -cne $true) { throw 'Successful calibration did not run the historical, modern and eight raw-value scenarios.' }
Assert-P4kxPathRestored $Baseline (Get-P4kxUserPathState)
foreach ($fault in @('missing','metadata')) {
    $Context.logs = Join-Path $Directory $fault
    $script:legacyFault = $fault; $script:mutation = 'none'; $script:state = Copy-TestPathState $Baseline
    $script:installed = $false; $script:cleanupFails = $false; $script:resets = 0; $script:installCount = 0
    $tracker = [pscustomobject]@{historical=0;modern=0}
    $results = @(Invoke-P4kxPathProbeScenarios $Context $Baseline @{Tracker=$tracker} $lifecycle 'missing-or-failed-previous.msi')
    if ($tracker.modern -ne 1 -or $tracker.historical -ne 0 -or $script:installCount -ne 8 -or
        $results.Count -ne 11 -or $script:resets -ne 18) { throw 'Failed predecessor calibration prevented modern/run-all cases or separate resets.' }
    $caught = $null
    try { Write-P4kxPathProbeReport $Context $Baseline $results }
    catch { $caught = $_.Exception }
    $expectedType = if ($fault -ceq 'missing') { [IO.FileNotFoundException] } else { [IO.InvalidDataException] }
    if ($caught -isnot [AggregateException] -or $caught.InnerExceptions.Count -ne 1 -or
        $caught.InnerExceptions[0].GetType() -ne $expectedType) { throw 'End-of-run failure lost the original predecessor exception.' }
    $report = Get-Content (Join-Path $Context.logs 'probe.json') -Raw | ConvertFrom-Json
    $skipped = @($report.cases | Where-Object outcome -CEQ skipped)
    if ($report.completed -ne 10 -or $report.failed -ne 1 -or $report.skipped -ne 1 -or
        $null -ne $report.legacy_original_preserved -or $skipped.Count -ne 1 -or
        $skipped[0].case -cne 'historical-observed-rest' -or $skipped[0].reason -cne 'legacy-calibration-failed') {
        throw 'Probe report hid calibration failure or pretended the dependent historical upgrade executed.'
    }
    Assert-P4kxPathRestored $Baseline (Get-P4kxUserPathState)
}
$Context.logs = Join-Path $Directory 'unresolved-ownership'
$script:legacyFault = 'missing'; $script:cleanupFails = $true; $script:resets = 0; $script:installCount = 0
$tracker = [pscustomobject]@{historical=0;modern=0}
$caught = $null
try { Invoke-P4kxPathProbeScenarios $Context $Baseline @{Tracker=$tracker} $lifecycle 'missing-previous.msi' | Out-Null }
catch { $caught = $_.Exception }
if ($caught -isnot [AggregateException] -or $caught.InnerExceptions.Count -ne 2 -or
    $caught.InnerExceptions[0] -isnot [IO.FileNotFoundException] -or
    $caught.InnerExceptions[1].Message -cne 'synthetic ownership cleanup failure' -or
    $tracker.modern -ne 0 -or $tracker.historical -ne 0 -or $script:installCount -ne 0 -or $script:resets -ne 0 -or
    (Test-Path -LiteralPath (Join-Path $Context.logs 'probe.json'))) { throw 'Unresolved ownership allowed further scenarios/reset or swallowed the original failure.' }
$script:cleanupFails = $false
'PASS: missing/failed predecessor calibration retains original error, skips only dependent upgrade, completes modern+eight, writes report and fails aggregate; unresolved ownership aborts without reset. All operations mocked.'
