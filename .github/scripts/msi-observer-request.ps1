$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-runner.ps1')

function Get-P4kxMsiObserverSources {
    Get-P4kxMsiDiagnosticSources $PSScriptRoot
    foreach ($name in @('msi-observer-request.ps1','msi-observer-process.ps1','msi-observer-finish-control.ps1','msi-observer-finish-progress.ps1','msi-observer-finish-supervisor.ps1','msi-observer-evidence.ps1','msi-observer-parent.ps1','invoke-msi-observer.ps1','msi-observer-progress.ps1','msi-observer-snapshot-caller.ps1','msi-observer-snapshot-delay.ps1','msi-observer-snapshot-worker.ps1','msi-snapshot-console.ps1','msi-detached-control.ps1','msi-feedback-worker-fixture.ps1','platform-contract.ps1')) {
        @{name=$name;sha256=(Get-P4kxDetachedDigest (Join-Path $PSScriptRoot $name))}
    }
}

function Assert-P4kxMsiObserverInvocation {
    param($Parameters, [string]$Target, [string]$Mode)
    $allowed = @('Target','Msi','NextMsi','NextVersion','UpgradeCode','OtherArchitectureMsi','FixtureDirectory','OutputDirectory','RegistryFixture','RegistryFixtureSha256','HelperExecutable','PreviousMsi')
    if ($Mode -ceq 'Control') { $allowed += 'AdmissionOnly' }
    if ($Parameters.Target -cne $Target -or @($Parameters.Keys | Where-Object { $_ -cnotin $allowed }).Count -ne 0 -or
        [string]::IsNullOrEmpty($Parameters.OutputDirectory)) { throw 'MSI observer invocation identity or authority differs.' }
    if ($Mode -ceq 'Control' -and ($Parameters.Msi -cnotin @('observer-wait','timeout','complete','admission') -or
        $Parameters.FixtureDirectory -cne $PSScriptRoot)) { throw 'Invalid harmless observer fixture invocation.' }
}

