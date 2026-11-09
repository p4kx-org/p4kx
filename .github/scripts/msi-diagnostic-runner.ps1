$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-process.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-phase.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')

function Get-P4kxMsiWorkerArguments {
    param([string]$Script, [System.Collections.IDictionary]$Parameters, [string]$Location, [string]$NativeDirectory)
    $arguments = [Collections.Generic.List[string]]::new()
    foreach ($value in @('-NoLogo','-NoProfile','-NonInteractive','-File',$Script)) { $arguments.Add($value) }
    foreach ($name in @('Target','Msi','NextMsi','NextVersion','UpgradeCode','OtherArchitectureMsi','FixtureDirectory','OutputDirectory','RegistryFixture','RegistryFixtureSha256','HelperExecutable','PreviousMsi')) {
        if ($Parameters.Keys -contains $name) { $arguments.Add('-'+$name); $arguments.Add([string]$Parameters[$name]) }
    }
    foreach ($value in @('-LifecycleWorker','-WorkerLocation',$Location,'-WorkerProcessDirectory',$NativeDirectory)) { $arguments.Add($value) }
    return ,$arguments.ToArray()
}

function Write-P4kxMsiDiagnosticInputs {
    param([System.Collections.IDictionary]$Parameters, [string]$Directory)
    $inputs = @()
    foreach ($name in @('Msi','NextMsi','OtherArchitectureMsi','PreviousMsi','RegistryFixture','HelperExecutable')) {
        if ($Parameters.Keys -notcontains $name -or -not $Parameters[$name]) { continue }
        $inputs += @{role=$name; sha256=(Get-FileHash -LiteralPath $Parameters[$name] -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    $fixtures = @(Get-ChildItem -LiteralPath $Parameters['FixtureDirectory'] -File)
    if ($fixtures.Count -gt 32) { throw 'MSI synthetic fixture inventory exceeds its bound.' }
    foreach ($file in $fixtures) { $inputs += @{role='synthetic-fixture'; name=$file.Name; sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()} }
    @{schema=1; role='native-msi-diagnostic-inputs'; revision=$env:GITHUB_SHA; run=$env:GITHUB_RUN_ID; attempt=$env:GITHUB_RUN_ATTEMPT; target=$Parameters['Target']; inputs=$inputs} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Directory 'diagnostic-inputs.json') -Encoding utf8NoBOM
}

function Write-P4kxMsiDiagnosticSources {
    param([System.Collections.IDictionary]$Parameters, [string]$ScriptDirectory, [string]$Directory)
    $sources = @()
    foreach ($name in @('check-msi.ps1','msi-diagnostic-runner.ps1','msi-diagnostic-budget.ps1','msi-diagnostic-phase.ps1','msi-diagnostic-utf8.ps1','portal-process.ps1','package-process.ps1','msi-path-verification.ps1','msi-raw-fixture.ps1','msi-installation.ps1','msi-invocation.ps1','msi-database.ps1','msi-embedded-helper.ps1','check-package-runtime.ps1')) {
        $sources += @{name=$name; sha256=(Get-FileHash -LiteralPath (Join-Path $ScriptDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    @{schema=1; role='native-msi-diagnostic-sources'; revision=$env:GITHUB_SHA; run=$env:GITHUB_RUN_ID; attempt=$env:GITHUB_RUN_ATTEMPT; target=$Parameters['Target']; process_arch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString(); sources=$sources} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Directory 'diagnostic-sources.json') -Encoding utf8NoBOM
}

function Invoke-P4kxMsiDiagnosticWorker {
    param([string]$Script, [System.Collections.IDictionary]$Parameters)
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') { throw 'MSI verification requires a disposable native Windows runner.' }
    if ((Get-Location).Provider.Name -cne 'FileSystem') { throw 'MSI worker requires a filesystem PowerShell location.' }
    $milliseconds = Get-P4kxMsiWorkerBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC
    $output = [IO.Path]::GetFullPath($Parameters['OutputDirectory'])
    $directory = Join-Path $output 'diagnostics'
    New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
    $arguments = Get-P4kxMsiWorkerArguments $Script $Parameters (Get-Location).ProviderPath ([IO.Directory]::GetCurrentDirectory())
    $observation = @{}
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $result=$null; $failure=$null; $journal=$null; $inspectionFailure=$null
    @{schema=1;role='native-msi-diagnostic-start';revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        target=$Parameters['Target'];process_arch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString();
        deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;budget_ms=$milliseconds;parent_pid=$PID;
        worker_script=[IO.Path]::GetFileName($Script);worker_sha256=(Get-FileHash -LiteralPath $Script -Algorithm SHA256).Hash.ToLowerInvariant();
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $directory 'diagnostic-start.json') -Encoding utf8NoBOM
    [Console]::Out.WriteLine('[P4KX-MSI-WORKER] launch-begin')
    [Console]::Out.Flush()
    try {
        $result = Invoke-P4kxPortalProcess (Get-Process -Id $PID).Path $arguments $directory 'lifecycle-worker' -Milliseconds $milliseconds -MaximumBytes 4MB -OnOutput (New-P4kxMsiPhaseForwarder) -Observation $observation
        if ($result.exit_code -ne 0) { throw "Native MSI worker exited $($result.exit_code); see captured diagnostics." }
    } catch { $failure=$_.Exception }
    try { $journal=Read-P4kxMsiPhaseJournal (Join-Path $directory 'phase-journal.jsonl') } catch { $inspectionFailure=$_.Exception }
    $proven = $null -eq $failure -and $null -eq $inspectionFailure -and $null -ne $result -and $result.exit_code -eq 0 -and
        $journal.present -and $journal.complete -and $journal.terminal -ceq 'end' -and $journal.active.Count -eq 0
    $persistenceFailure=$null
    try {
    @{schema=1; role='native-msi-diagnostic-result'; revision=$env:GITHUB_SHA; run=$env:GITHUB_RUN_ID; attempt=$env:GITHUB_RUN_ATTEMPT; target=$Parameters['Target'];
        deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC; budget_ms=$milliseconds; elapsed_ms=$timer.ElapsedMilliseconds;
        worker=$observation; completed_success=$proven; native_completion_unproven=(-not $proven); worker_result_returned=($null -ne $result); mutation_authority=$false;
        journal=$journal; journal_invalid=($null -ne $inspectionFailure)} |
        ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $directory 'diagnostic-result.json') -Encoding utf8NoBOM
    } catch { $persistenceFailure=$_.Exception }
    $diagnosticFailures=[Collections.Generic.List[Exception]]::new()
    foreach($originalFailure in @($failure,$inspectionFailure,$persistenceFailure)){if($null -ne $originalFailure){$diagnosticFailures.Add($originalFailure)}}
    if($diagnosticFailures.Count -gt 1){throw [AggregateException]::new('MSI worker, journal or result persistence failed.',$diagnosticFailures)}
    if($diagnosticFailures.Count -eq 1){throw $diagnosticFailures[0]}
    if (-not $proven) { throw 'MSI worker completion lacks a complete successful diagnostic journal.' }
}
