$ErrorActionPreference = 'Stop'

function Invoke-P4kxMacTool {
    param([Parameter(Mandatory)][string]$Tool, [Parameter(Mandatory)][string[]]$Arguments)
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "macOS tool '$Tool' failed with exit $LASTEXITCODE." }
}

function Get-P4kxMacSigningPlan {
    param([switch]$DryRun, [string]$SigningIdentity, [string]$NotaryProfile)

    if ($DryRun) {
        return [pscustomobject]@{ mode = 'ad-hoc'; reason = 'package-dry-run'; identity = '-'; profile = $null }
    }
    $missing = @(
        if (-not $SigningIdentity) { 'developer-id' }
        if (-not $NotaryProfile) { 'notarization' }
    )
    if ($missing.Count -gt 0) {
        return [pscustomobject]@{ mode = 'ad-hoc'; reason = ($missing -join '-and-') + '-unavailable'; identity = '-'; profile = $null }
    }
    if ($SigningIdentity -cnotmatch '^[0-9A-Fa-f]{40}$') {
        throw 'Developer ID signing requires the certificate fingerprint.'
    }
    [pscustomobject]@{ mode = 'developer-id'; reason = $null; identity = $SigningIdentity; profile = $NotaryProfile }
}

function Set-P4kxMacSignature {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)]$Plan,
        [string]$Identifier,
        [switch]$ExecutableCode
    )
    $arguments = @('--force', '--sign', $Plan.identity)
    if ($Plan.mode -ceq 'developer-id') {
        $arguments += '--timestamp'
        if ($ExecutableCode) { $arguments += @('--options', 'runtime') }
    } else {
        $arguments += '--timestamp=none'
    }
    if ($Identifier) { $arguments += @('--identifier', $Identifier) }
    Invoke-P4kxMacTool /usr/bin/codesign ($arguments + @($Path)) | Out-Host
}

function Assert-P4kxMacSignature {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)]$Plan,
        [switch]$Bundle,
        [switch]$ExecutableCode
    )
    $arguments = @('--verify', '--strict')
    if ($Bundle) { $arguments += '--deep' }
    if ($Plan.mode -ceq 'developer-id') {
        $requirement = 'anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists'
        $arguments += @('-R', "=$requirement")
    }
    Invoke-P4kxMacTool /usr/bin/codesign ($arguments + @($Path)) | Out-Host
    $description = @(Invoke-P4kxMacTool /usr/bin/codesign @('--display', '--verbose=4', $Path) 2>&1) -join "`n"
    if ($Plan.mode -ceq 'ad-hoc' -and $description -cnotmatch '(?m)^Signature=adhoc\s*$') {
        throw 'Expected an ad-hoc signature.'
    }
    if ($Plan.mode -ceq 'developer-id' -and $ExecutableCode -and
        $description -cnotmatch '(?m)^CodeDirectory [^\r\n]*flags=0x[0-9a-f]+\([^\r\n)]*\bruntime\b') {
        throw 'Developer ID code does not have hardened runtime enabled.'
    }
}

function Complete-P4kxMacNotarization {
    param([Parameter(Mandatory)][string]$Dmg, [Parameter(Mandatory)]$Plan, [string]$Keychain)
    if ($Plan.mode -ceq 'ad-hoc') { return 'not-requested' }
    $arguments = @('notarytool', 'submit', $Dmg, '--keychain-profile', $Plan.profile, '--wait', '--timeout', '30m', '--output-format', 'json')
    if ($Keychain) { $arguments += @('--keychain', $Keychain) }
    $submission = Invoke-P4kxMacTool /usr/bin/xcrun $arguments | ConvertFrom-Json
    if ($submission.status -cne 'Accepted') { throw "Apple notarization did not accept the DMG (submission $($submission.id))." }
    Invoke-P4kxMacTool /usr/bin/xcrun @('stapler', 'staple', $Dmg) | Out-Host
    Invoke-P4kxMacTool /usr/bin/xcrun @('stapler', 'validate', $Dmg) | Out-Host
    'stapled-and-validated'
}
