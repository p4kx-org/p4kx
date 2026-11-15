$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-device-evidence-intake.ps1')

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
