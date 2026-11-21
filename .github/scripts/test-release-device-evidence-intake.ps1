param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-device-evidence-intake.ps1')
. (Join-Path $PSScriptRoot 'arm64-device-evidence-intake.ps1')

function Assert-Rejected([scriptblock]$Action, [string]$Message) {
    $failure = try { & $Action } catch { $_.Exception.Message }
    if ($failure -cnotlike "*$Message*") { throw "Expected rejection containing '$Message'." }
}

$env:GITHUB_REPOSITORY = 'p4kx-org/p4kx'
$env:GITHUB_REPOSITORY_ID = '314'
$env:GITHUB_REF_NAME = 'v0.5.0'
$env:GITHUB_REF_TYPE = 'tag'
$env:GITHUB_REF = "refs/tags/$env:GITHUB_REF_NAME"
$env:GITHUB_SHA = 'a' * 40
$env:GITHUB_RUN_ID = '456'
$env:GITHUB_EVENT_NAME = 'push'

function git {
    if (($args -join ' ') -cne 'rev-parse --verify HEAD') { throw 'Unexpected local git call.' }
    $global:LASTEXITCODE = 0
    $env:GITHUB_SHA
}

function New-Run([long]$Id, [string]$Workflow, [string]$Conclusion, [string]$Event) {
    return @{
        id = $Id
        path = ".github/workflows/$Workflow@$env:GITHUB_REF"
        event = $Event
        status = 'completed'
        conclusion = $Conclusion
        head_branch = $env:GITHUB_REF_NAME
        head_sha = $env:GITHUB_SHA
        repository = @{ id = 314; full_name = $env:GITHUB_REPOSITORY }
        head_repository = @{ id = 314; full_name = $env:GITHUB_REPOSITORY }
    }
}

function New-Artifact([long]$Id, [long]$RunId, [long]$RepositoryId, [string]$Name = "macos-device-evidence-candidate-$env:GITHUB_RUN_ID") {
    return @{
        id = $Id
        name = $Name
        expired = $false
        digest = 'sha256:' + ('e' * 64)
        workflow_run = @{ id = $RunId; repository_id = $RepositoryId; head_repository_id = $RepositoryId; head_sha = $env:GITHUB_SHA }
    }
}

