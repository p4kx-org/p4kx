$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-feedback-chain-artifact.ps1')

function Copy-P4kxMsiChainMutation {
    param([string]$Directory,[string]$PreparedDirectory,[string]$ReplayDirectory)
    $capture=Join-Path $ReplayDirectory 'capture'
    $prepared=Join-Path $ReplayDirectory 'prepared'
    New-Item -ItemType Directory -Path (Join-Path $capture 'feedback'),(Join-Path $capture 'diagnostics'),$prepared -ErrorAction Stop | Out-Null
    foreach($name in @('step-feedback.json','chain-binding.json')){Copy-Item -LiteralPath (Join-Path $PreparedDirectory $name) -Destination (Join-Path $prepared $name)}
    foreach($relative in @('feedback/step-feedback.json','feedback/chain-binding.json','feedback/chain-admission.json','feedback/chain-return.json',
        'diagnostics/diagnostic-start.json','diagnostics/diagnostic-result.json','diagnostics/phase-journal.jsonl',
        'diagnostics/lifecycle-worker-stdout.log','diagnostics/lifecycle-worker-stderr.log')){
        $source=Join-Path $Directory $relative
        if(Test-Path -LiteralPath $source){Copy-Item -LiteralPath $source -Destination (Join-Path $capture $relative)}
    }
    return @{Directory=$capture;PreparedDirectory=$prepared}
}
function Assert-P4kxMsiChainMutations {
    param([string]$Directory,[string]$PreparedDirectory,[string]$ReplayDirectory,[hashtable]$Binding)
    $cases=@('minutes','source','stale','start')
    if($Binding.Mode -ceq 'outer-runner'){$cases+='phase'}
    if($Binding.Mode -ceq 'inner-watchdog'){$cases+='inner'}
    foreach($case in $cases){
        $paths=Copy-P4kxMsiChainMutation $Directory $PreparedDirectory (Join-Path $ReplayDirectory $case)
        $expected=$null
        if($case -cin @('minutes','source')){
            $name=if($case -ceq 'minutes'){'step-feedback.json'}else{'chain-binding.json'}
            $path=Join-Path $paths.PreparedDirectory $name
            $value=Read-P4kxMsiProbeJson $path
            if($case -ceq 'minutes'){$value.minutes=2;$expected='Chain original prepared receipt differs.'}
            else{
                $hash=$value.sources[0].sha256
                $head=if($hash[0] -ceq '0'){'1'}else{'0'}
                $value.sources[0].sha256=$head+$hash.Substring(1);$expected='Chain original source hash differs.'
            }
            $value | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            Copy-Item -LiteralPath $path -Destination (Join-Path $paths.Directory ('feedback/'+$name)) -Force
        }elseif($case -ceq 'stale'){
            $path=Join-Path $paths.Directory 'feedback/chain-admission.json'
            $value=Read-P4kxMsiProbeJson $path
            $value.admitted_at_utc=$value.prepared_deadline_utc
            $value | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            $expected='Native MSI diagnostic deadline expired or exceeds its job budget.'
        }elseif($case -ceq 'start'){
            $path=Join-Path $paths.Directory 'diagnostics/diagnostic-start.json'
            $value=Read-P4kxMsiProbeJson $path;$value.completed_success=$true
            $value | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            $expected='Chain parent start invents completion or differs from original input.'
        }elseif($case -ceq 'phase'){
            $path=Join-Path $paths.Directory 'diagnostics/phase-journal.jsonl'
            $text=Read-P4kxMsiDiagnosticUtf8 $path 1MB 'Mutation journal exceeds its bound.'
            $first=$text.Substring(0,$text.IndexOf([char]10)+1)
            [IO.File]::WriteAllText($path,$first,[Text.UTF8Encoding]::new($false))
            $expected='Interrupted chain lost original active scopes or invented terminal completion.'
        }else{
            $path=Join-Path $paths.Directory 'feedback/chain-admission.json'
            $value=Read-P4kxMsiProbeJson $path;$value.fixture_inner_subcase=$false
            $value | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            $expected='Chain confused the admitted deadline and explicit inner fixture subcase.'
        }
        $message=$null
        try{Assert-P4kxMsiChainArtifact @paths @Binding | Out-Null}catch{$message=$_.Exception.Message}
        if($message -cne $expected){throw "Actual downloaded chain mutation did not fail at its exact oracle: $case"}
    }
    return ,$cases
}
