param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-runner.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'MSI infrastructure test directory must be new.' }
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$root = [IO.Path]::GetFullPath($Directory)
$target = if ([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq 'Arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }

function Assert-DiagnosticTest([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$deadline = [DateTimeOffset]::UtcNow.AddMinutes(20).ToString('o')
Assert-DiagnosticTest ((Get-P4kxMsiWorkerBudget $deadline) -eq 600000) 'Worker budget is not capped at ten minutes.'
$shortBudget = Get-P4kxMsiWorkerBudget ([DateTimeOffset]::UtcNow.AddSeconds(30).ToString('o'))
Assert-DiagnosticTest ($shortBudget -gt 0 -and $shortBudget -le 30000) 'Late worker did not retain only its remaining job allowance.'
foreach ($invalid in @('', 'invalid', [DateTimeOffset]::UtcNow.AddMinutes(-1).ToString('o'), [DateTimeOffset]::UtcNow.AddMinutes(36).ToString('o'))) {
    $rejected=$false
    try { Get-P4kxMsiWorkerBudget $invalid | Out-Null } catch { $rejected=$true }
    Assert-DiagnosticTest $rejected 'Invalid, expired or inflated deadline was accepted.'
}

$stub = Join-Path $root 'worker argv fixture.ps1'
@'
param([string]$Target,[string]$Msi,[string]$NextMsi,[string]$NextVersion,[string]$UpgradeCode,[string]$OtherArchitectureMsi,[string]$FixtureDirectory,[string]$OutputDirectory,[string]$RegistryFixture,[string]$RegistryFixtureSha256,[string]$HelperExecutable,[string]$PreviousMsi,[switch]$LifecycleWorker,[string]$WorkerLocation,[string]$WorkerProcessDirectory)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false,$true)
Set-Location -LiteralPath $WorkerLocation
[IO.Directory]::SetCurrentDirectory($WorkerProcessDirectory)
@{parameters=$PSBoundParameters; worker=[bool]$LifecycleWorker; location=(Get-Location).ProviderPath; native=[IO.Directory]::GetCurrentDirectory()} | ConvertTo-Json -Depth 4 -Compress
'@ | Set-Content -LiteralPath $stub -Encoding utf8NoBOM
$locationDirectory=Join-Path $root 'PowerShell location'
$nativeDirectory=Join-Path $root 'native process directory'
New-Item -ItemType Directory -Path $locationDirectory,$nativeDirectory | Out-Null
$parameters=[ordered]@{Target=$target;Msi=('relative payload/space '+[char]0x00E9+'/current.msi');NextMsi='relative next.msi';NextVersion='0.4.3';UpgradeCode='synthetic upgrade';OtherArchitectureMsi='opposite.msi';FixtureDirectory='fixture';OutputDirectory='relative output';RegistryFixture='fixture.exe';RegistryFixtureSha256=('a'*64);HelperExecutable='helper.exe'}
$bindingTokens=$null;$bindingErrors=$null
$bindingAst=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'check-msi.ps1'),[ref]$bindingTokens,[ref]$bindingErrors)
if($bindingErrors.Count -ne 0 -or $null -eq $bindingAst.ParamBlock){throw 'Actual MSI parameter block does not parse.'}
$bindingProbe=[ScriptBlock]::Create($bindingAst.ParamBlock.Extent.Text+[char]10+'Get-P4kxMsiWorkerArguments $stub $PSBoundParameters (Get-Location).ProviderPath ([IO.Directory]::GetCurrentDirectory())')

$originalLocation=Get-Location
$originalNative=[IO.Directory]::GetCurrentDirectory()
try {
    Set-Location -LiteralPath $locationDirectory
    [IO.Directory]::SetCurrentDirectory($nativeDirectory)
    foreach ($withPrevious in @($false,$true)) {
        if ($withPrevious) { $parameters['PreviousMsi']=('predecessor space '+[char]0x00E9+'.msi') }
        $arguments=& $bindingProbe @parameters
        $result=Invoke-P4kxPortalProcess (Get-Process -Id $PID).Path $arguments $root $(if($withPrevious){'argv-previous'}else{'argv-current'})
        $actual=$result.stdout | ConvertFrom-Json -AsHashtable
        Assert-DiagnosticTest ($result.exit_code -eq 0 -and $actual.worker -eq $true) 'Actual hidden infrastructure worker did not receive its private switch.'
        foreach ($name in $parameters.Keys) { Assert-DiagnosticTest ($actual.parameters[$name] -ceq $parameters[$name]) "Worker argument changed: $name" }
        Assert-DiagnosticTest ($actual.parameters.ContainsKey('PreviousMsi') -eq $withPrevious) 'Unbound predecessor was fabricated.'
        Assert-DiagnosticTest ($actual.location -ceq $locationDirectory -and $actual.native -ceq $nativeDirectory) 'PowerShell and native current directories were not independently preserved.'
    }
} finally { Set-Location -LiteralPath $originalLocation.Path; [IO.Directory]::SetCurrentDirectory($originalNative) }

$phaseDirectory=Join-Path $root 'phases'
New-Item -ItemType Directory -Path $phaseDirectory | Out-Null
$context=New-P4kxMsiPhaseContext $phaseDirectory $target
$worker=Start-P4kxMsiPhase $context 'worker'
$leaf=Start-P4kxMsiPhase $context 'preamble/current-metadata'
$pending=Read-P4kxMsiPhaseJournal $context.journal
Assert-DiagnosticTest ($pending.events -eq 2 -and $pending.active.Count -eq 2 -and $pending.terminal -eq $null) 'Durable begin events did not retain the actual open scopes.'
Stop-P4kxMsiPhase $context $leaf
Stop-P4kxMsiPhase $context $worker
$complete=Read-P4kxMsiPhaseJournal $context.journal
Assert-DiagnosticTest ($complete.complete -and $complete.events -eq 4 -and $complete.active.Count -eq 0 -and $complete.terminal -ceq 'end') 'Completed phase journal was not independently readable.'
$bytes=[IO.File]::ReadAllBytes($context.journal)
[IO.File]::AppendAllText($context.journal, '{"incomplete":')
$partial=Read-P4kxMsiPhaseJournal $context.journal
Assert-DiagnosticTest (-not $partial.complete -and $partial.events -eq 4) 'Partial trailing event was promoted to a completed journal.'
[IO.File]::WriteAllBytes($context.journal,$bytes)
$firstLine=([IO.File]::ReadAllLines($context.journal))[0]
$malformed=$firstLine | ConvertFrom-Json -AsHashtable
$malformed['argv']='synthetic forbidden diagnostic field'
$rejected=$false
try { Assert-P4kxMsiPhaseEvent $malformed } catch { $rejected=$true }
Assert-DiagnosticTest $rejected 'Phase contract accepted an unapproved argv field.'

$forward=New-P4kxMsiPhaseForwarder
$console=[Console]::Out
$captured=[IO.StringWriter]::new()
try {
    [Console]::SetOut($captured)
    & $forward stdout ('private untagged infrastructure line'+[char]10+'[P4KX-MSI-PH')
    & $forward stdout ('ASE]'+$firstLine.Substring(0,17))
    & $forward stdout ($firstLine.Substring(17)+[char]10)
    & $forward stderr ('private stderr'+[char]10)
} finally { [Console]::SetOut($console) }
Assert-DiagnosticTest ($captured.ToString() -ceq '[P4KX-MSI-PHASE]'+$firstLine+[Environment]::NewLine) 'Fragmented phase forwarding leaked non-phase output or lost a phase.'
$captured.Dispose()

$failureContext=New-P4kxMsiPhaseContext (New-Item -ItemType Directory -Path (Join-Path $root 'cleanup-phase')).FullName $target
$failureContext.journal=Join-Path $root 'cleanup-phase'
$cleanup=Start-P4kxMsiPhase $failureContext 'final-cleanup' -RequiredCleanup
$cleanupRan=$true
Assert-DiagnosticTest ($null -eq $cleanup -and $cleanupRan -and $failureContext.failures.Count -eq 1) 'Telemetry failure prevented original required cleanup instead of retaining its failure.'

$primary=[InvalidOperationException]::new('synthetic primary operation failure')
$combined=Get-P4kxMsiDiagnosticFailure $failureContext $primary
Assert-DiagnosticTest ($combined -is [AggregateException] -and $combined.InnerExceptions.Count -eq 2 -and [object]::ReferenceEquals($combined.InnerExceptions[0],$primary) -and $failureContext.failures.Count -eq 0) 'Deferred telemetry error was swallowed or replaced the primary failure.'

$probe=Join-Path $root 'portal infrastructure.ps1'
@'
param([string]$Mode)
if ($Mode -ceq 'silent') { Start-Sleep -Seconds 20; exit 0 }
[Console]::Out.Write('infrastructure-stdout')
[Console]::Error.Write('infrastructure-stderr')
if ($Mode -ceq 'failure') { exit 37 }
'@ | Set-Content -LiteralPath $probe -Encoding utf8NoBOM
$executable=(Get-Process -Id $PID).Path
$baseline=Invoke-P4kxPortalProcess $executable @('-NoProfile','-NonInteractive','-File',$probe,'success') $root 'portal-default'
Assert-DiagnosticTest ($baseline.exit_code -eq 0 -and $baseline.stdout -ceq 'infrastructure-stdout' -and $baseline.stderr -ceq 'infrastructure-stderr') 'Default Portal stdout/stderr contract changed.'
$seen=@{stdout='';stderr=''}
$observer={param($stream,$chunk) $seen[$stream]+=$chunk}.GetNewClosure()
$observed=Invoke-P4kxPortalProcess $executable @('-NoProfile','-NonInteractive','-File',$probe,'success') $root 'portal-observed' -OnOutput $observer
Assert-DiagnosticTest ($observed.stdout -ceq $baseline.stdout -and $observed.stderr -ceq $baseline.stderr -and $seen.stdout -ceq $baseline.stdout -and $seen.stderr -ceq $baseline.stderr) 'Optional channel observer changed captured bytes or lost a channel.'
$failed=Invoke-P4kxPortalProcess $executable @('-NoProfile','-NonInteractive','-File',$probe,'failure') $root 'portal-exit'
Assert-DiagnosticTest ($failed.exit_code -eq 37) 'Real child nonzero exit was reclassified as success.'
foreach($mode in @('deadline','observer')) {
    $observation=@{};$rejected=$false
    $modeArgument=if($mode -ceq 'deadline'){'silent'}else{'success'}
    $callback=if($mode -ceq 'observer'){{throw 'synthetic infrastructure callback failure'}}else{$null}
    try { Invoke-P4kxPortalProcess $executable @('-NoProfile','-NonInteractive','-File',$probe,$modeArgument) $root ('portal-'+$mode) -Milliseconds 2000 -OnOutput $callback -Observation $observation | Out-Null } catch { $rejected=$true }
    Assert-DiagnosticTest ($rejected -and $observation.started -and $observation.exited -and $observation.signal_contract -ceq 'direct-process-only') 'Infrastructure failure lost the existing direct-process completion observation.'
    Assert-DiagnosticTest (-not (Get-Process -Id $observation.pid -ErrorAction SilentlyContinue)) 'Infrastructure test left its directly owned child alive.'
}
'PASS: MSI diagnostic infrastructure only: argv/cwd, budgets, durable phases, bounded parser/forwarding, cleanup-error retention and real Portal observer/exit/watchdog; no MSI, Registry, product GUI or original timeout reproduction.'
