<#
.SYNOPSIS
Checks native-completion uncertainty using actual source functions and mocked adapters.
#>
[CmdletBinding()]
param(
    [string]$SourceDirectory=$PSScriptRoot,
    [ValidateSet('Red','Green')][string]$Expectation='Green'
)
$ErrorActionPreference='Stop'
$script:checks=0;$script:defects=0
$definitions=@{
    'msi-raw-fixture.ps1'=@('Assert-P4kxMsiMutationReady','New-P4kxRawMsiName','Invoke-P4kxRawMsiFixture','Assert-P4kxRawMsiEnvelope','Get-P4kxRawMsiImage','Test-P4kxRawMsiValueEqual','Set-P4kxRawMsiPath','Restore-P4kxRawMsiImage')
    'msi-path-verification.ps1'=@('ConvertTo-P4kxRawPathFixture','Restore-P4kxMsiVerificationBaseline','Invoke-P4kxMsiRawPathCases','Invoke-P4kxMsiPathHelper','Invoke-P4kxMsiForeignEditProbe')
    'msi-installation.ps1'=@('Invoke-P4kxMsiexec')
}
foreach($file in $definitions.Keys){
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $SourceDirectory $file),[ref]$tokens,[ref]$errors)
    if($errors.Count -ne 0){throw 'Actual candidate source has parse errors.'}
    foreach($name in $definitions[$file]){
        $found=@($ast.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $name},$true))
        if($found.Count -eq 0 -and $name -ceq 'Assert-P4kxMsiMutationReady' -and $Expectation -ceq 'Red'){continue}
        if($found.Count -ne 1){throw "Expected one actual source function: $name"}
        . ([ScriptBlock]::Create($found[0].Extent.Text))
    }
}

