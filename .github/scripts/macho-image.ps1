function ConvertFrom-P4kxMachOVersion {
    param([Parameter(Mandatory)][uint32]$Word)
    return '{0}.{1}.{2}' -f ($Word -shr 16), (($Word -shr 8) -band 255), ($Word -band 255)
}

function Read-P4kxMachOImage {
    param([Parameter(Mandatory)][string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        # Apple mach_header_64 is 32 bytes followed by its bounded command table.
        if ($stream.Length -lt 32 -or $reader.ReadUInt32() -ne 0xfeedfacfu) {
            throw "Expected a thin 64-bit little-endian Mach-O image: $Path"
        }
        $machine = $reader.ReadUInt32()
        $subtype = $reader.ReadUInt32()
        $fileType = $reader.ReadUInt32()
        $count = $reader.ReadUInt32()
        $bytes = $reader.ReadUInt32()
        $stream.Position = 32
        $end = $stream.Position + $bytes
        if ($end -gt $stream.Length -or 8L * $count -gt $bytes) {
            throw "Mach-O load command table is truncated: $Path"
        }
        $version = $null
        for ($i = 0; $i -lt $count; $i++) {
            $start = $stream.Position
            if ($end - $start -lt 8) { throw "Truncated Mach-O load command: $Path" }
            $command = $reader.ReadUInt32()
            $length = $reader.ReadUInt32()
            if ($length -lt 8 -or $length % 8 -ne 0 -or $length -gt $end - $start) {
                throw "Invalid Mach-O load command length: $Path"
            }
            if ($command -in @(0x24, 0x25, 0x2f, 0x30, 0x32)) {
                if ($null -ne $version) { throw "Multiple Mach-O deployment commands: $Path" }
                $version = Read-P4kxMachODeployment $reader $command $length $Path
            }
            $stream.Position = $start + $length
        }
        if ($stream.Position -ne $end -or $null -eq $version) {
            throw "Mach-O command table or deployment information is incomplete: $Path"
        }
        return [ordered]@{
            machine = $machine
            cpu_subtype = $subtype
            file_type = $fileType
            platform = $version.platform
            min_os = $version.min_os
            sdk = $version.sdk
        }
    }
    finally { $reader.Dispose() }
}

function Read-P4kxMachODeployment {
    param([IO.BinaryReader]$Reader, [uint32]$Command, [uint32]$Length, [string]$Path)
    # LC_BUILD_VERSION includes a counted list of eight-byte build tools.
    if ($Command -eq 0x32 -and $Length -ge 24) {
        $platform = $Reader.ReadUInt32()
        $minimum = $Reader.ReadUInt32()
        $sdk = $Reader.ReadUInt32()
        if (24L + 8L * $Reader.ReadUInt32() -ne $Length) {
            throw "Invalid Mach-O build tool table: $Path"
        }
    } elseif ($Command -eq 0x24 -and $Length -eq 16) {
        $platform = 1
        $minimum = $Reader.ReadUInt32()
        $sdk = $Reader.ReadUInt32()
    } else {
        throw "Expected a macOS Mach-O deployment command: $Path"
    }
    return @{
        platform = $platform
        min_os = ConvertFrom-P4kxMachOVersion $minimum
        sdk = ConvertFrom-P4kxMachOVersion $sdk
    }
}

function Assert-P4kxMachOIdentity {
    param(
        [Parameter(Mandatory)]$Image,
        [Parameter(Mandatory)]$Platform,
        [ValidateSet('Executable', 'Dylib', 'Bundle')][string]$Kind = 'Executable'
    )
    $fileType = switch ($Kind) { Executable { 2 }; Dylib { 6 }; Bundle { 8 } }
    if ($Platform.os -cne 'macos' -or $Image.machine -ne $Platform.mach_cpu_type -or
        $null -eq $Image.cpu_subtype -or $Image.cpu_subtype -notin @(0, 1) -or
        $Image.file_type -ne $fileType -or $Image.platform -ne 1 -or
        $Image.min_os -cnotmatch '^\d+\.\d+\.\d+$' -or $Image.sdk -cnotmatch '^\d+\.\d+\.\d+$') {
        throw 'Mach-O CPU, file kind, platform or deployment identity differs.'
    }
    $minimum = [version]$Image.min_os
    $target = [version]("$($Platform.deployment_target).0")
    if ($minimum.Major -lt 11 -or $minimum -gt $target -or
        ($Kind -eq 'Executable' -and $minimum -ne $target)) {
        throw 'Mach-O minimum OS is incompatible with the macOS deployment target.'
    }
}

function Assert-P4kxMachOImage {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)]$Platform,
        [ValidateSet('Executable', 'Dylib', 'Bundle')][string]$Kind = 'Executable'
    )
    $image = Read-P4kxMachOImage -Path $Path
    Assert-P4kxMachOIdentity -Image $image -Platform $Platform -Kind $Kind
    return $image
}
