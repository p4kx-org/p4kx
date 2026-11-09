$ErrorActionPreference='Stop'

function Read-P4kxMsiDiagnosticUtf8 {
    param([string]$Path,[int]$MaximumBytes,[string]$BoundMessage)
    if($MaximumBytes -le 0 -or $MaximumBytes -eq [int]::MaxValue){throw 'Invalid diagnostic UTF8 byte bound.'}
    $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try{
        if($stream.Length -gt $MaximumBytes){throw $BoundMessage}
        $bytes=[byte[]]::new($MaximumBytes+1)
        $length=0
        while($length -lt $bytes.Length){
            $read=$stream.Read($bytes,$length,$bytes.Length-$length)
            if($read -eq 0){break}
            $length+=$read
        }
        if($length -gt $MaximumBytes){throw $BoundMessage}
    }finally{$stream.Dispose()}
    $offset=if($length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF){3}else{0}
    return [Text.UTF8Encoding]::new($false,$true).GetString($bytes,$offset,$length-$offset)
}