function Invoke-P4kxIntakeApi([string]$Resource, [switch]$Paginate) {
    $base = "repos/$env:GITHUB_REPOSITORY"
    switch -CaseSensitive ($Resource) {
        "$base/git/ref/tags/$env:GITHUB_REF_NAME" {
            return @{ ref = $env:GITHUB_REF; object = @{ type = 'tag'; sha = 'f' * 40 } }
        }
        "$base/git/tags/$('f' * 40)" {
            return @{ sha = 'f' * 40; object = @{ type = 'commit'; sha = $env:GITHUB_SHA } }
        }
        "$base/actions/artifacts?name=macos-device-evidence-candidate-$env:GITHUB_RUN_ID" {
            if (-not $Paginate) { throw 'Device intake lookup must cover all artifact result pages.' }
            return @(
                @{ artifacts = @((New-Artifact 999 49 315), (New-Artifact 998 48 314)) },
                @{ artifacts = @((New-Artifact 997 47 314), (New-Artifact 996 46 314), (New-Artifact 995 45 314)) }
            )
        }
        "$base/actions/artifacts?name=arm64-device-evidence-candidate-$env:GITHUB_RUN_ID" {
            if (-not $Paginate) { throw 'ARM64 desktop intake lookup must cover all artifact result pages.' }
            $name = "arm64-device-evidence-candidate-$env:GITHUB_RUN_ID"
            return @(
                @{ artifacts = @((New-Artifact 1004 50 315 $name), (New-Artifact 1003 46 314 $name)) },
                @{ artifacts = @((New-Artifact 1002 51 314 $name), (New-Artifact 1001 50 314 $name)) }
            )
        }
        "$base/actions/runs/51" { return New-Run 51 'arm64-device-evidence.yml' 'failure' 'workflow_dispatch' }
        "$base/actions/runs/50" { return New-Run 50 'arm64-device-evidence.yml' 'success' 'workflow_dispatch' }
        "$base/actions/runs/456/artifacts?name=arm64-device-runtime-aarch64-pc-windows-msvc" {
            return @{ total_count = 1; artifacts = @((New-Artifact 105 456 314 'arm64-device-runtime-aarch64-pc-windows-msvc')) }
        }
        "$base/actions/runs/456/artifacts?name=arm64-device-runtime-aarch64-unknown-linux-gnu" {
            return @{ total_count = 1; artifacts = @((New-Artifact 106 456 314 'arm64-device-runtime-aarch64-unknown-linux-gnu')) }
        }
        "$base/actions/runs/456" { return New-Run 456 'release.yml' 'failure' 'push' }
        "$base/actions/runs/456/artifacts?name=release-aarch64-apple-darwin" {
            return @{ total_count = 1; artifacts = @((New-Artifact 101 456 314 'release-aarch64-apple-darwin')) }
        }
        "$base/actions/runs/456/artifacts?name=attestations-aarch64-apple-darwin" {
            return @{ total_count = 1; artifacts = @((New-Artifact 102 456 314 'attestations-aarch64-apple-darwin')) }
        }
        "$base/actions/runs/456/artifacts?name=release-addon" {
            return @{ total_count = 1; artifacts = @((New-Artifact 103 456 314 'release-addon')) }
        }
        "$base/actions/runs/456/artifacts?name=attestations-addon" {
            return @{ total_count = 1; artifacts = @((New-Artifact 104 456 314 'attestations-addon')) }
        }
        "$base/actions/runs/48" { return New-Run 48 'ci.yml' 'success' 'workflow_dispatch' }
        "$base/actions/runs/47" { return New-Run 47 'macos-device-evidence.yml' 'failure' 'workflow_dispatch' }
        "$base/actions/runs/46" { return New-Run 46 'macos-device-evidence.yml' 'success' 'workflow_dispatch' }
        default { throw 'Unexpected synthetic GitHub query.' }
    }
}

$selected = Find-P4kxDeviceIntakeArtifact '456'
if ($selected['run-id'] -cne '46' -or $selected['artifact-id'] -cne '996') {
    throw 'Lookup must select the newest successfully completed exact intake run.'
}
Assert-Rejected { Find-P4kxDeviceIntakeArtifact '457' } 'active candidate run'
$arm64 = Find-P4kxDeviceIntakeArtifact '456' 'arm64'
if ($arm64['run-id'] -cne '50' -or $arm64['artifact-id'] -cne '1001') {
    throw 'ARM64 lookup must ignore unrelated owners, workflows and unsuccessful runs.'
}

$platform = Get-P4kxIntakeMacosPlatform
$minimum = [version]$platform.deployment_target
$older = "$($minimum.Major - 1).0"
$current = "$($minimum.Major).$($minimum.Minor + 1)"
Assert-P4kxIntakeCurrentVersion $current $platform
Assert-Rejected { Assert-P4kxIntakeCurrentVersion $older $platform } 'deployment target'

$env:GITHUB_EVENT_NAME = 'workflow_dispatch'
$candidate = Resolve-P4kxMacosIntakeCandidate '456' $current
if ($candidate['packages-id'] -cne '101' -or $candidate['bundles-id'] -cne '102' -or
    $candidate['extension-id'] -cne '103' -or $candidate['extension-bundle-id'] -cne '104') {
    throw 'Intake must use the same tagged candidate for the DMG and Blender extension.'
}
$arm64 = Resolve-P4kxArm64IntakeCandidate '456'
if ($arm64['run-id'] -cne '456' -or $arm64['runtime-ids'] -cne '105,106' -or
    $arm64['artifact-name'] -cne 'arm64-device-evidence-candidate-456' -or $arm64.version -cne '0.5.0') {
    throw 'ARM64 intake must bind the runtime receipts of both platforms to the original tag run.'
}
$env:GITHUB_EVENT_NAME = 'push'

