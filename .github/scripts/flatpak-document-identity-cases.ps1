$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-document-cases.ps1')

function Test-P4kxDocumentUnderlyingForgery {
    param($Case, [string]$Source)
    $underlying = Invoke-P4kxPortalMeasurement getfattr @('--only-values', '--name=user.document-portal.host-path', $Case.archive) $Case.directory 'underlying-xattr'
    if ($underlying -cne $Case.forged_path) { throw 'The synthetic underlying forgery was not actually present.' }
    $fuse = Invoke-P4kxPortalMeasurement getfattr @('--only-values', '--name=user.document-portal.host-path', $Source) $Case.directory 'fuse-xattr'
    $view = $fuse.TrimEnd([char]0)
    if ($view -ceq $Case.forged_path) { $classification = 'forwarded' }
    elseif ($view -ceq $Case.archive) { $classification = 'portal-synthesized' }
    else { throw 'The real FUSE xattr view has an unexplained value.' }
    return @{outcome='success'; underlying='forged'; fuse_view=$classification; product_visible_forgery_proven=$false}
}

function Get-P4kxDocumentId {
    param([string]$Path)
    $root = Join-Path $env:XDG_RUNTIME_DIR 'doc'
    if (-not $Path.StartsWith("$root/", [StringComparison]::Ordinal)) { throw 'Document identity is outside its normal mount.' }
    $parts = $Path.Substring($root.Length + 1) -split '/'
    if ($parts.Count -lt 2 -or $parts[0] -cnotmatch '^[0-9a-f]+$') { throw 'Document identity is malformed.' }
    return $parts[0]
}

function Test-P4kxDocumentHardlink {
    param($Case, [string]$Source)
    $directory = Join-Path $Case.directory 'hardlink-output'
    New-Item -ItemType Directory -Path (Join-Path $directory 'Data') -Force | Out-Null
    $target = Join-Path $directory 'Data/hello.txt'
    Invoke-P4kxLinuxTool ln @('--', $Case.archive, $target) | Out-Null
    $sourceIdentity = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $Case.archive) | Out-String).Trim()
    $targetIdentity = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $target) | Out-String).Trim()
    if ($sourceIdentity -cnotmatch '^[0-9]+:[0-9]+$' -or $sourceIdentity -cne $targetIdentity) { throw 'The native fixture is not an actual host hardlink.' }
    $grant = Add-P4kxDocumentCaseGrant $Case $directory -Writable
    if ((Get-P4kxDocumentId $Source) -ceq (Get-P4kxDocumentId $grant)) { throw 'Hardlink fixture did not create distinct document identities.' }
    $proxySource = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $Source) | Out-String).Trim()
    $proxyTarget = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', (Join-Path $grant 'Data/hello.txt')) | Out-String).Trim()
    $result = Invoke-P4kxDocumentCaseExtract $Case $Source $grant 'hardlink-refusal' 'ExistingSource'
    if ((Get-FileHash -LiteralPath $Case.archive -Algorithm SHA256).Hash -cne $Case.source_hash -or
        (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -cne $Case.source_hash) { throw 'A distinct document hardlink overwrote the Archive source.' }
    return @{outcome='success'; host_hardlink='proven'; separate_documents=$true; proxy_source=$proxySource; proxy_target=$proxyTarget; classification=$result.classification; source_unchanged='success'}
}
