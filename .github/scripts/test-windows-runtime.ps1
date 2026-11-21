param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
$runRoot = Join-Path ([IO.Path]::GetFullPath($Directory)) ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
$hostCpu = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
$platforms = @((Get-P4kxPlatformContract).targets | Where-Object os -EQ windows)
$script:analyzed = [Collections.Generic.List[string]]::new()

function Get-P4kxVisualStudio { return $script:studio }

function Get-P4kxPeDependencies {
    param([string]$Dumpbin, [string]$Path)
    if ($Dumpbin -cne $script:expectedAnalyzer) { throw 'Copy selected the wrong host/target import analyzer.' }
    $name = [IO.Path]::GetFileName($Path)
    $script:analyzed.Add($name)
    switch -Exact ($name) {
        'p4kx.exe' { return @('vcruntime140.dll', 'msvcp140.dll', 'api-ms-win-core-synthetic-l1-1-0.dll') }
        'vcruntime140.dll' { return @('vcruntime140_1.dll') }
        'msvcp140.dll' { return @('vcruntime140.dll') }
        'vcruntime140_1.dll' { return @() }
        default { throw "Unexpected synthetic PE import graph node: $name" }
    }
}

function New-TestPe {
    param([string]$Path, [int]$Machine, [byte]$Marker = 0)
    $bytes = [byte[]]::new(256)
    $bytes[0] = 0x4d; $bytes[1] = 0x5a; $bytes[60] = 128
    $bytes[128] = 0x50; $bytes[129] = 0x45; $bytes[200] = $Marker
    [BitConverter]::GetBytes([uint16]$Machine).CopyTo($bytes, 132)
    [IO.File]::WriteAllBytes($Path, $bytes)
}

function New-TestCrt {
    param([string]$Studio, [string]$Version, [string]$Architecture, [string]$Family, [int]$Machine)
    $path = Join-Path $Studio "VC/Redist/MSVC/$Version/$Architecture/$Family"
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    foreach ($name in @('vcruntime140.dll', 'msvcp140.dll', 'vcruntime140_1.dll', 'unreferenced.dll')) {
        New-TestPe -Path (Join-Path $path $name) -Machine $Machine -Marker 17
    }
    return Get-Item -LiteralPath $path
}

function New-TestStudio {
    param([string]$Name)
    $path = Join-Path $runRoot $Name
    foreach ($row in $platforms) {
        $tools = Join-Path $path "VC/Tools/MSVC/14.51.36231/bin/Host$hostCpu/$($row.msi_arch)"
        New-Item -ItemType Directory -Force -Path $tools | Out-Null
        'Synthetic import analyzer identity; never executed.' | Set-Content (Join-Path $tools 'dumpbin.exe')
    }
    return $path
}

function New-TestPayload {
    param([string]$Studio, $Platform)
    $path = Join-Path $Studio "payload-$($Platform.msi_arch)"
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    New-TestPe -Path (Join-Path $path 'p4kx.exe') -Machine $Platform.pe_machine
    $script:studio = $Studio
    $script:expectedAnalyzer = Join-Path $Studio "VC/Tools/MSVC/14.51.36231/bin/Host$hostCpu/$($Platform.msi_arch)/dumpbin.exe"
    $script:analyzed.Clear()
    return $path
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Pattern)
    try { & $Action } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw }
        return
    }
    throw "A malformed CRT selection was accepted instead of failing with: $Pattern"
}

function Assert-CopiedRuntime {
    param([string]$Studio, $Platform, [string]$Version, [string]$Family)
    $payload = New-TestPayload -Studio $Studio -Platform $Platform
    Copy-P4kxWindowsRuntime -Directory $payload -Platform $Platform
    Assert-P4kxWindowsRuntimeInventory -Directory $payload -Platform $Platform
    $inventory = Get-Content (Join-Path $payload 'windows-runtime.json') -Raw | ConvertFrom-Json
    $expected = @(@('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll') | Sort-Object)
    if ((($inventory.redistributables.name | Sort-Object) -join ',') -cne ($expected -join ',')) {
        throw 'The actual copy did not follow only the transitive required runtime imports.'
    }
    foreach ($dll in $inventory.redistributables) {
        $source = "Visual Studio/VC/Redist/MSVC/$Version/$($Platform.msi_arch)/$Family/$($dll.name)"
        if ($dll.source -cne $source) { throw 'The runtime inventory lost the actual CRT family/version/CPU identity.' }
    }
    if ((($script:analyzed | Sort-Object) -join ',') -cne ((@($expected + 'p4kx.exe') | Sort-Object) -join ',')) {
        throw 'Transitive dependency analysis was incomplete or repeated a cycle.'
    }
    $notice = Get-Content (Join-Path $payload 'MICROSOFT-REDISTRIBUTABLE-NOTICE.txt') -Raw
    if ($notice -notmatch '/2022/redistribution' -or $notice -notmatch '/2026/redistribution' -or
        $notice -match 'Visual Studio 2022 C\+\+|\(VC143\)') { throw 'The notice assumes a fixed CRT family or lacks official source lists.' }
}

