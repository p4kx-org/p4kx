$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-path-verification.ps1')
$checks=0
function Assert-Test([bool]$Value,[string]$Message){if(-not $Value){throw $Message};$script:checks++}
function Assert-Rejected([scriptblock]$Action){$rejected=$false;try{& $Action}catch{$rejected=$true};Assert-Test $rejected 'A malformed raw observation was accepted.'}
$context=@{revision=('1'*40);run='12';attempt='3';arch='x86_64';root='C:\runner\raw';counter=0}
$image=@{schema=1;role='disposable-ci-registry-fixture';revision=$context.revision;run='12';attempt='3';process_arch='x86_64';root='\\?\C:\runner\raw';image=@{path=$null};core=@{status='empty'}}
Assert-P4kxRawMsiEnvelope $context $image
$checks++
foreach($slot in @('role','revision','run','attempt','process_arch','root')){
    $bad=$image | ConvertTo-Json -Depth 8 | ConvertFrom-Json -AsHashtable
    $bad[$slot]='foreign'
    Assert-Rejected {Assert-P4kxRawMsiEnvelope $context $bad}
}
foreach($path in @(@{registry_type=1;bytes_hex=''},@{registry_type=2;bytes_hex='0000'},@{registry_type=1;bytes_hex='410000000000'},@{registry_type=2;bytes_hex='41000000ff'})){
    $image.image.path=$path
    Assert-P4kxRawMsiEnvelope $context $image
    Assert-Test (Test-P4kxRawMsiValueEqual $path $path) 'Raw identity lost zero bytes, multiple NULs or trailing bytes.'
}
Assert-Test (-not (Test-P4kxRawMsiValueEqual $null @{registry_type=1;bytes_hex=''})) 'Absent PATH was conflated with a present zero-byte value.'
Assert-Test (-not (Test-P4kxRawMsiValueEqual @{registry_type=1;bytes_hex='0000'} @{registry_type=2;bytes_hex='0000'})) 'Registry string kind was ignored.'
Assert-Test (-not (Test-P4kxRawMsiValueEqual @{registry_type=1;bytes_hex='4100'} @{registry_type=1;bytes_hex='41000000'})) 'Missing NUL was normalized.'
foreach($bad in @(@{registry_type=3;bytes_hex='0000'},@{registry_type=1;bytes_hex='0'},@{registry_type=2;bytes_hex='AF'},@{registry_type=1;bytes_hex=$null})){
    $image.image.path=$bad
    Assert-Rejected {Assert-P4kxRawMsiEnvelope $context $image}
}
$cases=@(Get-P4kxPathFixtureCases)
Assert-Test ($cases.Count -eq 8) 'Original eight PATH cases changed.'
$empty=ConvertTo-P4kxRawPathFixture (@($cases | Where-Object name -CEQ string_empty)[0].state)
Assert-Test ($empty.registry_type -eq 1 -and $empty.bytes_hex -ceq '0000') 'Empty REG_SZ fixture lost its actual terminator.'
$absent=ConvertTo-P4kxRawPathFixture (@($cases | Where-Object name -CEQ absent)[0].state)
Assert-Test ($null -eq $absent) 'Absent fixture became an empty string.'
Assert-Test ((New-P4kxRawMsiName $context capture) -ceq 'raw-0001-capture.json') 'Raw output name is not deterministic.'
Assert-Rejected {New-P4kxRawMsiName $context '../escape'}
"Raw fixture pure checks passed: $checks; no Registry or native execution."
