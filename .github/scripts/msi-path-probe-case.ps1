$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path-probe-source.ps1')

function Complete-P4kxPathProbeCase {
    param($Context, $Baseline, [string]$Directory)
    $index = 0
    foreach ($product in @(Get-P4kxInstalledProducts $Context.upgrade)) {
        Invoke-P4kxMsiexec @('/x', $product) (Join-Path $Directory "cleanup-uninstall-$index.log")
        $index++
    }
    Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
    Assert-P4kxLegacyOwnershipAbsent $Context
    $current = Get-P4kxUserPathState
    Set-P4kxPathFixture $Context $Baseline $current (Join-Path $Directory 'fixture-reset.path.json') | Out-Null
    Assert-P4kxPathRestored $Baseline (Get-P4kxUserPathState)
}

function Invoke-P4kxPathProbeCase {
    param($Context, $Baseline, [string]$Name, [scriptblock]$Action, [object[]]$ActionArguments=@())
    $directory = Join-Path $Context.logs $Name
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $before = Get-P4kxUserPathState
    $evidence = [pscustomobject]@{baseline=$before}
    $errorRecord = $null
    $primaryException = $null
    try {
        try { & $Action $directory $evidence @ActionArguments }
        catch { $errorRecord = $_; $primaryException = $_.Exception }
        $after = Get-P4kxUserPathState
        $outcome = [pscustomobject]@{
            case=$Name; outcome=$(if ($null -eq $errorRecord) { 'passed' } else { 'failed' })
            difference=(Get-P4kxPathDifference $evidence.baseline $after)
            host_before=$before; before=$evidence.baseline; after=$after
            failure=$(if ($null -ne $errorRecord) { $errorRecord.Exception.Message } else { $null })
        }
        $outcome | ConvertTo-Json -Depth 7 | Set-Content (Join-Path $directory 'case.json') -Encoding utf8NoBOM
    }
    catch {
        $primaryException = if ($null -eq $primaryException) { $_.Exception } else {
            [AggregateException]::new('PATH case and evidence capture both failed.', [Exception[]]@($primaryException, $_.Exception))
        }
        throw $primaryException
    }
    finally {
        try { Complete-P4kxPathProbeCase $Context $Baseline $directory }
        catch {
            if ($null -ne $primaryException) {
                throw [AggregateException]::new('PATH case and ownership cleanup/reset both failed.', [Exception[]]@($primaryException, $_.Exception))
            }
            throw
        }
    }
    return [pscustomobject]@{ result=$outcome; error=$errorRecord }
}

function Invoke-P4kxAdversarialPathCases {
    param($Context, $Baseline)
    foreach ($case in Get-P4kxPathFixtureCases) {
        $action = {
            param($directory, $evidence, $Context, $Baseline, $fixture)
            $state = Set-P4kxPathFixture $Context $fixture $Baseline (Join-Path $directory 'fixture-seed.path.json')
            $evidence.baseline = $state
            Invoke-P4kxMsiexec @('/i', $Context.msi) (Join-Path $directory 'install.log')
            Assert-P4kxInstalledProduct $Context.metadata $Context.folder $Context.shortcut
            Invoke-P4kxMsiexec @('/x', $Context.metadata.product) (Join-Path $directory 'uninstall.log')
            Assert-P4kxUninstalled $Context.upgrade $Context.folder $Context.shortcut $state
        }
        Invoke-P4kxPathProbeCase $Context $Baseline $case.name $action @($Context, $Baseline, $case.state)
    }
}

function Get-P4kxObservedLegacyRest {
    param($Context, [string]$PreviousMsi)
    $directory = Join-Path $Context.logs 'legacy-calibration'
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $previous = Get-P4kxMsiMetadata $PreviousMsi
    if ($previous.template -cne 'x64;1033' -or $previous.version -cne '0.2.0' -or
        $previous.upgrade.Trim('{}') -ine $Context.upgrade.Trim('{}')) { throw 'Historical calibration MSI identity differs.' }
    $keys = @(Get-P4kxPredecessorRegistryKeys $previous)
    $Context.legacy_keys = $keys
    Assert-P4kxLegacyOwnershipAbsent $Context
    $before = Get-P4kxUserPathState
    Invoke-P4kxMsiexec @('/i', $PreviousMsi) (Join-Path $directory 'install.log')
    Invoke-P4kxMsiexec @('/x', $previous.product) (Join-Path $directory 'uninstall.log')
    Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
    Assert-P4kxLegacyOwnershipAbsent $Context
    $after = Get-P4kxUserPathState
    Write-P4kxPathTransition $before $after (Join-Path $directory 'legacy-prior-mutation.path.json')
    return $after
}
