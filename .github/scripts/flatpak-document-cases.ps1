$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-documents.ps1')

function Add-P4kxDocumentCaseGrant {
    param($Case, [string]$Path, [switch]$Writable)
    $grant = Export-P4kxFlatpakDocument $Path $Case.application -Writable:$Writable
    $Case.documents.Add($grant)
    $mount = Invoke-P4kxLinuxTool findmnt @('--kernel', '--json', '--list', '--output', 'TARGET,FSTYPE,SOURCE', '--target', $grant) | Out-String
    $mount | Set-Content -LiteralPath (Join-Path $Case.directory "mount-$($Case.documents.Count).json") -Encoding utf8NoBOM
    Assert-P4kxFlatpakDocumentMount $mount (Join-Path $env:XDG_RUNTIME_DIR 'doc')
    return $grant
}

function Assert-P4kxDocumentSandboxTest {
    param($Case, [string[]]$Arguments, [string]$Name, [int]$ExpectedExit)
    $prefix = @('run', '--user', "--arch=$($Case.platform.unix_arch)", '--command=test', $Case.application)
    $result = Invoke-P4kxPortalProcess flatpak ($prefix + $Arguments) $Case.directory $Name
    if ($result.exit_code -ne $ExpectedExit -or $result.stderr.Trim()) { throw 'The actual sandbox grant did not have its required visibility or access mode.' }
}

function Invoke-P4kxDocumentCaseExtract {
    param($Case, [string]$Source, [string]$Output, [string]$Name,
        [ValidateSet('None', 'Installation', 'SourceMembership', 'ExistingSource')][string]$Expected = 'None',
        [string]$Entry = 'Data/hello.txt')
    return Invoke-P4kxFlatpakDocumentCommand $Case.prefix @('extract', $Source, '--filter', $Entry, '--conflict', 'overwrite', '--out', $Output) $Case.directory $Name $Expected
}

function Assert-P4kxDocumentHello {
    param($Case, [string]$Directory)
    $actual = Get-FileHash -LiteralPath (Join-Path $Directory 'Data/hello.txt') -Algorithm SHA256
    if ($actual.Hash -cne $Case.hello_hash) { throw 'The real document grant exported different bytes.' }
}

function Assert-P4kxDocumentFileOrigin {
    param([string]$Artifact, [string]$Receipt, [string]$ExpectedHash)
    $file = Get-Item -LiteralPath $Artifact
    $origin = @(Get-Content -LiteralPath $Receipt | ConvertFrom-Json)
    if ($origin.Count -lt 2 -or $origin[0].type -cne 'header' -or $origin[-1].type -cne 'end') {
        throw 'The writable Documents grant did not leave a complete origin receipt.'
    }
    $binding = $origin[0].binding
    $leasePaths = @(Get-ChildItem -LiteralPath $file.DirectoryName -Force -Filter '.p4kx-origin.lease*')
    if ($binding.type -cne 'file' -or $binding.path.encoding -cne 'utf8' -or $binding.path.value -cne $file.Name -or
        $binding.byteLength -cne $file.Length.ToString([Globalization.CultureInfo]::InvariantCulture) -or
        $binding.sha256 -cne $ExpectedHash.ToLowerInvariant() -or $leasePaths.Count -ne 0) {
        throw 'The writable Documents grant did not leave the exact origin binding and retire its lease.'
    }
}

