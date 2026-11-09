param(
    [Parameter(Mandatory)][string]$Directory,
    [string]$SourceDirectory=$PSScriptRoot
)
$ErrorActionPreference='Stop'
if (-not $IsWindows) { throw 'These mocked Windows MSI control-flow checks require a Windows host; no installer executes.' }
. (Join-Path $SourceDirectory 'msi-diagnostic-runner.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'MSI diagnostic failure test directory must be new.' }
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$root=[IO.Path]::GetFullPath($Directory)
$target=if([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq 'Arm64'){'aarch64-pc-windows-msvc'}else{'x86_64-pc-windows-msvc'}

function Read-DiagnosticSource([string]$Name) {
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $SourceDirectory $Name),[ref]$tokens,[ref]$errors)
    if($errors.Count -ne 0){throw 'Actual diagnostic candidate source does not parse.'}
    return $ast
}
function Contains-DiagnosticException([Exception]$Actual,[Exception]$Expected) {
    if([object]::ReferenceEquals($Actual,$Expected)){return $true}
    if($Actual -is [AggregateException]){foreach($inner in $Actual.InnerExceptions){if(Contains-DiagnosticException $inner $Expected){return $true}}}
    elseif($null -ne $Actual.InnerException){return Contains-DiagnosticException $Actual.InnerException $Expected}
    return $false
}
function Assert-FailureControl([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}

$runnerAst=Read-DiagnosticSource 'msi-diagnostic-runner.ps1'
$functions=@($runnerAst.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq 'Invoke-P4kxMsiDiagnosticWorker'},$true))
if($functions.Count -ne 1){throw 'Expected the single actual diagnostic worker function.'}
$actualWorker=[ScriptBlock]::Create($functions[0].Extent.Text)
$checkAst=Read-DiagnosticSource 'check-msi.ps1'
$tries=@($checkAst.FindAll({param($node) $node -is [Management.Automation.Language.TryStatementAst] -and $null -ne $node.Finally -and $node.Finally.Extent.Text.Contains("'final-cleanup'")},$true))
if($tries.Count -ne 1){throw 'Expected the actual lifecycle finally block.'}
$actualFinally=[ScriptBlock]::Create('try { throw $primaryFailure } finally '+$tries[0].Finally.Extent.Text)

$profileCommands=@($checkAst.FindAll({param($node) $node -is [Management.Automation.Language.CommandAst] -and $node.GetCommandName() -ceq 'New-Item' -and $node.Extent.Text.Contains('$profileFolder')},$true))
if($profileCommands.Count -ne 1){throw 'Expected actual profile creation command.'}
$sourceText=[IO.File]::ReadAllText((Join-Path $SourceDirectory 'check-msi.ps1'))
$profileStart=$profileCommands[0].Parent.Extent.StartOffset
$actualProfileLifecycle=[ScriptBlock]::Create($sourceText.Substring($profileStart,$tries[0].Extent.EndOffset-$profileStart))

function Test-SummaryPersistence([bool]$PortalFails,[string]$Name) {
    . $actualWorker
    $workerFailure=[InvalidOperationException]::new('synthetic actual worker adapter failure')
    $journalFailure=[IO.InvalidDataException]::new('synthetic actual journal adapter failure')
    $writeFailure=[IO.IOException]::new('synthetic actual result persistence failure')
    function Invoke-P4kxPortalProcess {
        if($PortalFails){throw $workerFailure}
        return @{exit_code=0;stdout='';stderr=''}
    }
    function Read-P4kxMsiPhaseJournal { throw $journalFailure }
    function Set-Content {
        param([Parameter(ValueFromPipeline)]$Value,[string]$LiteralPath,[string]$Encoding)
        process { throw $writeFailure }
    }
    $parameters=@{Target=$target;OutputDirectory=(Join-Path $root $Name)}
    $observed=$null
    try { Invoke-P4kxMsiDiagnosticWorker (Join-Path $root 'never-executed-stub.ps1') $parameters } catch { $observed=$_.Exception }
    Assert-FailureControl ($null -ne $observed) 'Controlled actual output failure was accepted.'
    Assert-FailureControl (Contains-DiagnosticException $observed $journalFailure) 'Result persistence replaced the original actual journal failure.'
    Assert-FailureControl (Contains-DiagnosticException $observed $writeFailure) 'Result persistence failure was swallowed.'
    if($PortalFails){Assert-FailureControl (Contains-DiagnosticException $observed $workerFailure) 'Result persistence replaced the original actual worker failure.'}
}