function Copy-TestValue($Value){return $Value | ConvertTo-Json -Depth 12 | ConvertFrom-Json -AsHashtable}
function Assert-Test([bool]$Value,[string]$Message){if(-not $Value){throw $Message};$script:checks++}
function Test-ContainsIdentity($Failure,$Expected){
    if([object]::ReferenceEquals($Failure,$Expected)){return $true}
    if($Failure -is [AggregateException]){foreach($inner in $Failure.InnerExceptions){if(Test-ContainsIdentity $inner $Expected){return $true}}}
    elseif($null -ne $Failure.InnerException){return Test-ContainsIdentity $Failure.InnerException $Expected}
    return $false
}
function Reset-Test([string]$Role,[string]$Phase,[string]$Mode='unknown'){
    $script:faultRole=$Role;$script:faultPhase=$Phase;$script:mode=$Mode;$script:fired=$false;$script:unknown=$false
    $script:commands=[Collections.Generic.List[object]]::new();$script:after=[Collections.Generic.List[string]]::new()
    $script:inputs=@{};$script:outputs=@{};$script:seed=$null;$script:waits=0;$script:kills=0;$script:disposed=0;$script:starts=0;$script:jsonWrites=0
    $script:original=[AggregateException]::new('Synthetic native completion is unproven.',[Exception[]]@([InvalidOperationException]::new('Synthetic teardown failed.')))
    $script:disposeFailure=[IO.IOException]::new('Synthetic disposal failure.')
    $script:binding='v1|4|'+('1'*32)+'|'+('2'*32)+'|'+('3'*64)
    $script:state=@{schema=1;role='disposable-ci-registry-fixture';revision=('1'*40);run='12';attempt='3';process_arch='x86_64';root='C:\runner\raw';
        image=@{path=@{registry_type=2;bytes_hex='41000000'};state=$null;sequence=$null;binding=$null;keys=@{environment=$true;installer=$false;ownership=$false;invocation=$false}};
        core=@{status='valid';phase='committed';owner=$null;reserved_binding=$null;journal_binding=$null}}
    $script:originalImage=Copy-TestValue $script:state
    $script:fixture=@{interrupted=$false;executable='fixture';sha256=('a'*64);root='C:\runner\raw';counter=0;revision=('1'*40);run='12';attempt='3';arch='x86_64'}
    $script:context=@{metadata=@{product='11111111-1111-1111-1111-111111111111';package='22222222-2222-2222-2222-222222222222'};
        msi='not-executed.msi';folder='C:\P4KX';upgrade='synthetic';shortcut='synthetic';logs='C:\runner';helper='helper';helper_sha256=('a'*64);helper_calls=0}
}
function Record-TestCommand([string]$Role,[string]$Phase){
    $script:commands.Add(@{role=$Role;phase=$Phase})
    if($script:unknown -and $Phase -cne 'capture'){$script:after.Add($Role+':'+$Phase)}
}
function Get-FileHash {param($LiteralPath,$Algorithm) return @{Hash=('a'*64)}}
function Get-Item {param($LiteralPath) return @{Length=128}}
function Test-Path {return $false}
function Get-Content {param($LiteralPath,[switch]$Raw,$Tail) if(-not $script:outputs.ContainsKey($LiteralPath)){throw 'Unmocked file read.'};return $script:outputs[$LiteralPath]}
function Write-P4kxRawMsiJson {param($Path,$Value) $script:jsonWrites++;$script:inputs[$Path]=Copy-TestValue $Value}
function Assert-P4kxInstallationOwnershipAbsent {}
function Assert-P4kxLegacyOwnershipAbsent {}
function Assert-P4kxInstalledProduct {}
function Assert-P4kxRawMsiInstalled {}
function Assert-P4kxRawMsiPathEqual {}
function Get-P4kxInstalledProducts {return @()}
function Get-P4kxUserPathState {return @{exists=$false}}
function Write-P4kxPathTransition {}
function New-P4kxMsiexecStartInfo {param($Arguments,$Log) return @{arguments=$Arguments;log=$Log}}
function Get-P4kxPathFixtureCases {return @(@{name='first';state=@{exists=$false}},@{name='second';state=@{exists=$false}},@{name='third';state=@{exists=$false}})}
function New-Object {throw 'NATIVE SENTINEL: COM construction is forbidden.'}
function Set-TestInstalled {
    $script:seed=Copy-TestValue $script:state.image.path
    $prefix=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4KX\;')).ToLowerInvariant()
    $script:state.image.path=@{registry_type=2;bytes_hex=$prefix+$script:seed.bytes_hex}
    $script:state.core.owner=@{product=('1'*32);package=('2'*32);claim='inserted'};$script:state.core.phase='applied'
}
function Invoke-P4kxPortalProcess {
    param($Executable,$Arguments,$Directory,$Name,$Environment)
    if($Executable -cnotin @('helper','fixture')){throw 'NATIVE SENTINEL: unexpected executable.'}
    $phase=$Arguments[0];Record-TestCommand $Executable $phase
    if(-not $script:fired -and $Executable -ceq $script:faultRole -and $phase -ceq $script:faultPhase){
        $script:fired=$true
        if($script:mode -ceq 'exit37'){return @{exit_code=37}}
        $script:unknown=$true;throw $script:original
    }
    if($Executable -ceq 'helper'){
        switch -CaseSensitive ($phase){
            reserve {$script:state.core.reserved_binding=$script:binding}
            prepare {$script:state.core.reserved_binding=$null;$script:state.core.journal_binding=$script:binding;$script:state.core.phase='prepared'}
            apply {Set-TestInstalled}
            rollback {
                $prefix=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4kxSynthetic\external;')).ToLowerInvariant()
                $tail=if($null -ne $script:seed){$script:seed.bytes_hex}else{$script:state.image.path.bytes_hex}
                $script:state.image.path=@{registry_type=2;bytes_hex=$prefix+$tail};$script:state.core.owner=$null
                $script:state.core.reserved_binding=$null;$script:state.core.phase='rolledBack'
            }
            default {throw 'NATIVE SENTINEL: unexpected helper command.'}
        }
    }else{
        switch -CaseSensitive ($phase){
            capture {}
            'seed-path' {$script:state.image.path=Copy-TestValue $script:inputs[(Join-Path $Directory $Arguments[3])].path}
            restore {$script:state=Copy-TestValue $script:inputs[(Join-Path $Directory $Arguments[3])]}
            default {throw 'NATIVE SENTINEL: unexpected fixture command.'}
        }
        $script:outputs[(Join-Path $Directory $Arguments[-1])]=$script:state | ConvertTo-Json -Depth 12
    }
    return @{exit_code=0}
}
function Start-P4kxMsiProcess {
    param($StartInfo)
    $script:starts++;Record-TestCommand msi start
    if($script:faultPhase -ceq 'start'){$script:unknown=$true;throw $script:original}
    $process=[pscustomobject]@{ExitCode=$(if($script:mode -ceq 'exit37'){37}else{0})}
    $process | Add-Member ScriptMethod WaitForExit {
        param($Milliseconds)
        $script:waits++
        if($script:faultPhase -ceq 'wait'){$script:unknown=$true;throw $script:original}
        if($script:faultPhase -ceq 'timeout' -and $script:waits -eq 1){return $false}
        return $true
    }
    $process | Add-Member ScriptMethod Kill {param($Tree) $script:kills++}
    $process | Add-Member ScriptMethod Dispose {$script:disposed++;if($script:mode -ceq 'dispose-error'){throw $script:disposeFailure}}
    return $process
}
if($Expectation -ceq 'Red'){
    function Invoke-P4kxMsiexec {Record-TestCommand msi forbidden;throw 'NATIVE SENTINEL: the frozen static MSI process adapter is not executed.'}
}