function Assert-P4kxDocumentOriginLease {
    param($Case, [string]$Source, [string]$Outside)
    $directory = Join-Path $Case.outside 'Data'
    $artifact = Join-Path $directory 'hello.txt'
    $receipt = $artifact + '.p4kx-origin.jsonl'
    $lease = Join-Path $directory '.p4kx-origin.lease'
    Assert-P4kxDocumentFileOrigin $artifact $receipt $Case.hello_hash

    $next = Join-Path $directory 'empty.txt'
    $nextReceipt = $next + '.p4kx-origin.jsonl'
    $nextEntry = 'Data/empty.txt'
    $prior = (Get-FileHash -LiteralPath $receipt -Algorithm SHA256).Hash
    $owned = [IO.File]::Open($lease, [IO.FileMode]::CreateNew)
    try {
        $arguments = @('extract', $Source, '--filter', $nextEntry, '--conflict', 'overwrite', '--out', $Outside)
        $contended = Invoke-P4kxPortalProcess flatpak ($Case.prefix + $arguments) $Case.directory 'held-origin-lease' -Milliseconds 10000 -MaximumBytes 64KB
        if ($contended.exit_code -ne 1 -or -not $contended.stderr.Contains('the origin publication lease already exists') -or
            (Get-FileHash -LiteralPath $receipt -Algorithm SHA256).Hash -cne $prior -or
            (Test-Path -LiteralPath $next) -or (Test-Path -LiteralPath $nextReceipt)) {
            throw 'The granted path did not refuse the backing directory lease and keep the prior receipt.'
        }
        Assert-P4kxDocumentHello $Case $Case.outside
    } finally {
        $owned.Dispose()
        [IO.File]::Delete($lease)
    }

    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $Outside 'outside-after-lease' -Entry $nextEntry
    if ((Get-Item -LiteralPath $next).Length -ne 0 -or
        (Get-FileHash -LiteralPath $receipt -Algorithm SHA256).Hash -cne $prior) {
        throw 'Releasing the lease did not admit only the new empty entry.'
    }
    $emptyHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([byte[]]@()))
    Assert-P4kxDocumentFileOrigin $next $nextReceipt $emptyHash
    Assert-P4kxDocumentHello $Case $Case.outside
}

function Assert-P4kxDocumentInsideEmpty {
    param($Case)
    if (@(Get-ChildItem -LiteralPath $Case.inside -Force).Count -ne 0) {
        throw 'Protected installation destination contains output after refusal.'
    }
}

function Test-P4kxDocumentUnknownSource {
    param($Case, [string]$Source, [string]$Inside, [string]$Outside)
    Assert-P4kxDocumentSandboxTest $Case @('-e', (Join-Path ([IO.Path]::GetDirectoryName($Source)) 'Bin64')) 'hidden-bin64' 1
    Assert-P4kxDocumentSandboxTest $Case @('-w', $Source) 'read-only-source' 1
    $null = Invoke-P4kxFlatpakDocumentCommand $Case.prefix @('list', $Source) $Case.directory 'read-only-list'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $Outside 'outside-extract'
    Assert-P4kxDocumentHello $Case $Case.outside
    Assert-P4kxDocumentOriginLease $Case $Source $Outside
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $Inside 'source-parent-refusal' 'SourceMembership'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source (Join-Path $Inside 'absent-suffix') 'absent-suffix-refusal' 'SourceMembership'
    $direct = Join-Path $Case.game 'direct-refused'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $direct 'host-parent-refusal' 'SourceMembership'
    Assert-P4kxDocumentInsideEmpty $Case
    if (Test-Path -LiteralPath $direct) { throw 'A neutral source-membership refusal created output.' }
    return @{read_only_archive='success'; writable_export='success'; source_membership_refusal='success'; empty_protection_refusal='success'; absent_suffix_refusal='success'}
}

function Test-P4kxDocumentKnownInstallation {
    param($Case, [string]$Inside)
    $directory = Add-P4kxDocumentCaseGrant $Case $Case.game
    Assert-P4kxDocumentSandboxTest $Case @('-d', (Join-Path $directory 'Bin64')) 'visible-bin64' 0
    $null = Invoke-P4kxDocumentCaseExtract $Case (Join-Path $directory 'Data.p4k') $Inside 'known-installation-refusal' 'Installation'
    Assert-P4kxDocumentInsideEmpty $Case
    return 'success'
}

function Test-P4kxDocumentOrdinaryCopy {
    param($Case)
    $directory = Join-Path $Case.directory 'ordinary-copy'
    New-Item -ItemType Directory -Path $directory | Out-Null
    $archive = Join-Path $directory 'Data.p4k'
    Copy-Item -LiteralPath $Case.archive -Destination $archive
    $grant = Add-P4kxDocumentCaseGrant $Case $directory -Writable
    Assert-P4kxDocumentSandboxTest $Case @('-e', (Join-Path $grant 'Bin64')) 'copy-bin64-absent' 1
    $null = Invoke-P4kxDocumentCaseExtract $Case (Join-Path $grant 'Data.p4k') (Join-Path $grant 'export') 'ordinary-copy-extract'
    Assert-P4kxDocumentHello $Case (Join-Path $directory 'export')
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -cne $Case.source_hash) { throw 'Ordinary copied Archive was modified.' }
    return 'success'
}
