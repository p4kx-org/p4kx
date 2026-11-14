<#
.SYNOPSIS
Verifies the precise Documents-portal mount identity without mounting a filesystem.
#>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-documents.ps1')
$mountPoint = '/run/user/1001/doc'
$mount = @{target=$mountPoint; source='portal'; fstype='fuse.portal'}
$metadata = @{filesystems=@($mount)} | ConvertTo-Json -Depth 4
Assert-P4kxFlatpakDocumentMount -Metadata $metadata -MountPoint $mountPoint
foreach ($mutation in @(
    { param($value) $value.target = '/run/user/1001/doc-other' },
    { param($value) $value.target = '/' },
    { param($value) $value.source = 'another-portal' },
    { param($value) $value.fstype = 'fuseblk' },
    { param($value) $value.fstype = 'fuse.sshfs' },
    { param($value) $value.fstype = 'ext4' },
    { param($value) $value.Remove('target') },
    { param($value) $value.Remove('source') },
    { param($value) $value.Remove('fstype') }
)) {
    $changed = $mount.Clone()
    & $mutation $changed
    $rejected = $false
    try { Assert-P4kxFlatpakDocumentMount -Metadata (@{filesystems=@($changed)} | ConvertTo-Json -Depth 4) -MountPoint $mountPoint }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'A foreign, generic or incomplete portal mount was accepted.' }
}
foreach ($metadata in @(
    (@{filesystems=@()} | ConvertTo-Json -Depth 4),
    (@{filesystems=@($mount, $mount)} | ConvertTo-Json -Depth 4),
    'invalid-json'
)) {
    $rejected = $false
    try { Assert-P4kxFlatpakDocumentMount -Metadata $metadata -MountPoint $mountPoint } catch { $rejected = $true }
    if (-not $rejected) { throw 'Absent, ambiguous or malformed portal mount metadata was accepted.' }
}
'PASS: exact portal mount accepted; wrong path, source, filesystem, missing fields, ambiguity and malformed data rejected. No filesystem mounted.'

. (Join-Path $PSScriptRoot 'flatpak-document-identity-cases.ps1')
$script:DocumentExportCalls = 0
function Invoke-P4kxLinuxTool {
    param([string]$Executable, [string[]]$Arguments)
    $expected = @('document-export', '--unique', '--transient', '--app=org.p4kx.P4KX', '--allow-read', $script:DocumentAccess, '/synthetic/Data.p4k')
    if ($Executable -cne 'flatpak' -or $Arguments.Count -ne $expected.Count) { throw 'Document export changed its exact command contract.' }
    for ($index = 0; $index -lt $expected.Count; $index++) {
        if ($Arguments[$index] -cne $expected[$index]) { throw 'Document export changed an argument or access mode.' }
    }
    $script:DocumentExportCalls++
    return $script:DocumentExport
}
$previousRuntime = $env:XDG_RUNTIME_DIR
$identityCases = 0
try {
    $env:XDG_RUNTIME_DIR = '/run/user/1001'
    $root = Join-Path $env:XDG_RUNTIME_DIR 'doc'
    $token = 'AbCdEfGhIjKlMnOpQ1St_Q'
    if ($token.Length -ne 22) { throw 'The official token fixture must contain 22 bytes.' }
    foreach ($id in @('aabbccdd', 'AABBccDD', $token)) {
        foreach ($writable in @($false, $true)) {
            $script:DocumentAccess = if ($writable) { '--allow-write' } else { '--forbid-write' }
            $script:DocumentExport = "$root/$id/Data.p4k"
            $path = Export-P4kxFlatpakDocument '/synthetic/Data.p4k' 'org.p4kx.P4KX' -Writable:$writable
            if ($path -cne $script:DocumentExport -or (Get-P4kxDocumentId $path) -cne $id) { throw 'Document export or ID parsing changed the opaque token bytes.' }
            $identityCases++
        }
    }
    $invalid = @('', '/ab', '.', '..', ('ab' + [char]0 + 'cd'), 'é', 'a-b', ('A' * 33), ('ab' + [char]10 + 'cd'), ('ab' + [char]13 + 'cd'), ('ab' + [char]10), ('ab' + [char]13))
    foreach ($id in $invalid) {
        $script:DocumentAccess = '--forbid-write'
        $script:DocumentExport = "$root/$id/Data.p4k"
        $exportRejected = $false
        try { Export-P4kxFlatpakDocument '/synthetic/Data.p4k' 'org.p4kx.P4KX' | Out-Null } catch { $exportRejected = $_.Exception.Message -ceq 'Document export did not return an actual portal path.' }
        $identityRejected = $false
        try { Get-P4kxDocumentId $script:DocumentExport | Out-Null } catch { $identityRejected = $_.Exception.Message -ceq 'Document identity is malformed.' }
        if (-not $exportRejected -or -not $identityRejected) { throw 'A malformed or oversized document ID was accepted or failed for an unrelated reason.' }
        $identityCases++
    }
    $outside = "$root-other/$token/Data.p4k"
    $script:DocumentExport = $outside
    $exportRejected = $false
    try { Export-P4kxFlatpakDocument '/synthetic/Data.p4k' 'org.p4kx.P4KX' | Out-Null } catch { $exportRejected = $_.Exception.Message -ceq 'Document export did not return an actual portal path.' }
    $identityRejected = $false
    try { Get-P4kxDocumentId $outside | Out-Null } catch { $identityRejected = $_.Exception.Message -ceq 'Document identity is outside its normal mount.' }
    if (-not $exportRejected -or -not $identityRejected) { throw 'An ID outside the exact mount prefix was accepted.' }
    $identityCases++
}
finally { $env:XDG_RUNTIME_DIR = $previousRuntime }
"PASS: $identityCases document identity cases; $script:DocumentExportCalls exact mocked exports; opaque official and legacy IDs, access modes, case, component bounds and mount prefix. No native command or service started."
