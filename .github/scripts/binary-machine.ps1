function Get-ImageMachine {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Os)
    $stream = [System.IO.File]::OpenRead($Path)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 64) { throw "Truncated executable: $Path" }
        if ($Os -eq 'windows') {
            if ($reader.ReadUInt16() -ne 0x5a4d) { throw "Missing DOS header: $Path" }
            $stream.Position = 0x3c
            $peOffset = $reader.ReadUInt32()
            if ($peOffset -gt $stream.Length - 6) { throw "Invalid PE header position: $Path" }
            $stream.Position = $peOffset
            if ($reader.ReadUInt32() -ne 0x00004550) { throw "Missing PE signature: $Path" }
            return $reader.ReadUInt16()
        }
        if ($reader.ReadUInt32() -ne 0x464c457f -or $reader.ReadByte() -ne 2 -or $reader.ReadByte() -ne 1) {
            throw "Expected a 64-bit little-endian ELF image: $Path"
        }
        $stream.Position = 18
        return $reader.ReadUInt16()
    } finally {
        $reader.Dispose()
    }
}
