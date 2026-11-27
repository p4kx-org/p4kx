function Assert-P4kxMacosDmgSmoke {
    param($Package, [string]$ArchiveHash, [string]$SigningHash)

    if ($Package.package -cne 'dmg' -or $Package.mount -cne 'read-only' -or
        $Package.relocation -cne 'success' -or $Package.cli_symlink -cne 'success') {
        throw 'The macOS package smoke lacks DMG mount, relocation or symlink verification.'
    }
    if ($SigningHash -cnotmatch '^[0-9a-f]{64}$' -or $Package.signing_sha256 -cne $SigningHash) {
        throw 'The macOS package smoke signing digest differs from the release build.'
    }
    Assert-P4kxMacosPackageSigning $Package.signing $ArchiveHash
}

function Assert-P4kxMacosHostedPackageRuntime {
    param($Runtime, $Minimum, $Platform, [string]$ArchiveHash)

    Assert-P4kxMacosHostedRole $Runtime.build_host $Platform minimum
    Assert-P4kxMacosHostedRole $Runtime.runtime_host $Platform minimum
    Assert-P4kxMacosDmgSmoke $Minimum $ArchiveHash $Runtime.signing_sha256
    $current = $Minimum.current_hosted_smoke
    if ($null -eq $current) { throw 'The current macOS hosted package smoke is required.' }
    if ($current.scope -cne $Runtime.scope -or $current.version -cne $Runtime.version -or
        $current.revision -cne $Runtime.revision -or $current.workflow_run -cne $Runtime.workflow_run -or
        $current.workflow_attempt -cnotmatch '^[1-9][0-9]*$') {
        throw 'The current macOS package smoke belongs to a different release or workflow run.'
    }
    Assert-P4kxMacosHostedRole $current.runtime_host $Platform current
    Assert-P4kxExecutedPackageRuntime $current $Runtime $Platform $ArchiveHash (Get-P4kxPackagedApplicationChecks)
    Assert-P4kxMacosDmgSmoke $current $ArchiveHash $Runtime.signing_sha256
    if ($current.signing.mode -cne $Minimum.signing.mode -or $current.signing.reason -cne $Minimum.signing.reason) {
        throw 'The two hosted macOS runs identify different signing receipts.'
    }
    if ($null -ne $current.runtime.metal_adapter) {
        throw 'The current macOS package smoke must keep the producer Metal test with the build evidence.'
    }
}
