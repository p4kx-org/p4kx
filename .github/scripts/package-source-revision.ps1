# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

function Get-P4kxPackageSourceRevision {
    $revision = @(& git rev-parse HEAD)
    if ($LASTEXITCODE -ne 0 -or $revision.Count -ne 1 -or
        $revision[0] -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$' -or
        ($env:GITHUB_SHA -and $env:GITHUB_SHA -cne $revision[0])) {
        throw 'Package producer requires the checked-out workflow revision.'
    }
    return $revision[0]
}
