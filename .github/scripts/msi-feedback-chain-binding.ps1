$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-feedback-probe-artifact.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')

function Get-P4kxMsiChainSources {
    $repository=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
    foreach($relative in @('.github/workflows/msi-feedback-chain-probe.yml','.github/scripts/prepare-msi-feedback.ps1',
        '.github/scripts/prepare-msi-feedback-chain.ps1','.github/scripts/invoke-msi-feedback-chain.ps1',
        '.github/scripts/msi-feedback-chain-binding.ps1','.github/scripts/msi-feedback-chain-artifact.ps1',
        '.github/scripts/msi-feedback-chain-mutations.ps1',
        '.github/scripts/msi-feedback-probe-artifact.ps1',
        '.github/scripts/verify-msi-feedback-run.ps1','.github/scripts/platform-contract.ps1','.config/platforms.json',
        '.github/scripts/msi-diagnostic-runner.ps1','.github/scripts/msi-diagnostic-budget.ps1',
        '.github/scripts/portal-process.ps1','.github/scripts/package-process.ps1','.github/scripts/msi-feedback-worker-fixture.ps1',
        '.github/scripts/msi-diagnostic-phase.ps1','.github/scripts/msi-diagnostic-utf8.ps1')){
        @{name=$relative;sha256=(Get-FileHash -LiteralPath (Join-Path $repository $relative) -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
}
function Assert-P4kxMsiChainBinding {
    param([hashtable]$Binding,[string]$Target,[string]$Mode,[string]$Revision,[string]$Head,[string]$Run,[string]$Attempt)
    $arch=if($Target -ceq 'x86_64-pc-windows-msvc'){'X64'}elseif($Target -ceq 'aarch64-pc-windows-msvc'){'Arm64'}else{throw 'Unknown chain target.'}
    if($Binding.schema -ne 1 -or $Binding.role -cne 'native-msi-feedback-chain' -or $Binding.revision -cne $Revision -or
        $Binding.head_revision -cne $Head -or $Binding.run -cne $Run -or $Binding.attempt -cne $Attempt -or
        $Binding.target -cne $Target -or $Binding.mode -cne $Mode -or $Binding.process_arch -cne $arch){throw 'Chain source/run/CPU/mode binding differs.'}
    $expected=@(Get-P4kxMsiChainSources)
    if($Binding.sources.Count -ne $expected.Count){throw 'Chain source inventory differs.'}
    foreach($source in $expected){
        $matching=@($Binding.sources | Where-Object {$_.name -ceq $source.name})
        if($matching.Count -ne 1 -or $matching[0].sha256 -cne $source.sha256){throw 'Chain original source hash differs.'}
    }
}
function Assert-P4kxMsiChainPreparation {
    param([hashtable]$Prepared,[hashtable]$Binding)
    if($Prepared.schema -ne 1 -or $Prepared.role -cne 'native-msi-step-feedback' -or $Prepared.revision -cne $Binding.revision -or
        $Prepared.run -cne $Binding.run -or $Prepared.attempt -cne $Binding.attempt -or $Prepared.target -cne $Binding.target -or
        ($Prepared.minutes -isnot [int] -and $Prepared.minutes -isnot [long]) -or $Prepared.minutes -ne 1){throw 'Chain original prepared receipt differs.'}
    $computed=ConvertFrom-P4kxMsiBudgetTimestamp $Prepared.computed_at_utc
    $remaining=Get-P4kxMsiRemainingBudget $Prepared.deadline_utc $computed
    if($remaining -le 60000 -or $remaining -gt 90000){throw 'Chain preparation does not retain its explicit one-to-two-minute interval.'}
    Assert-P4kxMsiStepAdmission $Prepared.deadline_utc ([string]$Prepared.minutes) $Prepared.computed_at_utc $computed
}

function Test-P4kxMsiChainWatchdogFailure([Exception]$Failure) {
    if($null -eq $Failure){return $false}
    if($Failure.Message -ceq 'Portal measurement timed out.'){return $true}
    if($Failure -is [AggregateException]){
        if($Failure.InnerExceptions.Count -eq 0){return $false}
        foreach($inner in $Failure.InnerExceptions){if(-not (Test-P4kxMsiChainWatchdogFailure $inner)){return $false}}
        return $true
    }elseif($null -ne $Failure.InnerException){return Test-P4kxMsiChainWatchdogFailure $Failure.InnerException}
    return $false
}
