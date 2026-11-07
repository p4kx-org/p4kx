[CmdletBinding(DefaultParameterSetName='Verify')]
param(
    [Parameter(Mandatory,ParameterSetName='Fixture')]
    [ValidateSet('x86_64-pc-windows-msvc','aarch64-pc-windows-msvc')][string]$FixtureTarget,
    [Parameter(Mandatory,ParameterSetName='Fixture')]
    [ValidatePattern('\A(?:[0-9a-f]{40}|[0-9a-f]{64})\z')][string]$FixtureRevision,
    [Parameter(Mandatory,ParameterSetName='Fixture')]
    [ValidatePattern('\A[0-9a-f]{64}\z')][string]$FixtureMsiSha256
)
$revision=if($PSCmdlet.ParameterSetName -ceq 'Fixture'){$FixtureRevision}else{'1'*40}
$msiHash=if($PSCmdlet.ParameterSetName -ceq 'Fixture'){$FixtureMsiSha256}else{'a'*64}
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-path-verification.ps1')
$checks=0
function Assert-Test([bool]$Value,[string]$Message){if(-not $Value){throw $Message};$script:checks++}
function Copy-TestValue($Value){return $Value | ConvertTo-Json -Depth 12 | ConvertFrom-Json -AsHashtable}
function New-TestImage($Path){
    return @{schema=1;role='disposable-ci-registry-fixture';revision=$revision;run='12';attempt='3';process_arch='x86_64';root='C:\runner\raw';
        image=@{path=$Path;state=$null;sequence=$null;binding=$null;keys=@{environment=$true;installer=$false;ownership=$false;invocation=$false}};
        core=@{status='valid';phase='committed';owner=$null;reserved_binding=$null;journal_binding=$null}}
}
$metadata=@{product='11111111-1111-1111-1111-111111111111';package='22222222-2222-2222-2222-222222222222'}
$context=@{msi='current.msi';metadata=$metadata;upgrade='upgrade';folder='C:\P4KX';shortcut='shortcut';logs=$PSScriptRoot;helper='helper';helper_calls=0}
$fixture=@{}
$original=New-TestImage @{registry_type=2;bytes_hex='41000000'}
$script:state=Copy-TestValue $original
$script:seeds=0;$script:installs=0;$script:failNext=$false;$script:failCleanup=$false;$script:failCleanupAfterInstall=$false
$script:originalFailure=[InvalidOperationException]::new('synthetic original failure')
function Get-P4kxRawMsiImage($Fixture){return Copy-TestValue $script:state}
function Set-P4kxRawMsiPath($Fixture,$Expected,$Path){
    Assert-Test (Test-P4kxRawMsiValueEqual $Expected.image.path $script:state.image.path) 'Mock CAS was not bound to the current raw value.'
    $script:state.image.path=Copy-TestValue $Path;$script:seeds++;return Copy-TestValue $script:state
}
function Restore-P4kxRawMsiImage($Fixture,$Expected,$Original){$script:state=Copy-TestValue $Original;return Copy-TestValue $script:state}
function Get-P4kxInstalledProducts($Upgrade){if($null -ne $script:state.core.owner){return @($metadata.product)};return @()}
function Assert-P4kxInstallationOwnershipAbsent($Upgrade,$Folder,$Shortcut){if($script:failCleanup){throw 'synthetic unresolved ownership'};Assert-Test ($null -eq $script:state.core.owner) 'Cleanup left an owner.'}
function Assert-P4kxInstalledProduct($Metadata,$Folder,$Shortcut){Assert-Test ($null -ne $script:state.core.owner) 'Mock installation has no owner.'}
function Set-TestInstalled {
    $entry=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4KX\')).ToLowerInvariant()
    $script:state.image.path=if($null -eq $script:state.image.path){@{registry_type=1;bytes_hex=$entry+'0000'}}else{@{registry_type=$script:state.image.path.registry_type;bytes_hex=$entry+'3b00'+$script:state.image.path.bytes_hex}}
    $script:state.core.owner=@{product=('1'*32);package=('2'*32);claim='inserted'}
    $script:state.core.phase='committed'
}
function Invoke-P4kxMsiexec($Arguments,$Log,$ExpectedCode=0,$RawFixture){
    $before=Copy-TestValue $script:state
    if($Arguments[0] -ceq '/i'){
        $script:installs++
        if($script:failNext){$script:failNext=$false;$script:failCleanup=$script:failCleanupAfterInstall;throw $script:originalFailure}
        $script:seeded=Copy-TestValue $script:state
        Set-TestInstalled
    }else{$script:state=Copy-TestValue $script:seeded}
    return @{raw_before=$before;raw_after=(Copy-TestValue $script:state);exit_code=$ExpectedCode}
}
$failures=[Collections.Generic.List[Exception]]::new()
$results=Invoke-P4kxMsiRawPathCases $context $fixture $original $failures
Assert-Test ($results.Count -eq 8 -and $failures.Count -eq 0 -and $script:installs -eq 8) 'The eight original cases did not all run.'
Assert-Test (@($results | Where-Object outcome -CNE success).Count -eq 0) 'A valid pure raw scenario failed.'
$script:failNext=$true;$script:installs=0;$failures.Clear()
$failedResults=Invoke-P4kxMsiRawPathCases $context $fixture $original $failures
Assert-Test ($failedResults.Count -eq 8 -and $script:installs -eq 8 -and $failures.Count -eq 1) 'One case failure prevented the remaining owned cases.'
Assert-Test ([object]::ReferenceEquals($failures[0],$script:originalFailure)) 'Run-all replaced the original exception.'
$script:failNext=$true;$script:failCleanupAfterInstall=$true;$script:installs=0;$failures.Clear();$caught=$null
try{Invoke-P4kxMsiRawPathCases $context $fixture $original $failures | Out-Null}catch{$caught=$_.Exception}
Assert-Test ($caught -is [AggregateException] -and $script:installs -eq 1 -and [object]::ReferenceEquals($caught.InnerExceptions[0],$script:originalFailure)) 'Unresolved ownership allowed another installer write or replaced its original failure.'
$script:failCleanup=$false;$script:failCleanupAfterInstall=$false
$fixture.interrupted=$true;$blocked=$false
try{Restore-P4kxMsiVerificationBaseline $context $fixture $original | Out-Null}catch{$blocked=$true}
Assert-Test $blocked 'An interrupted MSI transaction allowed a fixture reset.'
$fixture.interrupted=$false
$script:state=Copy-TestValue $original
function Invoke-P4kxMsiPathHelper($Context,$Arguments){
    switch($Arguments[0]){
        reserve {
            Assert-Test ($Arguments[5] -ceq 'C:\P4KX\.') 'Helper reserve did not use the exact private folder ABI.'
            $script:state.core.reserved_binding='v1|4|'+('1'*32)+'|'+('2'*32)+'|'+('3'*64)
        }
        prepare {$script:state.core.journal_binding=$Arguments[3];$script:state.core.reserved_binding=$null;$script:state.core.phase='prepared'}
        apply {Set-TestInstalled;$script:state.core.phase='applied'}
        rollback {
            $script:state.image.path=@{registry_type=2;bytes_hex=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4kxSynthetic\external;C:\P4kxSynthetic\base'+[char]0)).ToLowerInvariant()}
            $script:state.core.owner=$null;$script:state.core.phase='rolledBack'
        }
        default {throw 'Unexpected native-helper operation in pure mock.'}
    }
}
$adapter=Invoke-P4kxMsiForeignEditProbe $context $fixture $original
Assert-Test ($adapter.outcome -ceq 'success') 'The narrow external-edit/rollback mock failed.'
$report=@{schema=1;scope='native-synthetic';target='x86_64-pc-windows-msvc';revision=$revision;run='12';attempt='3';raw_root='C:\runner\raw';
    msi_sha256=$msiHash;helper_sha256=('b'*64);fixture_sha256=('c'*64);folder=$context.folder;msi_identity=$metadata;raw_cases=$results;adapter=$adapter;legacy=@{calibration='success';upgrade='success'}}
$report.lifecycle_steps=@(foreach($pair in @{install=0;repair=0;rollback=1603;upgrade=0;'downgrade-blocked'=1603;uninstall=0;'wrong-cpu-rejected'=1633}.GetEnumerator()){
    @{name=$pair.Key;exit_code=$pair.Value;before=$original;after=$original}
})
$platform=@{target=$report.target;arch='amd64'}
Assert-P4kxMsiPathVerification $report $platform $report.revision $report.msi_sha256
$checks++
$serialized=@{installer=@{path_contract=$report}} | ConvertTo-Json -Depth 12 | ConvertFrom-Json
Assert-P4kxMsiPathVerification $serialized.installer.path_contract $platform $report.revision $report.msi_sha256
$checks++
$arm=Copy-TestValue $report
$arm.target='aarch64-pc-windows-msvc';$arm.legacy=$null
$arm.lifecycle_steps=@($arm.lifecycle_steps | Where-Object name -CNE wrong-cpu-rejected)
foreach($name in @('amd64-compatibility-install','same-version-native-replacement','same-version-amd64-replacement','replacement-uninstall')){
    $arm.lifecycle_steps+=@(@{name=$name;exit_code=0;before=(Copy-TestValue $original);after=(Copy-TestValue $original)})
}
function Set-TestArm($Value){
    if($Value -is [Collections.IDictionary]){
        if($Value.Contains('process_arch')){$Value.process_arch='aarch64'}
        foreach($child in @($Value.Values)){Set-TestArm $child}
    }elseif($Value -is [array]){foreach($child in $Value){Set-TestArm $child}}
}
Set-TestArm $arm
Assert-P4kxMsiPathVerification $arm @{target=$arm.target;arch='arm64'} $report.revision $report.msi_sha256
$checks++
if($PSCmdlet.ParameterSetName -ceq 'Fixture'){
    if($FixtureTarget -ceq $report.target){return $report}
    return $arm
}
foreach($mutation in @('seed','active-type','case-missing','case-duplicate','action-missing','exit-code','revision','adapter','foreign-bytes','cleanup')){
    $bad=Copy-TestValue $report
    switch($mutation){
        seed {$bad.raw_cases[0].before.image.path.bytes_hex='0000'}
        'active-type' {$bad.raw_cases[0].installed.image.path.registry_type=1}
        'case-missing' {$bad.raw_cases=@($bad.raw_cases | Select-Object -Skip 1)}
        'case-duplicate' {$bad.raw_cases+=@($bad.raw_cases[0])}
        'action-missing' {$bad.lifecycle_steps=@($bad.lifecycle_steps | Where-Object name -CNE repair)}
        'exit-code' {$bad.lifecycle_steps[0].exit_code=37}
        revision {$bad.raw_cases[0].installed.revision=('2'*40)}
        adapter {$bad.adapter.outcome='skipped'}
        'foreign-bytes' {$bad.adapter.after.image.path.bytes_hex='0000'}
        cleanup {$bad.raw_cases[0].fixture_reset.fixture_only=$false}
    }
    $rejected=$false;try{Assert-P4kxMsiPathVerification $bad $platform $report.revision $report.msi_sha256}catch{$rejected=$true}
    Assert-Test $rejected 'A mutated or incomplete MSI PATH report was accepted.'
}
"Reduced MSI pure/mock checks passed: $checks; no COM, Registry, MSI, helper or notification execution."
& (Join-Path $PSScriptRoot 'test-msi-invocation-uncertainty.ps1')
