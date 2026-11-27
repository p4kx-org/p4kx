$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-device-evidence-github.ps1')
. (Join-Path $PSScriptRoot 'package-release-evidence.ps1')

function Get-P4kxIntakeMacosPlatform {
    $platforms = @((Get-P4kxPlatformContract).targets | Where-Object os -CEQ 'macos')
    if ($platforms.Count -ne 1) { throw 'Expected one macOS target in the release platform contract.' }
    return $platforms[0]
}

function Assert-P4kxIntakeCurrentVersion([string]$Version, $Platform) {
    $parsed = $null
    if ($Version -cnotmatch '^[0-9]+\.[0-9]+(?:\.[0-9]+)?$' -or
        -not [version]::TryParse($Version, [ref]$parsed) -or $parsed -lt [version]$Platform.deployment_target) {
        throw 'The verified current stable macOS version must meet the platform deployment target.'
    }
}

function Resolve-P4kxMacosIntakeCandidate([string]$RunId, [string]$MacosCurrentVersion) {
    Assert-P4kxIntakeTagContext 'workflow_dispatch'
    if (-not (Test-P4kxWorkflowRunId $RunId)) { throw 'A positive candidate workflow run ID is required.' }
    $platform = Get-P4kxIntakeMacosPlatform
    Assert-P4kxIntakeCurrentVersion $MacosCurrentVersion $platform
    Assert-P4kxIntakeCandidateRun $RunId
    return @{
        'run-id' = $RunId
        'artifact-name' = "macos-device-evidence-candidate-$RunId"
        'version' = $env:GITHUB_REF_NAME.Substring(1)
        'current-version' = $MacosCurrentVersion
        'packages-id' = Get-P4kxCandidateArtifact $RunId "release-$($platform.target)"
        'bundles-id' = Get-P4kxCandidateArtifact $RunId "attestations-$($platform.target)"
        'extension-id' = Get-P4kxCandidateArtifact $RunId 'release-addon'
        'extension-bundle-id' = Get-P4kxCandidateArtifact $RunId 'attestations-addon'
    }
}

function Get-P4kxIntakeDmg([string]$Assets, [string]$Version, $Platform) {
    $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package dmg
    return Get-P4kxIntakeFile $Assets $name
}

function Save-P4kxIntakeReceipt([string]$Directory, $Runtime, $Platform, [string]$CurrentVersion, [string]$ExtensionHash) {
    $dispatch = Get-Content -LiteralPath $env:GITHUB_EVENT_PATH -Raw | ConvertFrom-Json
    $json = $dispatch.inputs.'macos-device-evidence'
    if ($json -isnot [string] -or -not $json.TrimStart().StartsWith('[')) {
        throw 'Supply the native macOS device receipt as a JSON array.'
    }
    try {
        $records = ConvertFrom-Json -InputObject $json -NoEnumerate
    } catch {
        throw 'The native macOS device receipt is invalid JSON.'
    }
    if ($records -isnot [array]) { throw 'The native macOS device receipt must contain the two device checks.' }
    Assert-P4kxMacosReleaseDevices $Runtime $Platform $CurrentVersion $records $ExtensionHash
    $path = Join-Path $Directory 'device-evidence.json'
    Set-Content -LiteralPath $path -Value $json -Encoding utf8NoBOM -NoNewline
    return $path
}

function New-P4kxIntakeBinding([string]$CandidateRunId, [string]$IntakeRunId, [string]$CurrentVersion,
    [string]$DmgHash, [string]$ExtensionHash, [string]$ReceiptHash, $Platform) {
    return [ordered]@{
        schema = 2L
        repository = $env:GITHUB_REPOSITORY
        repository_id = $env:GITHUB_REPOSITORY_ID
        ref = $env:GITHUB_REF
        revision = $env:GITHUB_SHA
        candidate_run_id = $CandidateRunId
        intake_run_id = $IntakeRunId
        target = $Platform.target
        deployment_target = $Platform.deployment_target
        macos_current_version = $CurrentVersion
        dmg_sha256 = $DmgHash
        blender_extension_sha256 = $ExtensionHash
        receipt_sha256 = $ReceiptHash
    }
}