function New-P4kxMsiObserverRequest {
    param([string]$Directory, [string]$Invocation, [ValidateSet('Lifecycle','Control')][string]$Mode, [int]$ControlMilliseconds)
    if (-not $IsWindows -or [IO.Path]::Exists($Directory)) { throw 'MSI observer requires a fresh native Windows directory.' }
    $row = Get-P4kxPlatform -Target $env:P4KX_TARGET
    Assert-P4kxNativeHost $row
    if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
        [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString() -ine $row.runner_arch -or
        $null -eq (Get-P4kxDetachedTrackingDigest)) { throw 'MSI observer requires native hosted-runner identity and original process tracking.' }
    $parameters = Read-P4kxDetachedReceipt $Invocation
    Assert-P4kxMsiObserverInvocation $parameters $row.target $Mode
    $prepared = [DateTimeOffset]::UtcNow
    if ($Mode -ceq 'Lifecycle') {
        Assert-P4kxMsiStepAdmission $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $env:P4KX_MSI_STEP_MINUTES $env:P4KX_MSI_STEP_COMPUTED_AT_UTC $prepared
        $milliseconds = [int]$env:P4KX_MSI_STEP_MINUTES*60000
        $script = 'check-msi.ps1'
    } else {
        if ($ControlMilliseconds -le 0 -or $ControlMilliseconds -gt 100000) { throw 'Invalid harmless observer allowance.' }
        $milliseconds = [int][Math]::Min($ControlMilliseconds, (Get-P4kxMsiRemainingBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC $prepared))
        $script = 'msi-feedback-worker-fixture.ps1'
    }
    New-Item -ItemType Directory -Path $Directory | Out-Null
    Write-P4kxMsiDiagnosticSources $parameters $PSScriptRoot $Directory
    return @{schema=1;role='native-msi-observer-request';mode=$Mode;revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        target=$row.target;directory=[IO.Path]::GetFullPath($Directory);invocation=[IO.Path]::GetFullPath($Invocation);invocation_sha256=(Get-P4kxDetachedDigest $Invocation);
        output_directory=[IO.Path]::GetFullPath($parameters.OutputDirectory);location=(Get-Location).ProviderPath;native_directory=[IO.Directory]::GetCurrentDirectory();
        deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;observer_deadline_utc=$prepared.AddMilliseconds($milliseconds).ToString('o');
        prepared_at_utc=$prepared.ToString('o');observer_budget_ms=$milliseconds;step_minutes=$env:P4KX_MSI_STEP_MINUTES;step_computed_at_utc=$env:P4KX_MSI_STEP_COMPUTED_AT_UTC;
        script=$script;script_sha256=(Get-P4kxDetachedDigest (Join-Path $PSScriptRoot $script));tracking_sha256=(Get-P4kxDetachedTrackingDigest);sources=@(Get-P4kxMsiObserverSources);
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
}

function Assert-P4kxMsiObserverBinding {
    param($Request, [string]$Directory)
    $row = Get-P4kxPlatform -Target $env:P4KX_TARGET
    Assert-P4kxNativeHost $row
    if ($Request.schema -ne 1 -or $Request.role -cne 'native-msi-observer-request' -or $Request.mode -cnotin @('Lifecycle','Control') -or
        $Request.directory -cne [IO.Path]::GetFullPath($Directory) -or $Request.revision -cne $env:GITHUB_SHA -or
        $Request.run -cne $env:GITHUB_RUN_ID -or $Request.attempt -cne $env:GITHUB_RUN_ATTEMPT -or $Request.target -cne $row.target -or
        $Request.deadline_utc -cne $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC -or $Request.tracking_sha256 -cne (Get-P4kxDetachedTrackingDigest) -or
        $Request.completed_success -ne $false -or $Request.native_completion_unproven -ne $true -or $Request.mutation_authority -ne $false -or
        $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $null -eq $Request.tracking_sha256 -or
        [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString() -ine $row.runner_arch -or
        $Request.invocation_sha256 -cne (Get-P4kxDetachedDigest $Request.invocation)) { throw 'MSI observer source, invocation, native CPU, tracking or original deadline differs.' }
    $parameters = Read-P4kxDetachedReceipt $Request.invocation
    Assert-P4kxMsiObserverInvocation $parameters $Request.target $Request.mode
    if ($Request.output_directory -cne [IO.Path]::GetFullPath($parameters.OutputDirectory)) { throw 'MSI observer output differs from its original invocation.' }
    $prepared = ConvertFrom-P4kxMsiBudgetTimestamp $Request.prepared_at_utc
    $observerDeadline = ConvertFrom-P4kxMsiBudgetTimestamp $Request.observer_deadline_utc
    $deadline = ConvertFrom-P4kxMsiBudgetTimestamp $Request.deadline_utc
    if ($prepared -gt [DateTimeOffset]::UtcNow -or $Request.observer_budget_ms -le 0 -or
        $observerDeadline -ne $prepared.AddMilliseconds($Request.observer_budget_ms) -or $observerDeadline -gt $deadline) { throw 'MSI observer allowance differs from its original deadline.' }
    $script = if ($Request.mode -ceq 'Lifecycle') {'check-msi.ps1'} else {'msi-feedback-worker-fixture.ps1'}
    if ($Request.mode -ceq 'Lifecycle') {
        Assert-P4kxMsiStepAdmission $Request.deadline_utc $Request.step_minutes $Request.step_computed_at_utc $prepared
        if ($Request.observer_budget_ms -ne [int]$Request.step_minutes*60000) { throw 'MSI observer changed its original step allowance.' }
    } elseif ($Request.observer_budget_ms -gt 100000) { throw 'Harmless observer allowance exceeds its control bound.' }
    if ($Request.script -cne $script -or $Request.script_sha256 -cne (Get-P4kxDetachedDigest (Join-Path $PSScriptRoot $script))) { throw 'MSI observer worker selection differs.' }
    $expected = @(Get-P4kxMsiObserverSources)
    $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if ($Request.sources.Count -ne $expected.Count) { throw 'MSI observer source inventory is incomplete.' }
    foreach ($source in $Request.sources) {
        if (-not $names.Add($source.name) -or @($expected | Where-Object { $_.name -ceq $source.name -and $_.sha256 -ceq $source.sha256 }).Count -ne 1) { throw 'MSI observer source bytes or inventory differ.' }
    }
}
