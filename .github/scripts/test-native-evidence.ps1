param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'native-test-fixtures.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$contract = Get-P4kxPlatformContract
$revision = 'a' * 40
. (Join-Path $PSScriptRoot 'macho-test-image.ps1')
$pins = & (Join-Path $PSScriptRoot 'check-build-tools.ps1')
$compiler = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
$nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)

foreach ($row in $contract.targets) {
    $bytes = [byte[]]::new(256)
    $machine = Get-P4kxNativeMachine -Platform $row
    if ($row.os -eq 'windows') {
        [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
        [BitConverter]::GetBytes([uint32]128).CopyTo($bytes, 0x3c)
        [BitConverter]::GetBytes([uint32]0x00004550).CopyTo($bytes, 128)
        [BitConverter]::GetBytes([uint16]$machine).CopyTo($bytes, 132)
    } elseif ($row.os -ceq 'macos') {
        $bytes = New-P4kxMachOTestImage -Machine $machine
    } else {
        [BitConverter]::GetBytes([uint32]0x464c457f).CopyTo($bytes, 0)
        $bytes[4] = 2
        $bytes[5] = 1
        [BitConverter]::GetBytes([uint16]$machine).CopyTo($bytes, 18)
    }
    $path = Join-Path $Directory "$($row.target).bin"
    [IO.File]::WriteAllBytes($path, $bytes)
    if ((Get-ImageMachine -Path $path -Os $row.os) -ne $machine) { throw 'Machine identity mismatch.' }
    if ($row.os -ceq 'macos') { Test-P4kxMachOImages -Directory $Directory -Platform $row }
    $bytes[0] = 0
    [IO.File]::WriteAllBytes($path, $bytes)
    $rejected = $false
    try { Get-ImageMachine -Path $path -Os $row.os | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Malformed binary was accepted.' }

    $baseline = New-P4kxTestEvidence -Platform $row -Version '0.5.0' -Revision $revision -Compiler $compiler -Nextest $nextest
    Assert-P4kxArchiveEvidence -Metadata $baseline -Platform $row -Revision $revision -Compiler $compiler -Nextest $nextest -ArchiveHash ('b' * 64)
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
        try { Assert-P4kxArchiveEvidence -Metadata $changed -Platform $row -Revision $revision -Compiler $compiler -Nextest $nextest -ArchiveHash ('b' * 64) } catch { $rejected = $true }
        if (-not $rejected) { throw 'Mismatched native archive evidence was accepted.' }
    }
    if ($row.os -ceq 'macos') {
        foreach ($mutation in @(
            { param($m) $m.binaries[0].min_os = '26.1.0' },
            { param($m) $m.binaries[0].cpu_subtype = 2 },
            { param($m) $m.binaries[0].platform = 2 },
            { param($m) $m.build_host.os_version = '15.7.1' },
            { param($m) $m.build_host.architecture = 'x64' }
        )) {
            $changed = $baseline | ConvertTo-Json -Depth 8 | ConvertFrom-Json
            & $mutation $changed
            $rejected = $false
            try { Assert-P4kxArchiveEvidence -Metadata $changed -Platform $row -Revision $revision -Compiler $compiler -Nextest $nextest -ArchiveHash ('b' * 64) } catch { $rejected = $true }
            if (-not $rejected) { throw 'Mismatched macOS host or Mach-O archive evidence was accepted.' }
        }
        foreach ($mutation in @(
            { param($m) $m.runtime_host.os_build = 'unknown' },
            { param($m) $m.runtime.metal_adapter = 'skipped' },
            { param($m) $m.runtime.PSObject.Properties.Remove('metal_adapter') }
        )) {
            $changed = $baseline | ConvertTo-Json -Depth 8 | ConvertFrom-Json
            & $mutation $changed
            $rejected = $false
            try { Assert-P4kxRuntimeEvidence -Metadata $changed -Platform $row -Version '0.5.0' -Revision $revision } catch { $rejected = $true }
            if (-not $rejected) { throw 'Incomplete macOS native runtime evidence was accepted.' }
        }
    }
    $baseline.runtime.cli_convert = 'skipped'
    $rejected = $false
    try { Assert-P4kxRuntimeEvidence -Metadata $baseline -Platform $row -Version '0.5.0' -Revision $revision } catch { $rejected = $true }
    if (-not $rejected) { throw 'Skipped native runtime check was accepted.' }
}
Write-Output 'PASS: native image machines, malformed images, cross-target/source/tool/hash evidence and skipped runtime rejection.'
