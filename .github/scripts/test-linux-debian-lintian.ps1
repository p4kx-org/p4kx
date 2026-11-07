<#
.SYNOPSIS
Verifies the upstream-only Lintian rule with actual synthetic Debian packages.
#>
param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-debian.ps1')
if (-not $IsLinux) { throw 'Debian Lintian package regression requires native Linux.' }
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root)) { throw 'Debian Lintian test directory must be new.' }
$package = Join-Path $root 'package'
$documents = Join-Path $package 'usr/share/doc/p4kx'
$control = Join-Path $package 'DEBIAN'
$binary = Join-Path $package 'usr/bin'
$manual = Join-Path $package 'usr/share/man/man1'
New-Item -ItemType Directory -Force -Path $control, $documents, $binary, $manual | Out-Null
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
@'
Package: p4kx
Version: 0.0.0-1
Section: utils
Priority: optional
Architecture: all
Installed-Size: 1
Maintainer: P4KX contributors <lucas.greuloch@gmail.com>
Homepage: https://github.com/p4kx-org/p4kx
Description: synthetic upstream package for dependency-policy verification
 This package tests the scope of a package-local Debian upload policy rule.
'@ | Set-Content -LiteralPath (Join-Path $control 'control') -Encoding utf8NoBOM
$command = "#!/bin/sh`nexit 0`n"
[IO.File]::WriteAllText((Join-Path $binary 'p4kx'), $command, [Text.UTF8Encoding]::new($false))
Invoke-P4kxLinuxTool chmod @('0755', (Join-Path $binary 'p4kx')) | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'packaging/linux/man/p4kx.1') -Destination $manual
Invoke-P4kxLinuxTool gzip @('--no-name', '--best', (Join-Path $manual 'p4kx.1')) | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE-MIT') -Destination (Join-Path $documents 'copyright')
$changelog = "p4kx (0.0.0-1) unstable; urgency=medium`n`n  * Synthetic package policy verification.`n`n -- P4KX contributors <lucas.greuloch@gmail.com>  Sat, 07 Nov 2026 00:00:00 +0000`n"
[IO.File]::WriteAllText((Join-Path $documents 'changelog.Debian'), $changelog, [Text.UTF8Encoding]::new($false))
Invoke-P4kxLinuxTool gzip @('--no-name', '--best', (Join-Path $documents 'changelog.Debian')) | Out-Null
$asset = Join-Path $root 'p4kx_0.0.0-1_all.deb'

function Assert-P4kxFixtureLintian {
    <#
    .SYNOPSIS
    Builds the current fixture and verifies exact Lintian outcomes without relaxing severities.
    #>
    param([int]$ExitCode, [string]$Tag)
    $measurement = Invoke-P4kxLinuxTool du @('--kilobytes', '--summarize', (Join-Path $package 'usr'))
    $measured = ($measurement -split '\s+', 2)[0]
    if ($measured -cnotmatch '^[0-9]+$') { throw 'Synthetic installed size could not be measured.' }
    $path = Join-Path $control 'control'
    $header = Get-Content -LiteralPath $path -Raw
    ($header -replace '(?m)^Installed-Size:.*$', "Installed-Size: $measured") | Set-Content -LiteralPath $path -Encoding utf8NoBOM
    Invoke-P4kxLinuxTool dpkg-deb @('--build', '--root-owner-group', $package, $asset) | Out-Null
    $lines = @(& lintian --color never --show-overrides --fail-on 'error,warning' $asset 2>&1 | ForEach-Object { $_.ToString() })
    $status = $LASTEXITCODE
    $lines | Set-Content -LiteralPath (Join-Path $root "lintian-$Tag.log") -Encoding utf8NoBOM
    if ($status -ne $ExitCode -or -not @($lines | Where-Object { $_ -match [regex]::Escape($Tag) }).Count) {
        $lines | ForEach-Object { Write-Host $_ }
        throw "Synthetic package expected Lintian exit $ExitCode and tag $Tag, got $status."
    }
    if ($ExitCode -eq 0 -and @($lines | Where-Object { $_ -cmatch '^[EW]:' }).Count) { throw 'Overridden fixture retained a real Lintian failure.' }
    if ($ExitCode -eq 0 -or $Tag -ceq 'no-copyright-file') {
        if (-not @($lines | Where-Object { $_ -cmatch '^O: p4kx: initial-upload-closes-no-bugs(?:\s|$)' }).Count) { throw 'The specific package-local override was not reported as active.' }
    }
}

$previousExit = $global:LASTEXITCODE
try {
    Assert-P4kxFixtureLintian -ExitCode 2 -Tag initial-upload-closes-no-bugs
    Install-P4kxDebianLintianOverrides -PackageRoot $package
    Assert-P4kxFixtureLintian -ExitCode 0 -Tag initial-upload-closes-no-bugs
    Remove-Item -LiteralPath (Join-Path $documents 'copyright')
    Assert-P4kxFixtureLintian -ExitCode 2 -Tag no-copyright-file
}
finally { $global:LASTEXITCODE = $previousExit }
'PASS: real Lintian rejects an unqualified initial upload, accepts the upstream package-local rule, and still rejects a missing copyright file.'
