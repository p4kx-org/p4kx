$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-device-evidence-github.ps1')
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'arm64-device-evidence.ps1')

function Get-P4kxArm64IntakeTargets {
    $targets = @((Get-P4kxPlatformContract).targets | Where-Object { Test-P4kxArm64DesktopTarget $_ })
    if ($targets.Count -ne 2) { throw 'Device evidence requires the Windows and Linux ARM64 targets.' }
    return $targets
}

function Resolve-P4kxArm64IntakeCandidate([string]$RunId) {
    Assert-P4kxIntakeTagContext 'workflow_dispatch'
    if (-not (Test-P4kxWorkflowRunId $RunId)) { throw 'A positive candidate workflow run ID is required.' }
    Assert-P4kxIntakeCandidateRun $RunId
    $runtimeIds = @(foreach ($platform in Get-P4kxArm64IntakeTargets) {
        Get-P4kxCandidateArtifact $RunId "arm64-device-runtime-$($platform.target)"
    })
    return @{
        'run-id' = $RunId
        'artifact-name' = "arm64-device-evidence-candidate-$RunId"
        'runtime-ids' = $runtimeIds -join ','
        'version' = $env:GITHUB_REF_NAME.Substring(1)
    }
}

function Get-P4kxArm64IntakeRuntimeFile([string]$Assets, [string]$Version, $Platform) {
    Get-P4kxIntakeFile $Assets "P4KX-$Version-$($Platform.target)-runtime.json"
}

function New-P4kxArm64IntakeBinding([string]$CandidateRunId, [string]$IntakeRunId, [string]$ReceiptHash, [hashtable]$RuntimeHashes) {
    return [ordered]@{
        schema = 1L
        repository = $env:GITHUB_REPOSITORY
        repository_id = $env:GITHUB_REPOSITORY_ID
        ref = $env:GITHUB_REF
        revision = $env:GITHUB_SHA
        candidate_run_id = $CandidateRunId
        intake_run_id = $IntakeRunId
        windows_runtime_sha256 = $RuntimeHashes.windows
        linux_runtime_sha256 = $RuntimeHashes.linux
        receipt_sha256 = $ReceiptHash
    }
}

function Read-P4kxArm64IntakeRecords {
    $dispatch = Get-Content -LiteralPath $env:GITHUB_EVENT_PATH -Raw | ConvertFrom-Json
    $json = $dispatch.inputs.'arm64-device-evidence'
    if ($json -isnot [string] -or -not $json.TrimStart().StartsWith('[')) {
        throw 'Supply the Windows and Linux ARM64 device receipts as a JSON array.'
    }
    try {
        $records = ConvertFrom-Json -InputObject $json -NoEnumerate
    } catch {
        throw 'The ARM64 device receipts are invalid JSON.'
    }
    Assert-P4kxArm64ReceiptRows $records (Get-P4kxPlatformContract)
    return @{ Json = $json; Records = $records }
}

function Save-P4kxArm64DeviceIntake([string]$Assets, [string]$Directory, [string]$Version, [string]$CandidateRunId) {
    $intake = Read-P4kxArm64IntakeRecords
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-arm64-device-intake-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    $hashes = @{}
    foreach ($platform in Get-P4kxArm64IntakeTargets) {
        $runtimePath = Get-P4kxArm64IntakeRuntimeFile $Assets $Version $platform
        $bundle = Get-P4kxIntakeFile $Assets "P4KX-$Version-$($platform.target)-provenance.sigstore.json"
        Assert-P4kxIntakeAttestation $runtimePath $bundle '.github/workflows/release-attest.yml' $trust $CandidateRunId 'push'
        $runtime = Read-P4kxIntakeRuntime $runtimePath $Version $CandidateRunId $platform
        $records = @($intake.Records | Where-Object target -CEQ $platform.target)
        Assert-P4kxArm64DeviceEvidence -Runtime $runtime -Platform $platform -Version $Version -Revision $env:GITHUB_SHA -ReleaseRunId $CandidateRunId -Records $records
        $hashes[$platform.os] = Get-P4kxIntakeHash $runtimePath
    }
    $receipt = Join-Path $Directory 'device-evidence.json'
    Set-Content -LiteralPath $receipt -Value $intake.Json -Encoding utf8NoBOM -NoNewline
    $binding = New-P4kxArm64IntakeBinding $CandidateRunId $env:GITHUB_RUN_ID (Get-P4kxIntakeHash $receipt) $hashes
    $binding | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'binding.json') -Encoding utf8NoBOM
}

function Read-P4kxArm64DeviceIntake([string]$Directory, [string]$Assets, [string]$Version, [string]$CandidateRunId, [string]$IntakeRunId) {
    Assert-P4kxIntakeTagContext 'push'
    if ($CandidateRunId -cne $env:GITHUB_RUN_ID -or $Version -cne $env:GITHUB_REF_NAME.Substring(1) -or
        -not (Test-P4kxWorkflowRunId $IntakeRunId)) {
        throw 'ARM64 device receipt verification requires the active tagged candidate and exact intake run.'
    }
    $path = Get-P4kxIntakeFile $Directory 'binding.json'
    $bundle = Get-P4kxIntakeFile $Directory 'binding.sigstore.json'
    $receipt = Get-P4kxIntakeFile $Directory 'device-evidence.json'
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-arm64-device-intake-reader-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    Assert-P4kxIntakeAttestation $path $bundle '.github/workflows/arm64-device-evidence.yml' $trust $IntakeRunId 'workflow_dispatch'
    $hashes = @{}
    foreach ($platform in Get-P4kxArm64IntakeTargets) {
        $runtimePath = Get-P4kxArm64IntakeRuntimeFile $Assets $Version $platform
        $hashes[$platform.os] = Get-P4kxIntakeHash $runtimePath
    }
    $expected = New-P4kxArm64IntakeBinding $CandidateRunId $IntakeRunId (Get-P4kxIntakeHash $receipt) $hashes
    $binding = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    Assert-P4kxIntakeBinding $binding $expected
    return $receipt
}