function Test-ActualCleanupFinally {
    param([switch]$CleanupFails)
    $phaseDirectory=Join-Path $root $(if($CleanupFails){'actual-finally-error'}else{'actual-finally'})
    New-Item -ItemType Directory -Path $phaseDirectory | Out-Null
    $diagnostics=New-P4kxMsiPhaseContext $phaseDirectory $target
    $worker=Start-P4kxMsiPhase $diagnostics 'worker'
    $primaryFailure=[InvalidOperationException]::new('synthetic primary verification failure')
    $expectedCleanupFailure=[IO.IOException]::new('synthetic actual restore failure')
    $cleanupEvidence=@{restore=0;path=0}
    $context=@{};$fixture=@{};$originalRaw=$null;$originalPath=$null
    $markers=@();$profileFolder=Join-Path $root 'never-mutated-profile';$logs=$root
    function Restore-P4kxMsiVerificationBaseline { $cleanupEvidence.restore++; if($CleanupFails){throw $expectedCleanupFailure} }
    function Get-ChildItem { return 'synthetic retained profile entry' }
    function Get-P4kxUserPathState { return $null }
    function Write-P4kxPathTransition { $cleanupEvidence.path++ }
    $observed=$null
    try { & $actualFinally } catch { $observed=$_.Exception }
    Assert-FailureControl (Contains-DiagnosticException $observed $primaryFailure) 'Actual finally changed the original primary failure.'
    if($CleanupFails){
        Assert-FailureControl (Contains-DiagnosticException $observed $expectedCleanupFailure) 'Actual cleanup lost the original restore failure.'
        Assert-FailureControl ($cleanupEvidence.restore -eq 1 -and $cleanupEvidence.path -eq 0) 'Actual failed restore continued or skipped its original cleanup control flow.'
        $events=@(Get-Content -LiteralPath $diagnostics.journal | ForEach-Object { $_|ConvertFrom-Json -AsHashtable })
        $errors=@($events|Where-Object { $_.phase -ceq 'final-cleanup' -and $_.status -ceq 'error' })
        $ends=@($events|Where-Object { $_.phase -ceq 'final-cleanup' -and $_.status -ceq 'end' })
        Assert-FailureControl ($errors.Count -eq 1 -and $ends.Count -eq 0) 'Actual failed cleanup was falsely labeled successful completion.'
    }else{
    Assert-FailureControl ($cleanupEvidence.restore -eq 1 -and $cleanupEvidence.path -eq 1) 'Actual original cleanup was skipped or duplicated.'
    $journal=Read-P4kxMsiPhaseJournal $diagnostics.journal
    Assert-FailureControl ($journal.active.Count -eq 1 -and $journal.active[0] -ceq 'worker') 'Actual completed cleanup remains falsely open after the primary verification failure.'
    $events=@(Get-Content -LiteralPath $diagnostics.journal | ForEach-Object { $_|ConvertFrom-Json -AsHashtable })
    $ends=@($events|Where-Object { $_.phase -ceq 'final-cleanup' -and $_.status -ceq 'end' })
    Assert-FailureControl ($ends.Count -eq 1) 'Actual finally lacks exactly one cleanup completion event.'
    }
    Stop-P4kxMsiPhase $diagnostics $worker -Failure $primaryFailure
}

