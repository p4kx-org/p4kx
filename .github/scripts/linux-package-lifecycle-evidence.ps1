$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-debian.ps1')

function Get-P4kxLinuxLifecycleFixtureVersion {
    return '0.0.0'
}

function Assert-P4kxLinuxLifecycleEvidence {
    param([object[]]$Rows, [object[]]$Packages, $Platform, [string]$Version, [string]$Revision)
    if ($Rows.Count -ne 2) { throw 'Linux lifecycle evidence must cover both installed formats exactly once.' }
    $fixtureVersion = Get-P4kxLinuxLifecycleFixtureVersion
    foreach ($format in @('deb', 'flatpak')) {
        $row = @($Rows | Where-Object package -CEQ $format)
        $package = @($Packages | Where-Object package -CEQ $format)
        if ($row.Count -ne 1 -or $package.Count -ne 1) { throw 'Linux lifecycle format evidence is missing or duplicated.' }
        $proof = $row[0]
        if ($proof.schema -ne 1 -or $proof.target -cne $Platform.target -or $proof.revision -cne $Revision -or
            $proof.fixture_kind -cne 'same-source-lower-package-version' -or
            $proof.baseline.version -cne $fixtureVersion -or $proof.candidate.version -cne $Version -or
            $proof.baseline.archive_sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            $proof.baseline.archive_sha256 -ceq $proof.candidate.archive_sha256 -or
            $proof.candidate.archive_sha256 -cne $package[0].archive_sha256 -or
            $proof.candidate_launch -cne 'success' -or $proof.uninstalled -cne 'success' -or
            $proof.profile_preserved -cne 'success') {
            throw 'Linux lifecycle evidence differs from the executed native release package.'
        }
        if ($format -ceq 'deb') {
            if ($proof.baseline.identity -cne "p4kx:$($Platform.deb_arch)" -or
                $proof.candidate.identity -cne $proof.baseline.identity -or
                $proof.baseline.installed -cne (Get-P4kxDebianVersion $fixtureVersion) -or
                $proof.candidate.installed -cne (Get-P4kxDebianVersion $Version)) {
                throw 'Debian lifecycle package identity or installed version differs.'
            }
        } else {
            $flatpak = $package[0].producer.flatpak
            if ($proof.baseline.identity -cne $flatpak.app_ref -or $proof.candidate.identity -cne $flatpak.app_ref -or
                $proof.baseline.installed -cnotmatch '^[0-9a-f]{64}$' -or
                $proof.baseline.installed -ceq $proof.candidate.installed -or
                $proof.candidate.installed -cne $flatpak.app_commit) {
                throw 'Flatpak lifecycle ref or deployed commit differs.'
            }
        }
    }
}
