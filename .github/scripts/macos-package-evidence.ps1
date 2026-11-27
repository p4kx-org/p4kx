$ErrorActionPreference = 'Stop'

function Read-P4kxMacSigningReceipt {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)]$Build,
        [switch]$DryRun
    )
    $digest = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Build.signing_sha256 -cne $digest) { throw 'The macOS signing receipt digest differs from the producer.' }
    $receipt = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    $fields = @($receipt.PSObject.Properties.Name | Sort-Object)
    if (($fields -join ',') -cne 'archive_sha256,mode,notarization,package,reason,signature' -or
        $receipt.package -cne 'dmg' -or $receipt.signature -cne 'verified' -or
        $receipt.archive_sha256 -cnotmatch '^[0-9a-f]{64}$' -or $receipt.archive_sha256 -cne $Build.archive_sha256) {
        throw 'The macOS signing receipt does not identify the verified final DMG.'
    }
    if ($receipt.mode -ceq 'developer-id' -and -not $DryRun -and
        $null -eq $receipt.reason -and $receipt.notarization -ceq 'stapled-and-validated') {
        return $receipt
    }
    $reasons = if ($DryRun) {
        @('package-dry-run')
    } else {
        @('developer-id-unavailable', 'notarization-unavailable', 'developer-id-and-notarization-unavailable')
    }
    if ($receipt.mode -ceq 'ad-hoc' -and $receipt.reason -cin $reasons -and $receipt.notarization -ceq 'not-requested') {
        return $receipt
    }
    throw 'The selected macOS signing or notarization mode has invalid evidence.'
}

function Assert-P4kxMacMetalEvidence {
    param([Parameter(Mandatory)]$Evidence, [Parameter(Mandatory)]$Build)
    if ($Evidence.target -cne $Build.target -or $Evidence.revision -cne $Build.revision -or
        $Evidence.archive_sha256 -cne $Build.archive_sha256 -or $Evidence.outcome -cne 'success') {
        throw 'Native Metal execution does not belong to this build and final DMG.'
    }
}

function Complete-P4kxMacHostedPackageSmoke {
    param(
        [Parameter(Mandatory)][System.Collections.IDictionary]$Smoke,
        [Parameter(Mandatory)]$Build,
        [Parameter(Mandatory)]$Signing,
        [Parameter(Mandatory)]$Metal
    )
    $Smoke.archive_sha256 = $Build.archive_sha256
    $Smoke.signing = $Signing
    $Smoke.signing_sha256 = $Build.signing_sha256
    $Smoke.mount = 'read-only'
    $Smoke.relocation = 'success'
    $Smoke.cli_symlink = 'success'
    $Smoke.runtime.metal_adapter = $Metal.outcome
    return $Smoke
}

function New-P4kxMacosCurrentHostedSmoke {
    param($Runtime)

    if (@($Runtime.package_smokes).Count -ne 1) { throw 'Expected one current macOS package smoke.' }
    $package = $Runtime.package_smokes[0]
    $checks = [ordered]@{}
    foreach ($check in (Get-P4kxPackagedApplicationChecks)) { $checks[$check] = $package.runtime.$check }
    return [ordered]@{
        scope = $Runtime.scope
        version = $Runtime.version
        revision = $Runtime.revision
        workflow_run = $Runtime.workflow_run
        workflow_attempt = $Runtime.workflow_attempt
        runtime_host = $Runtime.runtime_host
        package = $package.package
        target = $package.target
        archive_sha256 = $package.archive_sha256
        binaries = $package.binaries
        signing = $package.signing
        signing_sha256 = $package.signing_sha256
        mount = $package.mount
        relocation = $package.relocation
        cli_symlink = $package.cli_symlink
        runtime = $checks
    }
}
