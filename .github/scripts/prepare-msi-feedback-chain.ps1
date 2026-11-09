param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][ValidateSet('complete','inner-watchdog','outer-runner')][string]$Mode,
    [Parameter(Mandatory)][string]$Directory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-feedback-chain-binding.ps1')
Assert-P4kxNativeHost (Get-P4kxPlatform $Target)
if(-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted'){throw 'Chain requires disposable native Windows.'}
if($env:GITHUB_SHA -cnotmatch '\A[0-9a-f]{40}\z' -or $env:P4KX_MSI_PROBE_HEAD -cnotmatch '\A[0-9a-f]{40}\z' -or
    $env:GITHUB_RUN_ID -cnotmatch '\A[1-9][0-9]*\z' -or $env:GITHUB_RUN_ATTEMPT -cnotmatch '\A[1-9][0-9]*\z' -or $env:P4KX_TARGET -cne $Target){throw 'Invalid chain service/source binding.'}
if(Test-Path -LiteralPath $Directory){throw 'Chain preparation directory must be new.'}
$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=[DateTimeOffset]::UtcNow.AddSeconds(90).ToString('o')
"P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC" >> $env:GITHUB_ENV
& (Join-Path $PSScriptRoot 'prepare-msi-feedback.ps1') -Directory $Directory
$binding=@{schema=1;role='native-msi-feedback-chain';revision=$env:GITHUB_SHA;head_revision=$env:P4KX_MSI_PROBE_HEAD;
    run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;target=$Target;mode=$Mode;
    process_arch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString();sources=@(Get-P4kxMsiChainSources)}
Assert-P4kxMsiChainBinding $binding $Target $Mode $env:GITHUB_SHA $env:P4KX_MSI_PROBE_HEAD $env:GITHUB_RUN_ID $env:GITHUB_RUN_ATTEMPT
$prepared=Read-P4kxMsiProbeJson (Join-Path $Directory 'step-feedback.json')
Assert-P4kxMsiChainPreparation $prepared $binding
$binding | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Directory 'chain-binding.json') -Encoding utf8NoBOM
[Console]::Out.WriteLine('[P4KX-MSI-CHAIN-PREPARED]'+($prepared | ConvertTo-Json -Compress))
