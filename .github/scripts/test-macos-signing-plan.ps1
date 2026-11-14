$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-signing.ps1')

$fingerprint = 'a' * 40
$cases = @(
    @{ arguments = @{ DryRun = $true; SigningIdentity = 'unused'; NotaryProfile = 'unused' }; mode = 'ad-hoc'; reason = 'package-dry-run' }
    @{ arguments = @{}; mode = 'ad-hoc'; reason = 'developer-id-and-notarization-unavailable' }
    @{ arguments = @{ SigningIdentity = $fingerprint }; mode = 'ad-hoc'; reason = 'notarization-unavailable' }
    @{ arguments = @{ NotaryProfile = 'local-profile' }; mode = 'ad-hoc'; reason = 'developer-id-unavailable' }
    @{ arguments = @{ SigningIdentity = $fingerprint; NotaryProfile = 'local-profile' }; mode = 'developer-id'; reason = $null }
)
foreach ($case in $cases) {
    $arguments = $case.arguments
    $plan = Get-P4kxMacSigningPlan @arguments
    if ($plan.mode -cne $case.mode -or $plan.reason -cne $case.reason) {
        throw 'macOS signing availability did not select the expected mode and reason.'
    }
    if ($plan.mode -ceq 'ad-hoc' -and ($plan.identity -cne '-' -or $null -ne $plan.profile)) {
        throw 'Ad-hoc signing retained Developer ID credentials.'
    }
    if ($plan.mode -ceq 'developer-id' -and ($plan.identity -cne $fingerprint -or $plan.profile -cne 'local-profile')) {
        throw 'Developer ID signing lost its selected credentials.'
    }
}
try {
    $null = Get-P4kxMacSigningPlan -SigningIdentity 'invalid' -NotaryProfile 'local-profile'
} catch {
    if ($_.Exception.Message -cnotmatch 'certificate fingerprint') { throw }
    'macOS signing policy contract passed.'
    return
}
throw 'An invalid configured Developer ID identity must fail.'