foreach ($row in $platforms) {
    foreach ($family in @('Microsoft.VC143.CRT', 'Microsoft.VC145.CRT')) {
        $studio = New-TestStudio -Name "$($row.msi_arch)-$family"
        $older = New-TestCrt $studio '14.9.99999' $row.msi_arch 'Microsoft.VC143.CRT' $row.pe_machine
        $selected = New-TestCrt $studio '14.51.36231' $row.msi_arch $family $row.pe_machine
        $foreign = $platforms | Where-Object msi_arch -NE $row.msi_arch
        $null = New-TestCrt $studio '14.51.36231' $foreign.msi_arch 'Microsoft.VC145.CRT' $foreign.pe_machine
        $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC145.DebugCRT' $foreign.pe_machine
        $null = New-TestCrt $studio '14.51.36231' "$($row.msi_arch)/onecore" 'Microsoft.VC145.CRT' $foreign.pe_machine
        New-Item -ItemType Directory -Force -Path (Join-Path $studio 'VC/Redist/MSVC/99.0.0-preview') | Out-Null
        $actual = Get-P4kxCrtDirectory -Redistributables (Join-Path $studio 'VC/Redist/MSVC') -Architecture $row.msi_arch
        if ($actual.FullName -cne $selected.FullName -or $actual.FullName -ceq $older.FullName) {
            throw 'CRT selection did not use the newest stable numeric version and exact target architecture.'
        }
        Assert-CopiedRuntime $studio $row '14.51.36231' $family
    }
}

foreach ($row in $platforms) {
    $foreign = $platforms | Where-Object msi_arch -NE $row.msi_arch
    $studio = New-TestStudio -Name "missing-architecture-$($row.msi_arch)"
    $null = New-TestCrt $studio '14.9.99999' $row.msi_arch 'Microsoft.VC143.CRT' $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' $foreign.msi_arch 'Microsoft.VC145.CRT' $foreign.pe_machine
    Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') $row.msi_arch } 'Missing target C\+\+ redistributable directory'

    $studio = New-TestStudio -Name "debug-onecore-only-$($row.msi_arch)"
    $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC145.DebugCRT' $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' "$($row.msi_arch)/onecore" 'Microsoft.VC145.CRT' $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' $foreign.msi_arch 'Microsoft.VC145.CRT' $foreign.pe_machine
    Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') $row.msi_arch } 'found 0'

    $studio = New-TestStudio -Name "ambiguous-$($row.msi_arch)"
    $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC143.CRT' $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC145.CRT' $row.pe_machine
    Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') $row.msi_arch } 'found 2'

    $studio = New-TestStudio -Name "file-not-directory-$($row.msi_arch)"
    $target = Join-Path $studio "VC/Redist/MSVC/14.51.36231/$($row.msi_arch)"
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    'Not a CRT directory.' | Set-Content (Join-Path $target 'Microsoft.VC145.CRT')
    Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') $row.msi_arch } 'found 0'

    $studio = New-TestStudio -Name "wrong-machine-$($row.msi_arch)"
    $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC145.CRT' $foreign.pe_machine
    $payload = New-TestPayload $studio $row
    Assert-Rejected { Copy-P4kxWindowsRuntime $payload $row } 'Wrong CPU in CRT dependency'
    if (@(Get-ChildItem -LiteralPath $payload -Filter '*.dll').Count -ne 0) { throw 'A foreign CPU DLL was copied before rejection.' }

    $studio = New-TestStudio -Name "missing-dependency-$($row.msi_arch)"
    $null = New-TestCrt $studio '14.9.99999' $row.msi_arch 'Microsoft.VC143.CRT' $row.pe_machine
    $target = Join-Path $studio "VC/Redist/MSVC/14.51.36231/$($row.msi_arch)/Microsoft.VC145.CRT"
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    New-TestPe (Join-Path $target 'vcruntime140.dll') $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' $row.msi_arch 'Microsoft.VC145.DebugCRT' $row.pe_machine
    $null = New-TestCrt $studio '14.51.36231' $foreign.msi_arch 'Microsoft.VC145.CRT' $foreign.pe_machine
    $payload = New-TestPayload $studio $row
    Assert-Rejected { Copy-P4kxWindowsRuntime $payload $row } 'Missing target CRT dependency'
    if (Test-Path -LiteralPath (Join-Path $payload 'windows-runtime.json')) { throw 'Incomplete runtime copy was recorded as a complete inventory.' }
}

$studio = New-TestStudio -Name 'no-stable-redist-version'
New-Item -ItemType Directory -Force -Path (Join-Path $studio 'VC/Redist/MSVC/99.0.0-preview') | Out-Null
Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') 'x64' } 'No Microsoft C\+\+ redistributable inventory'

if ($IsWindows) {
    $studio = New-TestStudio -Name 'junction-is-not-regular'
    $backing = New-TestCrt $studio '14.51.36231' 'onecore' 'Microsoft.VC145.CRT' $platforms[0].pe_machine
    $target = Join-Path $studio 'VC/Redist/MSVC/14.51.36231/x64'
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    New-Item -ItemType Junction -Path (Join-Path $target 'Microsoft.VC145.CRT') -Target $backing.FullName | Out-Null
    Assert-Rejected { Get-P4kxCrtDirectory (Join-Path $studio 'VC/Redist/MSVC') 'x64' } 'found 0'
}

'PASS: VC143/VC145 CRT selection and actual synthetic PE copy/inventory for AMD64/ARM64; numeric-version, CPU, debug/onecore, regular-directory and ambiguity rejection. No native application/runtime was executed.'
