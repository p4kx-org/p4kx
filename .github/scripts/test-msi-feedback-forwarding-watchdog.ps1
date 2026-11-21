param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)]
    [ValidateSet('x86_64-pc-windows-msvc','aarch64-pc-windows-msvc')]
    [string]$Target,
    [string]$SourceDirectory=$PSScriptRoot
)
$ErrorActionPreference='Stop'
$processArch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
$osArch=[Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$expected=if($Target -ceq 'aarch64-pc-windows-msvc'){'Arm64'}else{'X64'}
if(-not $IsWindows -or $processArch -cne $expected -or $osArch -cne $expected){
    throw 'The forwarding fixture requires its native Windows host and PowerShell.'
}
. (Join-Path $SourceDirectory 'portal-process.ps1')
. (Join-Path $SourceDirectory 'msi-diagnostic-phase.ps1')
. (Join-Path $SourceDirectory 'msi-feedback-chain-binding.ps1')

$root=[IO.Path]::GetFullPath($Directory)
if(Test-Path -LiteralPath $root){throw 'Forwarding watchdog fixture directory must be new.'}
New-Item -ItemType Directory -Path $root | Out-Null
$worker=Join-Path $root 'forwarding-worker.ps1'
@'
param([string]$SourceDirectory,[string]$Target,[string]$Directory,[int]$WitnessDelay)
$ErrorActionPreference='Stop'
. (Join-Path $SourceDirectory 'msi-diagnostic-phase.ps1')
$context=New-P4kxMsiPhaseContext $Directory $Target
$phase=Start-P4kxMsiPhase $context 'forwarding-fixture'

Start-Sleep -Milliseconds $WitnessDelay
$witness=Join-Path $Directory 'worker-alive-past-budget'
[IO.File]::WriteAllText($witness+'.stage',[string]$PID)
[IO.File]::Move($witness+'.stage',$witness)
Start-Sleep -Seconds 10
Stop-P4kxMsiPhase $context $phase
'@ | Set-Content -LiteralPath $worker -Encoding utf8NoBOM

function Invoke-ForwardingControl {
    param([string]$Name,[switch]$Block)
    $case=Join-Path $root $Name
    New-Item -ItemType Directory -Path $case | Out-Null
    $budget=4000
    $witnessDelay=$budget+500
    $witness=Join-Path $case 'worker-alive-past-budget'
    $forward=New-P4kxMsiPhaseForwarder
    $state=@{triggered=$false; stdout=''; witness_while_waiting=$false; callback_elapsed_ms=0; child_exited_before_replay=$false; logs_present_before_replay=$false}
    $observed=@{}
    $callback={
        param([string]$Stream,[string]$Chunk)
        if($Stream -ceq 'stdout' -and -not $state.triggered){
            $state.stdout+=$Chunk
            if($state.stdout.StartsWith('[P4KX-MSI-PHASE]',[StringComparison]::Ordinal) -and
                $state.stdout.Contains([char]10)){
                $state.triggered=$true
                $state.child_exited_before_replay=$observed.exited -eq $true
                $state.logs_present_before_replay=(Test-Path -LiteralPath (Join-Path $case 'forwarding-stdout.log')) -and
                    (Test-Path -LiteralPath (Join-Path $case 'forwarding-stderr.log'))
                if($Block){
                    $held=[Diagnostics.Stopwatch]::StartNew()
                    while(-not (Test-Path -LiteralPath $witness) -and $held.ElapsedMilliseconds -lt $witnessDelay+3000){
                        Start-Sleep -Milliseconds 20
                    }
                    $state.callback_elapsed_ms=$held.ElapsedMilliseconds
                    $state.witness_while_waiting=Test-Path -LiteralPath $witness
                }
            }
        }
        & $forward $Stream $Chunk
    }.GetNewClosure()
    $failure=$null
    $timer=[Diagnostics.Stopwatch]::StartNew()
    try{
        Invoke-P4kxPortalProcess (Get-Process -Id $PID).Path @(
            '-NoProfile','-NonInteractive','-File',$worker,$SourceDirectory,$Target,$case,[string]$witnessDelay
        ) $case 'forwarding' -Milliseconds $budget -MaximumBytes 4096 -OnOutput $callback -Observation $observed | Out-Null
    }catch{$failure=$_.Exception}
    $elapsed=$timer.ElapsedMilliseconds
    if(-not (Test-P4kxMsiChainWatchdogFailure $failure) -or -not $state.triggered -or
        $observed.started -ne $true -or $observed.exited -ne $true -or
        $observed.signal_contract -cne 'direct-process-only'){
        throw 'Forwarding fixture lacks callback entry, watchdog timeout or owned-child termination.'
    }
    $workerWitness=Test-Path -LiteralPath $witness
    if($workerWitness -and ((Get-Content -Raw -LiteralPath $witness) -cne [string]$observed.pid)){
        throw 'The alive witness belongs to another process.'
    }
    if($workerWitness -ne $state.witness_while_waiting){
        throw 'Fixture observed a witness outside its deliberately blocked callback.'
    }
    return @{
        case=$Name; callback_blocked=[bool]$Block; budget_ms=$budget; witness_delay_ms=$witnessDelay
        elapsed_ms=$elapsed; callback_elapsed_ms=$state.callback_elapsed_ms
        live_child_while_callback_waited=$workerWitness
        timeout_reported=$true; child_exited=$true; child_pid=$observed.pid; signal_contract=$observed.signal_contract
        child_exited_before_replay=$state.child_exited_before_replay
        logs_present_before_replay=$state.logs_present_before_replay
    }
}

$immediate=Invoke-ForwardingControl 'immediate'
$blocked=Invoke-ForwardingControl 'blocked' -Block

$isolated=-not $immediate.live_child_while_callback_waited -and -not $blocked.live_child_while_callback_waited -and
    $immediate.child_exited_before_replay -and $blocked.child_exited_before_replay -and
    $immediate.logs_present_before_replay -and $blocked.logs_present_before_replay -and
    $blocked.callback_elapsed_ms -gt $blocked.budget_ms
$outcome=if($isolated){'terminated-and-captured-before-replay'}else{'forwarding-entered-supervision'}
@{
    schema=1; role='native-msi-forwarding-watchdog-fixture'; target=$Target
    revision=$env:GITHUB_SHA; run=$env:GITHUB_RUN_ID; attempt=$env:GITHUB_RUN_ATTEMPT
    architecture=$processArch; outcome=$outcome; release_runtime_evidence=$false
    actual_installer_executed=$false; cases=@($immediate,$blocked)
    portal_sha256=(Get-FileHash -LiteralPath (Join-Path $SourceDirectory 'portal-process.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()
    worker_sha256=(Get-FileHash -LiteralPath $worker -Algorithm SHA256).Hash.ToLowerInvariant()
    phase_sha256=(Get-FileHash -LiteralPath (Join-Path $SourceDirectory 'msi-diagnostic-phase.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'forwarding-watchdog.json') -Encoding utf8NoBOM
if(-not $isolated){throw 'Output replay ran before ordered capture and owned-child termination.'}
"Native forwarding watchdog fixture: $outcome."
