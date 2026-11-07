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
