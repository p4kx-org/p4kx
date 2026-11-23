$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-device-evidence-github.ps1')
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'windows-desktop-device-evidence.ps1')

function Get-P4kxWindowsDesktopIntakeTarget {
    $targets = @((Get-P4kxPlatformContract).targets | Where-Object { Test-P4kxWindowsDesktopTarget $_ })
    if ($targets.Count -ne 1) { throw 'Device evidence requires one Windows AMD64 desktop target.' }
    return $targets[0]
}

function Resolve-P4kxWindowsDesktopIntakeCandidate([string]$RunId) {
    Assert-P4kxIntakeTagContext 'workflow_dispatch'
    if (-not (Test-P4kxWorkflowRunId $RunId)) { throw 'A positive candidate workflow run ID is required.' }
    Assert-P4kxIntakeCandidateRun $RunId
    $platform = Get-P4kxWindowsDesktopIntakeTarget
    return @{
        'run-id' = $RunId
        'artifact-name' = "windows-desktop-device-evidence-candidate-$RunId"
        'runtime-id' = Get-P4kxCandidateArtifact $RunId "windows-desktop-device-runtime-$($platform.target)"
        'version' = $env:GITHUB_REF_NAME.Substring(1)
    }
}

function Get-P4kxWindowsDesktopIntakeRuntimeFile([string]$Assets, [string]$Version, $Platform) {
    Get-P4kxIntakeFile $Assets "P4KX-$Version-$($Platform.target)-runtime.json"
}

function New-P4kxWindowsDesktopIntakeBinding([string]$CandidateRunId, [string]$IntakeRunId, [string]$ReceiptHash, [string]$RuntimeHash) {
    return [ordered]@{
        schema = 1L
        repository = $env:GITHUB_REPOSITORY
        repository_id = $env:GITHUB_REPOSITORY_ID
        ref = $env:GITHUB_REF
        revision = $env:GITHUB_SHA
        candidate_run_id = $CandidateRunId
        intake_run_id = $IntakeRunId
        windows_runtime_sha256 = $RuntimeHash
        receipt_sha256 = $ReceiptHash
    }
}

function Read-P4kxWindowsDesktopIntakeRecords {
    $dispatch = Get-Content -LiteralPath $env:GITHUB_EVENT_PATH -Raw | ConvertFrom-Json
    $json = $dispatch.inputs.'windows-desktop-device-evidence'
    if ($json -isnot [string] -or -not $json.TrimStart().StartsWith('[')) {
        throw 'Supply the Windows AMD64 desktop device receipts as a JSON array.'
    }
    try {
        $records = ConvertFrom-Json -InputObject $json -NoEnumerate
    } catch {
        throw 'The Windows desktop device receipts are invalid JSON.'
    }
    Assert-P4kxWindowsDesktopReceiptRows $records (Get-P4kxPlatformContract)
    return @{ Json = $json; Records = $records }
}

function Save-P4kxWindowsDesktopDeviceIntake([string]$Assets, [string]$Directory, [string]$Version, [string]$CandidateRunId) {
    $intake = Read-P4kxWindowsDesktopIntakeRecords
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-windows-desktop-device-intake-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    $platform = Get-P4kxWindowsDesktopIntakeTarget
    $runtimePath = Get-P4kxWindowsDesktopIntakeRuntimeFile $Assets $Version $platform
    $bundle = Get-P4kxIntakeFile $Assets "P4KX-$Version-$($platform.target)-provenance.sigstore.json"
    Assert-P4kxIntakeAttestation $runtimePath $bundle '.github/workflows/release-attest.yml' $trust $CandidateRunId 'push'
    $runtime = Read-P4kxIntakeRuntime $runtimePath $Version $CandidateRunId $platform
    Assert-P4kxWindowsDesktopEvidence -Runtime $runtime -Platform $platform -Version $Version -Revision $env:GITHUB_SHA -ReleaseRunId $CandidateRunId -Records $intake.Records
    $receipt = Join-Path $Directory 'device-evidence.json'
    Set-Content -LiteralPath $receipt -Value $intake.Json -Encoding utf8NoBOM -NoNewline
    $binding = New-P4kxWindowsDesktopIntakeBinding $CandidateRunId $env:GITHUB_RUN_ID (Get-P4kxIntakeHash $receipt) (Get-P4kxIntakeHash $runtimePath)
    $binding | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'binding.json') -Encoding utf8NoBOM
}

function Read-P4kxWindowsDesktopDeviceIntake([string]$Directory, [string]$Assets, [string]$Version, [string]$CandidateRunId, [string]$IntakeRunId) {
    Assert-P4kxIntakeTagContext 'push'
    if ($CandidateRunId -cne $env:GITHUB_RUN_ID -or $Version -cne $env:GITHUB_REF_NAME.Substring(1) -or
        -not (Test-P4kxWorkflowRunId $IntakeRunId)) {
        throw 'Windows desktop device receipt verification requires the active tagged candidate and exact intake run.'
    }
    $path = Get-P4kxIntakeFile $Directory 'binding.json'
    $bundle = Get-P4kxIntakeFile $Directory 'binding.sigstore.json'
    $receipt = Get-P4kxIntakeFile $Directory 'device-evidence.json'
    $trust = Join-Path $env:RUNNER_TEMP 'p4kx-windows-desktop-device-intake-reader-trusted-root.json'
    Save-P4kxIntakeTrustedRoot $trust
    Assert-P4kxIntakeAttestation $path $bundle '.github/workflows/windows-desktop-device-evidence.yml' $trust $IntakeRunId 'workflow_dispatch'
    $platform = Get-P4kxWindowsDesktopIntakeTarget
    $runtimePath = Get-P4kxWindowsDesktopIntakeRuntimeFile $Assets $Version $platform
    $expected = New-P4kxWindowsDesktopIntakeBinding $CandidateRunId $IntakeRunId (Get-P4kxIntakeHash $receipt) (Get-P4kxIntakeHash $runtimePath)
    $binding = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    Assert-P4kxIntakeBinding $binding $expected
    return $receipt
}