function Test-ActualPreconditionTelemetryFailure {
    $phaseDirectory=Join-Path $root 'preconditions-phases'
    New-Item -ItemType Directory -Path $phaseDirectory | Out-Null
    $diagnostics=New-P4kxMsiPhaseContext $phaseDirectory $target
    $worker=Start-P4kxMsiPhase $diagnostics 'worker'
    $preconditionPhase=Start-P4kxMsiPhase $diagnostics 'preamble/preconditions'
    $expectedTelemetryFailure=[IO.IOException]::new('synthetic actual preconditions telemetry failure')
    $evidence=@{restore=0;path=0;unexpected_native=0;markers_present=$false}
    $profileFolder=Join-Path $root 'preconditions-profile'
    $logs=$root;$context=@{};$fixture=@{};$originalRaw=$null;$originalPath=$null;$PreviousMsi=$null
    $actualStop=(Get-Item 'Function:Stop-P4kxMsiPhase').ScriptBlock
    function Stop-P4kxMsiPhase {
        param([hashtable]$Context,[hashtable]$Token,[Exception]$Failure=$null,[switch]$RequiredCleanup)
        if($null -ne $Token -and $Token.phase -ceq 'preamble/preconditions'){
            $evidence.markers_present=(Test-Path -LiteralPath (Join-Path $profileFolder 'settings.toml')) -and (Test-Path -LiteralPath (Join-Path $profileFolder 'archive.key'))
            throw $expectedTelemetryFailure
        }
        & $actualStop $Context $Token -Failure $Failure -RequiredCleanup:$RequiredCleanup
    }
    function Restore-P4kxMsiVerificationBaseline { $evidence.restore++ }
    function Get-P4kxUserPathState { return $null }
    function Write-P4kxPathTransition { $evidence.path++ }
    function Invoke-P4kxMsiexec { $evidence.unexpected_native++; throw 'Forbidden native adapter in infrastructure oracle.' }
    function Get-P4kxMsiMetadata { $evidence.unexpected_native++; throw 'Forbidden metadata adapter in infrastructure oracle.' }
    function Invoke-P4kxMsiRawPathCases { $evidence.unexpected_native++; return @() }
    function Invoke-P4kxMsiForeignEditProbe { $evidence.unexpected_native++; return @{} }
    $observed=$null
    try { & $actualProfileLifecycle } catch { $observed=$_.Exception }
    Assert-FailureControl $evidence.markers_present 'Actual synthetic profile markers did not exist at the injected telemetry boundary.'
    Assert-FailureControl (Contains-DiagnosticException $observed $expectedTelemetryFailure) 'Actual profile/lifecycle suffix replaced the original telemetry failure.'
    Assert-FailureControl ($evidence.unexpected_native -eq 0) 'Actual preconditions fault did not stop before installer/runtime adapters.'
    Assert-FailureControl ($evidence.restore -eq 1 -and $evidence.path -eq 1 -and -not (Test-Path -LiteralPath $profileFolder)) 'Actual telemetry fault bypassed original profile/marker cleanup.'
    Stop-P4kxMsiPhase $diagnostics $worker -Failure $expectedTelemetryFailure
}

$savedActions=$env:GITHUB_ACTIONS;$savedRunner=$env:RUNNER_ENVIRONMENT;$savedDeadline=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
$failures=[Collections.Generic.List[Exception]]::new()
try {
    $env:GITHUB_ACTIONS='true';$env:RUNNER_ENVIRONMENT='github-hosted'
    $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddMinutes(20).ToString('o')
    foreach($case in @(
        @{name='worker-journal-write';action={Test-SummaryPersistence $true 'worker-journal-write'}},
        @{name='journal-write';action={Test-SummaryPersistence $false 'journal-write'}},
        @{name='actual-cleanup-finally';action={Test-ActualCleanupFinally}},
        @{name='actual-cleanup-error';action={Test-ActualCleanupFinally -CleanupFails}},
        @{name='actual-preconditions-telemetry';action={Test-ActualPreconditionTelemetryFailure}}
    )){
        try{& $case.action;Write-Host ('PASS: '+$case.name)}
        catch{$failures.Add($_.Exception);Write-Host ('FAIL: '+$case.name)}
    }
} finally {
    $env:GITHUB_ACTIONS=$savedActions;$env:RUNNER_ENVIRONMENT=$savedRunner;$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$savedDeadline
}
if($failures.Count){throw [AggregateException]::new('Actual MSI diagnostic control-flow infrastructure checks failed.',$failures)}
'PASS: five actual diagnostic failure-control paths; all native adapters mocked, no installer/Registry/product GUI, not the original timeout reproduction.'