function Assert-UnknownOutcome($Caught,[string]$Label){
    Assert-Test (Test-ContainsIdentity $Caught $script:original) "$Label replaced the original aggregate error identity."
    if($Expectation -ceq 'Red'){
        if($script:after.Count -ne 0 -or -not $script:fixture.interrupted){$script:defects++;Write-Output "RED $Label`: interrupted=$($script:fixture.interrupted), later=$($script:after -join ',')"}
        else{throw "$Label unexpectedly reproduced no defect in the frozen source."}
    }else{
        Assert-Test ($script:fixture.interrupted -and $script:after.Count -eq 0) "$Label allowed another mutation after unproven completion."
    }
}
foreach($phase in @('reserve','prepare','apply','rollback')){
    Reset-Test helper $phase
    $caught=$null;try{Invoke-P4kxMsiForeignEditProbe $script:context $script:fixture $script:originalImage|Out-Null}catch{$caught=$_.Exception}
    Assert-UnknownOutcome $caught "helper-$phase"
}
foreach($phase in @('capture','seed-path','restore')){
    Reset-Test fixture $phase
    $caught=$null;try{Invoke-P4kxMsiForeignEditProbe $script:context $script:fixture $script:originalImage|Out-Null}catch{$caught=$_.Exception}
    if($phase -ceq 'restore'){
        $later=$null;try{Invoke-P4kxMsiRawPathCases $script:context $script:fixture $script:originalImage ([Collections.Generic.List[Exception]]::new())|Out-Null}catch{$later=$_.Exception}
    }
    Assert-UnknownOutcome $caught "fixture-$phase"
}
Reset-Test fixture seed-path
$failures=[Collections.Generic.List[Exception]]::new();$caught=$null
try{Invoke-P4kxMsiRawPathCases $script:context $script:fixture $script:originalImage $failures|Out-Null}catch{$caught=$_.Exception}
if($Expectation -ceq 'Red'){
    Assert-Test ($failures.Count -gt 0 -and (Test-ContainsIdentity $failures[0] $script:original)) 'The raw-case primary aggregate disappeared.'
    Assert-Test (@($script:commands | Where-Object {$_.role -ceq 'fixture' -and $_.phase -ceq 'seed-path'}).Count -gt 1) 'The expected next-case defect did not reproduce.'
    $script:defects++;Write-Output 'RED raw-next-case: later cases mutated after fixture uncertainty.'
}else{
    Assert-UnknownOutcome $caught raw-next-case
    Assert-Test (@($script:commands | Where-Object {$_.role -ceq 'fixture' -and $_.phase -ceq 'seed-path'}).Count -eq 1) 'Another raw case seeded Registry inputs.'
}

