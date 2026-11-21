function Set-P4kxMachOTestWord {
    param([byte[]]$Bytes, [int]$Offset, [uint32]$Word)
    [BitConverter]::GetBytes($Word).CopyTo($Bytes, $Offset)
}

function New-P4kxMachOTestImage {
    param([uint32]$Machine, [uint32]$Kind = 2, [uint32]$Minimum = 0x001a0000)
    $bytes = [byte[]]::new(88)
    Set-P4kxMachOTestWord $bytes 0 0xfeedfacfu
    Set-P4kxMachOTestWord $bytes 4 $Machine
    Set-P4kxMachOTestWord $bytes 12 $Kind
    Set-P4kxMachOTestWord $bytes 16 1
    Set-P4kxMachOTestWord $bytes 20 24
    Set-P4kxMachOTestWord $bytes 32 0x32
    Set-P4kxMachOTestWord $bytes 36 24
    Set-P4kxMachOTestWord $bytes 40 1
    Set-P4kxMachOTestWord $bytes 44 $Minimum
    Set-P4kxMachOTestWord $bytes 48 0x001a0100
    return ,$bytes
}

function Test-P4kxMachOImages {
    param([string]$Directory, $Platform)
    $path = Join-Path $Directory 'macho-identity.bin'
    $original = New-P4kxMachOTestImage -Machine $Platform.mach_cpu_type
    [IO.File]::WriteAllBytes($path, $original)
    $image = Assert-P4kxMachOImage -Path $path -Platform $Platform -Kind Executable
    if ($image.cpu_subtype -ne 0 -or $image.platform -ne 1 -or $image.file_type -ne 2 -or
        $image.min_os -cne "$($Platform.deployment_target).0" -or $image.sdk -cne '26.1.0') {
        throw 'Mach-O reader changed the actual ARM64 header/deployment identity.'
    }
    foreach ($change in @(
        { param($b) Set-P4kxMachOTestWord $b 0 0xbebafecau },
        { param($b) Set-P4kxMachOTestWord $b 4 0x01000007 },
        { param($b) Set-P4kxMachOTestWord $b 8 2 },
        { param($b) Set-P4kxMachOTestWord $b 12 6 },
        { param($b) Set-P4kxMachOTestWord $b 16 2 },
        { param($b) Set-P4kxMachOTestWord $b 20 1024 },
        { param($b) Set-P4kxMachOTestWord $b 32 0x25 },
        { param($b) Set-P4kxMachOTestWord $b 36 0 },
        { param($b) Set-P4kxMachOTestWord $b 36 20 },
        { param($b) Set-P4kxMachOTestWord $b 40 2 },
        { param($b) Set-P4kxMachOTestWord $b 44 0x00190000 },
        { param($b) Set-P4kxMachOTestWord $b 44 0x001a0100 },
        { param($b) Set-P4kxMachOTestWord $b 52 1 },
        {
            param($b)
            Set-P4kxMachOTestWord $b 16 2
            Set-P4kxMachOTestWord $b 20 48
            [Array]::Copy($b, 32, $b, 56, 24)
        }
    )) {
        $changed = $original.Clone()
        & $change $changed
        [IO.File]::WriteAllBytes($path, $changed)
        $rejected = $false
        try { Assert-P4kxMachOImage -Path $path -Platform $Platform -Kind Executable | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw "Invalid Mach-O header or load command was admitted: $change" }
    }
    $library = New-P4kxMachOTestImage -Machine $Platform.mach_cpu_type -Kind 6 -Minimum 0x000e0000
    [IO.File]::WriteAllBytes($path, $library)
    if ((Assert-P4kxMachOImage -Path $path -Platform $Platform -Kind Dylib).min_os -cne '14.0.0') {
        throw 'Bundled ARM64 dylib with a compatible lower deployment target was refused.'
    }
    $legacy = New-P4kxMachOTestImage -Machine $Platform.mach_cpu_type -Kind 8
    Set-P4kxMachOTestWord $legacy 20 16
    Set-P4kxMachOTestWord $legacy 32 0x24
    Set-P4kxMachOTestWord $legacy 36 16
    Set-P4kxMachOTestWord $legacy 40 0x000e0000
    Set-P4kxMachOTestWord $legacy 44 0x001a0000
    [IO.File]::WriteAllBytes($path, $legacy)
    if ((Assert-P4kxMachOImage -Path $path -Platform $Platform -Kind Bundle).min_os -cne '14.0.0') {
        throw 'The bundled Mach-O legacy minimum command was not read.'
    }
}
