$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-document-cases.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-evidence.ps1')

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
    if ($parts.Count -lt 2 -or $parts[0] -cnotmatch '\A[A-Za-z0-9_]{1,32}\z') { throw 'Document identity is malformed.' }
    return $parts[0]
}

function Invoke-P4kxDocumentHardlinkRefusal {
    param($Case, [string]$Source, [string]$Output)
    $arguments = @('extract', $Source, '--filter', 'Data/hello.txt', '--conflict', 'overwrite', '--out', $Output, '--json')
    $result = Invoke-P4kxPortalProcess flatpak ($Case.prefix + $arguments) $Case.directory 'hardlink-refusal' -Milliseconds 120000 -MaximumBytes 2MB
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Case.directory 'hardlink-command.json') -Encoding utf8NoBOM
    $command = @{exit_code=$result.exit_code; report=($result.stdout | ConvertFrom-Json -Depth 8)}
    $classification = Assert-P4kxFlatpakEntryRefusal $command
    return @{command=$command; classification=$classification}
}

function Assert-P4kxDocumentHardlinkBytes {
    param($Case, [string]$Directory, [string]$Target, [string]$ForeignHash)
    $foreign = Join-Path $Directory 'unrelated.txt'
    $state = @{source_sha256=(Get-FileHash -LiteralPath $Case.archive -Algorithm SHA256).Hash;
        target_sha256=(Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash;
        foreign_sha256=(Get-FileHash -LiteralPath $foreign -Algorithm SHA256).Hash;
        entries=@(Get-ChildItem -LiteralPath $Directory -Recurse -Force | ForEach-Object { [IO.Path]::GetRelativePath($Directory, $_.FullName).Replace('\', '/') } | Sort-Object)}
    $state | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Case.directory 'hardlink-content.json') -Encoding utf8NoBOM
    if ($state.source_sha256 -cne $Case.source_hash -or $state.target_sha256 -cne $Case.source_hash -or
        $state.foreign_sha256 -cne $ForeignHash -or ($state.entries -join '|') -cne 'Data|Data/hello.txt|unrelated.txt') {
        throw 'A hardlink refusal changed source bytes, foreign output or the output inventory.'
    }
}

function Test-P4kxDocumentHardlink {
    param($Case, [string]$Source)
    $directory = Join-Path $Case.directory 'hardlink-output'
    New-Item -ItemType Directory -Path (Join-Path $directory 'Data') -Force | Out-Null
    $target = Join-Path $directory 'Data/hello.txt'
    $foreign = Join-Path $directory 'unrelated.txt'
    [IO.File]::WriteAllText($foreign, 'unrelated synthetic output', [Text.UTF8Encoding]::new($false))
    $foreignHash = (Get-FileHash -LiteralPath $foreign -Algorithm SHA256).Hash
    Invoke-P4kxLinuxTool ln @('--', $Case.archive, $target) | Out-Null
    $sourceIdentity = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $Case.archive) | Out-String).Trim()
    $targetIdentity = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $target) | Out-String).Trim()
    if ($sourceIdentity -cnotmatch '^[0-9]+:[0-9]+$' -or $sourceIdentity -cne $targetIdentity) { throw 'The native fixture is not an actual host hardlink.' }
    $grant = Add-P4kxDocumentCaseGrant $Case $directory -Writable
    if ((Get-P4kxDocumentId $Source) -ceq (Get-P4kxDocumentId $grant)) { throw 'Hardlink fixture did not create distinct document identities.' }
    $proxySource = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', $Source) | Out-String).Trim()
    $proxyTarget = (Invoke-P4kxLinuxTool stat @('--dereference', '--format=%d:%i', (Join-Path $grant 'Data/hello.txt')) | Out-String).Trim()
    $failures = [Collections.Generic.List[Exception]]::new()
    $result = $null
    try { $result = Invoke-P4kxDocumentHardlinkRefusal $Case $Source $grant } catch { $failures.Add($_.Exception) }
    try { Assert-P4kxDocumentHardlinkBytes $Case $directory $target $foreignHash } catch { $failures.Add($_.Exception) }
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Hardlink refusal or unchanged-output verification failed.', $failures) }
    return @{outcome='success'; host_hardlink='proven'; separate_documents=$true; proxy_source=$proxySource; proxy_target=$proxyTarget;
        classification=$result.classification; command=$result.command; source_unchanged='success'; target_unchanged='success'; foreign_output_unchanged='success'}
}
