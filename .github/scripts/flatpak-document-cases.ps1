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
        [ValidateSet('None', 'Installation', 'SourceMembership', 'ExistingSource')][string]$Expected = 'None')
    return Invoke-P4kxFlatpakDocumentCommand $Case.prefix @('extract', $Source, '--filter', 'Data/hello.txt', '--conflict', 'overwrite', '--out', $Output) $Case.directory $Name $Expected
}

function Assert-P4kxDocumentHello {
    param($Case, [string]$Directory)
    $actual = Get-FileHash -LiteralPath (Join-Path $Directory 'Data/hello.txt') -Algorithm SHA256
    if ($actual.Hash -cne $Case.hello_hash) { throw 'The real document grant exported different bytes.' }
}

function Test-P4kxDocumentUnknownSource {
    param($Case, [string]$Source, [string]$Inside, [string]$Outside)
    Assert-P4kxDocumentSandboxTest $Case @('-e', (Join-Path ([IO.Path]::GetDirectoryName($Source)) 'Bin64')) 'hidden-bin64' 1
    Assert-P4kxDocumentSandboxTest $Case @('-w', $Source) 'read-only-source' 1
    $null = Invoke-P4kxFlatpakDocumentCommand $Case.prefix @('list', $Source) $Case.directory 'read-only-list'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $Outside 'outside-extract'
    Assert-P4kxDocumentHello $Case $Case.outside
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $Inside 'source-parent-refusal' 'SourceMembership'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source (Join-Path $Inside 'absent-suffix') 'absent-suffix-refusal' 'SourceMembership'
    $direct = Join-Path $Case.game 'direct-refused'
    $null = Invoke-P4kxDocumentCaseExtract $Case $Source $direct 'host-parent-refusal' 'SourceMembership'
    if ((Get-ChildItem -LiteralPath $Case.inside -Force).Count -ne 0 -or (Test-Path -LiteralPath $direct)) { throw 'A neutral source-membership refusal created output.' }
    return @{read_only_archive='success'; writable_export='success'; source_membership_refusal='success'; empty_protection_refusal='success'; absent_suffix_refusal='success'}
}

function Test-P4kxDocumentKnownInstallation {
    param($Case, [string]$Inside)
    $directory = Add-P4kxDocumentCaseGrant $Case $Case.game
    Assert-P4kxDocumentSandboxTest $Case @('-d', (Join-Path $directory 'Bin64')) 'visible-bin64' 0
    $null = Invoke-P4kxDocumentCaseExtract $Case (Join-Path $directory 'Data.p4k') $Inside 'known-installation-refusal' 'Installation'
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
