param(
    [Parameter(Mandatory)][string]$Directory,
    [string]$SourceDirectory=$PSScriptRoot
)
$ErrorActionPreference='Stop'
. (Join-Path $SourceDirectory 'msi-diagnostic-phase.ps1')
if(Test-Path -LiteralPath $Directory){throw 'Journal encoding fixture directory must be new.'}
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$utf8=[Text.UTF8Encoding]::new($false,$true)
$target=if([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq 'Arm64'){'aarch64-pc-windows-msvc'}else{'x86_64-pc-windows-msvc'}

function Assert-JournalEncoding([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function New-JournalEncodingFixture([string]$Name,[int]$Children) {
    $folder=Join-Path $Directory $Name
    New-Item -ItemType Directory -Path $folder | Out-Null
    $context=New-P4kxMsiPhaseContext $folder $target
    $console=[Console]::Out
    try{
        [Console]::SetOut([IO.TextWriter]::Null)
        $worker=Start-P4kxMsiPhase $context 'worker'
        for($index=0;$index -lt $Children;$index++){
            $child=Start-P4kxMsiPhase $context 'fixture/read'
            Stop-P4kxMsiPhase $context $child
        }
        Stop-P4kxMsiPhase $context $worker
    }finally{[Console]::SetOut($console)}
    return $context.journal
}
function Assert-JournalDecodeRejected([string]$Name,[byte[]]$Bytes) {
    $path=Join-Path $Directory ($Name+'.jsonl')
    [IO.File]::WriteAllBytes($path,$Bytes)
    $failure=$null
    try{Read-P4kxMsiPhaseJournal $path | Out-Null}catch{$failure=$_.Exception}
    Assert-JournalEncoding ($null -ne $failure -and $failure.GetBaseException() -is [Text.DecoderFallbackException]) ($Name+': actual journal reader must reject strict decoding, not promote another encoding or fail only JSON shape.')
}
$original=New-JournalEncodingFixture 'original' 1
$bytes=[IO.File]::ReadAllBytes($original)
$text=$utf8.GetString($bytes)
$baseline=Read-P4kxMsiPhaseJournal $original
Assert-JournalEncoding ($baseline.present -and $baseline.complete -and $baseline.events -eq 4 -and $baseline.active.Count -eq 0 -and $baseline.terminal -ceq 'end') 'Actual original writer did not produce its four closed UTF8 events.'
$failures=[Collections.Generic.List[Exception]]::new()
$cases=@(
    @{name='utf8-and-utf8-bom';action={
        $path=Join-Path $Directory 'utf8-bom.jsonl'
        [IO.File]::WriteAllBytes($path,([byte[]]@(0xEF,0xBB,0xBF)+$bytes))
        $actual=Read-P4kxMsiPhaseJournal $path
        Assert-JournalEncoding ($actual.complete -and $actual.events -eq 4 -and $actual.active.Count -eq 0 -and $actual.terminal -ceq 'end') 'Actual reader changed a genuine UTF8 BOM journal.'
    }},
    @{name='utf16-original-events';action={
        $encoded=[byte[]]([Text.Encoding]::Unicode.GetPreamble()+[Text.Encoding]::Unicode.GetBytes($text))
        Assert-JournalDecodeRejected 'utf16-original-events' $encoded
    }},
    @{name='invalid-utf8-tail';action={
        Assert-JournalDecodeRejected 'invalid-utf8-tail' ([byte[]]($bytes+[byte]0xFF))
    }},
    @{name='incomplete-line-and-missing';action={
        $path=Join-Path $Directory 'incomplete.jsonl'
        $lines=$text.Split([char]10)
        $prefix=$lines[0]+[char]10+$lines[1]+[char]10+'{"incomplete":'
        [IO.File]::WriteAllBytes($path,$utf8.GetBytes($prefix))
        $actual=Read-P4kxMsiPhaseJournal $path
        Assert-JournalEncoding ($actual.present -and -not $actual.complete -and $actual.events -eq 2 -and $actual.active.Count -eq 2 -and $actual.active -ccontains 'worker' -and $actual.active -ccontains 'fixture/read' -and $null -eq $actual.terminal) 'Incomplete trailing UTF8 changed the actual open-scope contract.'
        $missing=Read-P4kxMsiPhaseJournal (Join-Path $Directory 'missing.jsonl')
        Assert-JournalEncoding (-not $missing.present -and -not $missing.complete -and $missing.events -eq 0) 'Missing journal changed its original observation.'
    }},
    @{name='exact-one-mib-cap';action={
        $source=New-JournalEncodingFixture 'cap-original' 1023
        $padded=[Text.StringBuilder]::new(1MB)
        foreach($line in [IO.File]::ReadAllLines($source,$utf8)){
            $padding=511-$utf8.GetByteCount($line)
            Assert-JournalEncoding ($padding -ge 0) 'Original bounded event unexpectedly exceeds the fixed cap-fixture line width.'
            [void]$padded.Append($line).Append(' '*$padding).Append([char]10)
        }
        $path=Join-Path $Directory 'exact-cap.jsonl'
        $payload=$utf8.GetBytes($padded.ToString())
        Assert-JournalEncoding ($payload.Length -eq 1MB) 'Cap fixture is not exactly the original one-MiB bound.'
        [IO.File]::WriteAllBytes($path,$payload)
        $actual=Read-P4kxMsiPhaseJournal $path
        Assert-JournalEncoding ($actual.complete -and $actual.events -eq 2048 -and $actual.active.Count -eq 0 -and $actual.terminal -ceq 'end') 'Actual reader refused valid original events exactly at one MiB.'
        [IO.File]::WriteAllBytes($path,([byte[]]($payload+[byte]0x20)))
        $message=$null
        try{Read-P4kxMsiPhaseJournal $path | Out-Null}catch{$message=$_.Exception.Message}
        Assert-JournalEncoding ($message -ceq 'MSI phase journal exceeds its byte bound.') 'Actual reader changed or failed to enforce its original one-MiB cap.'
    }}
)
foreach($case in $cases){
    try{& $case.action;Write-Host ('PASS: '+$case.name)}
    catch{$failures.Add($_.Exception);Write-Host ('FAIL: '+$case.name)}
}
if($failures.Count){throw [AggregateException]::new('Actual phase journal strict-encoding controls failed.',$failures)}
'PASS: original journal writer/reader encoding, incomplete line and byte-cap controls only; no native installer or timeout-cause evidence.'
