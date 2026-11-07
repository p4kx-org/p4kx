$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path-probe-case.ps1')

function Invoke-P4kxLegacyPathProbe {
    param($Context, $Baseline, [string]$PreviousMsi)
    $capture = [pscustomobject]@{rest=$null;previous=$null}
    $action = {
        param($directory, $evidence, $Context, $PreviousMsi, $capture)
        $capture.previous = Resolve-P4kxMsiInput $PreviousMsi
        $capture.rest = Get-P4kxObservedLegacyRest $Context $capture.previous
    }
    $case = Invoke-P4kxPathProbeCase $Context $Baseline 'legacy-calibration' $action @($Context, $PreviousMsi, $capture)
    return [pscustomobject]@{case=$case;rest=$capture.rest;previous=$capture.previous}
}

function Invoke-P4kxPathProbeScenarios {
    param($Context, $Baseline, [hashtable]$Calls, [string]$LifecycleScript, [string]$PreviousMsi)
    if ($PreviousMsi) {
        $calibration = Invoke-P4kxLegacyPathProbe $Context $Baseline $PreviousMsi
        $calibration.case
        if ($null -eq $calibration.case.error) {
            $historical = {
                param($directory, $evidence, $Context, $Baseline, $calibration, $LifecycleScript, $Calls)
                $state = Set-P4kxPathFixture $Context $calibration.rest $Baseline (Join-Path $directory 'fixture-seed.path.json')
                $evidence.baseline = $state
                & $LifecycleScript @Calls -PreviousMsi $calibration.previous -OutputDirectory $directory
            }
            Invoke-P4kxPathProbeCase $Context $Baseline 'historical-observed-rest' $historical @($Context, $Baseline, $calibration, $LifecycleScript, $Calls)
        }
        else {
            [pscustomobject]@{
                result=[pscustomobject]@{case='historical-observed-rest';outcome='skipped';reason='legacy-calibration-failed'}
                error=$null
            }
        }
    }
    $modern = {
        param($directory, $evidence, $Baseline, $LifecycleScript, $Calls)
        Assert-P4kxPathRestored $Baseline (Get-P4kxUserPathState)
        & $LifecycleScript @Calls -OutputDirectory $directory
    }
    Invoke-P4kxPathProbeCase $Context $Baseline 'modern-original-baseline' $modern @($Baseline, $LifecycleScript, $Calls)
    Invoke-P4kxAdversarialPathCases $Context $Baseline
}

function Write-P4kxPathProbeReport {
    param($Context, $Baseline, [object[]]$Results)
    $failed = @($Results | Where-Object { $_.result.outcome -ceq 'failed' })
    $skipped = @($Results | Where-Object { $_.result.outcome -ceq 'skipped' })
    $calibration = @($Results | Where-Object { $_.result.case -ceq 'legacy-calibration' })
    $legacyPreserved = if ($calibration.Count -eq 1 -and $calibration[0].result.outcome -ceq 'passed') {
        $calibration[0].result.difference -ceq 'unchanged'
    } else { $null }
    [ordered]@{
        schema_version=1;role='prepend_hypothesis';target=$Context.platform.target;release_default='last'
        completed=($Results.Count - $skipped.Count);failed=$failed.Count;skipped=$skipped.Count
        cases=@($Results | ForEach-Object { $_.result });legacy_original_preserved=$legacyPreserved
    } | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Context.logs 'probe.json') -Encoding utf8NoBOM
    Assert-P4kxPathRestored $Baseline (Get-P4kxUserPathState)
    if ($failed.Count -gt 0) {
        $errors = @($failed | ForEach-Object { $_.error.Exception })
        throw [AggregateException]::new('Prepend MSI PATH hypothesis failed native exact-preservation cases.', [Exception[]]$errors)
    }
}