function Save-P4kxMacosDeviceIntake([string]$Assets, [string]$Bundles, [string]$ExtensionAssets, [string]$ExtensionBundles,
    [string]$Directory, [string]$Version, [string]$CandidateRunId, [string]$CurrentVersion) {
    $platform = Get-P4kxIntakeMacosPlatform
    $dmg = Get-P4kxIntakeDmg $Assets $Version $platform
    $prefix = "P4KX-$Version-$($platform.target)"
    $runtimePath = Get-P4kxIntakeFile $Assets "$prefix-runtime.json"
    $bundle = Get-P4kxIntakeFile $Bundles "$prefix-provenance.sigstore.json"
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-device-intake-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    foreach ($subject in @($dmg, $runtimePath)) {
        Assert-P4kxIntakeAttestation $subject $bundle '.github/workflows/release-attest.yml' $trust $CandidateRunId 'push'
    }
    $extension = Get-P4kxReleaseBlenderExtension $ExtensionAssets $Version
    $extensionBundle = Get-P4kxIntakeFile $ExtensionBundles "P4KX-$Version-addon-provenance.sigstore.json"
    Assert-P4kxIntakeAttestation $extension $extensionBundle '.github/workflows/release.yml' $trust $CandidateRunId 'push'
    $extensionHash = Get-P4kxIntakeHash $extension
    $runtime = Read-P4kxIntakeRuntime $runtimePath $Version $CandidateRunId $platform
    Assert-P4kxPackageRuntime $runtime $platform $Assets $Version
    $receipt = Save-P4kxIntakeReceipt $Directory $runtime $platform $CurrentVersion $extensionHash
    $binding = New-P4kxIntakeBinding $CandidateRunId $env:GITHUB_RUN_ID $CurrentVersion $runtime.archive_sha256 $extensionHash (Get-P4kxIntakeHash $receipt) $platform
    $binding | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'binding.json') -Encoding utf8NoBOM
}

function Read-P4kxMacosDeviceIntake([string]$Directory, [string]$Assets, [string]$Version,
    [string]$CandidateRunId, [string]$IntakeRunId, [string]$ExpectedCurrentVersion) {
    Assert-P4kxIntakeTagContext 'push'
    if ($CandidateRunId -cne $env:GITHUB_RUN_ID -or $Version -cne $env:GITHUB_REF_NAME.Substring(1) -or
        -not (Test-P4kxWorkflowRunId $IntakeRunId)) {
        throw 'Device receipt verification requires the active tagged candidate and the exact intake run.'
    }
    $platform = Get-P4kxIntakeMacosPlatform
    $dmg = Get-P4kxIntakeDmg $Assets $Version $platform
    $path = Get-P4kxIntakeFile $Directory 'binding.json'
    $bundle = Get-P4kxIntakeFile $Directory 'binding.sigstore.json'
    $receipt = Get-P4kxIntakeFile $Directory 'device-evidence.json'
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-device-intake-reader-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    Assert-P4kxIntakeAttestation $path $bundle '.github/workflows/macos-device-evidence.yml' $trust $IntakeRunId 'workflow_dispatch'
    $binding = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $current = $binding.macos_current_version
    Assert-P4kxIntakeCurrentVersion $current $platform
    if ($ExpectedCurrentVersion -and $current -cne $ExpectedCurrentVersion) {
        throw 'Device receipt current macOS version differs from the release configuration.'
    }
    $extension = Get-P4kxReleaseBlenderExtension $Assets $Version
    $extensionHash = Get-P4kxIntakeHash $extension
    $expected = New-P4kxIntakeBinding $CandidateRunId $IntakeRunId $current (Get-P4kxIntakeHash $dmg) $extensionHash (Get-P4kxIntakeHash $receipt) $platform
    Assert-P4kxIntakeBinding $binding $expected
    return @{ Path = $receipt; CurrentVersion = $current }
}
