param(
    [string]$Target,[string]$Msi,[string]$NextMsi,[string]$NextVersion,[string]$UpgradeCode,
    [string]$OtherArchitectureMsi,[string]$FixtureDirectory,[string]$OutputDirectory,
    [string]$RegistryFixture,[string]$RegistryFixtureSha256,[string]$HelperExecutable,[string]$PreviousMsi,
    [switch]$LifecycleWorker,[string]$WorkerLocation,[string]$WorkerProcessDirectory
)
$ErrorActionPreference='Stop'
if(-not $LifecycleWorker -or $Msi -cnotin @('complete','timeout','outer-runner')){throw 'Invalid diagnostic-only fixture invocation.'}
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false,$true)
Set-Location -LiteralPath $WorkerLocation
[IO.Directory]::SetCurrentDirectory($WorkerProcessDirectory)
. (Join-Path $FixtureDirectory 'msi-diagnostic-phase.ps1')
$context=New-P4kxMsiPhaseContext (Join-Path $OutputDirectory 'diagnostics') $Target
$worker=Start-P4kxMsiPhase $context 'worker'
$wait=Start-P4kxMsiPhase $context 'fixture/wait'
[Console]::Out.WriteLine('feedback-fixture-stdout')
[Console]::Out.Flush()
[Console]::Error.WriteLine('feedback-fixture-stderr')
[Console]::Error.Flush()
if($Msi -ceq 'timeout'){Start-Sleep -Seconds 20}
elseif($Msi -ceq 'outer-runner'){Start-Sleep -Seconds 120}
Stop-P4kxMsiPhase $context $wait
Stop-P4kxMsiPhase $context $worker
