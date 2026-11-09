param(
    [Parameter(Mandatory)][string]$Directory,
    [string]$SourceDirectory=$PSScriptRoot
)
$ErrorActionPreference='Stop'
. (Join-Path $SourceDirectory 'msi-feedback-probe-artifact.ps1')
if(Test-Path -LiteralPath $Directory){throw 'Probe JSON reader fixture directory must be new.'}
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$utf8=[Text.UTF8Encoding]::new($false,$true)

function Assert-ProbeJsonReader([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function Test-ProbeDecoderFailure([Exception]$Failure) {
    if($null -eq $Failure){return $false}
    if($Failure -is [Text.DecoderFallbackException]){return $true}
    return Test-ProbeDecoderFailure $Failure.InnerException
}
function Assert-ProbeDecodeRejected([string]$Name,[byte[]]$Bytes) {
    $path=Join-Path $Directory ($Name+'.json')
    [IO.File]::WriteAllBytes($path,$Bytes)
    $failure=$null
    try{Read-P4kxMsiProbeJson $path | Out-Null}catch{$failure=$_.Exception}
    Assert-ProbeJsonReader (Test-ProbeDecoderFailure $failure) ($Name+': actual reader must reject at strict UTF8 decoding, not accept replacement text or fail for unrelated JSON shape.')
}
$failures=[Collections.Generic.List[Exception]]::new()
$cases=@(
    @{name='valid-unicode';action={
        $note=[string][char]0x00E9+[char]0x96EA+[char]0xFFFD
        $path=Join-Path $Directory 'unicode.json'
        [IO.File]::WriteAllBytes($path,$utf8.GetBytes('{"schema":1,"role":"native-msi-runner-fuse-probe","note":"'+$note+'"}'))
        $actual=Read-P4kxMsiProbeJson $path
        Assert-ProbeJsonReader ($actual.schema -eq 1 -and $actual.role -ceq 'native-msi-runner-fuse-probe' -and $actual.note -ceq $note) 'Actual reader changed valid Unicode or a genuine encoded replacement character.'
        [IO.File]::WriteAllBytes($path,([byte[]]@(0xEF,0xBB,0xBF)+$utf8.GetBytes('{"note":"'+$note+'"}')))
        Assert-ProbeJsonReader ((Read-P4kxMsiProbeJson $path).note -ceq $note) 'Actual reader lost a valid UTF8 BOM payload.'
    }},
    @{name='invalid-utf8-additive-property';action={
        $prefix=$utf8.GetBytes('{"schema":1,"role":"native-msi-runner-fuse-probe","note":"')
        $suffix=$utf8.GetBytes('"}')
        Assert-ProbeDecodeRejected 'invalid-utf8' ([byte[]]($prefix+[byte]0xFF+$suffix))
    }},
    @{name='utf16-bom-has-no-fallback';action={
        $bytes=[byte[]]([Text.Encoding]::Unicode.GetPreamble()+[Text.Encoding]::Unicode.GetBytes('{"schema":1,"note":"valid JSON in a different encoding"}'))
        Assert-ProbeDecodeRejected 'utf16-bom' $bytes
    }},
    @{name='exact-byte-bound';action={
        $prefix='{"schema":1,"note":"';$suffix='"}'
        $length=65536-$utf8.GetByteCount($prefix+$suffix)
        $path=Join-Path $Directory 'exact-bound.json'
        $text=$prefix+('a'*$length)+$suffix
        [IO.File]::WriteAllBytes($path,$utf8.GetBytes($text))
        Assert-ProbeJsonReader ((Get-Item -LiteralPath $path).Length -eq 65536) 'Exact-bound fixture does not have the original byte length.'
        Assert-ProbeJsonReader ((Read-P4kxMsiProbeJson $path).note.Length -eq $length) 'Actual reader refused valid JSON exactly at 64KiB.'
        [IO.File]::WriteAllBytes($path,$utf8.GetBytes($text+' '))
        $message=$null
        try{Read-P4kxMsiProbeJson $path | Out-Null}catch{$message=$_.Exception.Message}
        Assert-ProbeJsonReader ($message -ceq 'Probe JSON exceeds its fixed diagnostic bound.') 'Actual reader changed or failed to enforce the existing 64KiB cap.'
    }}
)
foreach($case in $cases){
    try{& $case.action;Write-Host ('PASS: '+$case.name)}
    catch{$failures.Add($_.Exception);Write-Host ('FAIL: '+$case.name)}
}
if($failures.Count){throw [AggregateException]::new('Actual probe JSON reader encoding/bound controls failed.',$failures)}
'PASS: actual probe reader strict UTF8, valid Unicode/BOM and exact existing byte cap; no native runtime or installer evidence.'
