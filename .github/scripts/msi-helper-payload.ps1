$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')

function Assert-P4kxMsiHelperImports {
    param([string[]]$Imports, $Platform, [string]$SystemDirectory)
    if ($Imports.Count -eq 0 -or @($Imports | Sort-Object -Unique).Count -ne $Imports.Count) { throw 'Embedded helper imports are missing or duplicated.' }
    foreach ($name in $Imports) {
        if ($name -cnotmatch '^[a-z0-9_.-]+\.dll$' -or $name -match '^(vcruntime|msvcp|concrt|ucrtbase|msvcr|api-ms-win-crt-)') {
            throw 'The embedded helper must not require an installed or dynamically loaded C++/C runtime payload.'
        }
        if ($name -match '^(api-ms-win-|ext-ms-)') { continue }
        $system = Join-Path $SystemDirectory $name
        if (-not (Test-Path -LiteralPath $system -PathType Leaf) -or
            (Get-ImageMachine -Path $system -Os windows) -ne $Platform.pe_machine) {
            throw 'Embedded helper imports must close over native Windows system libraries.'
        }
    }
}

<#
.SYNOPSIS
Checks target-bound embedded-helper provenance without executing its native adapter.
#>
function Assert-P4kxMsiHelperPayload {
    param([string]$Directory, $Platform, [string]$Version, [string]$Revision,
        [string]$Notices, [string]$ExpectedCargoLockSha256)
    if ($ExpectedCargoLockSha256 -cnotmatch '\A[0-9a-f]{64}\z') {
        throw 'Expected Cargo.lock SHA-256 is missing or malformed.'
    }
    $inventory = Get-Content -LiteralPath (Join-Path $Directory 'p4kx-msi-helper.json') -Raw | ConvertFrom-Json
    if ($inventory.cargo_lock_sha256 -isnot [string] -or
        $inventory.cargo_lock_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or
        $inventory.cargo_lock_sha256 -cne $ExpectedCargoLockSha256) {
        throw 'Embedded helper Cargo.lock binding differs.'
    }
    $binary = Join-Path $Directory 'p4kx-msi-path.exe'
    $sbomPath = Join-Path $Directory 'p4kx-msi-helper.cdx.json'
    $sbom = Get-Content -LiteralPath $sbomPath -Raw | ConvertFrom-Json
    if ($inventory.schema_version -ne 1 -or $inventory.role -cne 'embedded-msi-helper' -or
        $inventory.target -cne $Platform.target -or $inventory.version -cne $Version -or $inventory.revision -cne $Revision -or
        $Revision -cnotmatch '^[0-9a-f]{40}(?:[0-9a-f]{24})?$' -or $inventory.crt -cne 'static-msvc' -or
        $inventory.compiler -notmatch '^rustc [^\r\n]+' -or $inventory.rustflags -cne '-Ctarget-feature=+crt-static' -or $inventory.machine -ne $Platform.pe_machine -or
        (Get-ImageMachine -Path $binary -Os windows) -ne $Platform.pe_machine -or
        (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant() -cne $inventory.sha256 -or
        (Get-FileHash -LiteralPath $sbomPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $inventory.sbom_sha256 -or
        (Get-FileHash -LiteralPath (Join-Path $Directory 'p4kx-msi-helper-NOTICE.txt') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $inventory.helper_notice_sha256 -or
        (Get-FileHash -LiteralPath $Notices -Algorithm SHA256).Hash.ToLowerInvariant() -cne $inventory.notices_sha256) {
        throw 'Embedded helper CPU, build, binary, dependency or notice provenance differs.'
    }
    foreach ($pair in @(@('p4kx:target',$Platform.target),@('p4kx:binary','p4kx-msi-path'),@('p4kx:role','embedded-msi-helper'))) {
        $properties = @($sbom.metadata.properties | Where-Object { $_.name -ceq $pair[0] })
        if ($properties.Count -ne 1 -or $properties[0].value -cne $pair[1]) { throw 'Embedded helper SBOM target or role differs.' }
    }
    $hashes = @($sbom.metadata.component.hashes | Where-Object alg -CEQ 'SHA-256')
    if ($sbom.bomFormat -cne 'CycloneDX' -or $sbom.metadata.component.name -cne 'p4kx-msi-path' -or
        $sbom.metadata.component.version -cne $Version -or -not $sbom.components -or
        $hashes.Count -ne 1 -or $hashes[0].content -cne $inventory.sha256) { throw 'Embedded helper SBOM does not bind its executable/dependency closure.' }
    $analyzer = Get-P4kxDumpbin $Platform (Get-P4kxVisualStudio)
    $actual = @(Get-P4kxPeDependencies $analyzer $binary | Sort-Object -Unique)
    if (($actual -join ',') -cne (@($inventory.imports | Sort-Object) -join ',')) { throw 'Embedded helper actual imports differ from recorded evidence.' }
    Assert-P4kxMsiHelperImports -Imports @($inventory.imports) -Platform $Platform -SystemDirectory (Join-Path $env:SystemRoot 'System32')
}