if($Expectation -ceq 'Green'){
    foreach($role in @('helper','fixture')){
        $phase=if($role -ceq 'helper'){'apply'}else{'seed-path'}
        Reset-Test $role $phase exit37
        $caught=$null;try{Invoke-P4kxMsiForeignEditProbe $script:context $script:fixture $script:originalImage|Out-Null}catch{$caught=$_.Exception}
        Assert-Test ($null -ne $caught -and $caught.Data['result'].exit_code -eq 37) 'The actual nonzero completed exit code was lost.'
        Assert-Test (-not $script:fixture.interrupted) 'A proven completed nonzero exit was treated as an unknown process.'
        Assert-Test (@($script:commands | Where-Object {$_.role -ceq 'fixture' -and $_.phase -ceq 'restore'}).Count -eq 1) 'Completed failures no longer allow owned fixture restoration.'
    }
    foreach($phase in @('start','wait')){
        Reset-Test msi $phase
        $caught=$null;try{Invoke-P4kxMsiexec @('/i','not-executed') 'not-written.log' -RawFixture $script:fixture|Out-Null}catch{$caught=$_.Exception}
        Assert-UnknownOutcome $caught "msi-$phase"
        $before=$script:jsonWrites
        $blocked=$null;try{Set-P4kxRawMsiPath $script:fixture $script:state $null|Out-Null}catch{$blocked=$_.Exception}
        Assert-Test ($null -ne $blocked -and $script:jsonWrites -eq $before -and $script:after.Count -eq 0) 'MSI uncertainty allowed fixture mutation or input publication.'
    }
    Reset-Test msi completed exit37
    $caught=$null;try{Invoke-P4kxMsiexec @('/i','not-executed') 'not-written.log' -RawFixture $script:fixture|Out-Null}catch{$caught=$_.Exception}
    Assert-Test ($null -ne $caught -and $caught.Message -match 'MSI exited 37, expected 0' -and -not $script:fixture.interrupted) 'A completed MSI failure lost its code or unnecessarily blocked recovery.'
    Reset-Test helper apply
    try{Invoke-P4kxMsiForeignEditProbe $script:context $script:fixture $script:originalImage|Out-Null}catch{$caught=$_.Exception}
    Get-P4kxRawMsiImage $script:fixture|Out-Null
    Assert-Test ($script:fixture.interrupted -and $script:after.Count -eq 0) 'Read-only observation cleared the uncertainty latch.'
    Reset-Test msi timeout
    $caught=$null;try{Invoke-P4kxMsiexec @('/i','not-executed') 'not-written.log' -RawFixture $script:fixture|Out-Null}catch{$caught=$_.Exception}
    Assert-Test ($null -ne $caught -and $script:kills -eq 1 -and $script:waits -eq 2 -and $script:fixture.interrupted) 'A returned Kill and later wait erased the timeout uncertainty.'
    Reset-Test msi wait dispose-error
    $caught=$null;try{Invoke-P4kxMsiexec @('/i','not-executed') 'not-written.log' -RawFixture $script:fixture|Out-Null}catch{$caught=$_.Exception}
    Assert-Test (Test-ContainsIdentity $caught $script:original) 'Disposal hid the original incomplete-process aggregate.'
    Assert-Test ($script:fixture.interrupted -and $script:disposed -eq 1) 'Disposal skipped the uncertainty latch or ran twice.'
}
if($Expectation -ceq 'Red'){Assert-Test ($script:defects -eq 8) 'Not all eight uncertain-completion defects reproduced.'}
"$Expectation checks passed: $script:checks; reproduced defects: $script:defects. Actual source functions, mocked processes only; no native calls or files."
