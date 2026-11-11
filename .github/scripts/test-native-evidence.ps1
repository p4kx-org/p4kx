param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'native-test-fixtures.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$contract = Get-P4kxPlatformContract
$revision = 'a' * 40

foreach ($row in $contract.targets) {
    $bytes = [byte[]]::new(256)
    $machine = if ($row.os -eq 'windows') { $row.pe_machine } else { $row.elf_machine }
    if ($row.os -eq 'windows') {
        [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
        [BitConverter]::GetBytes([uint32]128).CopyTo($bytes, 0x3c)
        [BitConverter]::GetBytes([uint32]0x00004550).CopyTo($bytes, 128)
        [BitConverter]::GetBytes([uint16]$machine).CopyTo($bytes, 132)
    } else {
        [BitConverter]::GetBytes([uint32]0x464c457f).CopyTo($bytes, 0)
        $bytes[4] = 2
        $bytes[5] = 1
        [BitConverter]::GetBytes([uint16]$machine).CopyTo($bytes, 18)
    }
    $path = Join-Path $Directory "$($row.target).bin"
    [IO.File]::WriteAllBytes($path, $bytes)
    if ((Get-ImageMachine -Path $path -Os $row.os) -ne $machine) { throw 'Machine identity mismatch.' }
    $bytes[0] = 0
    [IO.File]::WriteAllBytes($path, $bytes)
    $rejected = $false
    try { Get-ImageMachine -Path $path -Os $row.os | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Malformed binary was accepted.' }

    $baseline = New-P4kxTestEvidence -Platform $row -Version '0.5.0' -Revision $revision -Compiler '1.99.1' -Nextest '0.9.151'
    Assert-P4kxArchiveEvidence -Metadata $baseline -Platform $row -Revision $revision -Compiler '1.99.1' -Nextest '0.9.151' -ArchiveHash ('b' * 64)
    Assert-P4kxRuntimeEvidence -Metadata $baseline -Platform $row -Version '0.5.0' -Revision $revision
    foreach ($mutation in @(
        { param($m) $m.target = 'wrong-architecture' },
        { param($m) $m.os = if ($m.os -ceq 'windows') { 'linux' } else { 'windows' } },
        { param($m) $m.arch = if ($m.arch -ceq 'amd64') { 'arm64' } else { 'amd64' } },
        { param($m) $m.PSObject.Properties.Remove('os') },
        { param($m) $m.PSObject.Properties.Remove('arch') },
        { param($m) $m.compiler_host = 'wrong-host' },
        { param($m) $m.archive_sha256 = 'd' * 64 },
        { param($m) $m.revision = 'd' * 40 },
        { param($m) $m.binaries[0].machine = 0 },
        { param($m) $m.binaries[0].name = 'p4kx-mcp' }
    )) {
        $changed = $baseline | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        & $mutation $changed
        $rejected = $false
        try { Assert-P4kxArchiveEvidence -Metadata $changed -Platform $row -Revision $revision -Compiler '1.99.1' -Nextest '0.9.151' -ArchiveHash ('b' * 64) } catch { $rejected = $true }
        if (-not $rejected) { throw 'Mismatched native archive evidence was accepted.' }
    }
    $baseline.runtime.cli_convert = 'skipped'
    $rejected = $false
    try { Assert-P4kxRuntimeEvidence -Metadata $baseline -Platform $row -Version '0.5.0' -Revision $revision } catch { $rejected = $true }
    if (-not $rejected) { throw 'Skipped native runtime check was accepted.' }
}
Write-Output 'PASS: native image machines, malformed images, cross-target/source/tool/hash evidence and skipped runtime rejection.'