$expected = New-P4kxIntakeBinding '456' '46' $current ('d' * 64) ('b' * 64) ('e' * 64) $platform
$binding = $expected | ConvertTo-Json | ConvertFrom-Json
Assert-P4kxIntakeBinding $binding $expected
foreach ($field in @('candidate_run_id', 'intake_run_id', 'ref', 'revision', 'dmg_sha256', 'blender_extension_sha256', 'receipt_sha256')) {
    $changed = $expected | ConvertTo-Json | ConvertFrom-Json
    $changed.$field = 'different'
    Assert-Rejected { Assert-P4kxIntakeBinding $changed $expected } $field
}
$binding.schema = '2'
Assert-Rejected { Assert-P4kxIntakeBinding $binding $expected } 'schema'
$binding.schema = 2L
$binding | Add-Member -NotePropertyName 'unknown' -NotePropertyValue $true
Assert-Rejected { Assert-P4kxIntakeBinding $binding $expected } 'wrong field set'

'Offline intake checks passed: annotated tag, paginated artifact owner and success, exact run binding, platform minimum and all critical digest mismatches.'

if (Test-Path -LiteralPath $Directory) { throw 'Use a fresh device-intake fixture directory.' }
$assets = Join-Path $Directory 'assets'
$intake = Join-Path $Directory 'intake'
New-Item -ItemType Directory -Path $assets, $intake | Out-Null
$env:RUNNER_TEMP = $Directory
function Save-P4kxIntakeTrustedRoot([string]$Path) {
    Set-Content -LiteralPath $Path -Value 'synthetic trusted root'
}
function Assert-P4kxIntakeAttestation([string]$Subject, [string]$Bundle, [string]$Workflow, [string]$TrustedRoot,
    [string]$RunId, [string]$Event) {
    if ((Split-Path -Leaf $Subject) -cne 'binding.json' -or (Split-Path -Leaf $Bundle) -cne 'binding.sigstore.json' -or
        $Workflow -cne '.github/workflows/arm64-device-evidence.yml' -or $RunId -cne '50' -or
        $Event -cne 'workflow_dispatch' -or -not (Test-Path -LiteralPath $TrustedRoot -PathType Leaf)) {
        throw 'Wrong ARM64 binding attestation policy.'
    }
}
$hashes = @{}
foreach ($target in Get-P4kxArm64IntakeTargets) {
    $runtime = Join-Path $assets "P4KX-0.5.0-$($target.target)-runtime.json"
    Set-Content -LiteralPath $runtime -Value $target.target
    $hashes[$target.os] = Get-P4kxIntakeHash $runtime
}
$receipt = Join-Path $intake 'device-evidence.json'
Set-Content -LiteralPath $receipt -Value '["synthetic native observations"]'
$bundle = Join-Path $intake 'binding.sigstore.json'
Set-Content -LiteralPath $bundle -Value 'synthetic attestation'
$bindingPath = Join-Path $intake 'binding.json'
$expected = New-P4kxArm64IntakeBinding '456' '50' (Get-P4kxIntakeHash $receipt) $hashes
$expected | ConvertTo-Json | Set-Content -LiteralPath $bindingPath
if ((Read-P4kxArm64DeviceIntake $intake $assets '0.5.0' '456' '50') -cne $receipt) {
    throw 'The reader must return the exact signed ARM64 receipt path.'
}
foreach ($field in @('candidate_run_id', 'intake_run_id', 'ref', 'revision', 'windows_runtime_sha256', 'linux_runtime_sha256', 'receipt_sha256')) {
    $changed = $expected | ConvertTo-Json | ConvertFrom-Json
    $changed.$field = 'different'
    $changed | ConvertTo-Json | Set-Content -LiteralPath $bindingPath
    Assert-Rejected { Read-P4kxArm64DeviceIntake $intake $assets '0.5.0' '456' '50' } $field
}
Assert-Rejected { Read-P4kxArm64DeviceIntake $intake $assets '0.5.0' '457' '50' } 'active tagged candidate'
Assert-Rejected { Read-P4kxArm64DeviceIntake $intake $assets '0.5.0' '456' '49' } 'Wrong ARM64 binding attestation policy.'

'Offline ARM64 intake checks passed: exact cross-platform runtime artifacts, tag/run ownership, signer policy and signed runtime/receipt digests.'
