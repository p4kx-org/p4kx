param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][ValidateSet('complete','timeout')][string]$Mode,
    [Parameter(Mandatory)][string]$Directory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-phase.ps1')
$row=Get-P4kxPlatform $Target
Assert-P4kxNativeHost $row
$processArch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
if(-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $processArch -ine $row.runner_arch){throw 'Probe requires the disposable native Windows CPU.'}
if($env:GITHUB_SHA -cnotmatch '\A[0-9a-f]{40}\z' -or $env:P4KX_MSI_PROBE_HEAD -cnotmatch '\A[0-9a-f]{40}\z' -or $env:GITHUB_RUN_ID -cnotmatch '\A[1-9][0-9]*\z' -or $env:GITHUB_RUN_ATTEMPT -cnotmatch '\A[1-9][0-9]*\z'){throw 'Invalid probe source/run binding.'}
if(Test-Path -LiteralPath $Directory){throw 'Diagnostic-only probe directory must be new.'}
$feedback=Join-Path $Directory 'feedback'
$diagnostics=Join-Path $Directory 'diagnostics'
New-Item -ItemType Directory -Path $feedback,$diagnostics -Force | Out-Null
$sources=@(foreach($name in @('invoke-msi-feedback-probe.ps1','msi-diagnostic-phase.ps1','msi-diagnostic-utf8.ps1')){
    @{name=$name;sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $name) -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$binding=@{schema=1;role='native-msi-runner-fuse-probe';revision=$env:GITHUB_SHA;head_revision=$env:P4KX_MSI_PROBE_HEAD;
    run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$Target;process_arch=$processArch;mode=$Mode;sources=$sources}
$binding | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $feedback 'step-feedback.json') -Encoding utf8NoBOM
$context=New-P4kxMsiPhaseContext $diagnostics $Target
$worker=Start-P4kxMsiPhase $context 'worker'
$phase=Start-P4kxMsiPhase $context 'fixture/runner-stop'
$result=$binding.Clone()
$result.completed_success=$false;$result.native_completion_unproven=$true;$result.mutation_authority=$false
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $diagnostics 'diagnostic-result.json') -Encoding utf8NoBOM
'Flushed diagnostic-only active scopes; no MSI/Registry/product payload.' | Set-Content -LiteralPath (Join-Path $diagnostics 'probe.log') -Encoding utf8NoBOM
if($Mode -ceq 'timeout'){Start-Sleep -Seconds 120}
Stop-P4kxMsiPhase $context $phase
Stop-P4kxMsiPhase $context $worker
$result.completed_success=$true;$result.native_completion_unproven=$false
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $diagnostics 'diagnostic-result.json') -Encoding utf8NoBOM
