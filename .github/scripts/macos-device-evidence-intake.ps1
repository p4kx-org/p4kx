$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-device-evidence-github.ps1')
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
    Assert-P4kxIntakeTag
    $run = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/actions/runs/$RunId"
    if (-not (Test-P4kxIntakeRun $run $RunId '.github/workflows/release.yml' 'push') -or
        $run.conclusion -cnotin @('failure', 'success')) {
        throw 'Device evidence requires a completed owning release workflow run at the exact pushed tag and commit.'
    }
    return @{
        'run-id' = $RunId
        'artifact-name' = "macos-device-evidence-candidate-$RunId"
        'version' = $env:GITHUB_REF_NAME.Substring(1)
        'current-version' = $MacosCurrentVersion
        'packages-id' = Get-P4kxMacosCandidateArtifact $RunId "release-$($platform.target)"
        'bundles-id' = Get-P4kxMacosCandidateArtifact $RunId "attestations-$($platform.target)"
    }
}

function Get-P4kxIntakeFile([string]$Directory, [string]$Name) {
    $file = Get-Item -LiteralPath (Join-Path $Directory $Name)
    if ($file -isnot [IO.FileInfo] -or $file.Name -cne $Name -or $file.Length -le 0 -or
        ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Device evidence input '$Name' is not an exact non-empty regular file."
    }
    return $file.FullName
}

function Get-P4kxIntakeHash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-P4kxIntakeDmg([string]$Assets, [string]$Version, $Platform) {
    $name = Get-P4kxAssetName -Target $Platform.target -Version $Version -Package dmg
    return Get-P4kxIntakeFile $Assets $name
}

function Read-P4kxIntakeRuntime([string]$Path, [string]$Assets, [string]$Version, [string]$RunId, $Platform) {
    $runtime = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($runtime.workflow_run -cne "https://github.com/$env:GITHUB_REPOSITORY/actions/runs/$RunId") {
        throw 'Native macOS runtime evidence belongs to another workflow run.'
    }
    $pins = & (Join-Path $PSScriptRoot 'check-build-tools.ps1')
    $compiler = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
    $nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
    Assert-P4kxArchiveEvidence $runtime $Platform $env:GITHUB_SHA $compiler $nextest
    Assert-P4kxRuntimeEvidence $runtime $Platform $Version $env:GITHUB_SHA
    Assert-P4kxPackageRuntime $runtime $Platform $Assets $Version
    return $runtime
}

function Save-P4kxIntakeReceipt([string]$Directory, $Runtime, $Platform, [string]$CurrentVersion) {
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
    Assert-P4kxMacosReleaseDevices $Runtime $Platform $CurrentVersion $records
    $path = Join-Path $Directory 'device-evidence.json'
    Set-Content -LiteralPath $path -Value $json -Encoding utf8NoBOM -NoNewline
    return $path
}

function New-P4kxIntakeBinding([string]$CandidateRunId, [string]$IntakeRunId, [string]$CurrentVersion,
    [string]$DmgHash, [string]$ReceiptHash, $Platform) {
    return [ordered]@{
        schema = 1L
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
        receipt_sha256 = $ReceiptHash
    }
}

function Save-P4kxMacosDeviceIntake([string]$Assets, [string]$Bundles, [string]$Directory,
    [string]$Version, [string]$CandidateRunId, [string]$CurrentVersion) {
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
    $runtime = Read-P4kxIntakeRuntime $runtimePath $Assets $Version $CandidateRunId $platform
    $receipt = Save-P4kxIntakeReceipt $Directory $runtime $platform $CurrentVersion
    $binding = New-P4kxIntakeBinding $CandidateRunId $env:GITHUB_RUN_ID $CurrentVersion $runtime.archive_sha256 (Get-P4kxIntakeHash $receipt) $platform
    $binding | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'binding.json') -Encoding utf8NoBOM
}

function Assert-P4kxIntakeBinding($Binding, $Expected) {
    if ($Binding -isnot [pscustomobject] -or @($Binding.PSObject.Properties).Count -ne $Expected.Count) {
        throw 'Native macOS device receipt binding has the wrong field set.'
    }
    foreach ($field in $Expected.Keys) {
        $property = $Binding.PSObject.Properties[$field]
        if ($null -eq $property -or $property.Name -cne $field -or
            $property.Value -isnot ($Expected[$field].GetType()) -or $property.Value -cne $Expected[$field]) {
            throw "Native macOS device receipt binding differs in '$field'."
        }
    }
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
    $expected = New-P4kxIntakeBinding $CandidateRunId $IntakeRunId $current (Get-P4kxIntakeHash $dmg) (Get-P4kxIntakeHash $receipt) $platform
    Assert-P4kxIntakeBinding $binding $expected
    return @{ Path = $receipt; CurrentVersion = $current }
}
