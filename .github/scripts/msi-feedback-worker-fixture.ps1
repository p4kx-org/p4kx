param(
    [string]$Target,[string]$Msi,[string]$NextMsi,[string]$NextVersion,[string]$UpgradeCode,
    [string]$OtherArchitectureMsi,[string]$FixtureDirectory,[string]$OutputDirectory,
    [string]$RegistryFixture,[string]$RegistryFixtureSha256,[string]$HelperExecutable,[string]$PreviousMsi,
    [switch]$AdmissionOnly,[switch]$LifecycleWorker,[string]$WorkerLocation,[string]$WorkerProcessDirectory
)
$ErrorActionPreference='Stop'
if(-not $LifecycleWorker -or $Msi -cnotin @('complete','timeout','outer-runner','admission','observer-wait') -or
    [bool]$AdmissionOnly -ne ($Msi -ceq 'admission')){throw 'Invalid diagnostic-only fixture invocation.'}
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false,$true)
Set-Location -LiteralPath $WorkerLocation
[IO.Directory]::SetCurrentDirectory($WorkerProcessDirectory)
. (Join-Path $FixtureDirectory 'msi-diagnostic-phase.ps1')
. (Join-Path $FixtureDirectory 'msi-diagnostic-budget.ps1')
$context=New-P4kxMsiPhaseContext (Join-Path $OutputDirectory 'diagnostics') $Target
$worker=Start-P4kxMsiPhase $context 'worker'
$wait=Start-P4kxMsiPhase $context 'fixture/wait'
[Console]::Out.WriteLine('feedback-fixture-stdout')
[Console]::Out.Flush()
[Console]::Error.WriteLine('feedback-fixture-stderr')
[Console]::Error.Flush()
if($Msi -ceq 'observer-wait'){
    [IO.File]::WriteAllText((Join-Path $OutputDirectory 'observer-control-active.json'),'{"schema":1,"role":"harmless-msi-observer-active"}')
    $limit=ConvertFrom-P4kxMsiBudgetTimestamp $env:P4KX_MSI_WORKER_DEADLINE_UTC
    while(-not [IO.File]::Exists((Join-Path $OutputDirectory 'observer-control-release.json'))){
        if([DateTimeOffset]::UtcNow -ge $limit){throw 'Harmless observer fixture release deadline reached.'}
        Start-Sleep -Milliseconds 25
    }
}
elseif($Msi -ceq 'timeout'){Start-Sleep -Seconds 20}
elseif($Msi -ceq 'outer-runner'){Start-Sleep -Seconds 120}
Stop-P4kxMsiPhase $context $wait
Stop-P4kxMsiPhase $context $worker
